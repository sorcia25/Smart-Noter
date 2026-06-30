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
    #[serde(default = "default_ai_provider")]
    pub ai_provider: String, // "local" | "openai" | "anthropic" | "azure"
    #[serde(default)]
    pub provider_models: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub azure_endpoint: String,
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
            ai_provider: "local".into(),
            provider_models: std::collections::BTreeMap::new(),
            azure_endpoint: String::new(),
        }
    }
}

fn default_true() -> bool {
    true
}
fn default_diar_model() -> String {
    "default".into()
}
fn default_ai_provider() -> String {
    "local".into()
}

impl AppSettings {
    /// The chat model to use for a given cloud provider. Per-provider models take
    /// precedence; otherwise a sensible per-provider default (empty for Azure,
    /// whose "model" is the user's deployment name, and for local).
    pub fn model_for(&self, provider: &str) -> String {
        self.provider_models
            .get(provider)
            .filter(|m| !m.is_empty())
            .cloned()
            .unwrap_or_else(|| default_model_for(provider))
    }
}

/// Default chat model per cloud provider. Azure has no default (deployment name);
/// local has no API model.
pub fn default_model_for(provider: &str) -> String {
    match provider {
        "openai" => "gpt-4o-mini",
        "anthropic" => "claude-3-5-sonnet-latest",
        _ => "", // azure (deployment name) / local
    }
    .to_string()
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

    #[test]
    fn defaults_include_ai_provider_fields() {
        let d = AppSettings::default();
        assert_eq!(d.ai_provider, "local");
        assert!(d.provider_models.is_empty());
    }

    #[test]
    fn legacy_blob_without_ai_provider_uses_defaults() {
        // A persisted blob from Sub-5 (no aiProvider/aiModel). Must deserialize + fill.
        // The old aiModel key is now unknown and serde ignores it; providerModels defaults empty.
        let json = r##"{
            "theme":"light","accent":"#10b981","language":"es","avatarStyle":"circle",
            "aiChatVisible":true,"captureMode":"system","defaultDevice":"system-loopback",
            "recordingQuality":"WAV 48k","runLocal":true,"autoDeleteAudio":false,
            "transcriptionProvider":"local","transcriptionModel":"large-v3",
            "autoTranscribe":true,"nativeLanguage":"es","defaultTemplate":"tecnica",
            "identifySpeakers":true,"diarizationModel":"default","autoGenerateSummary":true
        }"##;
        let parsed: AppSettings = serde_json::from_str(json).expect("legacy blob must deserialize");
        assert_eq!(parsed.ai_provider, "local");
        assert!(parsed.provider_models.is_empty());
    }

    #[test]
    fn model_for_uses_per_provider_then_defaults() {
        let mut s = AppSettings::default();
        assert_eq!(s.model_for("openai"), "gpt-4o-mini"); // default
        assert_eq!(s.model_for("anthropic"), "claude-3-5-sonnet-latest");
        assert_eq!(s.model_for("azure"), ""); // no default
        s.provider_models
            .insert("openai".into(), "gpt-5-mini".into());
        s.provider_models.insert("azure".into(), "my-deploy".into());
        assert_eq!(s.model_for("openai"), "gpt-5-mini"); // per-provider wins
        assert_eq!(s.model_for("azure"), "my-deploy");
        s.provider_models.insert("openai".into(), "".into()); // empty stored → default wins
        assert_eq!(s.model_for("openai"), "gpt-4o-mini");
    }

    #[test]
    fn default_azure_endpoint_is_empty() {
        let d = AppSettings::default();
        assert_eq!(d.azure_endpoint, "");
    }

    #[test]
    fn legacy_blob_without_azure_endpoint_uses_empty_default() {
        // A persisted blob from Sub-6a (no azureEndpoint). Must deserialize + fill "".
        // The old aiModel key is now unknown and serde ignores it; providerModels defaults empty.
        let json = r##"{
            "theme":"light","accent":"#10b981","language":"es","avatarStyle":"circle",
            "aiChatVisible":true,"captureMode":"system","defaultDevice":"system-loopback",
            "recordingQuality":"WAV 48k","runLocal":true,"autoDeleteAudio":false,
            "transcriptionProvider":"local","transcriptionModel":"large-v3",
            "autoTranscribe":true,"nativeLanguage":"es","defaultTemplate":"tecnica",
            "identifySpeakers":true,"diarizationModel":"default","autoGenerateSummary":true,
            "aiProvider":"azure","aiModel":"gpt-4o"
        }"##;
        let parsed: AppSettings =
            serde_json::from_str(json).expect("legacy blob without azureEndpoint must deserialize");
        assert_eq!(parsed.azure_endpoint, "");
        assert_eq!(parsed.ai_provider, "azure");
        assert!(parsed.provider_models.is_empty());
    }
}
