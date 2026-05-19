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

/// Buffers interleaved i32 samples and encodes to FLAC on `finalize()`.
///
/// `flacenc` is a batch encoder: it requires all samples up front, so we
/// accumulate them in `buf` during `write()` calls and flush to disk in one
/// shot when `finalize()` is called.
pub struct FlacWriterImpl {
    buf: Vec<i32>,
    path: PathBuf,
    sample_count: u64,
    channels: u16,
    sample_rate: u32,
}

impl FlacWriterImpl {
    pub fn create(path: PathBuf, sample_rate: u32, channels: u16) -> Result<Self, AudioError> {
        // Validate the parent directory is reachable; defer file creation to finalize()
        // so an interrupted session doesn't leave a 0-byte file at `path`.
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() && !parent.exists() {
                return Err(AudioError::Other(format!(
                    "directory does not exist: {}",
                    parent.display()
                )));
            }
        }
        Ok(Self {
            buf: Vec::new(),
            path,
            sample_count: 0,
            channels,
            sample_rate,
        })
    }
}

impl AudioWriter for FlacWriterImpl {
    fn write(&mut self, samples: &[f32]) -> Result<(), AudioError> {
        self.buf.extend(
            samples
                .iter()
                .map(|&s| (s.clamp(-1.0, 1.0) * 32_767.0) as i32),
        );
        self.sample_count += samples.len() as u64;
        Ok(())
    }

    fn finalize(self: Box<Self>) -> Result<FinalizeResult, AudioError> {
        use flacenc::bitsink::ByteSink;
        use flacenc::component::BitRepr;
        use flacenc::error::Verify;
        use flacenc::source::MemSource;

        let config = flacenc::config::Encoder::default()
            .into_verified()
            .map_err(|(_enc, e)| AudioError::Other(format!("FLAC config: {e:?}")))?;

        let block_size = config.block_size;
        let source = MemSource::from_samples(
            &self.buf,
            self.channels as usize,
            16,
            self.sample_rate as usize,
        );

        let stream = flacenc::encode_with_fixed_block_size(&config, source, block_size)
            .map_err(|e| AudioError::Other(format!("FLAC create: {e}")))?;

        let mut sink = ByteSink::new();
        stream
            .write(&mut sink)
            .map_err(|e| AudioError::Other(format!("FLAC write: {e}")))?;

        // Write to a sibling `.tmp` path and atomically rename so a panic or
        // I/O failure mid-write doesn't leave a corrupted FLAC at the destination.
        let tmp = self.path.with_extension(match self.path.extension() {
            Some(ext) => format!("{}.tmp", ext.to_string_lossy()),
            None => "tmp".to_string(),
        });
        std::fs::write(&tmp, sink.as_slice()).map_err(|e| classify_create_error(e, &tmp))?;
        std::fs::rename(&tmp, &self.path).map_err(|e| classify_create_error(e, &self.path))?;

        let bytes = std::fs::metadata(&self.path).map(|m| m.len()).unwrap_or(0);

        Ok(FinalizeResult {
            path: self.path,
            bytes,
            sample_count: self.sample_count,
        })
    }
}

fn classify_create_error(e: std::io::Error, path: &Path) -> AudioError {
    use std::io::ErrorKind;
    if matches!(e.kind(), ErrorKind::StorageFull) {
        AudioError::DiskFull {
            path: path.display().to_string(),
        }
    } else {
        AudioError::Other(format!("create {}: {}", path.display(), e))
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
}
