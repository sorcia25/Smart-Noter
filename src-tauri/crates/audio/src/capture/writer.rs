//! Format-agnostic writer trait + concrete WAV implementation.
//! FLAC follows in the next task.

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
        if matches!(io.kind(), ErrorKind::StorageFull | ErrorKind::Other) {
            return AudioError::DiskFull {
                path: path.display().to_string(),
            };
        }
    }
    AudioError::Other(format!("WAV write: {e}"))
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
}
