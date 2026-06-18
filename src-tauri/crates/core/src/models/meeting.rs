use super::{Action, Participant};
use crate::Bilingual;
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Type, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MeetingSummary {
    pub id: String,
    pub title: Bilingual,
    pub template: String,
    pub date: String,
    pub duration_sec: i64,
    pub participants: Vec<Participant>,
    pub word_count: i64,
}

#[derive(Debug, Clone, Type, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptLine {
    pub id: i64,
    pub t: String,
    pub speaker_id: String,
    pub text: Bilingual,
}

#[derive(Debug, Clone, Type, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MeetingDetail {
    pub id: String,
    pub title: Bilingual,
    pub template: String,
    pub date: String,
    pub duration_sec: i64,
    pub device_used: Option<String>,
    pub word_count: i64,
    pub summary: Option<Bilingual>,
    pub participants: Vec<Participant>,
    pub actions: Vec<Action>,
    pub decisions: Vec<Bilingual>,
    pub blockers: Vec<Bilingual>,
    pub transcript: Vec<TranscriptLine>,
}
