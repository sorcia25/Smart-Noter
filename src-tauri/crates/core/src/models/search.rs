use super::MeetingSummary;
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Type, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub meeting: MeetingSummary,
    /// Snippet of the best-matching column, with matches wrapped between the
    /// markers \u{2068} (start) and \u{2069} (end) for the frontend to highlight.
    pub snippet: String,
}
