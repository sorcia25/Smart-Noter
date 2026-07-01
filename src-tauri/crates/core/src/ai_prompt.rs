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
        // Raw whole-second markers ("[35]"), NOT mm:ss — so the model copies a plain
        // integer into `t` and never emits leading zeros (which are invalid JSON).
        .map(|(t, s, txt)| format!("[{t}] {s}: {txt}"))
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
         Cada línea de la transcripción empieza con una marca [N] donde N es el segundo de audio (entero) en que ocurre.\n\
         Devuelve SOLO un objeto JSON válido con las claves exactas: \"summary\" (string, en {lang}),\n\
         \"decisions\" (array de objetos {{\"text\":..,\"t\":<segundos>}}),\n\
         \"blockers\" (array de objetos {{\"text\":..,\"t\":<segundos>}}), \"actions\"\n\
         (array de objetos {{\"text\":..,\"owner\":..|null,\"due\":..|null,\"t\":<segundos>}}),\n\
         \"highlights\" (array de 3 a 5 objetos {{\"label\":..,\"t\":<segundos>}} con momentos clave\n\
         que NO estén ya cubiertos por una decisión/acción/bloqueo). En todos los casos \"t\" es un\n\
         número ENTERO de segundos (sin comillas y SIN ceros a la izquierda, por ejemplo 35 y no 0035),\n\
         copiado de la marca [N] de la línea correspondiente. No añadas texto fuera del JSON."
    );
    let user = format!("Transcripción:\n{body}");
    (system, user)
}

/// Strip leading zeros from JSON number literals (outside strings) so a model that
/// copied a zero-padded timestamp (`"t": 0035`) still parses — `0035` is invalid JSON.
fn strip_leading_zeros(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    let mut in_str = false;
    let mut escaped = false;
    let mut prev = ' ';
    while i < chars.len() {
        let c = chars[i];
        if in_str {
            out.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_str = false;
            }
            prev = c;
            i += 1;
            continue;
        }
        if c == '"' {
            in_str = true;
            out.push(c);
            prev = c;
            i += 1;
            continue;
        }
        // A '0' that starts a number token (prev not a digit/'.') and is followed by
        // another digit is a leading zero — skip it.
        if c == '0'
            && i + 1 < chars.len()
            && chars[i + 1].is_ascii_digit()
            && !prev.is_ascii_digit()
            && prev != '.'
        {
            while i + 1 < chars.len() && chars[i] == '0' && chars[i + 1].is_ascii_digit() {
                i += 1;
            }
            continue;
        }
        out.push(c);
        prev = c;
        i += 1;
    }
    out
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
    // A small model may copy a zero-padded timestamp into a number (e.g. `"t": 0035`),
    // which is invalid JSON. Strip leading zeros from numeric literals first.
    let cleaned = strip_leading_zeros(&raw[start..=end]);
    let r: RawAnalysis = serde_json::from_str(&cleaned).map_err(|e| e.to_string())?;

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
    fn tolerates_zero_padded_timestamps() {
        // A small local model copied a "[00:35]" mark into "t": 0035 (invalid JSON).
        // Also: a padded value INSIDE a string label must NOT be altered.
        let raw = r#"{"summary":"S.","decisions":[{"text":"D1","t":0035}],
          "blockers":[],"actions":[],"highlights":[{"label":"a las 00:07","t":0007}]}"#;
        let a = parse_analysis(raw, "es").unwrap();
        assert_eq!(a.decisions[0].t_seconds, Some(35));
        assert_eq!(a.highlights[0].t_seconds, 7);
        assert_eq!(a.highlights[0].label, "a las 00:07"); // string untouched
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
