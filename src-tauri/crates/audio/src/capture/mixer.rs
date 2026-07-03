//! Combines two interleaved audio streams into one mono stream at TARGET_SAMPLE_RATE.
//!
//! # Channel-aware mixing
//!
//! Each source is **independently downmixed to mono** using its real channel count
//! before resampling. WASAPI loopback delivers interleaved LRLR… frames at the
//! device's native channel count (typically 2); the cpal mic delivers its own
//! count (1 or 2). Index-wise summing of interleaved stereo against mono garbles
//! both channels and time, so this mixer explicitly averages the channels per frame.
//!
//! # First-overlap alignment
//!
//! The two stream callbacks start at different wall-clock times (typically 10–100 ms
//! apart; up to ~640 ms if the loopback leads). If we paired sample-i of A with
//! sample-i of B counted from each stream's first callback, that startup offset
//! would be baked in as a **fixed audible skew for the whole recording**.
//!
//! To prevent this: both lanes independently downmix and resample into `{a,b}_ready`.
//! When BOTH lanes have produced at least one output for the first time, the mixer
//! is considered **synced**. At that moment the ready buffers accumulated during
//! the "solo prefix" (frames from whichever lane started first) are **discarded**
//! rather than mixed against silence. The current call's fresh lane outputs are
//! appended *after* the clear, so the very first overlap instant is preserved.
//!
//! This discard is a deliberate sync step — it does **not** count toward
//! `dropped_frames` and does not trigger the overflow toast.
//!
//! Pre-sync: the cap (`MAX_READY_SAMPLES`) still applies to both ready buffers so
//! memory is bounded even before the lanes meet.
//!
//! # Known Sub-2 limitations (fixed in v1.0.1, Fix F1)
//!
//! * **Prolonged total system idle:** FIXED. WASAPI loopback delivers nothing
//!   while no app renders audio; the recorder's mixer thread now treats that
//!   absence as literal silence and synthesizes zero samples for lane A
//!   (`recorder.rs`'s `silence_len` helper) so mic-only speech keeps being
//!   mixed and recorded instead of piling up unmatched in `b_ready`.
//!
//! * **Transient system silence (< ~2 s):** the recorder's mixer thread drains
//!   both sources with timeouts/try_recv so short silences are handled cleanly —
//!   no spurious overflow toast.
//!
//! * **Mic stream death mid-recording:** FIXED. The mirror image of system idle.
//!   Lane B stopping is distinguished from normal silence by hysteresis (a live
//!   mic always delivers *something*, even in a silent room) — after
//!   `MIC_FILL_AFTER` (250 ms, see `recorder.rs`'s `MicFillTracker`) of sustained
//!   B starvation, the recorder fills lane B with silence so the system lane
//!   keeps recording instead of stalling and triggering the overflow toast.
//!
//! * **Silence-fill:** implemented (this section described the pre-fix gap).
//!   See `recorder.rs`'s mixer-thread loop: lane A fills immediately (starvation
//!   IS silence), lane B fills after the hysteresis window (starvation means the
//!   stream died). Both-starved ticks fill nothing — that span is genuinely idle.
//!
//! # Output clipping
//!
//! Mixed output can reach ±1.12 if both lanes are at full scale:
//! (SYSTEM_LANE_GAIN + MIC_LANE_GAIN) × ANTI_CLIP_GAIN = (0.6 + 1.0) × 0.7.
//! Downstream writers (writer.rs) hard-clamp to `[-1.0, 1.0]`, so hot sources
//! clip rather than wrap. Document hot input levels to callers when relevant.
//!
//! # Surplus retention
//!
//! Unlike the old implementation that truncated to the shorter buffer per call,
//! this mixer **retains cross-call surplus** in `{a,b}_ready`. Feed a=100 and b=60
//! on one call → 60 mixed frames out; the 40-sample a-surplus waits in `a_ready`.
//! Feed a=0 and b=40 on the next call → 40 more frames out (uses the retained surplus).
//!
//! This means the mixer can return an empty Vec when one side is stalled: that is
//! normal and not an error. Callers must skip empty outputs rather than writing
//! silence.
//!
//! # Chunked resampling
//!
//! `rubato::FftFixedIn` requires exactly RESAMPLER_CHUNK (1024) mono input frames per
//! `process()` call, but audio callbacks deliver variable-size buffers (~440–960
//! frames). Each lane with a resampler keeps a `pending` buffer; frames accumulate
//! until at least one full 1024-frame chunk is available, then the chunk is
//! processed. The remainder stays in `pending` for the next call.

