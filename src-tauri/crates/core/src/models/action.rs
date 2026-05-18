use crate::Bilingual;
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Type, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Action {
    pub id: String,
    pub meeting_id: String,
    pub text: Bilingual,
    pub owner_participant_id: Option<String>,
    pub due: Option<String>, // ISO8601 date
    pub done: bool,
}
