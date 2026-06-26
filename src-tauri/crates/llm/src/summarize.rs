use crate::{engine::LocalLlm, AiError};
use serde::Deserialize;
use smart_noter_core::models::ai::{ExtractedAction, MeetingAnalysis};
use smart_noter_core::traits::{AnalysisInput, Summarizer};
use smart_noter_core::Bilingual;
use std::sync::atomic::AtomicBool;

// ---------------------------------------------------------------------------
// Internal serde types (mirrors the JSON schema the LLM must emit)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct RawAnalysis {
    summary: String,
    #[serde(default)]
    decisions: Vec<String>,
    #[serde(default)]
    blockers: Vec<String>,
    #[serde(default)]
    actions: Vec<RawAction>,
}

#[derive(Deserialize)]
struct RawAction {
    text: String,
    #[serde(default)]
    owner: Option<String>,
    #[serde(default)]
    due: Option<String>,
}

// ---------------------------------------------------------------------------
// Public functions
// ---------------------------------------------------------------------------

/// Find the first balanced `{...}` block and parse it; tolerant to prose around it.
///
/// This allows the LLM to emit surrounding text such as "Claro, aquí está:"
/// before the JSON object and still produce a valid `MeetingAnalysis`.
pub fn parse_analysis(raw: &str, lang: &str) -> Result<MeetingAnalysis, AiError> {
    let start = raw
        .find('{')
        .ok_or_else(|| AiError::Parse("no JSON".into()))?;
    let end = raw
        .rfind('}')
        .ok_or_else(|| AiError::Parse("no JSON".into()))?;
    if end <= start {
        return Err(AiError::Parse("no JSON".into()));
    }
    let r: RawAnalysis =
        serde_json::from_str(&raw[start..=end]).map_err(|e| AiError::Parse(e.to_string()))?;

    let summary = if lang == "en" {
        Bilingual {
            es: String::new(),
            en: Some(r.summary),
        }
    } else {
        Bilingual {
            es: r.summary,
            en: None,
        }
    };

    Ok(MeetingAnalysis {
        summary,
        decisions: r.decisions,
        blockers: r.blockers,
        actions: r
            .actions
            .into_iter()
            .map(|a| ExtractedAction {
                text: a.text,
                owner_hint: a.owner,
                due: a.due,
            })
            .collect(),
    })
}

/// Build a template-aware instruction that forces the LLM to emit a single JSON object.
pub fn build_prompt(input: &AnalysisInput) -> String {
    let body: String = input
        .transcript
        .iter()
        .map(|(s, t)| format!("{s}: {t}"))
        .collect::<Vec<_>>()
        .join("\n");
    let sections = input.template_sections.join(", ");
    let lang = &input.lang;

    format!(
        "Eres un asistente que resume reuniones. Plantilla con secciones: [{sections}].\n\
         Devuelve SOLO un objeto JSON válido con las claves exactas: \"summary\" (string, en {lang}),\n\
         \"decisions\" (array de strings), \"blockers\" (array de strings), \"actions\"\n\
         (array de objetos {{\"text\":..,\"owner\":..|null,\"due\":..|null}}). No añadas texto fuera del JSON.\n\
         \n\
         Transcripción:\n\
         {body}"
    )
}

// ---------------------------------------------------------------------------
// Summarizer implementation
// ---------------------------------------------------------------------------

pub struct LocalSummarizer<'a> {
    pub llm: &'a LocalLlm,
}

impl Summarizer for LocalSummarizer<'_> {
    fn analyze(
        &self,
        input: &AnalysisInput,
        progress: &mut dyn FnMut(u32),
        abort: &AtomicBool,
    ) -> Result<MeetingAnalysis, String> {
        progress(5);
        let prompt = build_prompt(input);
        let mut sink = |_: &str| {};
        let raw = self
            .llm
            .generate(&prompt, 1024, &mut sink, abort)
            .map_err(|e| e.to_string())?;
        progress(80);

        let analysis = parse_analysis(&raw, &input.lang).or_else(|_| {
            // Retry with a stricter instruction when the first parse fails.
            let strict =
                format!("{prompt}\n\nIMPORTANTE: responde ÚNICAMENTE el JSON, empezando por {{.");
            let raw2 = self
                .llm
                .generate(&strict, 1024, &mut |_| {}, abort)
                .map_err(|e| e.to_string())?;
            parse_analysis(&raw2, &input.lang).map_err(|e| e.to_string())
        })?;

        progress(100);
        Ok(analysis)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_json_block_amid_prose() {
        let raw = r#"Claro, aquí está:
    {"summary":"Resumen.","decisions":["D1"],"blockers":[],"actions":[{"text":"Hacer X","owner":"Ana","due":"2026-07-01"}]}
    Espero que ayude."#;
        let a = parse_analysis(raw, "es").unwrap();
        assert_eq!(a.summary.es, "Resumen.");
        assert_eq!(a.decisions, vec!["D1"]);
        assert!(a.blockers.is_empty());
        assert_eq!(a.actions[0].text, "Hacer X");
        assert_eq!(a.actions[0].owner_hint.as_deref(), Some("Ana"));
    }

    #[test]
    fn errors_on_no_json() {
        assert!(parse_analysis("no json here", "es").is_err());
    }

    #[test]
    fn parses_minimal_json_with_only_summary() {
        // Exercises #[serde(default)] on decisions/blockers/actions.
        let raw = r#"{"summary":"Solo resumen."}"#;
        let a = parse_analysis(raw, "es").unwrap();
        assert_eq!(a.summary.es, "Solo resumen.");
        assert!(a.decisions.is_empty());
        assert!(a.blockers.is_empty());
        assert!(a.actions.is_empty());
    }

    #[test]
    fn parses_english_lang_into_en_field() {
        let raw = r#"{"summary":"English summary.","decisions":[],"blockers":[],"actions":[]}"#;
        let a = parse_analysis(raw, "en").unwrap();
        assert_eq!(a.summary.en.as_deref(), Some("English summary."));
        assert_eq!(a.summary.es, "");
    }

    #[test]
    fn errors_on_malformed_json() {
        let raw = r#"{"summary": "oops" BROKEN}"#;
        assert!(parse_analysis(raw, "es").is_err());
    }
}