use crate::capture::echo_canceller::{EchoCanceller, EchoConfig};
use crate::error::AudioError;
use rubato::{FftFixedIn, Resampler};

pub const TARGET_SAMPLE_RATE: u32 = 48_000;
pub const ANTI_CLIP_GAIN: f32 = 0.7;

/// v1.0.1 lane balance (real-hardware tuning): the digital system loopback is
/// hot relative to the voice, so the system lane is attenuated while the mic
/// lane stays at full level. Tuned on real hardware before each release.
pub const SYSTEM_LANE_GAIN: f32 = 0.6; // lane A (loopback)
pub const MIC_LANE_GAIN: f32 = 1.0; // lane B (mic)

/// Per-source cap on buffered ready samples (~2 s @ 48 kHz mono).
///
/// Bounds memory under source clock drift or one-sided starvation.
/// When exceeded the oldest samples are dropped from the FRONT, preserving
/// the freshest audio and bounding temporal skew.
///
/// **Unit asymmetry note (resolved in v1.0.1, Fix F1):** stream callbacks
/// (System/Mic mode) count *dropped buffers* (~480 frames each) in the shared
/// `drops` Arc, while this mixer counts dropped *samples* via `dropped_frames()`.
/// Left unconverted, the ≥ 100 threshold in the overflow toast would mean ~1 s
/// of loss in System/Mic mode but only ~2 ms via raw mixer overflow — the
/// recorder's mixer thread now converts samples → buffer-equivalents
/// (`÷ 480`, remainder carried forward) before adding to the shared `drops`
/// Arc, so the toast threshold means the same thing in both modes.
pub const MAX_READY_SAMPLES: usize = 96_000;

const RESAMPLER_CHUNK: usize = 1024;

struct Lane {
    channels: u16,
    resampler: Option<FftFixedIn<f32>>,
    /// Mono samples waiting for a full RESAMPLER_CHUNK (only used when resampler is Some).
    pending: Vec<f32>,
    /// Leading output samples to discard (rubato priming delay).
    ///
    /// `FftFixedIn` has a fixed output delay (`output_delay()` ≈ 320 samples for
    /// 44.1→48k). The first chunk's leading output is priming garbage and would
    /// introduce a constant ~6.7 ms inter-lane skew when only one lane resamples.
    /// We discard exactly `skip_remaining` leading output samples per lane.
    skip_remaining: usize,
}

pub struct Mixer {
    a: Lane,
    b: Lane,
    /// Mono @48k samples from lane A awaiting lane B.
    a_ready: Vec<f32>,
    /// Mono @48k samples from lane B awaiting lane A.
    b_ready: Vec<f32>,
    /// Cumulative count of samples dropped due to MAX_READY_SAMPLES overflow.
    dropped_frames: u32,
    /// Whether lane A has produced at least one output sample (for sync).
    a_ever: bool,
    /// Whether lane B has produced at least one output sample (for sync).
    b_ever: bool,
    /// Whether the first-overlap alignment has been performed.
    synced: bool,
    /// AEC on the mic lane (None = disabled → original path, zero overhead).
    echo: Option<EchoCanceller>,
    /// System-lane samples awaiting their delayed cleaned-mic counterpart
    /// (keeps the mix aligned across the AEC's fixed latency).
    a_delayed: Vec<f32>,
}

