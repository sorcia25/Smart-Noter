//! 16 kHz SpeexDSP acoustic echo canceller for the mic lane.
//!
//! Wraps `aec-rs` (SpeexDSP). The Mixer feeds this the two TIME-ALIGNED mono@48k
//! lanes (mic = near-end, system loopback = far-end reference). Internally it
//! downsamples both to 16 kHz, cancels per Speex `frame_size` frame on i16, and
//! upsamples the cleaned mic back to 48 kHz. Output is delayed by a fixed internal
//! latency (resampler priming + one frame); the caller aligns the system lane via
//! its own delay FIFO. See the module design note about 16 kHz band-limiting.

use crate::error::AudioError;
use aec_rs::{Aec, AecConfig};
use rubato::{FftFixedIn, Resampler};

/// Tunable AEC parameters. Defaults target 16 kHz; calibrate on hardware.
#[derive(Debug, Clone)]
pub struct EchoConfig {
    /// Speex frame size in samples @16 kHz (10 ms = 160).
    pub frame_size: usize,
    /// Adaptive filter tail length in samples @16 kHz (100 ms = 1600). Must
    /// exceed the acoustic delay (~10-40 ms) + room reverb.
    pub filter_length: i32,
    /// Speex preprocessor (denoise + residual echo suppress). Starts FALSE —
    /// it over-attenuates near-end voice in double-talk.
    pub enable_preprocess: bool,
}

impl Default for EchoConfig {
    fn default() -> Self {
        Self {
            frame_size: 160,
            filter_length: 1600,
            enable_preprocess: false,
        }
    }
}

const DOWN_CHUNK_48: usize = 1024; // 48k input frames per downsample chunk
const UP_CHUNK_16: usize = 512; // 16k input frames per upsample chunk

pub struct EchoCanceller {
    aec: Aec,
    frame_size: usize,
    down_mic: FftFixedIn<f32>,
    down_ref: FftFixedIn<f32>,
    up_out: FftFixedIn<f32>,
    mic_pending48: Vec<f32>,
    ref_pending48: Vec<f32>,
    mic16: Vec<f32>,
    ref16: Vec<f32>,
    out_pending16: Vec<f32>,
}

impl EchoCanceller {
    pub fn new(cfg: EchoConfig) -> Result<Self, AudioError> {
        let aec = Aec::new(&AecConfig {
            frame_size: cfg.frame_size,
            filter_length: cfg.filter_length,
            sample_rate: 16_000,
            enable_preprocess: cfg.enable_preprocess,
        });
        let mk = |inr: usize, outr: usize, chunk: usize| {
            FftFixedIn::<f32>::new(inr, outr, chunk, 2, 1)
                .map_err(|e| AudioError::Other(format!("aec rubato init: {e}")))
        };
        Ok(Self {
            aec,
            frame_size: cfg.frame_size,
            down_mic: mk(48_000, 16_000, DOWN_CHUNK_48)?,
            down_ref: mk(48_000, 16_000, DOWN_CHUNK_48)?,
            up_out: mk(16_000, 48_000, UP_CHUNK_16)?,
            mic_pending48: Vec::new(),
            ref_pending48: Vec::new(),
            mic16: Vec::new(),
            ref16: Vec::new(),
            out_pending16: Vec::new(),
        })
    }

    /// Feed one aligned pair of mono@48k buffers (`mic.len() == reference.len()`).
    /// Returns cleaned mic@48k, in order, delayed by the fixed internal latency.
    /// The returned length is NOT equal to the input length — the caller aligns the
    /// system lane with its own delay FIFO.
    pub fn process(&mut self, mic: &[f32], reference: &[f32]) -> Vec<f32> {
        self.mic_pending48.extend_from_slice(mic);
        self.ref_pending48.extend_from_slice(reference);

        // Downsample mic and ref in LOCKSTEP so their 16k streams stay sample-aligned
        // (identical resamplers → identical priming delay → no relative skew).
        while self.mic_pending48.len() >= DOWN_CHUNK_48 && self.ref_pending48.len() >= DOWN_CHUNK_48
        {
            let mchunk: Vec<f32> = self.mic_pending48.drain(..DOWN_CHUNK_48).collect();
            let rchunk: Vec<f32> = self.ref_pending48.drain(..DOWN_CHUNK_48).collect();
            if let Ok(o) = self.down_mic.process(&[&mchunk], None) {
                if let Some(c) = o.into_iter().next() {
                    self.mic16.extend(c);
                }
            }
            if let Ok(o) = self.down_ref.process(&[&rchunk], None) {
                if let Some(c) = o.into_iter().next() {
                    self.ref16.extend(c);
                }
            }
        }

        // Cancel per Speex frame on i16.
        let mut rec_i = vec![0i16; self.frame_size];
        let mut echo_i = vec![0i16; self.frame_size];
        let mut out_i = vec![0i16; self.frame_size];
        while self.mic16.len() >= self.frame_size && self.ref16.len() >= self.frame_size {
            for (d, s) in rec_i.iter_mut().zip(self.mic16.drain(..self.frame_size)) {
                *d = f32_to_i16(s);
            }
            for (d, s) in echo_i.iter_mut().zip(self.ref16.drain(..self.frame_size)) {
                *d = f32_to_i16(s);
            }
            self.aec.cancel_echo(&rec_i, &echo_i, &mut out_i);
            self.out_pending16
                .extend(out_i.iter().map(|&s| i16_to_f32(s)));
        }

        // Upsample cleaned 16k → 48k.
        let mut out48 = Vec::new();
        while self.out_pending16.len() >= UP_CHUNK_16 {
            let chunk: Vec<f32> = self.out_pending16.drain(..UP_CHUNK_16).collect();
            if let Ok(o) = self.up_out.process(&[&chunk], None) {
                if let Some(c) = o.into_iter().next() {
                    out48.extend(c);
                }
            }
        }
        out48
    }
}

#[inline]
fn f32_to_i16(x: f32) -> i16 {
    (x.clamp(-1.0, 1.0) * 32_767.0) as i16
}
#[inline]
fn i16_to_f32(x: i16) -> f32 {
    x as f32 / 32_768.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructs_at_default_config() {
        let ec = EchoCanceller::new(EchoConfig::default());
        assert!(ec.is_ok());
    }

    /// Feed 1 second of aligned mic+ref at 48k in 480-sample (10 ms) ticks; the total
    /// cleaned output must be within one internal-latency window of the input length
    /// (streaming resample + frame buffering delays a bounded prefix, nothing more).
    #[test]
    fn output_length_tracks_input_minus_bounded_latency() {
        let mut ec = EchoCanceller::new(EchoConfig::default()).unwrap();
        let mut total_out = 0usize;
        let ticks = 100; // 100 * 480 = 48_000 samples = 1 s
        for _ in 0..ticks {
            let mic = vec![0.1f32; 480];
            let refr = vec![0.0f32; 480];
            total_out += ec.process(&mic, &refr).len();
        }
        let input = ticks * 480;
        // Latency is a few resampler chunks + one frame — well under 0.2 s @48k.
        assert!(
            total_out > input - 9_600,
            "output {total_out} lost too much vs {input}"
        );
        assert!(
            total_out <= input,
            "output {total_out} exceeds input {input}"
        );
    }
}
