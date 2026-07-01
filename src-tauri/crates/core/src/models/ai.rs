use crate::Bilingual;
use serde::{Deserialize, Serialize};
use specta::Type;

/// One provider's config as the UI sees it. NEVER contains the full key.
#[derive(Debug, Clone, Type, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConfig {
    pub domain: String,   // "ai" | "transcription"
    pub provider: String, // "local" | "openai" | "anthropic" | "azure"
    pub configured: bool, // a key is stored for this provider
    pub key_last4: Option<String>,
    pub model: String, // selected model id for this domain
}

/// An action item extracted from the meeting transcript (IPC type).
#[derive(Debug, Clone, Type, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractedAction {
    pub text: String,
    pub owner_hint: Option<String>,
    pub due: Option<String>,
    /// Audio anchor in seconds, taken from the transcript's `[mm:ss]` markers.
    /// `None` if the LLM didn't provide one.
    pub t_seconds: Option<u32>,
}

/// A summary item (decision/blocker) with an optional audio anchor.
#[derive(Debug, Clone)]
pub struct MarkedItem {
    pub text: String,
    pub t_seconds: Option<u32>,
}

/// A key moment the LLM flagged that isn't already a decision/action/blocker.
#[derive(Debug, Clone)]
pub struct Highlight {
    pub label: String,
    pub t_seconds: u32,
}

/// The structured result of analyzing one meeting transcript (internal Rust type).
#[derive(Debug, Clone)]
pub struct MeetingAnalysis {
    pub summary: Bilingual,
    pub decisions: Vec<MarkedItem>,
    pub blockers: Vec<MarkedItem>,
    pub actions: Vec<ExtractedAction>,
    pub highlights: Vec<Highlight>,
}

impl Default for MeetingAnalysis {
    fn default() -> Self {
        Self {
            summary: Bilingual::new(""),
            decisions: Vec::new(),
            blockers: Vec::new(),
            actions: Vec::new(),
            highlights: Vec::new(),
        }
    }
}

/// One retrieval chunk of a transcript + its embedding (internal Rust type).
#[derive(Debug, Clone)]
pub struct Chunk {
    pub idx: i64,
    pub text: String,
    pub vector: Vec<f32>,
}

/// A chat message in the AI Q&A history (IPC type).
#[derive(Debug, Clone, Type, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    pub id: i64,
    pub role: String,
    pub content: String,
    pub created_at: String,
}
