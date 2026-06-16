use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use crate::error::{TranscriptionError, TranscriptionErrorCode};

/// One downloadable Whisper model (ggml `.bin` from Hugging Face).
#[derive(Debug, Clone)]
pub struct ModelSpec {
    pub id: &'static str,
    pub display_name: &'static str,
    pub size_mb: u32,
    pub sha256: &'static str,
    pub url: &'static str,
}

impl ModelSpec {
    pub fn file_name(&self) -> String {
        format!("ggml-{}.bin", self.id)
    }
}

/// Pinned to `ggerganov/whisper.cpp` ggml releases on Hugging Face.
/// sha256 values obtained from git-LFS pointer files (oid sha256:) at
/// https://huggingface.co/ggerganov/whisper.cpp/raw/main/ggml-<model>.bin
/// size_mb = bytes / 1_048_576, rounded (from LFS pointer `size` field).
pub const CATALOG: &[ModelSpec] = &[
    ModelSpec {
        id: "base",
        display_name: "Whisper Base",
        size_mb: 141,
        sha256: "60ed5bc3dd14eea856493d334349b405782ddcaf0028d4b5df4088345fba2efe",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin",
    },
    ModelSpec {
        id: "small",
        display_name: "Whisper Small",
        size_mb: 465,
        sha256: "1be3a9b2063867b937e64e2ec7483364a79917e157fa98c5d94b5c1fffea987b",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin",
    },
    ModelSpec {
        id: "medium",
        display_name: "Whisper Medium",
        size_mb: 1463,
        sha256: "6c14d5adee5f86394037b4e4e8b59f1673b6cee10e3cf0b11bbdbee79c156208",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.bin",
    },
    ModelSpec {
        id: "large-v3",
        display_name: "Whisper Large v3",
        size_mb: 2951,
        sha256: "64d182b440b98d5203c4f9bd541544d84c605196c4f7b845dfa11fb23594d1e2",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3.bin",
    },
];

pub fn find(id: &str) -> Option<&'static ModelSpec> {
    CATALOG.iter().find(|m| m.id == id)
}

/// `<app_data>/models`. Caller passes the Tauri app-data dir so this stays
/// platform-agnostic and testable.
pub fn models_dir(app_data: &Path) -> PathBuf {
    app_data.join("models")
}

pub fn model_path(app_data: &Path, id: &str) -> Option<PathBuf> {
    find(id).map(|m| models_dir(app_data).join(m.file_name()))
}

/// A catalog entry plus on-disk status (the shape the command layer returns to the UI).
#[derive(Debug, Clone)]
pub struct ModelStatus {
    pub id: &'static str,
    pub display_name: &'static str,
    pub size_mb: u32,
    pub downloaded: bool,
}

pub fn list(app_data: &Path) -> Vec<ModelStatus> {
    CATALOG
        .iter()
        .map(|m| ModelStatus {
            id: m.id,
            display_name: m.display_name,
            size_mb: m.size_mb,
            downloaded: models_dir(app_data).join(m.file_name()).is_file(),
        })
        .collect()
}

fn err(code: TranscriptionErrorCode, message: impl Into<String>) -> TranscriptionError {
    TranscriptionError {
        code,
        message: message.into(),
    }
}

pub fn verify_sha256(path: &Path, expected: &str) -> Result<(), TranscriptionError> {
    use sha2::{Digest, Sha256};
    let mut file = std::fs::File::open(path)
        .map_err(|e| err(TranscriptionErrorCode::DownloadFailed, e.to_string()))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 1 << 16];
    loop {
        let n = file
            .read(&mut buf)
            .map_err(|e| err(TranscriptionErrorCode::DownloadFailed, e.to_string()))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let got: String = hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    if got.eq_ignore_ascii_case(expected) {
        Ok(())
    } else {
        Err(err(
            TranscriptionErrorCode::DownloadFailed,
            format!("sha256 mismatch: got {got}"),
        ))
    }
}

pub fn delete(app_data: &Path, id: &str) -> Result<(), TranscriptionError> {
    let spec = find(id).ok_or_else(|| {
        err(
            TranscriptionErrorCode::DownloadFailed,
            format!("unknown model {id}"),
        )
    })?;
    let path = models_dir(app_data).join(spec.file_name());
    if path.exists() {
        std::fs::remove_file(&path)
            .map_err(|e| err(TranscriptionErrorCode::DownloadFailed, e.to_string()))?;
    }
    Ok(())
}

