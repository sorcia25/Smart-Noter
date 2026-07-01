use crate::models::ai::{ExtractedAction, Highlight, MarkedItem, MeetingAnalysis};
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
    decisions: Vec<RawItem>,
    #[serde(default)]
    blockers: Vec<RawItem>,
    #[serde(default)]
    actions: Vec<RawAction>,
    #[serde(default)]
    highlights: Vec<RawHighlight>,
}

#[derive(Deserialize)]
struct RawItem {
    text: String,
    #[serde(default)]
    t: Option<u32>,
}

#[derive(Deserialize)]
struct RawAction {
    text: String,
    #[serde(default)]
    owner: Option<String>,
    #[serde(default)]
    due: Option<String>,
    #[serde(default)]
    t: Option<u32>,
}

#[derive(Deserialize)]
struct RawHighlight {
    label: String,
    t: u32,
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
        .map(|(t, s, txt)| format!("[{:02}:{:02}] {s}: {txt}", t / 60, t % 60))
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
         Cada línea de la transcripción empieza con una marca [mm:ss] con el segundo de audio en que ocurre.\n\
         Devuelve SOLO un objeto JSON válido con las claves exactas: \"summary\" (string, en {lang}),\n\
         \"decisions\" (array de objetos {{\"text\":..,\"t\":<segundos>}}),\n\
         \"blockers\" (array de objetos {{\"text\":..,\"t\":<segundos>}}), \"actions\"\n\
         (array de objetos {{\"text\":..,\"owner\":..|null,\"due\":..|null,\"t\":<segundos>}}),\n\
         \"highlights\" (array de 3 a 5 objetos {{\"label\":..,\"t\":<segundos>}} con momentos clave\n\
         que NO estén ya cubiertos por una decisión/acción/bloqueo). En todos los casos \"t\" es el\n\
         segundo de audio donde ocurre, tomado de las marcas [mm:ss] de la transcripción. No añadas texto fuera del JSON."
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
        decisions: r
            .decisions
            .into_iter()
            .map(|i| MarkedItem {
                text: i.text,
                t_seconds: i.t,
            })
            .collect(),
        blockers: r
            .blockers
            .into_iter()
            .map(|i| MarkedItem {
                text: i.text,
                t_seconds: i.t,
            })
            .collect(),
        actions: r
            .actions
            .into_iter()
            .map(|a| ExtractedAction {
                text: a.text,
                owner_hint: a.owner,
                due: a.due,
                t_seconds: a.t,
            })
            .collect(),
        highlights: r
            .highlights
            .into_iter()
            .map(|h| Highlight {
                label: h.label,
                t_seconds: h.t,
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
    {"summary":"Resumen.","decisions":[{"text":"D1"}],"blockers":[],"actions":[{"text":"Hacer X","owner":"Ana","due":"2026-07-01"}]}
    Espero que ayude."#;
        let a = parse_analysis(raw, "es").unwrap();
        assert_eq!(a.summary.es, "Resumen.");
        assert_eq!(a.decisions[0].text, "D1");
        assert!(a.blockers.is_empty());
        assert_eq!(a.actions[0].text, "Hacer X");
        assert_eq!(a.actions[0].owner_hint.as_deref(), Some("Ana"));
    }

    #[test]
    fn parses_items_with_timestamps_and_highlights() {
        let raw = r#"{"summary":"S.","decisions":[{"text":"D1","t":84}],
      "blockers":[],"actions":[{"text":"A1","owner":"Ana","due":null,"t":185}],
      "highlights":[{"label":"Arranque","t":12}]}"#;
        let a = parse_analysis(raw, "es").unwrap();
        assert_eq!(a.decisions[0].text, "D1");
        assert_eq!(a.decisions[0].t_seconds, Some(84));
        assert_eq!(a.actions[0].t_seconds, Some(185));
        assert_eq!(a.highlights[0].label, "Arranque");
        assert_eq!(a.highlights[0].t_seconds, 12);
    }

    #[test]
    fn tolerates_missing_t_and_highlights() {
        let raw = r#"{"summary":"S.","decisions":[{"text":"D1"}],"actions":[{"text":"A1"}]}"#;
        let a = parse_analysis(raw, "es").unwrap();
        assert_eq!(a.decisions[0].t_seconds, None);
        assert_eq!(a.actions[0].t_seconds, None);
        assert!(a.highlights.is_empty());
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
