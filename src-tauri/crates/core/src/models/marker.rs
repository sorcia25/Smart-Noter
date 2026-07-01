use serde::{Deserialize, Serialize};
use specta::Type;

/// An audio marker: a timestamped, typed point in a meeting's recording.
/// `kind` = decision|action|blocker|highlight|manual; `source` = ai|manual.
#[derive(Debug, Clone, Type, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Marker {
    pub id: String,
    pub meeting_id: String,
    pub t_seconds: i64,
    pub kind: String,
    pub label: String,
    pub source: String,
    pub created_at: String,
}
