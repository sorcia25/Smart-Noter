use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TranscriptionErrorCode {
    ModelNotDownloaded,
    TranscriptionBusy,
    DecodeFailed,
    ModelLoadFailed,
    InferenceFailed,
    DownloadBusy,
    DownloadFailed,
    Cancelled,
    /// Provider/key misconfiguration (e.g. a cloud STT provider is selected but
    /// no API key / Azure endpoint / deployment is configured).
    ConfigError,
}

#[derive(Debug, thiserror::Error)]
#[error("{code:?}: {message}")]
pub struct TranscriptionError {
    pub code: TranscriptionErrorCode,
    pub message: String,
}

impl From<TranscriptionError> for smart_noter_core::AppError {
    fn from(e: TranscriptionError) -> Self {
        smart_noter_core::AppError::Transcription {
            code: format!("{:?}", e.code),
            message: e.message,
        }
    }
}
