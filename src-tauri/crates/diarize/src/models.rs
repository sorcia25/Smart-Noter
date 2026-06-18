use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use crate::error::{DiarizationError, DiarizationErrorCode};

/// One downloadable ONNX component of the diarization set.
#[derive(Debug, Clone)]
pub struct ModelSpec {
    pub id: &'static str, // stable id, also the on-disk file name
    pub display_name: &'static str,
    pub size_mb: u32,
    pub sha256: &'static str,
    pub url: &'static str,
}

impl ModelSpec {
    pub fn file_name(&self) -> String {
        format!("{}.onnx", self.id)
    }
}

/// The canonical diarization set: a pyannote-style segmentation model + a
/// speaker-embedding model. Both must be present to diarize.
/// Values verified by downloading the files during planning.
pub const CATALOG: &[ModelSpec] = &[
    ModelSpec {
        id: "segmentation",
        display_name: "Speaker Segmentation (pyannote 3.0)",
        size_mb: 6, // 5_992_913 bytes
        sha256: "220ad67ca923bef2fa91f2390c786097bf305bceb5e261d4af67b38e938e1079",
        // Direct .onnx on HuggingFace (avoids the .tar.bz2 archive on GitHub releases).
        url: "https://huggingface.co/csukuangfj/sherpa-onnx-pyannote-segmentation-3-0/resolve/main/model.onnx",
    },
    ModelSpec {
        id: "embedding",
        display_name: "Speaker Embedding (WeSpeaker CAM++)",
        size_mb: 28, // 29_292_684 bytes
        sha256: "c46fad10b5f81e1aa4a60c162714208577093655076c5450f8c469e522ec54ef",
        url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/speaker-recongition-models/wespeaker_en_voxceleb_CAM++.onnx",
    },
];

pub fn find(id: &str) -> Option<&'static ModelSpec> {
    CATALOG.iter().find(|m| m.id == id)
}

/// Diarization models live in `<app_data>/diarize-models` (separate from whisper's `models`).
pub fn models_dir(app_data: &Path) -> PathBuf {
    app_data.join("diarize-models")
}

pub fn model_path(app_data: &Path, id: &str) -> Option<PathBuf> {
    find(id).map(|m| models_dir(app_data).join(m.file_name()))
}

/// True only when EVERY component in the catalog is present on disk.
pub fn all_present(app_data: &Path) -> bool {
    CATALOG
        .iter()
        .all(|m| models_dir(app_data).join(m.file_name()).is_file())
}

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

fn err(code: DiarizationErrorCode, message: impl Into<String>) -> DiarizationError {
    DiarizationError {
        code,
        message: message.into(),
    }
}

pub fn verify_sha256(path: &Path, expected: &str) -> Result<(), DiarizationError> {
    use sha2::{Digest, Sha256};
    let mut file = std::fs::File::open(path)
        .map_err(|e| err(DiarizationErrorCode::DownloadFailed, e.to_string()))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 1 << 16];
    loop {
        let n = file
            .read(&mut buf)
            .map_err(|e| err(DiarizationErrorCode::DownloadFailed, e.to_string()))?;
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
            DiarizationErrorCode::DownloadFailed,
            format!("sha256 mismatch: got {got}"),
        ))
    }
}

pub fn delete(app_data: &Path, id: &str) -> Result<(), DiarizationError> {
    let spec = find(id).ok_or_else(|| {
        err(
            DiarizationErrorCode::DownloadFailed,
            format!("unknown model {id}"),
        )
    })?;
    let path = models_dir(app_data).join(spec.file_name());
    if path.exists() {
        std::fs::remove_file(&path)
            .map_err(|e| err(DiarizationErrorCode::DownloadFailed, e.to_string()))?;
    }
    Ok(())
}

