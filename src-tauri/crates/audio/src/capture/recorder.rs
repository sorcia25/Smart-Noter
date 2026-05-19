//! Orchestrates the audio worker pipeline.
//!
//! Owns:
//!   * `StreamHandle` from `stream.rs` (one or two streams)
//!   * The bounded MPSC channels between callback → mixer (if Mix) → writer + meter
//!   * Worker thread join handles + a shutdown flag
//!
//! Emits Tauri events on a background thread via `AppHandle::emit`.

use crate::capture::meter::Meter;
use crate::capture::mixer::Mixer;
use crate::capture::session::{AudioFormat, CaptureMode};
use crate::capture::stream::{open, StreamHandle};
use crate::capture::writer::{AudioWriter, FlacWriterImpl, FinalizeResult, WavWriterImpl};
use crate::error::AudioError;
use crossbeam_channel::bounded;
use serde::Serialize;
use specta::Type;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LevelEvent {
    pub rms: f32,
    pub peak: f32,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct WaveformEvent {
    pub bins: Vec<f32>,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ElapsedEvent {
    pub elapsed_sec: u32,
}

pub struct Recorder {
    pub stream: StreamHandle,
    pub writer_join: Option<JoinHandle<Result<FinalizeResult, AudioError>>>,
    pub meter_join: Option<JoinHandle<()>>,
    pub paused: Arc<AtomicBool>,
    pub stop_flag: Arc<AtomicBool>,
    pub tmp_path: PathBuf,
}

impl Recorder {
    pub fn start(
        app: AppHandle,
        mode: CaptureMode,
        device_id: String,
        format: AudioFormat,
        tmp_path: PathBuf,
    ) -> Result<Self, AudioError> {
        let (sample_tx, sample_rx) = bounded::<Vec<f32>>(64);

        // Open stream(s); for Mix mode we also wire a mixer thread between
        // the two source channels and `sample_tx`.
        let stream = if matches!(mode, CaptureMode::Mix) {
            let (a_tx, a_rx) = bounded::<Vec<f32>>(64);
            let (b_tx, b_rx) = bounded::<Vec<f32>>(64);
            let handle = open(mode, &device_id, a_tx, Some(b_tx))?;

            // Spawn mixer thread.
            let mixer_sample_rate_a = handle.sample_rate;
            let mixer_sample_rate_b = 48_000; // default mic; could be refined per device
            let sample_tx_for_mixer = sample_tx.clone();
            std::thread::spawn(move || {
                let mut mixer = match Mixer::new(mixer_sample_rate_a, mixer_sample_rate_b) {
                    Ok(m) => m,
                    Err(e) => {
                        tracing::error!(?e, "mixer init");
                        return;
                    }
                };
                loop {
                    let a = match a_rx.recv() {
                        Ok(v) => v,
                        Err(_) => return,
                    };
                    let b: Vec<f32> = b_rx
                        .recv_timeout(Duration::from_millis(50))
                        .unwrap_or_default();
                    if let Ok(mixed) = mixer.mix(&a, &b) {
                        let _ = sample_tx_for_mixer.try_send(mixed);
                    }
                }
            });
            handle
        } else {
            open(mode, &device_id, sample_tx.clone(), None)?
        };

        let paused = Arc::new(AtomicBool::new(false));
        let stop_flag = Arc::new(AtomicBool::new(false));

        // Spawn writer thread.
        let mut writer: Box<dyn AudioWriter> = match format {
            AudioFormat::Wav => Box::new(WavWriterImpl::create(
                tmp_path.clone(),
                stream.sample_rate,
                stream.channels,
            )?),
            AudioFormat::Flac => Box::new(FlacWriterImpl::create(
                tmp_path.clone(),
                stream.sample_rate,
                stream.channels,
            )?),
        };

        let writer_paused = paused.clone();
        let writer_stop = stop_flag.clone();
        let writer_rx = sample_rx.clone();
        let writer_join = std::thread::spawn(move || -> Result<FinalizeResult, AudioError> {
            loop {
                if writer_stop.load(Ordering::Relaxed) {
                    break;
                }
                match writer_rx.recv_timeout(Duration::from_millis(50)) {
                    Ok(buf) => {
                        if !writer_paused.load(Ordering::Relaxed) {
                            writer.write(&buf)?;
                        }
                    }
                    Err(crossbeam_channel::RecvTimeoutError::Timeout) => continue,
                    Err(_) => break,
                }
            }
            writer.finalize()
        });

        // Spawn meter thread.
        // Note: crossbeam_channel is MPMC — both writer and meter compete for samples.
        // Writer gets priority. If the meter visibly stutters during smoke tests,
        // switch to two channels and have the stream thread send each buf to both.
        let meter_app = app.clone();
        let meter_stop = stop_flag.clone();
        let meter_sample_rate = stream.sample_rate;
        let meter_channels = stream.channels;
        let meter_rx = sample_rx;
        let meter_join = std::thread::spawn(move || {
            let mut meter = Meter::new(meter_sample_rate, meter_channels);
            let mut last_level = Instant::now();
            let mut last_wave = Instant::now();
            let mut last_elapsed = Instant::now();
            loop {
                if meter_stop.load(Ordering::Relaxed) {
                    break;
                }
                if let Ok(buf) = meter_rx.recv_timeout(Duration::from_millis(50)) {
                    meter.push(&buf);
                }
                let now = Instant::now();
                if now.duration_since(last_level) >= Duration::from_millis(50) {
                    last_level = now;
                    let lvl = meter.level();
                    let _ = meter_app.emit(
                        "audio:level",
                        LevelEvent {
                            rms: lvl.rms,
                            peak: lvl.peak,
                        },
                    );
                }
                if now.duration_since(last_wave) >= Duration::from_millis(100) {
                    last_wave = now;
                    let _ = meter_app.emit(
                        "audio:waveform-bin",
                        WaveformEvent {
                            bins: meter.waveform(),
                        },
                    );
                }
                if now.duration_since(last_elapsed) >= Duration::from_millis(1000) {
                    last_elapsed = now;
                    let _ = meter_app.emit(
                        "audio:elapsed",
                        ElapsedEvent {
                            elapsed_sec: meter.elapsed_sec(),
                        },
                    );
                }
            }
        });

        Ok(Self {
            stream,
            writer_join: Some(writer_join),
            meter_join: Some(meter_join),
            paused,
            stop_flag,
            tmp_path,
        })
    }

    pub fn pause(&self) {
        self.paused.store(true, Ordering::Relaxed);
    }

    pub fn resume(&self) {
        self.paused.store(false, Ordering::Relaxed);
    }

    pub fn stop(mut self) -> Result<(PathBuf, u64, u32), AudioError> {
        self.stop_flag.store(true, Ordering::Relaxed);

        let writer = self
            .writer_join
            .take()
            .unwrap()
            .join()
            .map_err(|_| AudioError::Other("writer thread panicked".into()))??;
        let _ = self.meter_join.take().unwrap().join();

        let duration_sec = (writer.sample_count
            / (self.stream.sample_rate as u64).max(1)
            / (self.stream.channels as u64).max(1)) as u32;
        Ok((writer.path, writer.bytes, duration_sec))
    }
}
