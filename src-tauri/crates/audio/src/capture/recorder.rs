//! Orchestrates the audio worker pipeline.
//!
//! Owns:
//!   * `StreamHandle` from `stream.rs` (one or two streams)
//!   * Bounded channels forming the fan-out topology:
//!     `stream callback → source_rx → fan-out → writer_rx (lossless)`
//!     `                                       ↘ meter_rx  (lossy)`
//!   * Worker thread join handles + a shutdown flag
//!
//! Emits Tauri events on a background thread via `AppHandle::emit`.

use crate::capture::meter::Meter;
use crate::capture::mixer::Mixer;
use crate::capture::session::{AudioFormat, CaptureMode};
use crate::capture::stream::{open, StreamHandle};
use crate::capture::writer::{AudioWriter, FinalizeResult, FlacWriterImpl, WavWriterImpl};
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

/// Tracks recorded time, excluding paused spans.
///
/// Each `tick` call advances the internal accumulator by `now - last_tick`
/// only when `paused` is false. The first tick after construction contributes
/// the wall time since `new(now)` was called (or zero if paused=true).
pub(crate) struct RecordedClock {
    recorded: Duration,
    last_tick: Instant,
}

impl RecordedClock {
    /// Construct with an explicit initial instant so tests are deterministic.
    pub(crate) fn new(now: Instant) -> Self {
        Self {
            recorded: Duration::ZERO,
            last_tick: now,
        }
    }

    /// Advance the clock. Returns the total recorded duration so far.
    ///
    /// If `paused` is true the span since `last_tick` is discarded; `last_tick`
    /// is still advanced so a future unpause starts from `now`, not the past.
    pub(crate) fn tick(&mut self, paused: bool, now: Instant) -> Duration {
        let delta = now.saturating_duration_since(self.last_tick);
        if !paused {
            self.recorded += delta;
        }
        self.last_tick = now;
        self.recorded
    }
}

/// Fan-out thread: receives from `source_rx`, forwards losslessly to `writer_tx`
/// and lossily to `meter_tx` (meter is visualization-only — dropping is fine).
///
/// Exits when `stop` is set or `source_rx` disconnects. Dropping `writer_tx` /
/// `meter_tx` on exit signals the downstream threads.
pub(crate) fn spawn_fanout(
    source_rx: crossbeam_channel::Receiver<Vec<f32>>,
    writer_tx: crossbeam_channel::Sender<Vec<f32>>,
    meter_tx: crossbeam_channel::Sender<Vec<f32>>,
    stop: Arc<AtomicBool>,
) -> JoinHandle<()> {
    std::thread::spawn(move || loop {
        match source_rx.recv_timeout(Duration::from_millis(50)) {
            Ok(buf) => {
                // Meter is lossy — drop if the meter thread falls behind.
                let _ = meter_tx.try_send(buf.clone());
                // Writer is lossless — blocking send provides backpressure.
                // The source channel absorbs bursts; if it fills, the stream
                // callback's existing try_send-drop-with-overflow-counter
                // behavior is the spec'd degradation path.
                if writer_tx.send(buf).is_err() {
                    break;
                }
            }
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                if stop.load(Ordering::Relaxed) {
                    break;
                }
            }
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
        }
    })
}

pub struct Recorder {
    pub stream: StreamHandle,
    pub writer_join: Option<JoinHandle<Result<FinalizeResult, AudioError>>>,
    pub meter_join: Option<JoinHandle<()>>,
    pub fanout_join: Option<JoinHandle<()>>,
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
        // source channel: stream callback (or mixer) → fan-out thread.
        let (source_tx, source_rx) = bounded::<Vec<f32>>(64);