/// Stream-download one component to `tmp-<file>`, verify sha256, atomic rename.
pub fn download(
    app_data: &Path,
    id: &str,
    mut progress: impl FnMut(u32, u64, u64),
    is_cancelled: impl Fn() -> bool,
) -> Result<(), DiarizationError> {
    let spec = find(id).ok_or_else(|| {
        err(
            DiarizationErrorCode::DownloadFailed,
            format!("unknown model {id}"),
        )
    })?;
    let dir = models_dir(app_data);
    std::fs::create_dir_all(&dir)
        .map_err(|e| err(DiarizationErrorCode::DownloadFailed, e.to_string()))?;
    let final_path = dir.join(spec.file_name());
    let tmp_path = dir.join(format!("tmp-{}", spec.file_name()));

    let resp = ureq::get(spec.url)
        .call()
        .map_err(|e| err(DiarizationErrorCode::DownloadFailed, e.to_string()))?;
    let total: u64 = resp
        .header("Content-Length")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    let mut reader = resp.into_reader();
    let mut file = std::fs::File::create(&tmp_path)
        .map_err(|e| err(DiarizationErrorCode::DownloadFailed, e.to_string()))?;
    let mut buf = [0u8; 1 << 16];
    let mut downloaded: u64 = 0;
    loop {
        if is_cancelled() {
            let _ = std::fs::remove_file(&tmp_path);
            return Err(err(DiarizationErrorCode::Cancelled, "download cancelled"));
        }
        let n = reader
            .read(&mut buf)
            .map_err(|e| err(DiarizationErrorCode::DownloadFailed, e.to_string()))?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])
            .map_err(|e| err(DiarizationErrorCode::DownloadFailed, e.to_string()))?;
        downloaded += n as u64;
        let pct = (downloaded * 100).checked_div(total).unwrap_or(0) as u32;
        progress(pct, downloaded, total);
    }
    drop(file);

    verify_sha256(&tmp_path, spec.sha256).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp_path);
    })?;
    std::fs::rename(&tmp_path, &final_path)
        .map_err(|e| err(DiarizationErrorCode::DownloadFailed, e.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    fn hex(bytes: &[u8]) -> String {
        let mut h = Sha256::new();
        h.update(bytes);
        h.finalize().iter().map(|b| format!("{b:02x}")).collect()
    }

    #[test]
    fn catalog_has_two_components_with_complete_metadata() {
        let ids: Vec<&str> = CATALOG.iter().map(|m| m.id).collect();
        assert_eq!(ids, vec!["segmentation", "embedding"]);
        for m in CATALOG {
            assert!(!m.id.is_empty());
            assert!(!m.display_name.is_empty());
            assert_eq!(
                m.sha256.len(),
                64,
                "sha256 must be 64 hex chars for {}",
                m.id
            );
        }
    }

    #[test]
    fn all_present_true_only_when_both_files_exist() {
        let tmp = tempfile::tempdir().unwrap();
        let app = tmp.path();
        std::fs::create_dir_all(models_dir(app)).unwrap();
        assert!(!all_present(app));
        std::fs::write(models_dir(app).join("segmentation.onnx"), b"x").unwrap();
        assert!(!all_present(app)); // only one of two
        std::fs::write(models_dir(app).join("embedding.onnx"), b"y").unwrap();
        assert!(all_present(app));
    }

    #[test]
    fn list_marks_present_files_as_downloaded() {
        let tmp = tempfile::tempdir().unwrap();
        let app = tmp.path();
        std::fs::create_dir_all(models_dir(app)).unwrap();
        std::fs::write(models_dir(app).join("segmentation.onnx"), b"x").unwrap();
        let listed = list(app);
        assert!(
            listed
                .iter()
                .find(|m| m.id == "segmentation")
                .unwrap()
                .downloaded
        );
        assert!(
            !listed
                .iter()
                .find(|m| m.id == "embedding")
                .unwrap()
                .downloaded
        );
    }

    #[test]
    fn verify_sha256_accepts_match_rejects_mismatch() {
        let tmp = tempfile::tempdir().unwrap();
        let f = tmp.path().join("blob.onnx");
        std::fs::write(&f, b"hello").unwrap();
        assert!(verify_sha256(&f, &hex(b"hello")).is_ok());
        assert!(verify_sha256(&f, &"0".repeat(64)).is_err());
    }

    #[test]
    fn delete_removes_a_downloaded_component() {
        let tmp = tempfile::tempdir().unwrap();
        let app = tmp.path();
        std::fs::create_dir_all(models_dir(app)).unwrap();
        let p = models_dir(app).join("segmentation.onnx");
        std::fs::write(&p, b"x").unwrap();
        delete(app, "segmentation").unwrap();
        assert!(!p.exists());
    }
}
