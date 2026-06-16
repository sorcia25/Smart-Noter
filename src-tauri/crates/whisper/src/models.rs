use std::path::{Path, PathBuf};

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
