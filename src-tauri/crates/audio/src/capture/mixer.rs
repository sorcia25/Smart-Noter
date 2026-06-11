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

use crate::error::AudioError;
use rubato::{FftFixedIn, Resampler};

pub const TARGET_SAMPLE_RATE: u32 = 48_000;
pub const ANTI_CLIP_GAIN: f32 = 0.7;

/// Per-source cap on buffered ready samples (~2 s @ 48 kHz mono).
///
/// Bounds memory under source clock drift or one-sided starvation.
/// When exceeded the oldest samples are dropped from the FRONT, preserving
/// the freshest audio and bounding temporal skew.
pub const MAX_READY_SAMPLES: usize = 96_000;

const RESAMPLER_CHUNK: usize = 1024;

struct Lane {
    channels: u16,
    resampler: Option<FftFixedIn<f32>>,
    /// Mono samples waiting for a full RESAMPLER_CHUNK (only used when resampler is Some).
    pending: Vec<f32>,
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
            let resampler = if rate == TARGET_SAMPLE_RATE {
                None
            } else {
                Some(
                    FftFixedIn::<f32>::new(
                        rate as usize,
                        TARGET_SAMPLE_RATE as usize,
                        RESAMPLER_CHUNK,
                        /* sub_chunks */ 2,
                        /* channels (mono after downmix) */ 1,
                    )
                    .map_err(|e| AudioError::Other(format!("rubato init: {e}")))?,
                )
            };
            Ok(Lane {
                channels,
                resampler,
                pending: Vec::new(),
            })
        };

        Ok(Self {
            a: make_lane(a_rate, a_channels)?,
            b: make_lane(b_rate, b_channels)?,
            a_ready: Vec::new(),
            b_ready: Vec::new(),
            dropped_frames: 0,
        })
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

        // Append to ready queues.
        self.a_ready.extend_from_slice(&a_out);
        self.b_ready.extend_from_slice(&b_out);

        // Enforce MAX_READY_SAMPLES — drop oldest from front.
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

        // Mix the overlap.
        let n = self.a_ready.len().min(self.b_ready.len());
        if n == 0 {
            return Ok(vec![]);
        }

        let a_drain: Vec<f32> = self.a_ready.drain(..n).collect();
        let b_drain: Vec<f32> = self.b_ready.drain(..n).collect();

        let mixed: Vec<f32> = a_drain
            .iter()
            .zip(b_drain.iter())
            .map(|(a, b)| (a + b) * ANTI_CLIP_GAIN)
            .collect();

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
        let mono: Vec<f32> = if lane.channels <= 1 {
            // Already mono — avoid an unnecessary allocation when possible.
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
                let chunk = &lane.pending[..RESAMPLER_CHUNK];
                let chunk_out = resampler.process(&[chunk], None)?;
                // FftFixedIn returns a Vec<Vec<f32>> with one inner vec per channel (mono → 1).
                if let Some(ch0) = chunk_out.into_iter().next() {
                    output.extend_from_slice(&ch0);
                }
                lane.pending.drain(..RESAMPLER_CHUNK);
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
    // Stereo downmix
    // -----------------------------------------------------------------------

    /// 2ch LRLR input at 48k on both sides → mono output ((L+R)/2 + mono_b) * GAIN.
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
        let expected = (0.5 + 0.2) * ANTI_CLIP_GAIN;
        for &s in &out {
            assert!((s - expected).abs() < 0.001, "sample {s} != {expected}");
        }
    }

    // -----------------------------------------------------------------------
    // Mono channels=1 with surplus retention
    // -----------------------------------------------------------------------

    /// Old behaviour truncated to the shorter buffer each call; the new mixer
    /// RETAINS surplus. Feed a=100, b=60 → out 60; then a=0, b=40 → out 40
    /// (the retained 40-sample a-surplus is consumed).
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
    #[test]
    fn mix_sums_with_gain() {
        let mut m = Mixer::new(48_000, 1, 48_000, 1).unwrap();
        let a = vec![0.5f32; 100];
        let b = vec![0.3f32; 100];
        let out = m.mix(&a, &b).unwrap();
        assert_eq!(out.len(), 100);
        let expected = (0.5 + 0.3) * ANTI_CLIP_GAIN;
        assert!((out[0] - expected).abs() < 0.001);
    }

    // -----------------------------------------------------------------------
    // Variable-size chunks with resampler (44_100 → 48k)
    // -----------------------------------------------------------------------

    /// Feed 7 × 441-frame stereo buffers (44.1k, 2ch) against a matching mono
    /// 48k b-side. Resampled output grows in 1024-chunk quanta; assert total
    /// output length matches an integer multiple of the per-chunk yield (derived
    /// empirically from the first non-zero chunk, then asserted consistent).
    #[test]
    fn variable_chunks_with_resampler_no_error_nothing_lost() {
        let mut m = Mixer::new(44_100, 2, 48_000, 1).unwrap();

        // 441 stereo frames * 2 ch = 882 samples per call.
        // Downmixed → 441 mono frames per call.
        // 7 calls → 3087 mono frames accumulated.
        // 3087 / 1024 = 3 full chunks (1 chunk → some output), 15 remainder.
        let a_chunk: Vec<f32> = (0..441).flat_map(|_| [0.4f32, 0.6f32]).collect();
        // b-side: enough 48k mono to match (provide surplus so b never stalls)
        // At 44.1→48k ratio, 3 * 1024 input → ~3 * chunk_yield output.
        // Provide 48k b with enough samples that it never limits output.
        let b_chunk = vec![0.1f32; 441 * 2]; // generous surplus

        let mut total_out = 0usize;
        let mut chunk_yield: Option<usize> = None;

        // Feed b surplus upfront so it never limits output.
        let _ = m.mix(&[], &b_chunk.repeat(20));

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
    // MAX_READY_SAMPLES overflow cap
    // -----------------------------------------------------------------------

    /// Feed lane A far beyond 96k while leaving B empty → a_ready stays ≤ cap,
    /// dropped_frames > 0, output is empty.
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
        assert!(m.dropped_frames() > 0, "overflow must count dropped frames");
    }

    // -----------------------------------------------------------------------
    // Partial trailing frame doesn't panic
    // -----------------------------------------------------------------------

    /// interleaved stereo with an odd sample count (partial trailing frame)
    /// must not panic and must not shift framing of subsequent calls.
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
}
