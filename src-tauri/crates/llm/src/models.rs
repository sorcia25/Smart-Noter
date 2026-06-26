use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use crate::AiError;

/// One downloadable GGUF model (local LLM or embedding model).
#[derive(Debug, Clone)]
pub struct ModelSpec {
    pub id: &'static str,
    pub display_name: &'static str,
    pub size_mb: u32,
    /// SHA-256 hex digest of the final file.  When this is an empty string the
    /// integrity check is **skipped** (a warning is logged instead).  Real
    /// hashes are pinned during the Task-17 smoke test once the files have been
    /// actually downloaded and hashed locally.
    pub sha256: &'static str,
    pub url: &'static str,
}

impl ModelSpec {
    pub fn file_name(&self) -> String {
        format!("llm-{}.gguf", self.id)
    }
}

/// Catalog of bundled GGUF models.
///
/// LLM:
///   bartowski/Qwen2.5-3B-Instruct-GGUF – Q4_K_M quant ("Good quality,
///   default size for most use cases", ~1.93 GB).
///
/// Embeddings:
///   rodion-m/multilingual-e5-small-gguf – fp32, 476 MB.  Used for RAG
///   similarity search.  A smaller, quantised variant can replace this later.
///
/// SHA-256 fields are intentionally empty: the files are too large to hash
/// without downloading, and the HF repository pages do not publish them.
/// `download()` skips verification when `sha256` is empty (logs a warning).
pub const CATALOG: &[ModelSpec] = &[
    ModelSpec {
        id: "qwen2.5-3b-instruct-q4",
        display_name: "Qwen2.5-3B Instruct (Q4_K_M)",
        size_mb: 1979, // ~1.93 GB
        sha256: "",    // pinned during Task-17 smoke
        url: "https://huggingface.co/bartowski/Qwen2.5-3B-Instruct-GGUF/resolve/main/Qwen2.5-3B-Instruct-Q4_K_M.gguf",
    },
    ModelSpec {
        id: "e5-small-embed",
        display_name: "Multilingual E5 Small (FP32 embeddings)",
        size_mb: 476,
        sha256: "", // pinned during Task-17 smoke
        url: "https://huggingface.co/rodion-m/multilingual-e5-small-gguf/resolve/main/multilingual-e5-small-fp32.gguf",
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

/// Full path where the downloaded GGUF is stored.
/// Returns the path unconditionally (unlike whisper's `model_path` which
/// returns `Option` — here the id is always resolvable given a known catalog).
pub fn model_path(app_data: &Path, id: &str) -> PathBuf {
    // Use the file_name from the catalog entry when found; fall back to the
    // generic pattern so callers can call this with known IDs without unwrapping.
    let file = find(id)
        .map(|m| m.file_name())
        .unwrap_or_else(|| format!("llm-{id}.gguf"));
    models_dir(app_data).join(file)
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

fn ai_err(message: impl Into<String>) -> AiError {
    AiError::Download(message.into())
}

pub fn verify_sha256(path: &Path, expected: &str) -> Result<(), AiError> {
    use sha2::{Digest, Sha256};
    let mut file = std::fs::File::open(path).map_err(|e| ai_err(e.to_string()))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 1 << 16];
    loop {
        let n = file.read(&mut buf).map_err(|e| ai_err(e.to_string()))?;
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
        Err(ai_err(format!("sha256 mismatch: got {got}")))
    }
}

pub fn delete(app_data: &Path, id: &str) -> Result<(), AiError> {
    let spec = find(id).ok_or_else(|| AiError::ModelMissing(format!("unknown model {id}")))?;
    let path = models_dir(app_data).join(spec.file_name());
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| ai_err(e.to_string()))?;
    }
    Ok(())
}

