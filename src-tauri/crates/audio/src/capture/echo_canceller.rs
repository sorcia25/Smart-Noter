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
use rubato::FftFixedIn;

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

// Fields are wired up by the next task (Mixer integration); construction-only for now.
#[allow(dead_code)]
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructs_at_default_config() {
        let ec = EchoCanceller::new(EchoConfig::default());
        assert!(ec.is_ok());
    }
}
