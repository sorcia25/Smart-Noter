//! Format-agnostic writer trait + concrete WAV and FLAC implementations.

use crate::error::AudioError;
use hound::{SampleFormat, WavSpec, WavWriter};
use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

pub trait AudioWriter: Send {
    fn write(&mut self, samples: &[f32]) -> Result<(), AudioError>;
    fn finalize(self: Box<Self>) -> Result<FinalizeResult, AudioError>;
}

#[derive(Debug, Clone)]
pub struct FinalizeResult {
    pub path: PathBuf,
    pub bytes: u64,
    pub sample_count: u64,
}

pub struct WavWriterImpl {
    inner: Option<WavWriter<BufWriter<File>>>,
    path: PathBuf,
    sample_count: u64,
}

impl WavWriterImpl {
    pub fn create(path: PathBuf, sample_rate: u32, channels: u16) -> Result<Self, AudioError> {
        let spec = WavSpec {
            channels,
            sample_rate,
            bits_per_sample: 16,
            sample_format: SampleFormat::Int,
        };
        let writer = WavWriter::create(&path, spec)
            .map_err(|e| AudioError::Other(format!("WAV create: {e}")))?;
        Ok(Self {
            inner: Some(writer),
            path,
            sample_count: 0,
        })
    }
}

impl AudioWriter for WavWriterImpl {
    fn write(&mut self, samples: &[f32]) -> Result<(), AudioError> {
        let writer = self
            .inner
            .as_mut()
            .ok_or_else(|| AudioError::Other("writer already finalized".into()))?;
        for &s in samples {
            let clipped = s.clamp(-1.0, 1.0);
            let i = (clipped * 32_767.0) as i16;
            writer
                .write_sample(i)
                .map_err(|e| classify_io_error(e, &self.path))?;
        }
        self.sample_count += samples.len() as u64;
        Ok(())
    }

    fn finalize(mut self: Box<Self>) -> Result<FinalizeResult, AudioError> {
        let inner = self
            .inner
            .take()
            .ok_or_else(|| AudioError::Other("writer already finalized".into()))?;
        inner
            .finalize()
            .map_err(|e| AudioError::Other(format!("WAV finalize: {e}")))?;
        let bytes = std::fs::metadata(&self.path).map(|m| m.len()).unwrap_or(0);
        Ok(FinalizeResult {
            path: self.path,
            bytes,
            sample_count: self.sample_count,
        })
    }
}

fn classify_io_error(e: hound::Error, path: &Path) -> AudioError {
    use std::io::ErrorKind;
    if let hound::Error::IoError(io) = &e {
        if io.kind() == ErrorKind::StorageFull {
            return AudioError::DiskFull {
                path: path.display().to_string(),
            };
        }
    }
    AudioError::Other(format!("WAV write: {e}"))
}

// ---------------------------------------------------------------------------
// FLAC writer (flacenc — pure Rust, no native deps)
// ---------------------------------------------------------------------------

/// Streaming FLAC writer: encodes 4096-frame blocks to disk as samples arrive.
///
/// At `create` the file is opened and a placeholder header
/// (`"fLaC"` + STREAMINFO) is written so the byte offset of the audio frames
/// is fixed. Each `write()` call converts f32 → i16-range i32 samples and
/// flushes complete blocks immediately, so RAM usage is bounded to at most
/// `block_size * channels` pending i32 samples regardless of recording length.
///
/// `finalize()` encodes only the remaining tail (partial block), patches the
/// STREAMINFO in-place (seek to 0, rewrite header), and returns. The encode
/// cost at stop time is negligible — at most one partial block.
///
/// DiskFull errors are now surfaced **during recording** (on every `write()`
/// flush), not only at finalize as the old batch path did.
///
/// `path` here is always the session's `tmp-*` file; the session-level
/// `finalize_recording` performs the real tmp→final rename. The startup sweep
/// removes orphaned tmp files, so there is no reason to defer file creation or
/// do an atomic rename inside this writer.
pub struct FlacWriterImpl {
    file: Option<std::io::BufWriter<std::fs::File>>,
    path: PathBuf,
    pending: Vec<i32>, // interleaved, invariant: len < block_size * channels
    frame_number: usize,
    stream_info: flacenc::component::StreamInfo,
    md5: flacenc::source::Context,
    config: flacenc::error::Verified<flacenc::config::Encoder>,
    block_size: usize,
    header_len: usize, // bytes of magic+STREAMINFO written at create
    sample_count: u64, // raw f32 samples received (interleaved)
    channels: u16,
}

