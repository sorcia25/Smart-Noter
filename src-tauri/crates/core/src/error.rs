use serde::{Deserialize, Serialize};
use specta::Type;
use thiserror::Error;

#[derive(Debug, Clone, Type, Serialize, Deserialize, PartialEq, Eq)]
pub enum AudioErrorCode {
    DeviceNotFound,
    WasapiInit,
    FormatUnsupported,
    DiskFull,
    AlreadyRecording,
    NotRecording,
    MixerOverflow,
    Other,
}

#[derive(Debug, Error, Type, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "code", content = "message")]
pub enum AppError {
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Database error: {0}")]
    Database(String),
    #[error("Validation error: {0}")]
    Validation(String),
    #[error("Audio error ({code:?}): {message}")]
    Audio {
        code: AudioErrorCode,
        message: String,
    },
    #[error("Internal error: {0}")]
    Internal(String),
}

impl AppError {
    pub fn i18n_key(&self) -> &'static str {
        match self {
            AppError::NotFound(_) => "errors.notFound",
            AppError::Database(_) => "errors.database",
            AppError::Validation(_) => "errors.validation",
            AppError::Audio { code, .. } => match code {
                AudioErrorCode::DeviceNotFound => "audioError.DeviceNotFound",
                AudioErrorCode::WasapiInit => "audioError.WasapiInit",
                AudioErrorCode::FormatUnsupported => "audioError.FormatUnsupported",
                AudioErrorCode::DiskFull => "audioError.DiskFull",
                AudioErrorCode::AlreadyRecording => "audioError.AlreadyRecording",
                AudioErrorCode::NotRecording => "audioError.NotRecording",
                AudioErrorCode::MixerOverflow => "audioError.MixerOverflow",
                AudioErrorCode::Other => "audioError.Other",
            },
            AppError::Internal(_) => "errors.internal",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_to_tagged_json() {
        let err = AppError::NotFound("meeting m-999".into());
        let json = serde_json::to_string(&err).unwrap();
        assert_eq!(json, r#"{"code":"notFound","message":"meeting m-999"}"#);
    }

    #[test]
    fn each_variant_has_i18n_key() {
        assert_eq!(AppError::NotFound("x".into()).i18n_key(), "errors.notFound");
        assert_eq!(AppError::Database("x".into()).i18n_key(), "errors.database");
        assert_eq!(
            AppError::Validation("x".into()).i18n_key(),
            "errors.validation"
        );
        assert_eq!(AppError::Internal("x".into()).i18n_key(), "errors.internal");
    }

    #[test]
    fn audio_error_serializes_with_code_field() {
        let err = AppError::Audio {
            code: AudioErrorCode::DeviceNotFound,
            message: "loopback-001".into(),
        };
        let json = serde_json::to_string(&err).unwrap();
        assert_eq!(
            json,
            r#"{"code":"audio","message":{"code":"DeviceNotFound","message":"loopback-001"}}"#
        );
    }

    #[test]
    fn audio_error_i18n_key_routes_by_audio_code() {
        let err = AppError::Audio {
            code: AudioErrorCode::DiskFull,
            message: "C:/x".into(),
        };
        assert_eq!(err.i18n_key(), "audioError.DiskFull");
    }
}