        // Open stream(s); for Mix mode we also wire a mixer thread between
        // the two source channels and `source_tx`.
        let stream = if matches!(mode, CaptureMode::Mix) {
            let (a_tx, a_rx) = bounded::<Vec<f32>>(64);
            let (b_tx, b_rx) = bounded::<Vec<f32>>(64);
            let handle = open(mode, &device_id, a_tx, Some(b_tx))?;

            // Spawn mixer thread.
            let mixer_sample_rate_a = handle.sample_rate;
            // Falls back to 48 kHz only if Mix branch didn't populate it (shouldn't happen).
            let mixer_sample_rate_b = handle.mic_sample_rate.unwrap_or(48_000);
            let source_tx_for_mixer = source_tx.clone();
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
                        let _ = source_tx_for_mixer.try_send(mixed);
                    }
                }
            });
            handle
        } else {
            open(mode, &device_id, source_tx.clone(), None)?
        };

        let paused = Arc::new(AtomicBool::new(false));
        let stop_flag = Arc::new(AtomicBool::new(false));

        // Fan-out channels: writer is lossless (bounded 64), meter is lossy (bounded 8).
        let (writer_tx, writer_rx) = bounded::<Vec<f32>>(64);
        let (meter_tx, meter_rx) = bounded::<Vec<f32>>(8);

        // Spawn fan-out thread — sole consumer of source_rx, sole producer into
        // writer_rx and meter_rx.
        let fanout_join = spawn_fanout(source_rx, writer_tx, meter_tx, stop_flag.clone());

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

        // Spawn meter thread — consumes meter_rx (sole consumer of its own channel).
        let meter_app = app.clone();
        let meter_stop = stop_flag.clone();
        let meter_paused = paused.clone();
        let meter_sample_rate = stream.sample_rate;
        let meter_channels = stream.channels;
        let meter_join = std::thread::spawn(move || {
            let mut meter = Meter::new(meter_sample_rate, meter_channels);
            let mut clock = RecordedClock::new(Instant::now());
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
                    let is_paused = meter_paused.load(Ordering::Relaxed);
                    let recorded = clock.tick(is_paused, now);
                    let _ = meter_app.emit(
                        "audio:elapsed",
                        ElapsedEvent {
                            elapsed_sec: recorded.as_secs() as u32,
                        },
                    );
                }
            }
        });

        Ok(Self {
            stream,
            writer_join: Some(writer_join),
            meter_join: Some(meter_join),
            fanout_join: Some(fanout_join),
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

    /// Signals the worker threads to stop, joins them, and returns the finalised
    /// recording.
    ///
    /// **Join order:** set stop_flag → join fan-out → join writer → join meter.
    /// After the fan-out exits it drops `writer_tx`, so the writer observes a
    /// disconnected channel and cannot block forever waiting for more samples.
    ///
    /// **Blocking contract:** returns as soon as all three worker threads exit.
    /// The fan-out and writer each observe the stop flag at most every 50 ms
    /// (the `recv_timeout` interval). Worst-case latency: < 200 ms in normal
    /// operation (fan-out 50 ms + writer 50 ms + any in-flight write).
    pub fn stop(mut self) -> Result<(PathBuf, u64, u32), AudioError> {
        self.stop_flag.store(true, Ordering::Relaxed);

        // 1. Join fan-out first so writer_tx is dropped; writer then sees a
        //    disconnected channel and exits after draining in-flight buffers.
        let _ = self.fanout_join.take().unwrap().join();

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    // -----------------------------------------------------------------------
    // RecordedClock tests
    // -----------------------------------------------------------------------

    /// Construct with an explicit `now` so the first gap since construction
    /// is exactly zero; subsequent ticks use fabricated Instants.
    #[test]
    fn recorded_clock_first_tick_paused_gives_zero() {
        let t0 = Instant::now();
        let mut clock = RecordedClock::new(t0);
        // Tick immediately while paused — paused span is discarded.
        let t1 = t0 + Duration::from_millis(500);
        let recorded = clock.tick(true, t1);
        assert_eq!(recorded, Duration::ZERO, "paused tick must not accumulate");
    }

    #[test]
    fn recorded_clock_accumulates_when_not_paused() {
        let t0 = Instant::now();
        let mut clock = RecordedClock::new(t0);

        // 1 s running
        let t1 = t0 + Duration::from_secs(1);
        let r1 = clock.tick(false, t1);
        assert_eq!(r1, Duration::from_secs(1));

        // 1 s paused — must not add
        let t2 = t1 + Duration::from_secs(1);
        let r2 = clock.tick(true, t2);
        assert_eq!(r2, Duration::from_secs(1), "paused span must be frozen");

        // 2 s running — resumes from the paused value without a jump
        let t3 = t2 + Duration::from_secs(2);
        let r3 = clock.tick(false, t3);
        assert_eq!(
            r3,
            Duration::from_secs(3),
            "should accumulate 2 more seconds after resume"
        );
    }

    #[test]
    fn recorded_clock_no_jump_on_resume() {
        let t0 = Instant::now();
        let mut clock = RecordedClock::new(t0);

        // Run 5 s, pause 60 s, resume 1 s → total must be 6 s, not 66 s.
        clock.tick(false, t0 + Duration::from_secs(5));
        clock.tick(true, t0 + Duration::from_secs(65)); // 60 s paused
        let final_recorded = clock.tick(false, t0 + Duration::from_secs(66));
        assert_eq!(
            final_recorded,
            Duration::from_secs(6),
            "paused 60 s must not appear in recorded time"
        );
    }

    // -----------------------------------------------------------------------
    // Fan-out tests
    // -----------------------------------------------------------------------

    /// All N buffers sent through the source channel must arrive at the writer
    /// channel in order and byte-identical.
    #[test]
    fn fanout_delivers_all_buffers_to_writer_in_order() {
        let (source_tx, source_rx) = bounded::<Vec<f32>>(128);
        let (writer_tx, writer_rx) = bounded::<Vec<f32>>(128);
        let (meter_tx, meter_rx) = bounded::<Vec<f32>>(8);
        let stop = Arc::new(AtomicBool::new(false));

        let _fanout = spawn_fanout(source_rx, writer_tx, meter_tx, stop.clone());

        const N: usize = 100;
        for i in 0..N {
            let buf = vec![i as f32; 4]; // distinct payload per buffer
            source_tx.send(buf).unwrap();
        }
        // Drop source_tx so the fan-out exits naturally.
        drop(source_tx);

        // Consume whatever the meter received (we don't care about order/count).
        drop(meter_rx);

        // Collect all writer buffers.
        let mut received: Vec<Vec<f32>> = Vec::new();
        while let Ok(buf) = writer_rx.recv() {
            received.push(buf);
        }

        assert_eq!(received.len(), N, "writer must receive all {N} buffers");
        for (i, buf) in received.iter().enumerate() {
            assert_eq!(
                buf,
                &vec![i as f32; 4],
                "buffer {i} must be byte-identical and in order"
            );
        }
    }

    /// Writer path is lossless even when the meter channel is full and has no
    /// consumer (try_send drops silently; writer_tx.send blocks / backpressures).
    #[test]
    fn fanout_writer_lossless_when_meter_saturated() {
        let (source_tx, source_rx) = bounded::<Vec<f32>>(128);
        let (writer_tx, writer_rx) = bounded::<Vec<f32>>(256);
        // meter channel bounded(8) with no consumer — will fill up quickly.
        let (meter_tx, _meter_rx_not_consumed) = bounded::<Vec<f32>>(8);
        let stop = Arc::new(AtomicBool::new(false));

        let _fanout = spawn_fanout(source_rx, writer_tx, meter_tx, stop.clone());

        const N: usize = 100;
        for i in 0..N {
            source_tx.send(vec![i as f32]).unwrap();
        }
        drop(source_tx);

        let mut count = 0usize;
        while let Ok(_buf) = writer_rx.recv() {
            count += 1;
        }
        assert_eq!(
            count, N,
            "writer must receive all {N} buffers despite saturated meter"
        );
    }

    /// Fan-out terminates when the stop flag is set; writer_rx observes
    /// disconnect after any in-flight buffers are drained.
    #[test]
    fn fanout_terminates_on_stop_flag() {
        let (source_tx, source_rx) = bounded::<Vec<f32>>(64);
        let (writer_tx, writer_rx) = bounded::<Vec<f32>>(64);
        let (meter_tx, _meter_rx) = bounded::<Vec<f32>>(8);
        let stop = Arc::new(AtomicBool::new(false));

        let handle = spawn_fanout(source_rx, writer_tx, meter_tx, stop.clone());

        // Send a few buffers, then signal stop.
        source_tx.send(vec![1.0]).unwrap();
        source_tx.send(vec![2.0]).unwrap();
        stop.store(true, Ordering::Relaxed);
        // Drop source so the fan-out's recv_timeout can drain quickly.
        drop(source_tx);

        // Fan-out must exit — join must not hang.
        handle.join().expect("fanout thread must not panic");

        // writer_rx must be disconnected (fan-out dropped writer_tx on exit).
        // Drain whatever arrived, then confirm no more senders.
        while writer_rx.try_recv().is_ok() {}
        assert!(
            writer_rx.recv().is_err(),
            "writer_rx must be disconnected after fanout exits"
        );
    }

    /// Fan-out terminates when source_tx is dropped (natural end-of-stream),
    /// even without the stop flag.
    #[test]
    fn fanout_terminates_on_source_disconnect() {
        let (source_tx, source_rx) = bounded::<Vec<f32>>(64);
        let (writer_tx, writer_rx) = bounded::<Vec<f32>>(64);
        let (meter_tx, _meter_rx) = bounded::<Vec<f32>>(8);
        let stop = Arc::new(AtomicBool::new(false));

        let handle = spawn_fanout(source_rx, writer_tx, meter_tx, stop);

        drop(source_tx); // disconnect source — fan-out should exit
        handle
            .join()
            .expect("fanout must exit on source disconnect");

        // writer_rx must observe disconnect.
        while writer_rx.try_recv().is_ok() {}
        assert!(writer_rx.recv().is_err());
    }
}