impl FlacWriterImpl {
    pub fn create(path: PathBuf, sample_rate: u32, channels: u16) -> Result<Self, AudioError> {
        use flacenc::bitsink::ByteSink;
        use flacenc::component::BitRepr;
        use flacenc::component::Stream;
        use flacenc::error::Verify;

        let config = flacenc::config::Encoder::default()
            .into_verified()
            .map_err(|(_enc, e)| AudioError::Other(format!("FLAC config: {e:?}")))?;

        let block_size = config.block_size;

        let mut stream_info =
            flacenc::component::StreamInfo::new(sample_rate as usize, channels as usize, 16)
                .map_err(|e| AudioError::Other(format!("FLAC StreamInfo: {e}")))?;

        // Pre-set block sizes to match the batch encoder's behavior: it calls
        // set_block_sizes(block_size, block_size) before the encode loop so the
        // STREAMINFO placeholder is consistent with what update_frame_info will
        // later write for full-size blocks.
        stream_info
            .set_block_sizes(block_size, block_size)
            .map_err(|e| AudioError::Other(format!("FLAC set_block_sizes: {e}")))?;

        let md5 = flacenc::source::Context::new(16, channels as usize);

        // Write header placeholder — Stream with zero frames = "fLaC" magic +
        // STREAMINFO block only. STREAMINFO is a fixed 272-bit (34-byte) struct,
        // so the header byte length is constant and the seek-back patch at
        // finalize is always safe.
        let mut sink = ByteSink::new();
        Stream::with_stream_info(stream_info.clone())
            .write(&mut sink)
            .map_err(|e| AudioError::Other(format!("FLAC header write: {e}")))?;
        let header_bytes = sink.as_slice().to_vec();
        let header_len = header_bytes.len();

        let mut file = std::io::BufWriter::new(
            std::fs::File::create(&path).map_err(|e| classify_io_error_at("create", e, &path))?,
        );
        use std::io::Write;
        file.write_all(&header_bytes)
            .map_err(|e| classify_io_error_at("create", e, &path))?;

        Ok(Self {
            file: Some(file),
            path,
            pending: Vec::new(),
            frame_number: 0,
            stream_info,
            md5,
            config,
            block_size,
            header_len,
            sample_count: 0,
            channels,
        })
    }

    /// Encode and flush one full block from the front of `pending`.
    fn flush_block(&mut self) -> Result<(), AudioError> {
        use flacenc::bitsink::ByteSink;
        use flacenc::component::BitRepr;
        use flacenc::source::Fill;

        let ch = self.channels as usize;
        let chunk = self.pending[..self.block_size * ch].to_vec();

        // Fill FrameBuf (de-interleaves into channel-planar layout).
        let mut fb = flacenc::source::FrameBuf::with_size(ch, self.block_size)
            .map_err(|e| AudioError::Other(format!("FLAC FrameBuf: {e}")))?;
        fb.fill_interleaved(&chunk)
            .map_err(|e| AudioError::Other(format!("FLAC fill: {e}")))?;

        let frame = flacenc::encode_fixed_size_frame(
            &self.config,
            &fb,
            self.frame_number,
            &self.stream_info,
        )
        .map_err(|e| AudioError::Other(format!("FLAC encode: {e}")))?;

        self.stream_info.update_frame_info(&frame);

        // Feed the same interleaved chunk to the md5 accumulator.
        self.md5
            .fill_interleaved(&chunk)
            .map_err(|e| AudioError::Other(format!("FLAC md5: {e}")))?;

        let mut sink = ByteSink::new();
        frame
            .write(&mut sink)
            .map_err(|e| AudioError::Other(format!("FLAC frame write: {e}")))?;

        let file = self
            .file
            .as_mut()
            .ok_or_else(|| AudioError::Other("writer already finalized".into()))?;
        use std::io::Write;
        file.write_all(sink.as_slice())
            .map_err(|e| classify_io_error_at("write frame", e, &self.path))?;

        self.frame_number += 1;
        self.pending.drain(..self.block_size * ch);
        Ok(())
    }
}

