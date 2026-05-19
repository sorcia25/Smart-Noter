//! RMS + peak meter and waveform decimator.
//!
//! Consumes f32 sample buffers and exposes:
//! * `level()`        — last-block RMS + peak (0..1 normalised)
//! * `waveform()`     — rolling buffer of 36 normalised bins (one per ~100 ms)
//! * `samples_total()` — running count of samples observed (driver of `elapsed`)

use std::collections::VecDeque;

const WAVEFORM_BINS: usize = 36;

#[derive(Debug, Clone, Copy, Default)]
pub struct Level {
    pub rms: f32,
    pub peak: f32,
}

pub struct Meter {
    sample_rate: u32,
    channels: u16,
    // ~100 ms accumulator at the source sample rate. For 48 kHz mono → 4800 samples.
    bin_accum: Vec<f32>,
    bin_target: usize,
    bins: VecDeque<f32>,
    samples_total: u64,
    last_level: Level,
}

impl Meter {
    pub fn new(sample_rate: u32, channels: u16) -> Self {
        let frames_per_bin = (sample_rate as f32 * 0.100) as usize; // ~100 ms
        Self {
            sample_rate,
            channels,
            bin_accum: Vec::with_capacity(frames_per_bin * channels as usize),
            bin_target: frames_per_bin * channels as usize,
            bins: VecDeque::from(vec![0.0; WAVEFORM_BINS]),
            samples_total: 0,
            last_level: Level::default(),
        }
    }

    /// Push a block of **interleaved** f32 samples from all channels. For stereo at
    /// 48 kHz one second of audio = 96 000 samples (48 000 L+R frames × 2 channels).
    pub fn push(&mut self, samples: &[f32]) {
        if samples.is_empty() {
            return;
        }
        self.samples_total += samples.len() as u64;
        // Update last_level from this block.
        let mut sum_sq = 0.0;
        let mut peak: f32 = 0.0;
        for &s in samples {
            sum_sq += s * s;
            let a = s.abs();
            if a > peak {
                peak = a;
            }
        }
        let rms = (sum_sq / samples.len() as f32).sqrt();
        self.last_level = Level {
            rms: rms.min(1.0),
            peak: peak.min(1.0),
        };
        // Accumulate into the waveform bin.
        self.bin_accum.extend_from_slice(samples);
        while self.bin_accum.len() >= self.bin_target {
            let drained: Vec<f32> = self.bin_accum.drain(..self.bin_target).collect();
            let bin_peak = drained.iter().fold(0.0f32, |acc, &s| acc.max(s.abs()));
            if self.bins.len() == WAVEFORM_BINS {
                self.bins.pop_front();
            }
            self.bins.push_back(bin_peak.min(1.0));
        }
    }

    pub fn level(&self) -> Level {
        self.last_level
    }

    pub fn waveform(&self) -> Vec<f32> {
        self.bins.iter().copied().collect()
    }

    /// Total samples observed across all `push` calls. Includes every channel sample
    /// (i.e. interleaved, not frame-count).
    pub fn samples_total(&self) -> u64 {
        self.samples_total
    }

    pub fn elapsed_sec(&self) -> u32 {
        if self.sample_rate == 0 || self.channels == 0 {
            return 0;
        }
        (self.samples_total / (self.sample_rate as u64 * self.channels as u64)) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rms_of_sine_wave() {
        let mut m = Meter::new(48_000, 1);
        // 1000 samples of sine at amplitude 0.5
        let samples: Vec<f32> = (0..1000).map(|i| (i as f32 * 0.1).sin() * 0.5).collect();
        m.push(&samples);
        let lvl = m.level();
        assert!(
            (lvl.rms - 0.353).abs() < 0.05,
            "RMS≈0.353 for 0.5 sine, got {}",
            lvl.rms
        );
    }

    #[test]
    fn peak_clamps_to_one() {
        let mut m = Meter::new(48_000, 1);
        m.push(&[0.9, -0.95, 0.3, 0.5]);
        assert!((m.level().peak - 0.95).abs() < 0.001);
    }

    #[test]
    fn waveform_starts_with_36_zero_bins() {
        let m = Meter::new(48_000, 1);
        assert_eq!(m.waveform().len(), 36);
        assert!(m.waveform().iter().all(|&v| v == 0.0));
    }

    #[test]
    fn pushing_one_bin_worth_advances_history() {
        let mut m = Meter::new(48_000, 1);
        m.push(&vec![0.5; 4800]); // 100 ms at 48 kHz mono
        let wf = m.waveform();
        assert_eq!(wf.len(), 36);
        assert!(
            (wf[35] - 0.5).abs() < 0.001,
            "last bin should be the new peak"
        );
        assert_eq!(wf[0], 0.0, "oldest bin should still be the seed zero");
    }

    #[test]
    fn elapsed_advances_per_sample_count() {
        let mut m = Meter::new(48_000, 2); // stereo
        m.push(&vec![0.0; 96_000]); // 1 s of stereo (96000 samples / 48000 sr / 2 ch)
        assert_eq!(m.elapsed_sec(), 1);
    }
}
