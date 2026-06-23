use crate::Bilingual;
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Type, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Decision {
    pub id: i64,
    pub text: Bilingual,
}

#[derive(Debug, Clone, Type, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Blocker {
    pub id: i64,
    pub text: Bilingual,
}
