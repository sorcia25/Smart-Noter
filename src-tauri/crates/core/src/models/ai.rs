use crate::Bilingual;
use serde::{Deserialize, Serialize};
use specta::Type;

/// An action item extracted from the meeting transcript (IPC type).
#[derive(Debug, Clone, Type, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractedAction {
    pub text: String,
    pub owner_hint: Option<String>,
    pub due: Option<String>,
}

/// The structured result of analyzing one meeting transcript (internal Rust type).
#[derive(Debug, Clone)]
pub struct MeetingAnalysis {
    pub summary: Bilingual,
    pub decisions: Vec<String>,
    pub blockers: Vec<String>,
    pub actions: Vec<ExtractedAction>,
}

impl Default for MeetingAnalysis {
    fn default() -> Self {
        Self {
            summary: Bilingual::new(""),
            decisions: Vec::new(),
            blockers: Vec::new(),
            actions: Vec::new(),
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
