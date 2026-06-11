//! Errors for the audio crate. They convert into `smart_noter_core::AppError::Audio`
//! at the Tauri command boundary; this module's variants carry richer context
//! (WASAPI HRESULT, drop counters, etc.) that gets summarised before the IPC hop.

use serde::{Deserialize, Serialize};
use smart_noter_core::{AppError, AudioErrorCode};
use specta::Type;
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

/// Flat event DTO emitted as `audio:error`.
///
/// The `AudioError` enum uses adjacent tagging (`#[serde(tag = "code", content = "message")]`)
/// which serialises struct-variants as `{"code":"Foo","message":{...}}` (an object, not a
/// string) and OMITS the `message` field entirely for unit-like variants. The frontend
/// listener (`App.tsx`) expects the flat shape `{ code: AudioErrorCode; message: string }`,
/// so we MUST NOT emit `AudioError` directly. This DTO always serialises to that flat shape.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct AudioErrorEvent {
    pub code: AudioErrorCode,
    pub message: String,
}

impl From<&AudioError> for AudioErrorEvent {
    fn from(e: &AudioError) -> Self {
        Self {
            code: e.code(),
            message: e.to_string(),
        }
    }
}

impl AudioError {
    /// Return the `AudioErrorCode` for this error.
    ///
    /// Centralises the variant→code mapping so both `From<AudioError> for AppError`
    /// and the meter thread's `AudioErrorEvent` conversion share one source of truth.
    pub fn code(&self) -> AudioErrorCode {
        match self {
            AudioError::DeviceNotFound(_) => AudioErrorCode::DeviceNotFound,
            AudioError::WasapiInit { .. } => AudioErrorCode::WasapiInit,
            AudioError::FormatUnsupported(_) => AudioErrorCode::FormatUnsupported,
            AudioError::DiskFull { .. } => AudioErrorCode::DiskFull,
            AudioError::AlreadyRecording => AudioErrorCode::AlreadyRecording,
            AudioError::NotRecording => AudioErrorCode::NotRecording,
            AudioError::MixerOverflow { .. } => AudioErrorCode::MixerOverflow,
            AudioError::Other(_) => AudioErrorCode::Other,
        }
    }
}

impl From<AudioError> for AppError {
    fn from(e: AudioError) -> Self {
        AppError::Audio {
            code: e.code(),
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

    // -----------------------------------------------------------------------
    // AudioErrorEvent flat serialisation contract
    // -----------------------------------------------------------------------

    /// `AudioErrorEvent` must serialise to the flat shape `{"code":"...","message":"..."}`
    /// that the frontend `audio:error` listener expects. The code must be the bare enum
    /// string (not adjacently-tagged) and the message must be a plain string.
    #[test]
    fn audio_error_event_serializes_flat_mixer_overflow() {
        let e = AudioError::MixerOverflow { dropped: 150 };
        let ev = AudioErrorEvent::from(&e);
        let json = serde_json::to_string(&ev).unwrap();
        // code is bare enum string, message is a plain string containing "150"
        assert!(
            json.contains(r#""code":"MixerOverflow""#),
            "code must be plain string 'MixerOverflow', got: {json}"
        );
        assert!(
            json.contains("150"),
            "message must contain the drop count 150, got: {json}"
        );
        // message must NOT be an object (no nested braces after "message":)
        let msg_start = json.find(r#""message":"#).expect("message key present");
        let after_colon = json[msg_start + 10..].trim_start();
        assert!(
            after_colon.starts_with('"'),
            "message value must be a JSON string, not an object; json={json}"
        );
    }

    #[test]
    fn audio_error_event_serializes_flat_already_recording() {
        let e = AudioError::AlreadyRecording;
        let ev = AudioErrorEvent::from(&e);
        let json = serde_json::to_string(&ev).unwrap();
        assert!(
            json.contains(r#""code":"AlreadyRecording""#),
            "code must be 'AlreadyRecording', got: {json}"
        );
        // message must be a non-empty string
        let msg_start = json.find(r#""message":"#).expect("message key present");
        let after_colon = json[msg_start + 10..].trim_start();
        assert!(
            after_colon.starts_with('"') && after_colon.len() > 2,
            "message must be a non-empty JSON string; json={json}"
        );
    }
}
