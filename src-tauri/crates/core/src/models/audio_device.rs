use crate::Bilingual;
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Type, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioDevice {
    pub id: String,
    pub name: Bilingual,
    pub desc: Bilingual,
    pub icon: String,
    pub recommended: bool,
    pub active: bool,
}
