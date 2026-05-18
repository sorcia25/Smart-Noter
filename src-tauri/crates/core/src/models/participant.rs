use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Type, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Participant {
    pub id: String,
    pub meeting_id: String,
    pub label: String,
    pub name: Option<String>,
    pub color_class: String,
    pub word_count: i64,
    pub talk_pct: i64,
}
