//! Shared HTTP plumbing for OpenAI-shaped cloud LLM providers.
//!
//! These helpers are provider-neutral: OpenAI (`openai.rs`), Azure (`azure.rs`),
//! and any other Chat-Completions-API-compatible adapter import them from here
//! rather than from a peer module. Anthropic (`anthropic.rs`) has a different
//! wire format and supplies its own extractors, but may still reuse the bits
//! that match (e.g. `status_to_err`).

use serde_json::Value;

/// Default embeddings model used for cloud vector generation.
pub const EMBED_MODEL: &str = "text-embedding-3-small";

/// Build the JSON body for a /chat/completions request.
/// `stream: true` omits `response_format` (streaming chat doesn't support it).
pub(crate) fn build_chat_body(model: &str, system: &str, user: &str, stream: bool) -> Value {
    let messages = serde_json::json!([
        {"role": "system", "content": system},
        {"role": "user",   "content": user}
    ]);
    if stream {
        serde_json::json!({
            "model":       model,
            "messages":    messages,
            "temperature": 0.3,
            "stream":      true
        })
    } else {
        serde_json::json!({
            "model":       model,
            "messages":    messages,
            "temperature": 0.3,
            "response_format": {"type": "json_object"}
        })
    }
}

/// Extract `choices[0].message.content` from a non-streamed completion response.
pub(crate) fn extract_message_content(val: &Value) -> Option<String> {
    val["choices"][0]["message"]["content"]
        .as_str()
        .map(str::to_owned)
}

/// Extract `choices[0].delta.content` from one SSE payload string.
pub(crate) fn extract_delta(payload: &str) -> Option<String> {
    let val: Value = serde_json::from_str(payload).ok()?;
    val["choices"][0]["delta"]["content"]
        .as_str()
        .map(str::to_owned)
}

/// Build the JSON body for an /embeddings request.
pub(crate) fn build_embed_body(model: &str, texts: &[String]) -> Value {
    serde_json::json!({
        "model": model,
        "input": texts
    })
}

/// Parse the `data[].embedding` arrays from an /embeddings response.
///
/// The API documents that `data[]` may be returned in any order; each entry
/// carries an `index` field giving its position in the input. We sort by that
/// `index` so the returned vectors line up with the input texts. If an entry
/// has no `index`, we fall back to its position in the array.
pub(crate) fn parse_embed_response(val: &Value) -> Result<Vec<Vec<f32>>, String> {
    let data = val["data"]
        .as_array()
        .ok_or_else(|| "respuesta de embeddings inválida: falta 'data'".to_string())?;

    // Collect (index, vector) so we can reorder by the API-provided index.
    let mut indexed: Vec<(usize, Vec<f32>)> = data
        .iter()
        .enumerate()
        .map(|(pos, entry)| {
            let idx = entry["index"].as_u64().map(|i| i as usize).unwrap_or(pos);
            let arr = entry["embedding"]
                .as_array()
                .ok_or_else(|| "embedding ausente en la respuesta".to_string())?;
            let vector = arr
                .iter()
                .map(|v| {
                    v.as_f64()
                        .map(|f| f as f32)
                        .ok_or_else(|| "valor de embedding no numérico".to_string())
                })
                .collect::<Result<Vec<f32>, String>>()?;
            Ok((idx, vector))
        })
        .collect::<Result<Vec<_>, String>>()?;

    indexed.sort_by_key(|(idx, _)| *idx);
    Ok(indexed.into_iter().map(|(_, v)| v).collect())
}

/// Map an HTTP status code to a user-friendly Spanish error string.
///
/// `provider` is the human-facing provider name (e.g. "OpenAI", "Azure") used
/// for the generic fallbacks so the same helper serves every adapter.
pub(crate) fn status_to_err(status: reqwest::StatusCode, provider: &str) -> String {
    match status.as_u16() {
        401 => "API key inválida".to_string(),
        429 => "límite de uso alcanzado".to_string(),
        503 => format!("{provider} no disponible, reintenta"),
        s => format!("{provider} respondió {s}"),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_delta_returns_none_for_empty_content() {
        let payload = r#"{"choices":[{"delta":{}}]}"#;
        assert_eq!(extract_delta(payload), None);
    }

    #[test]
    fn extract_delta_extracts_content() {
        let payload = r#"{"choices":[{"delta":{"content":"tok"}}]}"#;
        assert_eq!(extract_delta(payload), Some("tok".to_string()));
    }

    #[test]
    fn parse_embed_response_handles_valid_data() {
        let val = serde_json::json!({"data": [{"index": 0, "embedding": [1.0, 2.0]}]});
        let result = parse_embed_response(&val).unwrap();
        assert_eq!(result, vec![vec![1.0_f32, 2.0_f32]]);
    }

    #[test]
    fn parse_embed_response_reorders_by_index() {
        // Entries returned OUT of order: index 1 first, then index 0.
        // The result must come back in input order (index 0, then index 1).
        let val = serde_json::json!({
            "data": [
                {"index": 1, "embedding": [0.3, 0.4]},
                {"index": 0, "embedding": [0.1, 0.2]}
            ]
        });
        let result = parse_embed_response(&val).unwrap();
        assert_eq!(result, vec![vec![0.1_f32, 0.2_f32], vec![0.3_f32, 0.4_f32]]);
    }

    #[test]
    fn parse_embed_response_falls_back_to_position_without_index() {
        // No `index` fields → preserve array position.
        let val = serde_json::json!({
            "data": [
                {"embedding": [1.0, 1.0]},
                {"embedding": [2.0, 2.0]}
            ]
        });
        let result = parse_embed_response(&val).unwrap();
        assert_eq!(result, vec![vec![1.0_f32, 1.0_f32], vec![2.0_f32, 2.0_f32]]);
    }

    #[test]
    fn status_to_err_uses_provider_name_and_known_arms() {
        use reqwest::StatusCode;
        assert_eq!(
            status_to_err(StatusCode::UNAUTHORIZED, "OpenAI"),
            "API key inválida"
        );
        assert_eq!(
            status_to_err(StatusCode::TOO_MANY_REQUESTS, "Azure"),
            "límite de uso alcanzado"
        );
        assert_eq!(
            status_to_err(StatusCode::SERVICE_UNAVAILABLE, "OpenAI"),
            "OpenAI no disponible, reintenta"
        );
        assert_eq!(
            status_to_err(StatusCode::INTERNAL_SERVER_ERROR, "Azure"),
            "Azure respondió 500"
        );
    }
}
