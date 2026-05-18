use crate::Bilingual;
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Type, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Template {
    pub id: String,
    pub color_class: String,
    pub icon: String,
    pub name: Bilingual,
    pub desc: Bilingual,
    pub sections: Vec<String>,
    pub is_default: bool,
}
