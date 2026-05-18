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
    pub capture_mode: CaptureMode,
    pub default_device: String,
    pub recording_quality: String,
    pub run_local: bool,
    pub auto_delete_audio: bool,
    pub transcription_provider: String,
    pub transcription_model: String,
    pub default_template: String,
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

#[derive(Debug, Clone, Type, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CaptureMode {
    System,
    Mic,
    Mix,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: Theme::Light,
            accent: "#10b981".into(),
            language: Language::Es,
            avatar_style: AvatarStyle::Circle,
            ai_chat_visible: true,
            capture_mode: CaptureMode::System,
            default_device: "system-loopback".into(),
            recording_quality: "WAV 48k".into(),
            run_local: true,
            auto_delete_audio: false,
            transcription_provider: "local".into(),
            transcription_model: "Whisper Large v3".into(),
            default_template: "tecnica".into(),
        }
    }
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
}