impl AudioWriter for FlacWriterImpl {
    fn write(&mut self, samples: &[f32]) -> Result<(), AudioError> {
        self.pending.extend(
            samples
                .iter()
                .map(|&s| (s.clamp(-1.0, 1.0) * 32_767.0) as i32),
        );
        self.sample_count += samples.len() as u64;

        let ch = self.channels as usize;
        while self.pending.len() >= self.block_size * ch {
            self.flush_block()?;
        }
        Ok(())
    }

    fn finalize(mut self: Box<Self>) -> Result<FinalizeResult, AudioError> {
        use flacenc::bitsink::ByteSink;
        use flacenc::component::BitRepr;
        use flacenc::component::Stream;
        use flacenc::source::Fill;
        use std::io::{Seek, SeekFrom, Write};

        let ch = self.channels as usize;

        // Encode tail (partial block) — mirrors the batch path's strategy:
        // MemSource::read_samples passes a short slice to fill_interleaved when
        // fewer samples remain than block_size, so FrameBuf::filled_size() < block_size.
        // encode_fixed_size_frame accepts this — it uses filled_size for the frame.
        if !self.pending.is_empty() {
            let tail = self.pending.clone();

            // For the tail, FrameBuf is allocated at full block_size (same as
            // batch path) but only filled with the remaining samples.
            let mut fb = flacenc::source::FrameBuf::with_size(ch, self.block_size)
                .map_err(|e| AudioError::Other(format!("FLAC FrameBuf tail: {e}")))?;
            fb.fill_interleaved(&tail)
                .map_err(|e| AudioError::Other(format!("FLAC fill tail: {e}")))?;

            let frame = flacenc::encode_fixed_size_frame(
                &self.config,
                &fb,
                self.frame_number,
                &self.stream_info,
            )
            .map_err(|e| AudioError::Other(format!("FLAC encode tail: {e}")))?;

            self.stream_info.update_frame_info(&frame);

            self.md5
                .fill_interleaved(&tail)
                .map_err(|e| AudioError::Other(format!("FLAC md5 tail: {e}")))?;

            let mut sink = ByteSink::new();
            frame
                .write(&mut sink)
                .map_err(|e| AudioError::Other(format!("FLAC frame write tail: {e}")))?;

            let file = self
                .file
                .as_mut()
                .ok_or_else(|| AudioError::Other("writer already finalized".into()))?;
            file.write_all(sink.as_slice())
                .map_err(|e| classify_io_error_at("write frame", e, &self.path))?;
        }

        // Finalize StreamInfo: total_samples is per-channel frame count.
        let per_channel = (self.sample_count / ch.max(1) as u64) as usize;
        self.stream_info.set_total_samples(per_channel);
        self.stream_info.set_md5_digest(&self.md5.md5_digest());

        // Flush then patch the header in-place.
        let file = self
            .file
            .as_mut()
            .ok_or_else(|| AudioError::Other("writer already finalized".into()))?;
        file.flush()
            .map_err(|e| classify_io_error_at("flush", e, &self.path))?;

        // Re-serialize header with updated StreamInfo — must be same byte length.
        let mut new_sink = ByteSink::new();
        Stream::with_stream_info(self.stream_info.clone())
            .write(&mut new_sink)
            .map_err(|e| AudioError::Other(format!("FLAC header rewrite: {e}")))?;
        let new_header = new_sink.as_slice();

        if new_header.len() != self.header_len {
            return Err(AudioError::Other(format!(
                "FLAC header size changed: expected {} bytes, got {} — internal error",
                self.header_len,
                new_header.len()
            )));
        }

        let inner = file.get_mut();
        inner
            .seek(SeekFrom::Start(0))
            .map_err(|e| classify_io_error_at("patch header", e, &self.path))?;
        inner
            .write_all(new_header)
            .map_err(|e| classify_io_error_at("patch header", e, &self.path))?;
        inner
            .flush()
            .map_err(|e| classify_io_error_at("flush", e, &self.path))?;

        // Take the file out to allow drop.
        drop(self.file.take());

        let bytes = std::fs::metadata(&self.path).map(|m| m.len()).unwrap_or(0);
        Ok(FinalizeResult {
            path: self.path,
            bytes,
            sample_count: self.sample_count,
        })
    }
}

