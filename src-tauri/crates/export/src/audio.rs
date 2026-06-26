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

    // MP3 can physically hold at most 2 channels, so the encoder input must be
    // mono or stereo. Resolve the source layout to one of those, matched
    // correct-by-construction to the PCM we feed LAME:
    //   1ch  → mono, MonoPcm
    //   2ch  → stereo, InterleavedPcm (it divides len/2 for the per-channel frame
    //          count, so it is ONLY valid for exactly 2 channels)
    //   >2ch → downmix to mono (average the N samples per frame). This is the one
    //          place a downmix is justified: the format cannot represent >2
    //          channels, and feeding InterleavedPcm here would silently garble.
    let ch = channels as usize;
    let mono_downmix: Vec<i16> = if channels > 2 {
        pcm.chunks_exact(ch)
            .map(|frame| {
                let sum: i32 = frame.iter().map(|&s| s as i32).sum();
                (sum / ch as i32) as i16
            })
            .collect()
    } else {
        Vec::new()
    };
    // Channels actually handed to LAME: 2 only for true stereo, else 1.
    let out_channels: u8 = if channels == 2 { 2 } else { 1 };

    let mut encoder = Builder::new().ok_or_else(|| ExportError::Mp3("builder init".into()))?;
    encoder
        .set_num_channels(out_channels)
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

    let mut out: Vec<u8> = Vec::new();
    let n = match channels {
        2 => {
            out.reserve(mp3lame_encoder::max_required_buffer_size(pcm.len()));
            encoder
                .encode(InterleavedPcm(&pcm), out.spare_capacity_mut())
                .map_err(|e| ExportError::Mp3(format!("encode: {e:?}")))?
        }
        1 => {
            out.reserve(mp3lame_encoder::max_required_buffer_size(pcm.len()));
            encoder
                .encode(MonoPcm(&pcm), out.spare_capacity_mut())
                .map_err(|e| ExportError::Mp3(format!("encode: {e:?}")))?
        }
        _ => {
            out.reserve(mp3lame_encoder::max_required_buffer_size(
                mono_downmix.len(),
            ));
            encoder
                .encode(MonoPcm(&mono_downmix), out.spare_capacity_mut())
                .map_err(|e| ExportError::Mp3(format!("encode: {e:?}")))?
        }
    };
    // SAFETY: `encode` wrote exactly `n` bytes into spare capacity.
    unsafe { out.set_len(out.len() + n) };

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

    /// Build a small 16-bit FLAC file via flacenc's batch encoder (the same
    /// path the audio crate's writer is validated against), so the claxon
    /// decode half of `wav_or_flac_to_mp3` gets real production-shaped input.
    fn write_flac(path: &std::path::Path, rate: u32, channels: u16, frames: usize) {
        use flacenc::bitsink::ByteSink;
        use flacenc::component::BitRepr;
        use flacenc::error::Verify;
        use flacenc::source::MemSource;

        let ch = channels as usize;
        let interleaved: Vec<i32> = (0..frames)
            .flat_map(|i| std::iter::repeat_n(((i % 200) as i32 - 100) * 50, ch))
            .collect();

        let config = flacenc::config::Encoder::default().into_verified().unwrap();
        let source = MemSource::from_samples(&interleaved, ch, 16, rate as usize);
        let stream =
            flacenc::encode_with_fixed_block_size(&config, source, config.block_size).unwrap();
        let mut sink = ByteSink::new();
        stream.write(&mut sink).unwrap();
        std::fs::write(path, sink.as_slice()).unwrap();
    }

    /// Asserts `bytes` begins like a real MP3 stream: an ID3 tag or an MPEG
    /// frame sync (0xFF 0xEx/0xFx).
    fn looks_like_mp3(bytes: &[u8]) {
        assert!(
            bytes.len() > 200,
            "mp3 should have frames, got {}",
            bytes.len()
        );
        let ok = &bytes[0..3] == b"ID3" || (bytes[0] == 0xFF && (bytes[1] & 0xE0) == 0xE0);
        assert!(ok, "looks like MP3: first bytes {:02X?}", &bytes[0..4]);
    }

    #[test]
    fn wav_transcodes_to_nonempty_mp3() {
        let dir = tempfile::tempdir().unwrap();
        let wav = dir.path().join("a.wav");
        write_wav(&wav, 44_100, 2, 44_100); // 1s stereo
        let mp3 = wav_or_flac_to_mp3(&wav).unwrap();
        looks_like_mp3(&mp3);
    }

    #[test]
    fn mono_wav_transcodes_to_nonempty_mp3() {
        // Mix-mode records mono (capture/stream.rs: "mixed output is mono") —
        // exercises the MonoPcm branch + its buffer reservation.
        let dir = tempfile::tempdir().unwrap();
        let wav = dir.path().join("m.wav");
        write_wav(&wav, 48_000, 1, 48_000); // 1s mono
        let mp3 = wav_or_flac_to_mp3(&wav).unwrap();
        looks_like_mp3(&mp3);
    }

    #[test]
    fn flac_transcodes_to_nonempty_mp3() {
        // Recordings can be FLAC — exercises the claxon decode path.
        let dir = tempfile::tempdir().unwrap();
        let flac = dir.path().join("f.flac");
        write_flac(&flac, 48_000, 1, 48_000); // 1s mono FLAC
        let mp3 = wav_or_flac_to_mp3(&flac).unwrap();
        looks_like_mp3(&mp3);
    }

    #[test]
    fn multichannel_wav_downmixes_to_nonempty_mp3() {
        // A >2-channel source (e.g. a multi-channel interface) must downmix to
        // mono rather than garble through InterleavedPcm.
        let dir = tempfile::tempdir().unwrap();
        let wav = dir.path().join("multi.wav");
        write_wav(&wav, 48_000, 3, 48_000); // 1s 3-channel
        let mp3 = wav_or_flac_to_mp3(&wav).unwrap();
        looks_like_mp3(&mp3);
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
