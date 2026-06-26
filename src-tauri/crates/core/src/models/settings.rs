use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Type, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub theme: Theme,
    pub accent: String,
    pub language: Language,
    pub avatar_style: AvatarStyle,
    pub ai_chat_visible: bool,
    /// Wire format mirrors `audio::capture::session::CaptureMode` ("system" | "mic" | "mix").
    /// Kept as `String` here so the persistence layer doesn't depend on the audio crate
    /// and the IPC bindings emit only one `CaptureMode` type alias.
    pub capture_mode: String,
    pub default_device: String,
    pub recording_quality: String,
    pub run_local: bool,
    pub auto_delete_audio: bool,
    pub transcription_provider: String,
    pub transcription_model: String,
    pub auto_transcribe: bool,
    pub native_language: String,
    pub default_template: String,
    #[serde(default = "default_true")]
    pub identify_speakers: bool,
    #[serde(default = "default_diar_model")]
    pub diarization_model: String,
    #[serde(default = "default_true")]
    pub auto_generate_summary: bool,
}

#[derive(Debug, Clone, Type, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Light,
    Dark,
}

#[derive(Debug, Clone, Type, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    Es,
    En,
}

#[derive(Debug, Clone, Type, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AvatarStyle {
    Circle,
    Square,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: Theme::Light,
            accent: "#10b981".into(),
            language: Language::Es,
            avatar_style: AvatarStyle::Circle,
            ai_chat_visible: true,
            capture_mode: "system".into(),
            default_device: "system-loopback".into(),
            recording_quality: "WAV 48k".into(),
            run_local: true,
            auto_delete_audio: false,
            transcription_provider: "local".into(),
            transcription_model: "large-v3".into(),
            auto_transcribe: true,
            native_language: "es".into(),
            default_template: "tecnica".into(),
            identify_speakers: true,
            diarization_model: "default".into(),
            auto_generate_summary: true,
        }
    }
}

fn default_true() -> bool {
    true
}
fn default_diar_model() -> String {
    "default".into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_settings_roundtrip_through_json() {
        let original = AppSettings::default();
        let json = serde_json::to_string(&original).unwrap();
        let parsed: AppSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.theme, original.theme);
        assert_eq!(parsed.language, original.language);
        assert_eq!(parsed.accent, original.accent);
    }

    #[test]
    fn theme_serializes_lowercase() {
        let json = serde_json::to_string(&Theme::Dark).unwrap();
        assert_eq!(json, r#""dark""#);
    }

    #[test]
    fn defaults_include_transcription_fields() {
        let d = AppSettings::default();
        assert_eq!(d.transcription_model, "large-v3");
        assert!(d.auto_transcribe);
        assert_eq!(d.native_language, "es");
    }

    #[test]
    fn defaults_enable_speaker_identification() {
        let d = AppSettings::default();
        assert!(d.identify_speakers);
        assert_eq!(d.diarization_model, "default");
    }

    #[test]
    fn legacy_blob_without_new_fields_uses_serde_defaults() {
        // A persisted blob from before these fields existed: omit identifySpeakers
        // and diarizationModel. Deserialization must succeed and fill defaults.
        let json = r##"{
            "theme":"light","accent":"#10b981","language":"es","avatarStyle":"circle",
            "aiChatVisible":true,"captureMode":"system","defaultDevice":"system-loopback",
            "recordingQuality":"WAV 48k","runLocal":true,"autoDeleteAudio":false,
            "transcriptionProvider":"local","transcriptionModel":"large-v3",
            "autoTranscribe":true,"nativeLanguage":"es","defaultTemplate":"tecnica"
        }"##;
        let parsed: AppSettings = serde_json::from_str(json).expect("legacy blob must deserialize");
        assert!(parsed.identify_speakers);
        assert_eq!(parsed.diarization_model, "default");
        assert!(parsed.auto_generate_summary);
    }
}