fn classify_io_error_at(op: &str, e: std::io::Error, path: &Path) -> AudioError {
    use std::io::ErrorKind;
    if matches!(e.kind(), ErrorKind::StorageFull) {
        AudioError::DiskFull {
            path: path.display().to_string(),
        }
    } else {
        AudioError::Other(format!("{op} {}: {e}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    fn tmp_path(name: &str) -> PathBuf {
        let dir = std::env::temp_dir();
        dir.join(format!("sn-test-{}-{}.wav", name, std::process::id()))
    }

    #[test]
    fn writes_wav_with_correct_header() {
        let path = tmp_path("header");
        let mut w = WavWriterImpl::create(path.clone(), 48_000, 2).unwrap();
        let samples = [0.0f32; 480];
        w.write(&samples).unwrap();
        let res = Box::new(w).finalize().unwrap();
        assert!(res.bytes > 44, "WAV header is ≥44 bytes plus payload");

        let mut file = std::fs::File::open(&path).unwrap();
        let mut header = [0u8; 12];
        file.read_exact(&mut header).unwrap();
        assert_eq!(&header[0..4], b"RIFF");
        assert_eq!(&header[8..12], b"WAVE");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn write_clamps_oversaturated_samples() {
        let path = tmp_path("clamp");
        let mut w = WavWriterImpl::create(path.clone(), 48_000, 1).unwrap();
        w.write(&[2.0, -2.0, 0.5]).unwrap();
        Box::new(w).finalize().unwrap();
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn finalize_returns_sample_count() {
        let path = tmp_path("count");
        let mut w = WavWriterImpl::create(path.clone(), 48_000, 1).unwrap();
        w.write(&[0.0; 1000]).unwrap();
        let res = Box::new(w).finalize().unwrap();
        assert_eq!(res.sample_count, 1000);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn flac_writer_writes_and_finalizes() {
        let path = std::env::temp_dir().join(format!("sn-flac-test-{}.flac", std::process::id()));
        let mut w = FlacWriterImpl::create(path.clone(), 48_000, 1).unwrap();
        // 480 mono samples — less than one block (4096), exercises the tail path.
        w.write(&[0.0; 480]).unwrap();
        let res = Box::new(w).finalize().unwrap();
        assert!(res.bytes > 0);
        assert_eq!(res.sample_count, 480);
        // FLAC magic: "fLaC" at offset 0
        let mut file = std::fs::File::open(&path).unwrap();
        let mut magic = [0u8; 4];
        use std::io::Read;
        file.read_exact(&mut magic).unwrap();
        assert_eq!(&magic, b"fLaC");
        std::fs::remove_file(&path).ok();
    }

    /// Golden test: byte-identical output to batch encoder.
    ///
    /// Generates a deterministic signal with enough samples for ≥2 full blocks
    /// plus a partial tail (2 channels, 48 kHz). Writes it through the new
    /// streaming FlacWriterImpl with deliberately odd chunk sizes to exercise
    /// the pending-boundary logic. Then encodes the same i32 samples with the
    /// batch path (encode_with_fixed_block_size + MemSource) and asserts the
    /// resulting byte vectors are equal.
    #[test]
    fn flac_streaming_output_matches_batch() {
        use flacenc::bitsink::ByteSink;
        use flacenc::component::BitRepr;
        use flacenc::error::Verify;
        use flacenc::source::MemSource;

        let sample_rate: u32 = 48_000;
        let channels: u16 = 2;
        let ch = channels as usize;

        // Default block_size is 4096. Use enough samples for 2 full blocks + tail.
        let config = flacenc::config::Encoder::default().into_verified().unwrap();
        let block_size = config.block_size;
        // 2 full blocks + 300 tail frames per channel → interleaved count
        let total_frames = block_size * 2 + 300;
        let total_interleaved = total_frames * ch;

        // Deterministic ramp signal in i32 range (−32767..=32767).
        let i32_samples: Vec<i32> = (0..total_interleaved)
            .map(|i| ((i % 65535) as i32) - 32767)
            .collect();

        // Convert i32 → f32 (inverse of the write() clamping path) for streaming input.
        let f32_samples: Vec<f32> = i32_samples.iter().map(|&v| v as f32 / 32_767.0).collect();

        // --- Streaming path ---
        let streaming_path =
            std::env::temp_dir().join(format!("sn-flac-golden-stream-{}.flac", std::process::id()));
        {
            let mut w =
                FlacWriterImpl::create(streaming_path.clone(), sample_rate, channels).unwrap();
            // Odd write sizes to stress-test pending-boundary logic.
            let chunks = [1000usize, block_size * 2 * ch + 1, 3];
            let mut pos = 0usize;
            for &chunk_len in &chunks {
                let end = (pos + chunk_len).min(f32_samples.len());
                if pos < end {
                    w.write(&f32_samples[pos..end]).unwrap();
                    pos = end;
                }
            }
            // Write any remainder.
            if pos < f32_samples.len() {
                w.write(&f32_samples[pos..]).unwrap();
            }
            Box::new(w).finalize().unwrap();
        }
        let streaming_bytes = std::fs::read(&streaming_path).unwrap();
        std::fs::remove_file(&streaming_path).ok();

        // --- Batch path ---
        // Re-derive i32 samples from f32 the same way write() does (round-trip).
        let i32_from_f32: Vec<i32> = f32_samples
            .iter()
            .map(|&s| (s.clamp(-1.0, 1.0) * 32_767.0) as i32)
            .collect();
        let source = MemSource::from_samples(&i32_from_f32, ch, 16, sample_rate as usize);
        let stream = flacenc::encode_with_fixed_block_size(&config, source, block_size).unwrap();
        let mut sink = ByteSink::new();
        stream.write(&mut sink).unwrap();
        let batch_bytes = sink.as_slice().to_vec();

        assert_eq!(
            streaming_bytes, batch_bytes,
            "streaming FLAC output must be byte-identical to batch encoder"
        );
    }

    /// Multi-write boundary test: multiple write() calls with sizes straddling
    /// block boundaries produce the same bytes as a single large write.
    #[test]
    fn flac_multi_write_self_consistent() {
        use flacenc::error::Verify;

        let sample_rate: u32 = 48_000;
        let channels: u16 = 1;
        let ch = channels as usize;
        let config = flacenc::config::Encoder::default().into_verified().unwrap();
        let block_size = config.block_size;
        // Enough for >2 full blocks + tail.
        let total = block_size * 2 * ch + 137;

        let f32_samples: Vec<f32> = (0..total)
            .map(|i| ((i % 200) as f32 / 200.0) - 0.5)
            .collect();

        // --- Single write ---
        let path_single =
            std::env::temp_dir().join(format!("sn-flac-single-{}.flac", std::process::id()));
        {
            let mut w = FlacWriterImpl::create(path_single.clone(), sample_rate, channels).unwrap();
            w.write(&f32_samples).unwrap();
            Box::new(w).finalize().unwrap();
        }
        let single_bytes = std::fs::read(&path_single).unwrap();
        std::fs::remove_file(&path_single).ok();

        // --- Multiple writes with straddling sizes ---
        let path_multi =
            std::env::temp_dir().join(format!("sn-flac-multi-{}.flac", std::process::id()));
        {
            let mut w = FlacWriterImpl::create(path_multi.clone(), sample_rate, channels).unwrap();
            // RAM invariant: pending.len() < block_size * channels after every write.
            let chunk_sizes = [1000usize, block_size * 2 * ch + 1, 3];
            let mut pos = 0usize;
            for &len in &chunk_sizes {
                let end = (pos + len).min(f32_samples.len());
                if pos < end {
                    w.write(&f32_samples[pos..end]).unwrap();
                    // Structural invariant: pending is always < one block.
                    assert!(
                        w.pending.len() < block_size * ch,
                        "pending invariant violated after write of {len} samples"
                    );
                    pos = end;
                }
            }
            if pos < f32_samples.len() {
                w.write(&f32_samples[pos..]).unwrap();
                assert!(w.pending.len() < block_size * ch);
            }
            Box::new(w).finalize().unwrap();
        }
        let multi_bytes = std::fs::read(&path_multi).unwrap();
        std::fs::remove_file(&path_multi).ok();

        assert_eq!(
            single_bytes, multi_bytes,
            "multi-write must produce identical bytes to single write"
        );
    }

    /// DiskFull classification unit test: verify classify_io_error_at maps
    /// StorageFull to AudioError::DiskFull regardless of op string.
    #[test]
    fn disk_full_classification() {
        use std::io;
        let path = PathBuf::from("/tmp/test.flac");
        let storage_full = io::Error::new(io::ErrorKind::StorageFull, "no space");
        let err = classify_io_error_at("write frame", storage_full, &path);
        assert!(
            matches!(err, AudioError::DiskFull { .. }),
            "StorageFull must map to AudioError::DiskFull, got: {err:?}"
        );
    }
}