/// Stream-download a model to `tmp-<file>` with progress, verify sha256, atomic rename.
/// `progress(pct, downloaded, total)` is called as bytes arrive. `is_cancelled()` is
/// polled to allow a cooperative abort.
pub fn download(
    app_data: &Path,
    id: &str,
    mut progress: impl FnMut(u32, u64, u64),
    is_cancelled: impl Fn() -> bool,
) -> Result<(), TranscriptionError> {
    let spec = find(id).ok_or_else(|| {
        err(
            TranscriptionErrorCode::DownloadFailed,
            format!("unknown model {id}"),
        )
    })?;
    let dir = models_dir(app_data);
    std::fs::create_dir_all(&dir)
        .map_err(|e| err(TranscriptionErrorCode::DownloadFailed, e.to_string()))?;
    let final_path = dir.join(spec.file_name());
    let tmp_path = dir.join(format!("tmp-{}", spec.file_name()));

    let resp = ureq::get(spec.url)
        .call()
        .map_err(|e| err(TranscriptionErrorCode::DownloadFailed, e.to_string()))?;
    let total: u64 = resp
        .header("Content-Length")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    let mut reader = resp.into_reader();
    let mut file = std::fs::File::create(&tmp_path)
        .map_err(|e| err(TranscriptionErrorCode::DownloadFailed, e.to_string()))?;
    let mut buf = [0u8; 1 << 16];
    let mut downloaded: u64 = 0;
    loop {
        if is_cancelled() {
            let _ = std::fs::remove_file(&tmp_path);
            return Err(err(TranscriptionErrorCode::Cancelled, "download cancelled"));
        }
        let n = reader
            .read(&mut buf)
            .map_err(|e| err(TranscriptionErrorCode::DownloadFailed, e.to_string()))?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])
            .map_err(|e| err(TranscriptionErrorCode::DownloadFailed, e.to_string()))?;
        downloaded += n as u64;
        let pct = (downloaded * 100).checked_div(total).unwrap_or(0) as u32;
        progress(pct, downloaded, total);
    }
    drop(file);

    verify_sha256(&tmp_path, spec.sha256).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp_path);
    })?;
    std::fs::rename(&tmp_path, &final_path)
        .map_err(|e| err(TranscriptionErrorCode::DownloadFailed, e.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod fs_tests {
    use super::*;
    use sha2::{Digest, Sha256};

    fn hex(bytes: &[u8]) -> String {
        let mut h = Sha256::new();
        h.update(bytes);
        h.finalize().iter().map(|b| format!("{b:02x}")).collect()
    }

    #[test]
    fn list_marks_present_files_as_downloaded() {
        let tmp = tempfile::tempdir().unwrap();
        let app = tmp.path();
        std::fs::create_dir_all(models_dir(app)).unwrap();
        // Drop a fake "base" file in place.
        std::fs::write(models_dir(app).join("ggml-base.bin"), b"x").unwrap();

        let listed = list(app);
        let base = listed.iter().find(|m| m.id == "base").unwrap();
        let large = listed.iter().find(|m| m.id == "large-v3").unwrap();
        assert!(base.downloaded);
        assert!(!large.downloaded);
    }

    #[test]
    fn verify_sha256_accepts_match_rejects_mismatch() {
        let tmp = tempfile::tempdir().unwrap();
        let f = tmp.path().join("blob.bin");
        std::fs::write(&f, b"hello").unwrap();
        let good = hex(b"hello");
        assert!(verify_sha256(&f, &good).is_ok());
        assert!(verify_sha256(&f, &"0".repeat(64)).is_err());
    }

    #[test]
    fn delete_removes_a_downloaded_model() {
        let tmp = tempfile::tempdir().unwrap();
        let app = tmp.path();
        std::fs::create_dir_all(models_dir(app)).unwrap();
        let p = models_dir(app).join("ggml-base.bin");
        std::fs::write(&p, b"x").unwrap();
        delete(app, "base").unwrap();
        assert!(!p.exists());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_has_the_four_models_with_complete_metadata() {
        let ids: Vec<&str> = CATALOG.iter().map(|m| m.id).collect();
        assert_eq!(ids, vec!["base", "small", "medium", "large-v3"]);
        for m in CATALOG {
            assert!(!m.id.is_empty());
            assert!(!m.display_name.is_empty());
            assert!(m.size_mb > 0);
            assert_eq!(
                m.sha256.len(),
                64,
                "sha256 must be 64 hex chars for {}",
                m.id
            );
            assert!(
                m.url.starts_with("https://"),
                "url must be https for {}",
                m.id
            );
        }
    }

    #[test]
    fn find_returns_model_by_id() {
        assert!(find("large-v3").is_some());
        assert!(find("nope").is_none());
    }

    #[test]
    fn file_name_is_derived_from_id() {
        assert_eq!(find("base").unwrap().file_name(), "ggml-base.bin");
    }
}
