//! Errors for the audio crate. They convert into `smart_noter_core::AppError::Audio`
//! at the Tauri command boundary; this module's variants carry richer context
//! (WASAPI HRESULT, drop counters, etc.) that gets summarised before the IPC hop.

use serde::{Deserialize, Serialize};
use smart_noter_core::{AppError, AudioErrorCode};
use thiserror::Error;

#[derive(Debug, Error, Serialize, Deserialize, Clone)]
#[serde(tag = "code", content = "message")]
pub enum AudioError {
    #[error("Device not found: {0}")]
    DeviceNotFound(String),

    #[error("Failed to initialize WASAPI: HRESULT={hresult:#x}")]
    WasapiInit { hresult: i32 },

    #[error("Format unsupported by device: {0}")]
    FormatUnsupported(String),

    #[error("Disk full while writing to {path}")]
    DiskFull { path: String },

    #[error("Recording session already active")]
    AlreadyRecording,

    #[error("No active recording session")]
    NotRecording,

    #[error("Audio pipeline overflow (dropped {dropped} frames)")]
    MixerOverflow { dropped: u32 },

    #[error("Unknown audio error: {0}")]
    Other(String),
}

impl From<AudioError> for AppError {
    fn from(e: AudioError) -> Self {
        let code = match &e {
            AudioError::DeviceNotFound(_) => AudioErrorCode::DeviceNotFound,
            AudioError::WasapiInit { .. } => AudioErrorCode::WasapiInit,
            AudioError::FormatUnsupported(_) => AudioErrorCode::FormatUnsupported,
            AudioError::DiskFull { .. } => AudioErrorCode::DiskFull,
            AudioError::AlreadyRecording => AudioErrorCode::AlreadyRecording,
            AudioError::NotRecording => AudioErrorCode::NotRecording,
            AudioError::MixerOverflow { .. } => AudioErrorCode::MixerOverflow,
            AudioError::Other(_) => AudioErrorCode::Other,
        };
        AppError::Audio {
            code,
            message: e.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_with_tagged_code() {
        let e = AudioError::DeviceNotFound("loopback-1".into());
        let json = serde_json::to_string(&e).unwrap();
        assert_eq!(json, r#"{"code":"DeviceNotFound","message":"loopback-1"}"#);
    }

    #[test]
    fn into_app_error_preserves_code() {
        let app: AppError = AudioError::DiskFull {
            path: "C:/x.wav".into(),
        }
        .into();
        match app {
            AppError::Audio { code, message } => {
                assert_eq!(code, AudioErrorCode::DiskFull);
                assert!(message.contains("C:/x.wav"));
            }
            other => panic!("expected Audio variant, got {other:?}"),
        }
    }

    #[test]
    fn mixer_overflow_format_is_helpful() {
        let e = AudioError::MixerOverflow { dropped: 137 };
        assert_eq!(
            format!("{e}"),
            "Audio pipeline overflow (dropped 137 frames)"
        );
    }
}
