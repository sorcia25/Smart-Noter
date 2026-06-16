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
}

#[derive(Debug, thiserror::Error)]
#[error("{code:?}: {message}")]
pub struct TranscriptionError {
    pub code: TranscriptionErrorCode,
    pub message: String,
}
