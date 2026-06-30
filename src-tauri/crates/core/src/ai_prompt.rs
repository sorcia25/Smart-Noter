use crate::models::ai::{ExtractedAction, MeetingAnalysis};
use crate::traits::AnalysisInput;
use crate::Bilingual;
use serde::Deserialize;

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

/// The logical (system, user) content for a summary request, provider-agnostic.
/// Local wraps this in ChatML; cloud adapters send it as messages[].
pub fn build_messages(input: &AnalysisInput, strict: bool) -> (String, String) {
    let body: String = input
        .transcript
        .iter()
        .map(|(s, t)| format!("{s}: {t}"))
        .collect::<Vec<_>>()
        .join("\n");
    let sections = input.template_sections.join(", ");
    let lang = &input.lang;

    let strict_prefix = if strict {
        "IMPORTANTE: responde ÚNICAMENTE el JSON empezando por {. Sin texto adicional.\n"
    } else {
        ""
    };

    let system = format!(
        "{strict_prefix}\
         Eres un asistente que resume reuniones. Plantilla con secciones: [{sections}].\n\
         Devuelve SOLO un objeto JSON válido con las claves exactas: \"summary\" (string, en {lang}),\n\
         \"decisions\" (array de strings), \"blockers\" (array de strings), \"actions\"\n\
         (array de objetos {{\"text\":..,\"owner\":..|null,\"due\":..|null}}). No añadas texto fuera del JSON."
    );
    let user = format!("Transcripción:\n{body}");
    (system, user)
}

/// Find the first balanced `{...}` block and parse it; tolerant to prose around it.
///
/// This allows the LLM to emit surrounding text such as "Claro, aquí está:"
/// before the JSON object and still produce a valid `MeetingAnalysis`.
pub fn parse_analysis(raw: &str, lang: &str) -> Result<MeetingAnalysis, String> {
    let start = raw.find('{').ok_or_else(|| "no JSON".to_string())?;
    let end = raw.rfind('}').ok_or_else(|| "no JSON".to_string())?;
    if end <= start {
        return Err("no JSON".to_string());
    }
    let r: RawAnalysis = serde_json::from_str(&raw[start..=end]).map_err(|e| e.to_string())?;

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