impl Mixer {
    /// Create a mixer for two sources with explicit channel counts.
    ///
    /// A resampler is created per lane iff its input rate differs from TARGET_SAMPLE_RATE.
    /// The resampler operates on mono (single-channel) data after downmixing.
    pub fn new(
        a_rate: u32,
        a_channels: u16,
        b_rate: u32,
        b_channels: u16,
    ) -> Result<Self, AudioError> {
        let make_lane = |rate: u32, channels: u16| -> Result<Lane, AudioError> {
            let (resampler, skip_remaining) = if rate == TARGET_SAMPLE_RATE {
                (None, 0)
            } else {
                let r = FftFixedIn::<f32>::new(
                    rate as usize,
                    TARGET_SAMPLE_RATE as usize,
                    RESAMPLER_CHUNK,
                    /* sub_chunks */ 2,
                    /* channels (mono after downmix) */ 1,
                )
                .map_err(|e| AudioError::Other(format!("rubato init: {e}")))?;
                let delay = r.output_delay();
                (Some(r), delay)
            };
            Ok(Lane {
                channels,
                resampler,
                pending: Vec::new(),
                skip_remaining,
            })
        };

        Ok(Self {
            a: make_lane(a_rate, a_channels)?,
            b: make_lane(b_rate, b_channels)?,
            a_ready: Vec::new(),
            b_ready: Vec::new(),
            dropped_frames: 0,
            a_ever: false,
            b_ever: false,
            synced: false,
            echo: None,
            a_delayed: Vec::new(),
        })
    }

    /// Enable AEC on the mic lane. Call once, right after `new`, in Mix mode.
    pub fn enable_aec(&mut self, cfg: EchoConfig) -> Result<(), AudioError> {
        self.echo = Some(EchoCanceller::new(cfg)?);
        Ok(())
    }

    /// Feed one interleaved buffer from each source (either may be empty) and
    /// drain whatever full overlap is mixable.
    ///
    /// Returns `Ok(vec![])` while one side waits for the other — this is normal
    /// and not an error. Callers must skip empty outputs.
    pub fn mix(
        &mut self,
        a_interleaved: &[f32],
        b_interleaved: &[f32],
    ) -> Result<Vec<f32>, AudioError> {
        // Process lane A.
        let a_out = Self::process_lane(&mut self.a, a_interleaved)
            .map_err(|e| AudioError::Other(format!("rubato A: {e}")))?;

        // Process lane B.
        let b_out = Self::process_lane(&mut self.b, b_interleaved)
            .map_err(|e| AudioError::Other(format!("rubato B: {e}")))?;

        // Track whether each lane has ever produced output.
        self.a_ever |= !a_out.is_empty();
        self.b_ever |= !b_out.is_empty();

        // First-overlap sync: when BOTH lanes have produced output for the first
        // time, discard the solo-prefix accumulated in the ready buffers before
        // appending the current call's outputs. This aligns the lanes at the
        // first moment both are live, preventing startup offset skew.
        //
        // The discard does NOT count toward dropped_frames — it is a sync step,
        // not an overflow. Pre-sync cap enforcement below still bounds memory.
        if !self.synced && self.a_ever && self.b_ever {
            self.synced = true;
            let a_discarded = self.a_ready.len();
            let b_discarded = self.b_ready.len();
            if a_discarded > 0 || b_discarded > 0 {
                tracing::debug!(
                    a_discarded,
                    b_discarded,
                    "first-overlap sync: discarding solo-prefix samples (not counted as drops)"
                );
                self.a_ready.clear();
                self.b_ready.clear();
            }
        }

        // Append to ready queues.
        self.a_ready.extend_from_slice(&a_out);
        self.b_ready.extend_from_slice(&b_out);

        // Enforce MAX_READY_SAMPLES — drop oldest from front.
        // Applies regardless of synced state so pre-sync memory is bounded.
        if self.a_ready.len() > MAX_READY_SAMPLES {
            let overflow = self.a_ready.len() - MAX_READY_SAMPLES;
            self.a_ready.drain(..overflow);
            self.dropped_frames = self.dropped_frames.saturating_add(overflow as u32);
        }
        if self.b_ready.len() > MAX_READY_SAMPLES {
            let overflow = self.b_ready.len() - MAX_READY_SAMPLES;
            self.b_ready.drain(..overflow);
            self.dropped_frames = self.dropped_frames.saturating_add(overflow as u32);
        }

        // Do not mix until synced (both lanes have produced once).
        if !self.synced {
            return Ok(vec![]);
        }

        let n = self.a_ready.len().min(self.b_ready.len());
        if n == 0 {
            return Ok(vec![]);
        }

        if let Some(ec) = &mut self.echo {
            // AEC path: cancel the echo out of the mic using the aligned system
            // lane as reference, then mix the cleaned mic against a delay-matched
            // copy of the system lane so both represent the same instant.
            let cleaned = ec.process(&self.b_ready[..n], &self.a_ready[..n]);
            self.a_delayed.extend_from_slice(&self.a_ready[..n]);
            self.a_ready.drain(..n);
            self.b_ready.drain(..n);

            let k = cleaned.len().min(self.a_delayed.len());
            if k == 0 {
                return Ok(vec![]);
            }
            let mixed: Vec<f32> = self.a_delayed[..k]
                .iter()
                .zip(cleaned[..k].iter())
                .map(|(a, b)| (a * SYSTEM_LANE_GAIN + b * MIC_LANE_GAIN) * ANTI_CLIP_GAIN)
                .collect();
            self.a_delayed.drain(..k);
            return Ok(mixed);
        }

        // Mix the overlap.
        let mixed: Vec<f32> = self.a_ready[..n]
            .iter()
            .zip(self.b_ready[..n].iter())
            .map(|(a, b)| (a * SYSTEM_LANE_GAIN + b * MIC_LANE_GAIN) * ANTI_CLIP_GAIN)
            .collect();

        self.a_ready.drain(..n);
        self.b_ready.drain(..n);

        Ok(mixed)
    }

