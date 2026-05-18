use serde::{Deserialize, Serialize};
use specta::Type;

/// A bilingual ES/EN string. ES is always present; EN is optional and falls back to ES.
#[derive(Debug, Clone, Type, Serialize, Deserialize)]
pub struct Bilingual {
    pub es: String,
    pub en: Option<String>,
}

impl Bilingual {
    pub fn new(es: impl Into<String>) -> Self {
        Self {
            es: es.into(),
            en: None,
        }
    }

    pub fn with_en(es: impl Into<String>, en: impl Into<String>) -> Self {
        Self {
            es: es.into(),
            en: Some(en.into()),
        }
    }

    pub fn pick(&self, lang: &str) -> &str {
        match lang {
            "en" => self.en.as_deref().unwrap_or(&self.es),
            _ => &self.es,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pick_returns_en_when_lang_en_and_en_present() {
        let b = Bilingual::with_en("Hola", "Hello");
        assert_eq!(b.pick("en"), "Hello");
    }

    #[test]
    fn pick_falls_back_to_es_when_en_missing() {
        let b = Bilingual::new("Hola");
        assert_eq!(b.pick("en"), "Hola");
    }

    #[test]
    fn pick_returns_es_for_lang_es() {
        let b = Bilingual::with_en("Hola", "Hello");
        assert_eq!(b.pick("es"), "Hola");
    }
}
