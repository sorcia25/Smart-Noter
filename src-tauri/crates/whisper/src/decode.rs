use crate::error::{TranscriptionError, TranscriptionErrorCode};
use std::path::Path;

const TARGET_RATE: u32 = 16_000;

fn err(message: impl Into<String>) -> TranscriptionError {
    TranscriptionError {
        code: TranscriptionErrorCode::DecodeFailed,
        message: message.into(),
    }
}

/// Decode a `.wav`/`.flac` file to 16 kHz mono f32 PCM in [-1, 1].
pub fn decode_to_pcm_16k_mono(path: &Path) -> Result<Vec<f32>, TranscriptionError> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let (interleaved, rate, channels) = match ext.as_str() {
        "wav" => read_wav(path)?,
        "flac" => read_flac(path)?,
        other => return Err(err(format!("unsupported extension: {other}"))),
    };
    let mono = downmix(&interleaved, channels);
    Ok(if rate == TARGET_RATE {
        mono
    } else {
        resample_linear(&mono, rate, TARGET_RATE)
    })
}

fn read_wav(path: &Path) -> Result<(Vec<f32>, u32, u16), TranscriptionError> {
    let mut reader = hound::WavReader::open(path).map_err(|e| err(e.to_string()))?;
    let spec = reader.spec();
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader
            .samples::<f32>()
            .map(|s| s.map_err(|e| err(e.to_string())))
            .collect::<Result<_, _>>()?,
        hound::SampleFormat::Int => {
            let max = (1i64 << (spec.bits_per_sample - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|s| s.map(|v| v as f32 / max).map_err(|e| err(e.to_string())))
                .collect::<Result<_, _>>()?
        }
    };
    Ok((samples, spec.sample_rate, spec.channels))
}

fn read_flac(path: &Path) -> Result<(Vec<f32>, u32, u16), TranscriptionError> {
    let mut reader = claxon::FlacReader::open(path).map_err(|e| err(e.to_string()))?;
    let info = reader.streaminfo();
    let max = (1i64 << (info.bits_per_sample - 1)) as f32;
    let mut samples = Vec::new();
    for s in reader.samples() {
        samples.push(s.map_err(|e| err(e.to_string()))? as f32 / max);
    }
    Ok((samples, info.sample_rate, info.channels as u16))
}

/// Average channels into mono. `interleaved` is frame-major (L,R,L,R,…).
fn downmix(interleaved: &[f32], channels: u16) -> Vec<f32> {
    let ch = channels.max(1) as usize;
    if ch == 1 {
        return interleaved.to_vec();
    }
    interleaved
        .chunks_exact(ch)
        .map(|frame| frame.iter().sum::<f32>() / ch as f32)
        .collect()
}

/// Simple linear-interpolation resampler. Adequate for 16 kHz speech fed to Whisper.
fn resample_linear(input: &[f32], from: u32, to: u32) -> Vec<f32> {
    if input.is_empty() || from == to {
        return input.to_vec();
    }
    let ratio = to as f64 / from as f64;
    let out_len = ((input.len() as f64) * ratio).round() as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let src = i as f64 / ratio;
        let i0 = src.floor() as usize;
        let i1 = (i0 + 1).min(input.len() - 1);
        let frac = (src - i0 as f64) as f32;
        out.push(input[i0] * (1.0 - frac) + input[i1] * frac);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // Write a tiny WAV (i16, given rate/channels) to a temp path and return it.
    fn write_wav(path: &std::path::Path, rate: u32, channels: u16, frames: usize) {
        let spec = hound::WavSpec {
            channels,
            sample_rate: rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut w = hound::WavWriter::create(path, spec).unwrap();
        for i in 0..frames {
            for _c in 0..channels {
                // a quiet ramp so it's not all-zero
                w.write_sample(((i % 100) as i16 - 50) * 100).unwrap();
            }
        }
        w.finalize().unwrap();
    }

    #[test]
    fn decodes_48k_stereo_to_16k_mono() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("a.wav");
        write_wav(&p, 48_000, 2, 48_000); // 1 second
        let pcm = decode_to_pcm_16k_mono(&p).unwrap();
        // ~1 second at 16 kHz mono, within a small rounding tolerance.
        assert!((pcm.len() as i64 - 16_000).abs() <= 2, "got {}", pcm.len());
        assert!(pcm.iter().all(|s| s.abs() <= 1.0));
    }

    #[test]
    fn passes_through_when_already_16k_mono() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("b.wav");
        write_wav(&p, 16_000, 1, 16_000);
        let pcm = decode_to_pcm_16k_mono(&p).unwrap();
        assert_eq!(pcm.len(), 16_000);
    }

    #[test]
    fn rejects_unknown_extension() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("c.mp3");
        std::fs::write(&p, b"x").unwrap();
        assert!(decode_to_pcm_16k_mono(&p).is_err());
    }
}
