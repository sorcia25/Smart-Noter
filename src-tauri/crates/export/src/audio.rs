use crate::ExportError;
use mp3lame_encoder::{Bitrate, Builder, FlushNoGap, InterleavedPcm, MonoPcm, Quality};
use std::path::Path;

/// Decode WAV or FLAC into interleaved i16 PCM at its native rate/channels —
/// NO downmix, NO resample (preserve the original recording for export).
fn decode_interleaved_i16(path: &Path) -> Result<(Vec<i16>, u32, u16), ExportError> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "wav" => read_wav_i16(path),
        "flac" => read_flac_i16(path),
        other => Err(ExportError::UnsupportedAudio(other.to_string())),
    }
}

fn read_wav_i16(path: &Path) -> Result<(Vec<i16>, u32, u16), ExportError> {
    let mut reader =
        hound::WavReader::open(path).map_err(|e| ExportError::Decode(e.to_string()))?;
    let spec = reader.spec();
    let samples: Vec<i16> = match spec.sample_format {
        hound::SampleFormat::Int => reader
            .samples::<i32>()
            .map(|s| {
                s.map(|v| {
                    let shift = spec.bits_per_sample.saturating_sub(16);
                    (v >> shift) as i16
                })
                .map_err(|e| ExportError::Decode(e.to_string()))
            })
            .collect::<Result<_, _>>()?,
        hound::SampleFormat::Float => reader
            .samples::<f32>()
            .map(|s| {
                s.map(|v| (v.clamp(-1.0, 1.0) * 32_767.0) as i16)
                    .map_err(|e| ExportError::Decode(e.to_string()))
            })
            .collect::<Result<_, _>>()?,
    };
    Ok((samples, spec.sample_rate, spec.channels))
}

fn read_flac_i16(path: &Path) -> Result<(Vec<i16>, u32, u16), ExportError> {
    let mut reader =
        claxon::FlacReader::open(path).map_err(|e| ExportError::Decode(e.to_string()))?;
    let info = reader.streaminfo();
    let shift = (info.bits_per_sample as i32 - 16).max(0);
    let mut samples = Vec::new();
    for s in reader.samples() {
        let v = s.map_err(|e| ExportError::Decode(e.to_string()))?;
        samples.push((v >> shift) as i16);
    }
    Ok((samples, info.sample_rate, info.channels as u16))
}

pub fn wav_or_flac_to_mp3(path: &Path) -> Result<Vec<u8>, ExportError> {
    let (pcm, rate, channels) = decode_interleaved_i16(path)?;

    let mut encoder = Builder::new().ok_or_else(|| ExportError::Mp3("builder init".into()))?;
    encoder
        .set_num_channels(channels.clamp(1, 2) as u8)
        .map_err(|e| ExportError::Mp3(format!("channels: {e:?}")))?;
    encoder
        .set_sample_rate(rate)
        .map_err(|e| ExportError::Mp3(format!("rate: {e:?}")))?;
    encoder
        .set_brate(Bitrate::Kbps128)
        .map_err(|e| ExportError::Mp3(format!("brate: {e:?}")))?;
    encoder
        .set_quality(Quality::Good)
        .map_err(|e| ExportError::Mp3(format!("quality: {e:?}")))?;
    let mut encoder = encoder
        .build()
        .map_err(|e| ExportError::Mp3(format!("build: {e:?}")))?;

    // InterleavedPcm<i16> only works for 2-channel (stereo) data — it divides
    // len/2 to get per-channel frame count. For mono we use MonoPcm<i16>.
    let mut out: Vec<u8> = Vec::new();
    if channels >= 2 {
        out.reserve(mp3lame_encoder::max_required_buffer_size(pcm.len()));
        let n = encoder
            .encode(InterleavedPcm(&pcm), out.spare_capacity_mut())
            .map_err(|e| ExportError::Mp3(format!("encode: {e:?}")))?;
        // SAFETY: `encode` wrote exactly `n` bytes into spare capacity.
        unsafe { out.set_len(out.len() + n) };
    } else {
        out.reserve(mp3lame_encoder::max_required_buffer_size(pcm.len()));
        let n = encoder
            .encode(MonoPcm(&pcm), out.spare_capacity_mut())
            .map_err(|e| ExportError::Mp3(format!("encode: {e:?}")))?;
        // SAFETY: `encode` wrote exactly `n` bytes into spare capacity.
        unsafe { out.set_len(out.len() + n) };
    }

    // Flush needs at least 7200 bytes spare.
    out.reserve(mp3lame_encoder::max_required_buffer_size(0).max(7200));
    let n = encoder
        .flush::<FlushNoGap>(out.spare_capacity_mut())
        .map_err(|e| ExportError::Mp3(format!("flush: {e:?}")))?;
    // SAFETY: `flush` wrote exactly `n` bytes into spare capacity.
    unsafe { out.set_len(out.len() + n) };

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

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
                w.write_sample(((i % 200) as i16 - 100) * 50).unwrap();
            }
        }
        w.finalize().unwrap();
    }

    #[test]
    fn wav_transcodes_to_nonempty_mp3() {
        let dir = tempfile::tempdir().unwrap();
        let wav = dir.path().join("a.wav");
        write_wav(&wav, 44_100, 2, 44_100); // 1s stereo
        let mp3 = wav_or_flac_to_mp3(&wav).unwrap();
        assert!(mp3.len() > 200, "mp3 should have frames, got {}", mp3.len());
        // MP3 starts with an ID3 tag or an MPEG frame sync (0xFF 0xEx/0xFx).
        let ok = &mp3[0..3] == b"ID3" || (mp3[0] == 0xFF && (mp3[1] & 0xE0) == 0xE0);
        assert!(ok, "looks like MP3: first bytes {:02X?}", &mp3[0..4]);
    }

    #[test]
    fn rejects_unknown_extension() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("x.ogg");
        std::fs::write(&p, b"x").unwrap();
        assert!(matches!(
            wav_or_flac_to_mp3(&p),
            Err(ExportError::UnsupportedAudio(_))
        ));
    }
}
