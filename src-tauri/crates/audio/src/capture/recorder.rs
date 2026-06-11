//! Orchestrates the audio worker pipeline.
//!
//! Owns:
//!   * `StreamHandle` from `stream.rs` (one or two streams)
//!   * Bounded channels forming the fan-out topology:
//!     `stream callback → source_rx → fan-out → writer_rx (lossless)`
//!     `                                       ↘ meter_rx  (lossy)`
//!   * Worker thread join handles + a shutdown flag
//!
//! **Teardown contract** (see `Recorder::stop`): the stream is dropped first,
//! which kills audio callbacks and (in Mix mode) causes the mixer thread to exit
//! — both are the sole owners of `source_tx` clones, so dropping them closes
//! `source_rx`. The fan-out observes `Disconnected`, drains its internal channel,
//! and exits (dropping `writer_tx`/`meter_tx`). The writer then drains `writer_rx`
//! and calls `finalize()`. The meter exits on its own `Disconnected`. The
//! `stop_flag` is set before the stream drop as a safety net in case a `source_tx`
//! clone is ever inadvertently retained; normal exit is via channel disconnect.
//!
//! Emits Tauri events on a background thread via `AppHandle::emit`.

use crate::capture::meter::Meter;
use crate::capture::mixer::Mixer;
use crate::capture::session::{AudioFormat, CaptureMode};
use crate::capture::stream::{open, StreamHandle};
use crate::capture::writer::{AudioWriter, FinalizeResult, FlacWriterImpl, WavWriterImpl};
use crate::error::{AudioError, AudioErrorEvent};
use crossbeam_channel::bounded;
use serde::Serialize;
use specta::Type;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
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
/// Exits when `stop` is set (safety-net Timeout arm) or `source_rx` disconnects
/// (normal drain-exit). Dropping `writer_tx` / `meter_tx` on exit signals the
/// downstream threads.
pub(crate) fn spawn_fanout(
    source_rx: crossbeam_channel::Receiver<Vec<f32>>,
    writer_tx: crossbeam_channel::Sender<Vec<f32>>,
    meter_tx: crossbeam_channel::Sender<Vec<f32>>,
    stop: Arc<AtomicBool>,
) -> JoinHandle<()> {
    std::thread::spawn(move || loop {
        match source_rx.recv_timeout(Duration::from_millis(50)) {
            Ok(buf) => {
                // Meter is lossy — avoid a wasted clone when the meter channel is full.
                if !meter_tx.is_full() {
                    let _ = meter_tx.try_send(buf.clone());
                }
                // Writer is lossless — blocking send provides backpressure.
                // The source channel absorbs bursts; if it fills, the stream
                // callback's existing try_send-drop-with-overflow-counter
                // behavior is the spec'd degradation path.
                if writer_tx.send(buf).is_err() {
                    break;
                }
            }
            // Safety net: only reachable if a source sender leaks; normal exit
            // is the Disconnected arm below.
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                if stop.load(Ordering::Relaxed) {
                    break;
                }
            }
            // Normal drain-exit: all source senders dropped AND channel emptied
            // (crossbeam delivers queued messages before reporting Disconnected).
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

            // Read the real source formats from the handle; warn on fallback (shouldn't happen).
            let loop_sample_rate = handle.loop_sample_rate.unwrap_or_else(|| {
                tracing::warn!(
                    "loop_sample_rate not populated in Mix handle; falling back to TARGET"
                );
                crate::capture::mixer::TARGET_SAMPLE_RATE
            });
            let loop_channels = handle.loop_channels.unwrap_or_else(|| {
                tracing::warn!("loop_channels not populated in Mix handle; falling back to 2");
                2
            });
            let mic_sample_rate = handle.mic_sample_rate.unwrap_or_else(|| {
                tracing::warn!(
                    "mic_sample_rate not populated in Mix handle; falling back to 48000"
                );
                48_000
            });
            let mic_channels = handle.mic_channels.unwrap_or_else(|| {
                tracing::warn!("mic_channels not populated in Mix handle; falling back to 1");
                1
            });

            // Clone the drops Arc for the mixer thread to sync dropped_frames back.
            let mixer_drops = handle.drops.clone();
            let source_tx_for_mixer = source_tx.clone();
            std::thread::spawn(move || {
                let mut mixer = match Mixer::new(
                    loop_sample_rate,
                    loop_channels,
                    mic_sample_rate,
                    mic_channels,
                ) {
                    Ok(m) => m,
                    Err(e) => {
                        tracing::error!(?e, "mixer init");
                        return;
                    }
                };

                // Drain the b_rx (mic) startup backlog before entering the main loop.
                // The mic callback fires immediately on stream open and can accumulate
                // up to 64 buffers (~640 ms) before the loopback thread starts. Those
                // buffered samples pre-date the first loopback callback, so the sync
                // logic in Mixer::mix() would have to discard them anyway — we discard
                // cheaply here instead of paying the full downmix+resample cost first.
                while b_rx.try_recv().is_ok() {}

                let mut last_synced: u32 = 0;
                loop {
                    // A (loopback) paces the loop via recv_timeout so that transient
                    // system silence (no app rendering audio → WASAPI delivers nothing)
                    // does not block the thread and starve B (mic). Old shape used
                    // a_rx.recv() (blocking) + b_rx.recv_timeout(50ms), which caused
                    // b_rx to fill (~64 buffers ≈ 640 ms) during silence → mic callback
                    // drops → spurious MixerOverflow toast at ~1.6 s of silence.
                    let mut a_buf = match a_rx.recv_timeout(Duration::from_millis(50)) {
                        Ok(v) => v,
                        Err(crossbeam_channel::RecvTimeoutError::Timeout) => Vec::new(),
                        // Teardown cascade: loopback stream dropped → a_rx disconnected.
                        // MUST exit here to allow the fan-out to observe Disconnected
                        // on source_rx and begin its own drain-exit.
                        Err(crossbeam_channel::RecvTimeoutError::Disconnected) => return,
                    };
                    // Catch up loopback bursts / uneven callback cadences.
                    while let Ok(more) = a_rx.try_recv() {
                        a_buf.extend(more);
                    }

                    // Mic (B) side: with A paced at ~10 ms (or 50 ms during silence),
                    // try_recv drain is enough — no blocking wait needed. B Disconnected
                    // (mic death) → empty Vec: recording continues with system audio only
                    // until ready-buffer cap; this is the documented Sub-2 semantics.
                    let mut b_buf: Vec<f32> = b_rx.try_recv().unwrap_or_default();
                    while let Ok(more) = b_rx.try_recv() {
                        b_buf.extend(more);
                    }

                    if let Ok(mixed) = mixer.mix(&a_buf, &b_buf) {
                        // Skip empty outputs — one side is waiting for the other.
                        if !mixed.is_empty() {
                            let _ = source_tx_for_mixer.try_send(mixed);
                        }
                    }
                    // Sync mixer overflow counter into the shared drops Arc.
                    let d = mixer.dropped_frames();
                    if d > last_synced {
                        mixer_drops.fetch_add(d - last_synced, Ordering::Relaxed);
                        last_synced = d;
                    }
                }
            });
            handle
        } else {
            open(mode, &device_id, source_tx.clone(), None)?
        };
        // NOTE: `source_tx` is NOT stored in `Self`. In Mic/System mode it was
        // cloned into the stream callback above; in Mix mode `source_tx_for_mixer`
        // is the only surviving clone (captured by the mixer thread). When the
        // stream is dropped in `stop()` the callbacks die, then the mixer exits
        // on `a_rx` disconnect — both clones are gone, closing `source_rx` and
        // unblocking the fan-out's Disconnected arm.

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

        let writer_app = app.clone();
        let writer_paused = paused.clone();
        let writer_stop = stop_flag.clone();
        let writer_join = std::thread::spawn(move || -> Result<FinalizeResult, AudioError> {
            let result = (|| -> Result<FinalizeResult, AudioError> {
                loop {
                    match writer_rx.recv_timeout(Duration::from_millis(50)) {
                        Ok(buf) => {
                            if !writer_paused.load(Ordering::Relaxed) {
                                writer.write(&buf)?;
                            }
                        }
                        // Safety net: only reachable if a source sender leaks; normal
                        // exit is the Disconnected arm below.
                        Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                            if writer_stop.load(Ordering::Relaxed) {
                                break;
                            }
                        }
                        // Normal drain-exit: fan-out dropped writer_tx AND all queued
                        // buffers have been consumed (crossbeam semantic).
                        Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
                    }
                }
                writer.finalize()
            })();
            if let Err(ref e) = result {
                let _ = writer_app.emit("audio:error", AudioErrorEvent::from(e));
            }
            result
        });

        // Spawn meter thread — consumes meter_rx (sole consumer of its own channel).
        let meter_app = app.clone();
        let meter_stop = stop_flag.clone();
        let meter_paused = paused.clone();
        let meter_sample_rate = stream.sample_rate;
        let meter_channels = stream.channels;
        // Clone the drops Arc so the meter thread can emit audio:error when pipeline overflows.
        let meter_drops: Arc<AtomicU32> = stream.drops.clone();
        let meter_join = std::thread::spawn(move || {
            let mut meter = Meter::new(meter_sample_rate, meter_channels);
            let mut clock = RecordedClock::new(Instant::now());
            let mut last_level = Instant::now();
            let mut last_wave = Instant::now();
            let mut last_elapsed = Instant::now();
            // Guard: emit audio:error at most once per session when drops >= 100.
            let mut overflow_emitted = false;
            loop {
                if meter_stop.load(Ordering::Relaxed) {
                    break;
                }
                match meter_rx.recv_timeout(Duration::from_millis(50)) {
                    Ok(buf) => meter.push(&buf),
                    Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
                    // Fan-out dropped meter_tx (pipeline dead or normal teardown).
                    Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
                }
                let now = Instant::now();
                // Tick every iteration (~50 ms granularity) so pause/resume
                // attribution is not quantized to 1-second boundaries.
                let recorded = clock.tick(meter_paused.load(Ordering::Relaxed), now);
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
                            elapsed_sec: recorded.as_secs() as u32,
                        },
                    );
                    // Check drops in the 1 Hz gate (cheap cadence). Emit audio:error
                    // exactly once per session when the pipeline drops >= 100.
                    if !overflow_emitted {
                        let dropped = meter_drops.load(Ordering::Relaxed);
                        if dropped >= 100 {
                            overflow_emitted = true;
                            let err = AudioError::MixerOverflow { dropped };
                            let _ = meter_app.emit("audio:error", AudioErrorEvent::from(&err));
                        }
                    }
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

    /// Signals the worker threads to stop and returns the finalised recording.
    ///
    /// **Teardown sequence:**
    /// 1. Capture `sample_rate`/`channels` from the stream handle (needed for the
    ///    duration calculation after the handle is consumed).
    /// 2. Set `stop_flag` (safety net against leaked `source_tx` clones).
    /// 3. Drop the stream — destroys audio callbacks (and causes the Mix mixer
    ///    thread to exit via `a_rx` disconnect), which drops all `source_tx`
    ///    clones. `source_rx` in the fan-out becomes disconnected.
    /// 4. Join fan-out — it drains `source_rx` until `Disconnected`, then exits,
    ///    dropping `writer_tx` and `meter_tx`.
    /// 5. Join writer — it drains `writer_rx` until `Disconnected` (all queued
    ///    buffers written), calls `finalize()`. FLAC now writes frames
    ///    incrementally during `write()`; `finalize()` only encodes the final
    ///    partial block and patches the STREAMINFO header (cheap).
    /// 6. Join meter — exits on `Disconnected` from the fan-out drop.
    ///
    /// **Lossless guarantee:** every buffer queued in `source_rx` and `writer_rx`
    /// before the stream drops is delivered to the file. Crossbeam yields queued
    /// messages before reporting `Disconnected`.
    ///
    /// **Stop flag role:** safety net only. Normal exit for every thread is the
    /// `Disconnected` arm. The flag ensures termination even if a `source_tx`
    /// clone is inadvertently retained elsewhere (50 ms Timeout arm fires).
    pub fn stop(self) -> Result<(PathBuf, u64, u32), AudioError> {
        // Destructure to allow partial moves (drop stream before joining threads).
        let Recorder {
            stream,
            mut writer_join,
            mut meter_join,
            mut fanout_join,
            stop_flag,
            tmp_path: _,
            paused: _,
        } = self;

        // Capture stream metadata before the handle is consumed.
        let sample_rate = stream.sample_rate;
        let channels = stream.channels;

        // Safety-net flag — set before stream drop so any poll-waiting threads
        // don't spin more than one 50 ms interval after teardown begins.
        stop_flag.store(true, Ordering::Relaxed);

        // 1. Drop stream first — terminates all audio callbacks, closing source_tx
        //    clones. Fan-out sees Disconnected on source_rx after draining.
        drop(stream);

        // 2. Join fan-out — drains source_rx losslessly, then drops writer_tx/meter_tx.
        let _ = fanout_join.take().unwrap().join();

        // 3. Join writer — drains writer_rx losslessly (Disconnected after fan-out
        //    drops writer_tx), then finalizes the file.
        let writer = writer_join
            .take()
            .unwrap()
            .join()
            .map_err(|_| AudioError::Other("writer thread panicked".into()))??;

        // 4. Join meter — exits on Disconnected (fan-out dropped meter_tx).
        let _ = meter_join.take().unwrap().join();

        let duration_sec =
            (writer.sample_count / (sample_rate as u64).max(1) / (channels as u64).max(1)) as u32;
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

    /// Fan-out terminates when the stop flag is set even when source_tx is
    /// still alive (the Timeout arm fires the safety-net check).
    #[test]
    fn fanout_terminates_on_stop_flag() {
        let (source_tx, source_rx) = bounded::<Vec<f32>>(64);
        let (writer_tx, writer_rx) = bounded::<Vec<f32>>(64);
        let (meter_tx, _meter_rx) = bounded::<Vec<f32>>(8);
        let stop = Arc::new(AtomicBool::new(false));

        let handle = spawn_fanout(source_rx, writer_tx, meter_tx, stop.clone());

        // Send a few buffers, then signal stop — keep source_tx alive so only
        // the Timeout/stop-flag arm can fire (not the Disconnected arm).
        source_tx.send(vec![1.0]).unwrap();
        source_tx.send(vec![2.0]).unwrap();
        stop.store(true, Ordering::Relaxed);

        // Fan-out must exit via the stop-flag Timeout arm — join must not hang.
        handle.join().expect("fanout thread must not panic");

        // Drop source_tx only after join to ensure it was alive throughout.
        drop(source_tx);

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

    /// Fan-out exits when writer_rx is dropped (blocking writer_tx.send wakes
    /// with Err). This pins the liveness property the whole teardown relies on:
    /// a rendezvous writer channel with no consumer unblocks when the receiver
    /// is dropped.
    #[test]
    fn fanout_exits_when_writer_rx_dropped() {
        // bounded(0) = rendezvous channel: send blocks until a receiver takes.
        let (source_tx, source_rx) = bounded::<Vec<f32>>(64);
        let (writer_tx, writer_rx) = bounded::<Vec<f32>>(0);
        let (meter_tx, _meter_rx) = bounded::<Vec<f32>>(8);
        let stop = Arc::new(AtomicBool::new(false));

        let handle = spawn_fanout(source_rx, writer_tx, meter_tx, stop);

        // Feed one buffer — fan-out will block on writer_tx.send because no
        // consumer is draining writer_rx.
        source_tx.send(vec![1.0]).unwrap();

        // Drop the writer receiver — crossbeam wakes the blocked sender with Err.
        drop(writer_rx);

        // Fan-out must exit (writer_tx.send returned Err → break).
        handle
            .join()
            .expect("fanout must exit when writer_rx is dropped");

        drop(source_tx);
    }
}