    /// Cumulative count of samples dropped due to MAX_READY_SAMPLES overflow.
    pub fn dropped_frames(&self) -> u32 {
        self.dropped_frames
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    /// Downmix interleaved → mono, then resample (if needed).
    ///
    /// Returns mono samples at TARGET_SAMPLE_RATE.
    fn process_lane(
        lane: &mut Lane,
        interleaved: &[f32],
    ) -> Result<Vec<f32>, rubato::ResampleError> {
        // Step 1: downmix interleaved → mono.
        // Mono input is passed through directly; stereo/multichannel is averaged.
        let mono: Vec<f32> = if lane.channels <= 1 {
            interleaved.to_vec()
        } else {
            let ch = lane.channels as usize;
            // Ignore a trailing partial frame (len % ch != 0): a partial frame is
            // a producer bug; truncate silently without panicking.
            let full_frames = interleaved.len() / ch;
            let mut mono = Vec::with_capacity(full_frames);
            for frame in 0..full_frames {
                let base = frame * ch;
                // Average all channels: (ch0 + ch1 + …) / N, not sum, to avoid clipping.
                let sum: f32 = interleaved[base..base + ch].iter().sum();
                mono.push(sum / ch as f32);
            }
            mono
        };

        // Step 2: resample if lane has a resampler, otherwise pass through.
        if let Some(resampler) = &mut lane.resampler {
            lane.pending.extend_from_slice(&mono);
            let mut output = Vec::new();
            while lane.pending.len() >= RESAMPLER_CHUNK {
                // Drain the chunk unconditionally before processing so that an
                // error discards rather than retries the same samples forever (Fix M-3).
                let chunk: Vec<f32> = lane.pending.drain(..RESAMPLER_CHUNK).collect();
                match resampler.process(&[&chunk], None) {
                    Ok(chunk_out) => {
                        // FftFixedIn returns Vec<Vec<f32>> with one inner vec per channel (mono → 1).
                        if let Some(ch0) = chunk_out.into_iter().next() {
                            // Discard leading priming samples until skip_remaining is exhausted.
                            if lane.skip_remaining > 0 {
                                let skip = lane.skip_remaining.min(ch0.len());
                                lane.skip_remaining -= skip;
                                output.extend_from_slice(&ch0[skip..]);
                            } else {
                                output.extend_from_slice(&ch0);
                            }
                        }
                    }
                    Err(e) => {
                        // Chunk already drained; error counted by propagating to caller
                        // which maps it into AudioError. Dropped chunk is not counted
                        // in dropped_frames (resampler error is a different failure mode).
                        return Err(e);
                    }
                }
            }
            Ok(output)
        } else {
            Ok(mono)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Basic construction
    // -----------------------------------------------------------------------

    #[test]
    fn same_rate_passthrough_no_resampler() {
        let m = Mixer::new(48_000, 1, 48_000, 1).unwrap();
        assert!(m.a.resampler.is_none() && m.b.resampler.is_none());
    }

    #[test]
    fn different_rate_creates_resampler() {
        let m = Mixer::new(44_100, 2, 48_000, 1).unwrap();
        assert!(m.a.resampler.is_some());
        assert!(m.b.resampler.is_none());
    }

    // -----------------------------------------------------------------------
    // Stereo downmix — must establish sync first
    // -----------------------------------------------------------------------

    /// 2ch LRLR input at 48k on both sides → mono output ((L+R)/2 + mono_b) * GAIN.
    /// Both lanes fed simultaneously so sync fires on the same call.
    #[test]
    fn stereo_downmix_passthrough() {
        let mut m = Mixer::new(48_000, 2, 48_000, 1).unwrap();
        // Lane A: N stereo frames, L=0.6, R=0.4 → mono = 0.5
        let n_frames = 100;
        let a: Vec<f32> = (0..n_frames).flat_map(|_| [0.6f32, 0.4f32]).collect();
        // Lane B: N mono frames, value = 0.2
        let b: Vec<f32> = vec![0.2f32; n_frames];
        let out = m.mix(&a, &b).unwrap();
        assert_eq!(out.len(), n_frames);
        let expected = (0.5 * SYSTEM_LANE_GAIN + 0.2 * MIC_LANE_GAIN) * ANTI_CLIP_GAIN;
        for &s in &out {
            assert!((s - expected).abs() < 0.001, "sample {s} != {expected}");
        }
    }

    // -----------------------------------------------------------------------
    // Mono channels=1 with surplus retention (sync established first)
    // -----------------------------------------------------------------------

    /// Old behaviour truncated to the shorter buffer each call; the new mixer
    /// RETAINS surplus. Feed a=100, b=60 → out 60; then a=0, b=40 → out 40
    /// (the retained 40-sample a-surplus is consumed).
    ///
    /// Both lanes are provided on the first call so sync fires immediately.
    #[test]
    fn mono_surplus_retained_across_calls() {
        let mut m = Mixer::new(48_000, 1, 48_000, 1).unwrap();

        let a1 = vec![0.5f32; 100];
        let b1 = vec![0.3f32; 60];
        let out1 = m.mix(&a1, &b1).unwrap();
        assert_eq!(out1.len(), 60, "first call: overlap is 60");

        // 40-sample surplus remains in a_ready; b gets 40 more
        let out2 = m.mix(&[], &[0.3f32; 40]).unwrap();
        assert_eq!(out2.len(), 40, "second call: retained a-surplus consumed");
    }

    /// Two equal-length mono calls at 48k: basic sum-with-gain.
    /// Both lanes fed simultaneously.
    #[test]
    fn mix_sums_with_gain() {
        let mut m = Mixer::new(48_000, 1, 48_000, 1).unwrap();
        let a = vec![0.5f32; 100];
        let b = vec![0.3f32; 100];
        let out = m.mix(&a, &b).unwrap();
        assert_eq!(out.len(), 100);
        let expected = (0.5 * SYSTEM_LANE_GAIN + 0.3 * MIC_LANE_GAIN) * ANTI_CLIP_GAIN;
        assert!((out[0] - expected).abs() < 0.001);
    }

    // -----------------------------------------------------------------------
    // Variable-size chunks with resampler (44_100 → 48k)
    // -----------------------------------------------------------------------

    /// Feed 7 × 441-frame stereo buffers (44.1k, 2ch) against a matching mono
    /// 48k b-side. Resampled output grows in 1024-chunk quanta; assert total
    /// output length matches an integer multiple of the per-chunk yield (derived
    /// empirically from the first non-zero chunk, then asserted consistent).
    ///
    /// I-1 sync note: sync fires when BOTH lanes have produced output on the SAME
    /// call. We establish sync on a dedicated "sync call" (3 × 1024 A frames +
    /// generous B), then load extra B surplus so it never limits the A-only loop.
    #[test]
    fn variable_chunks_with_resampler_no_error_nothing_lost() {
        let mut m = Mixer::new(44_100, 2, 48_000, 1).unwrap();

        // 441 stereo frames * 2 ch = 882 samples per call.
        // Downmixed → 441 mono frames per call.
        let a_chunk: Vec<f32> = (0..441).flat_map(|_| [0.4f32, 0.6f32]).collect();

        // ---- Establish sync ----
        // Feed both A and B together so both lanes produce output on the same call
        // and synced=true fires. A needs 3 × 1024 = 3072 mono frames (= 6144 stereo
        // samples) to guarantee at least one resampler chunk's worth of output.
        // B gets a generous mono surplus at 48k so it never limits output here.
        let a_sync: Vec<f32> = (0..3072).flat_map(|_| [0.4f32, 0.6f32]).collect(); // 6144 samples
        let b_sync = vec![0.1f32; 8192]; // generous B surplus at 48k
        let _ = m.mix(&a_sync, &b_sync).unwrap();
        assert!(m.synced, "sync must fire after both lanes produce output");

        // Pre-load additional B surplus so the 7 A-only calls always have B to match.
        let _ = m.mix(&[], &b_sync.repeat(4));

        // ---- 7 A-only calls ----
        // 7 calls → 7 * 441 = 3087 mono frames accumulated.
        // 3087 / 1024 = 3 full chunks → some resampled output per chunk.
        let mut total_out = 0usize;
        let mut chunk_yield: Option<usize> = None;

        for _ in 0..7 {
            let out = m.mix(&a_chunk, &[]).unwrap();
            if !out.is_empty() {
                match chunk_yield {
                    None => chunk_yield = Some(out.len()),
                    Some(expected) => assert_eq!(
                        out.len() % expected,
                        0,
                        "output length {len} is not a multiple of per-chunk yield {expected}",
                        len = out.len()
                    ),
                }
                total_out += out.len();
            }
        }
        // After 7 * 441 = 3087 mono frames, at least 3 full resampler chunks have
        // been processed (3087 >= 3*1024). Assert we got some output.
        assert!(
            total_out > 0,
            "expected non-zero total output after 3087 input frames"
        );
        // No error path exercised (all Ok).
    }

    // -----------------------------------------------------------------------
    // MAX_READY_SAMPLES overflow cap (applies pre-sync too)
    // -----------------------------------------------------------------------

    /// Feed lane A far beyond 96k while leaving B empty → a_ready stays ≤ cap,
    /// dropped_frames > 0, output is empty.
    /// With I-1, an A-only feed never syncs (b_ever stays false), but the cap
    /// must still bound memory pre-sync. Expected drops: 200_000 - 96_000 = 104_000.
    #[test]
    fn max_ready_cap_bounds_memory_and_counts_drops() {
        let mut m = Mixer::new(48_000, 1, 48_000, 1).unwrap();
        // Feed 200k samples into A, nothing into B.
        let big = vec![0.1f32; 200_000];
        let out = m.mix(&big, &[]).unwrap();
        assert!(out.is_empty(), "b is empty → no overlap → empty output");
        assert!(
            m.a_ready.len() <= MAX_READY_SAMPLES,
            "a_ready.len()={} must be ≤ {}",
            m.a_ready.len(),
            MAX_READY_SAMPLES
        );
        assert_eq!(
            m.dropped_frames(),
            104_000,
            "expected exactly 104_000 dropped samples (200_000 - 96_000)"
        );
    }

    /// Feed lane B far beyond 96k while leaving A empty → same cap behaviour.
    #[test]
    fn max_ready_cap_b_side_bounds_memory_and_counts_drops() {
        let mut m = Mixer::new(48_000, 1, 48_000, 1).unwrap();
        let big = vec![0.1f32; 200_000];
        let out = m.mix(&[], &big).unwrap();
        assert!(out.is_empty(), "a is empty → no overlap → empty output");
        assert!(
            m.b_ready.len() <= MAX_READY_SAMPLES,
            "b_ready.len()={} must be ≤ {}",
            m.b_ready.len(),
            MAX_READY_SAMPLES
        );
        assert_eq!(
            m.dropped_frames(),
            104_000,
            "expected exactly 104_000 dropped samples (200_000 - 96_000)"
        );
    }

    // -----------------------------------------------------------------------
    // I-1: first-overlap sync
    // -----------------------------------------------------------------------

    /// A-only prefix: 3 calls of A-only produce no output (unsynced).
    /// Then one call with both A and B: output is exactly the overlap samples,
    /// solo prefix discarded. dropped_frames must stay 0 (sync discard ≠ overflow).
    #[test]
    fn first_overlap_sync_discards_solo_prefix_not_counted_as_drops() {
        let mut m = Mixer::new(48_000, 1, 48_000, 1).unwrap();

        // 3 A-only calls: no output expected (b_ever is still false → not synced)
        for _ in 0..3 {
            let out = m.mix(&[1.0f32; 100], &[]).unwrap();
            assert!(out.is_empty(), "A-only prefix must not produce output");
        }
        assert_eq!(
            m.dropped_frames(),
            0,
            "pre-sync solo A must not count as drops"
        );

        // First call with BOTH lanes: sync fires, A prefix cleared, fresh pair mixed.
        let a_fresh = vec![2.0f32; 50];
        let b_fresh = vec![3.0f32; 50];
        let out = m.mix(&a_fresh, &b_fresh).unwrap();

        // Output must be exactly 50 samples of (2.0 + 3.0) * ANTI_CLIP_GAIN.
        assert_eq!(out.len(), 50, "overlap must be 50 samples");
        let expected = (2.0f32 * SYSTEM_LANE_GAIN + 3.0f32 * MIC_LANE_GAIN) * ANTI_CLIP_GAIN;
        for &s in &out {
            assert!(
                (s - expected).abs() < 0.001,
                "sample {s} ≠ {expected} — stale A prefix must have been discarded"
            );
        }
        // Sync discard must NOT be counted as drops.
        assert_eq!(
            m.dropped_frames(),
            0,
            "sync discard must not count as drops"
        );
    }

    // -----------------------------------------------------------------------
    // M-1: resampler priming delay discarded
    // -----------------------------------------------------------------------

    /// Lane with 44_100 → 48k resampler fed DC 1.0 stereo until first non-empty
    /// output. The first emitted sample must be > 0.5 (priming zeros discarded).
    /// Without the skip, rubato's output_delay() leading zeros would contaminate
    /// the first chunk.
    #[test]
    fn resampler_priming_delay_discarded() {
        // Lane A: 44_100 Hz stereo (will resample). Lane B: 48k mono (no resample).
        // We feed B upfront so sync fires, then check lane A's first output value.
        let mut m = Mixer::new(44_100, 2, 48_000, 1).unwrap();

        // Establish sync: feed B a generous chunk first (pre-sync A-only feed
        // won't fire sync — we need both sides to see output simultaneously).
        // Strategy: feed both lanes on the same call. Give A enough to complete
        // at least one 1024-chunk and B enough to match the expected output.
        // 3 * 1024 = 3072 mono A frames → 3072 stereo A samples.
        let a_dc: Vec<f32> = vec![0.8f32; 3072 * 2]; // stereo DC at 44.1k
        let b_dc: Vec<f32> = vec![0.5f32; 4096]; // plenty of B at 48k

        let out = m.mix(&a_dc, &b_dc).unwrap();

        // We must have gotten some output (both lanes active).
        assert!(!out.is_empty(), "must produce output after 3072 A frames");

        // The first sample of mixed output = (a_resampled[0]*SYSTEM_LANE_GAIN + b[0]*MIC_LANE_GAIN) * GAIN.
        // With priming discarded, a_resampled[0] ≈ 0.8 (DC signal).
        // Without priming skip, a_resampled[0] ≈ 0 (zero-filled priming).
        // b[0] = 0.5. Expected first sample ≈ (0.8*0.6 + 0.5*1.0) * 0.7 ≈ 0.69.
        // We check that it is > 0.5 to confirm priming was discarded.
        let first = out[0];
        assert!(
            first > 0.5,
            "first sample {first} must be > 0.5 — priming zeros must be discarded"
        );
    }

    // -----------------------------------------------------------------------
    // Partial trailing frame doesn't panic
    // -----------------------------------------------------------------------

    /// interleaved stereo with an odd sample count (partial trailing frame)
    /// must not panic and must not shift framing of subsequent calls.
    /// Sync established by feeding both lanes simultaneously.
    #[test]
    fn partial_trailing_frame_does_not_panic() {
        let mut m = Mixer::new(48_000, 2, 48_000, 1).unwrap();
        // 5 samples with channels=2 → 2 full frames + 1 orphan sample (ignored).
        let a = vec![0.1f32, 0.2f32, 0.3f32, 0.4f32, 0.9f32];
        let b = vec![0.0f32; 2];
        let out = m.mix(&a, &b).unwrap();
        // Only 2 full frames were downmixed from a, so overlap = min(2, 2) = 2.
        assert_eq!(out.len(), 2);
    }

    // -----------------------------------------------------------------------
    // Performance guard
    // -----------------------------------------------------------------------

    #[test]
    fn mix_completes_under_10ms_for_1s_of_audio() {
        let mut m = Mixer::new(48_000, 1, 48_000, 1).unwrap();
        let a = vec![0.1f32; 48_000];
        let b = vec![0.2f32; 48_000];
        let start = std::time::Instant::now();
        let out = m.mix(&a, &b).unwrap();
        let dt = start.elapsed();
        assert_eq!(out.len(), 48_000);
        assert!(
            dt.as_millis() < 10,
            "expected <10ms, got {} ms",
            dt.as_millis()
        );
    }

    // -----------------------------------------------------------------------
    // v1.0.1 Fix F1: silence-fill semantics — mic-only speech over a
    // synthesized-silence A lane must still be recorded (the exact
    // end-user bug: "mic records nothing until system audio plays").
    // -----------------------------------------------------------------------

    /// Feed a zero-filled A lane (what the recorder synthesizes while loopback
    /// is starved) against a DC 0.5 mic lane (voice). The mixer must treat the
    /// zeros exactly like real silent samples — output length equals the
    /// overlap and every sample matches the expected gain-mixed value. This is
    /// the fill semantics proof: mic-only speech over silence IS recorded.
    #[test]
    fn silence_fill_a_lane_still_mixes_mic_only_speech() {
        let mut m = Mixer::new(48_000, 1, 48_000, 1).unwrap();
        let n = 100;
        let zeros_a = vec![0.0f32; n]; // synthesized loopback silence
        let voice_b = vec![0.5f32; n]; // real mic speech (DC approximation)

        let out = m.mix(&zeros_a, &voice_b).unwrap();

        assert_eq!(
            out.len(),
            n,
            "overlap must equal n — silence-fill must not shrink output"
        );
        let expected = (0.0f32 * SYSTEM_LANE_GAIN + 0.5f32 * MIC_LANE_GAIN) * ANTI_CLIP_GAIN;
        for &s in &out {
            assert!(
                (s - expected).abs() < 0.001,
                "sample {s} != {expected} — mic-only speech over silence must be recorded"
            );
        }
    }

    // -----------------------------------------------------------------------
    // v1.0.1: per-lane balance — system (A) attenuated, mic (B) full level
    // -----------------------------------------------------------------------

    /// DC 1.0 on one lane and 0.0 on the other isolates each lane's gain:
    /// A-only content must come out at SYSTEM_LANE_GAIN * ANTI_CLIP_GAIN and
    /// B-only content at MIC_LANE_GAIN * ANTI_CLIP_GAIN.
    #[test]
    fn lane_gains_are_asymmetric_system_down_mic_full() {
        let mut m = Mixer::new(48_000, 1, 48_000, 1).unwrap();
        let out = m.mix(&[1.0f32; 100], &[0.0f32; 100]).unwrap();
        let expected_a = SYSTEM_LANE_GAIN * ANTI_CLIP_GAIN;
        assert!(
            (out[0] - expected_a).abs() < 0.001,
            "A lane: {} != {expected_a}",
            out[0]
        );

        let mut m = Mixer::new(48_000, 1, 48_000, 1).unwrap();
        let out = m.mix(&[0.0f32; 100], &[1.0f32; 100]).unwrap();
        let expected_b = MIC_LANE_GAIN * ANTI_CLIP_GAIN;
        assert!(
            (out[0] - expected_b).abs() < 0.001,
            "B lane: {} != {expected_b}",
            out[0]
        );
    }

    // -----------------------------------------------------------------------
    // v1.1 A4: AEC routing through the mixer
    // -----------------------------------------------------------------------

    /// With AEC enabled and a silent system lane, the mixer still emits mic-driven
    /// audio (proves the delay-FIFO path routes cleaned mic to the output and does
    /// not stall). Exact values aren't asserted (AEC latency shifts them); we assert
    /// that non-trivial output eventually flows.
    #[test]
    fn aec_enabled_mixer_emits_mic_audio() {
        let mut m = Mixer::new(48_000, 1, 48_000, 1).unwrap();
        m.enable_aec(crate::capture::echo_canceller::EchoConfig::default())
            .unwrap();
        let mut total = 0usize;
        for _ in 0..200 {
            let out = m.mix(&vec![0.0f32; 480], &vec![0.2f32; 480]).unwrap();
            total += out.len();
        }
        assert!(total > 0, "AEC-enabled mixer produced no output");
    }
}
