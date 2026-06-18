use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DiarizationErrorCode {
    ModelNotDownloaded,
    ModelLoadFailed,
    DiarizationFailed,
    DownloadBusy,
    DownloadFailed,
    Cancelled,
}

#[derive(Debug, thiserror::Error)]
#[error("{code:?}: {message}")]
pub struct DiarizationError {
    pub code: DiarizationErrorCode,
    pub message: String,
}

impl From<DiarizationError> for smart_noter_core::AppError {
    fn from(e: DiarizationError) -> Self {
        smart_noter_core::AppError::Transcription {
            code: format!("{:?}", e.code),
            message: e.message,
        }
    }
}