/// Stream-download a model to `tmp-<file>` with progress, verify sha256 (skipped
/// when `sha256` is empty), then atomic rename into place.
///
/// `progress(pct, downloaded, total)` is called as bytes arrive.
/// `is_cancelled()` is polled cooperatively to allow an abort.
pub fn download(
    app_data: &Path,
    id: &str,
    mut progress: impl FnMut(u32, u64, u64),
    is_cancelled: impl Fn() -> bool,
) -> Result<(), AiError> {
    let spec = find(id).ok_or_else(|| AiError::ModelMissing(format!("unknown model {id}")))?;
    let dir = models_dir(app_data);
    std::fs::create_dir_all(&dir).map_err(|e| ai_err(e.to_string()))?;
    let final_path = dir.join(spec.file_name());
    let tmp_path = dir.join(format!("tmp-{}", spec.file_name()));

    let resp = ureq::get(spec.url)
        .call()
        .map_err(|e| ai_err(e.to_string()))?;
    let total: u64 = resp
        .header("Content-Length")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    let mut reader = resp.into_reader();
    let mut file = std::fs::File::create(&tmp_path).map_err(|e| ai_err(e.to_string()))?;
    let mut buf = [0u8; 1 << 16];
    let mut downloaded: u64 = 0;
    loop {
        if is_cancelled() {
            let _ = std::fs::remove_file(&tmp_path);
            return Err(AiError::Download("download cancelled".into()));
        }
        let n = reader.read(&mut buf).map_err(|e| ai_err(e.to_string()))?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])
            .map_err(|e| ai_err(e.to_string()))?;
        downloaded += n as u64;
        let pct = (downloaded * 100).checked_div(total).unwrap_or(0) as u32;
        progress(pct, downloaded, total);
    }
    drop(file);

    if spec.sha256.is_empty() {
        // Real hash not yet pinned — skip verification and warn.
        eprintln!(
            "[llm::models] WARNING: sha256 not pinned for model '{}'; skipping integrity check",
            spec.id
        );
    } else {
        verify_sha256(&tmp_path, spec.sha256).inspect_err(|_| {
            let _ = std::fs::remove_file(&tmp_path);
        })?;
    }

    std::fs::rename(&tmp_path, &final_path).map_err(|e| ai_err(e.to_string()))?;
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
        // Drop a fake "qwen" file in place.
        let fname = find("qwen2.5-3b-instruct-q4").unwrap().file_name();
        std::fs::write(models_dir(app).join(&fname), b"x").unwrap();

        let listed = list(app);
        let qwen = listed
            .iter()
            .find(|m| m.id == "qwen2.5-3b-instruct-q4")
            .unwrap();
        let e5 = listed.iter().find(|m| m.id == "e5-small-embed").unwrap();
        assert!(qwen.downloaded);
        assert!(!e5.downloaded);
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
        let fname = find("qwen2.5-3b-instruct-q4").unwrap().file_name();
        let p = models_dir(app).join(&fname);
        std::fs::write(&p, b"x").unwrap();
        delete(app, "qwen2.5-3b-instruct-q4").unwrap();
        assert!(!p.exists());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_and_paths() {
        let dir = tempfile::tempdir().unwrap();
        assert!(CATALOG.iter().any(|m| m.id == "e5-small-embed"));
        assert!(model_path(dir.path(), "qwen2.5-3b-instruct-q4")
            .ends_with("llm-qwen2.5-3b-instruct-q4.gguf"));
        assert!(list(dir.path()).iter().all(|m| !m.downloaded));
    }

    #[test]
    fn catalog_has_both_models_with_complete_metadata() {
        let ids: Vec<&str> = CATALOG.iter().map(|m| m.id).collect();
        assert_eq!(ids, vec!["qwen2.5-3b-instruct-q4", "e5-small-embed"]);
        for m in CATALOG {
            assert!(!m.id.is_empty());
            assert!(!m.display_name.is_empty());
            assert!(m.size_mb > 0);
            assert!(
                m.url.starts_with("https://"),
                "url must be https for {}",
                m.id
            );
        }
    }

    #[test]
    fn find_returns_model_by_id() {
        assert!(find("qwen2.5-3b-instruct-q4").is_some());
        assert!(find("e5-small-embed").is_some());
        assert!(find("nope").is_none());
    }

    #[test]
    fn file_name_has_llm_prefix() {
        assert_eq!(
            find("qwen2.5-3b-instruct-q4").unwrap().file_name(),
            "llm-qwen2.5-3b-instruct-q4.gguf"
        );
        assert_eq!(
            find("e5-small-embed").unwrap().file_name(),
            "llm-e5-small-embed.gguf"
        );
    }

    #[test]
    fn model_path_includes_models_subdir() {
        let dir = tempfile::tempdir().unwrap();
        let p = model_path(dir.path(), "e5-small-embed");
        assert!(p.to_string_lossy().contains("models"));
        assert!(p.to_string_lossy().ends_with("llm-e5-small-embed.gguf"));
    }
}
