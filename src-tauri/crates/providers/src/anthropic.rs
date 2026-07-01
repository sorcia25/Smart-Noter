use std::ops::ControlFlow;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde_json::Value;
use smart_noter_core::models::ai::{Chunk, MeetingAnalysis};
use smart_noter_core::traits::{AnalysisInput, ChatEngine, Summarizer};

use crate::http_common::{build_chat_system_prompt, status_to_err};
use crate::sse::read_sse;

/// Sentinel returned by `embed()` so the factory (Task B5) knows to fall back
/// to local embeddings. Exported from this crate for the factory to import.
pub const ANTHROPIC_NO_EMBEDDINGS: &str = "anthropic-no-embeddings";

/// Human-facing provider name used in error messages.
const PROVIDER: &str = "Anthropic";

/// Required header value mandated by the Anthropic Messages API.
const ANTHROPIC_VERSION: &str = "2023-06-01";

/// Upper bound on tokens the model may generate per /messages request.
/// Required by the Anthropic API (it has no implicit default).
const MAX_TOKENS: u32 = 1024;

// ---------------------------------------------------------------------------
// Public struct
// ---------------------------------------------------------------------------

pub struct AnthropicProvider {
    pub api_key: String,
    pub model: String,
    /// Base URL, e.g. "https://api.anthropic.com/v1". Injectable for tests.
    pub base: String,
}

impl AnthropicProvider {
    /// Production constructor — defaults base to the real Anthropic API.
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            api_key,
            model,
            base: "https://api.anthropic.com/v1".to_string(),
        }
    }

    fn client() -> Result<reqwest::blocking::Client, String> {
        reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|e| format!("error construyendo cliente HTTP: {e}"))
    }

    /// Build the JSON body for a POST /messages request.
    ///
    /// Note: Anthropic requires `system` as a TOP-LEVEL field, not inside `messages`.
    /// When `stream` is true, the `"stream": true` flag is added to the same base
    /// object (one source of truth — the two modes can't silently diverge).
    fn build_messages_body(model: &str, system: &str, user: &str, stream: bool) -> Value {
        let mut body = serde_json::json!({
            "model":      model,
            "max_tokens": MAX_TOKENS,
            "system":     system,
            "messages": [{"role": "user", "content": user}]
        });
        if stream {
            body["stream"] = serde_json::json!(true);
        }
        body
    }
}

// ---------------------------------------------------------------------------
// Local extractors (Anthropic wire format differs from OpenAI)
// ---------------------------------------------------------------------------

/// Extract text from a non-streamed Anthropic Messages response.
///
/// Response shape: `{"content": [{"type": "text", "text": "..."}]}`
/// Returns `content[0].text` or None if absent.
fn extract_text_content(val: &Value) -> Option<String> {
    val["content"][0]["text"].as_str().map(str::to_owned)
}

/// Extract a text token from one SSE `data:` payload in the Anthropic stream.
///
/// Anthropic SSE events:
/// - `content_block_delta` with `delta.type == "text_delta"` → carry token text
/// - `content_block_delta` with other delta types (e.g. `input_json_delta` for
///   tool-use) → no token, return None
/// - `message_start`, `content_block_start`, `message_delta`, `message_stop`,
///   `ping`, etc. → no token, return None
///
/// Payload example:
/// ```json
/// {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hola"}}
/// ```
fn extract_anthropic_delta(payload: &str) -> Option<String> {
    let val: Value = serde_json::from_str(payload).ok()?;
    if val["type"].as_str()? != "content_block_delta" {
        return None;
    }
    // Only text_delta carries answer tokens; ignore input_json_delta (tool-use) etc.
    if val["delta"]["type"].as_str()? != "text_delta" {
        return None;
    }
    val["delta"]["text"].as_str().map(str::to_owned)
}

// ---------------------------------------------------------------------------
// Summarizer impl
// ---------------------------------------------------------------------------

impl Summarizer for AnthropicProvider {
    fn analyze(
        &self,
        input: &AnalysisInput,
        progress: &mut dyn FnMut(u32),
        abort: &AtomicBool,
    ) -> Result<MeetingAnalysis, String> {
        if abort.load(Ordering::Relaxed) {
            return Err("cancelado".to_string());
        }

        progress(10);

        let client = Self::client()?;

        // --- attempt 1 ---
        let (system, user) = smart_noter_core::ai_prompt::build_messages(input, false);
        let body = Self::build_messages_body(&self.model, &system, &user, false);

        let resp = client
            .post(format!("{}/messages", self.base))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .json(&body)
            .send()
            .map_err(|e| format!("sin conexión con Anthropic: {e}"))?;

        if !resp.status().is_success() {
            return Err(status_to_err(resp.status(), PROVIDER));
        }

        let val: Value = resp
            .json()
            .map_err(|e| format!("error parseando respuesta de Anthropic: {e}"))?;

        let content = extract_text_content(&val)
            .ok_or_else(|| "respuesta de Anthropic sin contenido".to_string())?;

        match smart_noter_core::ai_prompt::parse_analysis(&content, &input.lang) {
            Ok(analysis) => {
                progress(100);
                Ok(analysis)
            }
            Err(_parse_err) => {
                // Honor a cancel requested during the gap between the two calls.
                if abort.load(Ordering::Relaxed) {
                    return Err("cancelado".to_string());
                }

                // --- attempt 2 (strict prompt — Anthropic has no response_format:json_object) ---
                let (system2, user2) = smart_noter_core::ai_prompt::build_messages(input, true);
                let body2 = Self::build_messages_body(&self.model, &system2, &user2, false);

                let resp2 = client
                    .post(format!("{}/messages", self.base))
                    .header("x-api-key", &self.api_key)
                    .header("anthropic-version", ANTHROPIC_VERSION)
                    .json(&body2)
                    .send()
                    .map_err(|e| format!("sin conexión con Anthropic: {e}"))?;

                if !resp2.status().is_success() {
                    return Err(status_to_err(resp2.status(), PROVIDER));
                }

                let val2: Value = resp2
                    .json()
                    .map_err(|e| format!("error parseando respuesta de Anthropic: {e}"))?;

                let content2 = extract_text_content(&val2)
                    .ok_or_else(|| "respuesta de Anthropic sin contenido".to_string())?;

                let analysis = smart_noter_core::ai_prompt::parse_analysis(&content2, &input.lang)?;
                progress(100);
                Ok(analysis)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// ChatEngine impl
// ---------------------------------------------------------------------------

impl ChatEngine for AnthropicProvider {
    /// Anthropic has no embeddings API. Returns an error; note the provider factory
    /// routes Anthropic to the local embedder by provider name (it does not inspect
    /// this returned value), so this Err is only hit if `embed` is ever called directly.
    fn embed(&self, _texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
        Err(ANTHROPIC_NO_EMBEDDINGS.to_string())
    }

    fn answer(
        &self,
        question: &str,
        context: &[Chunk],
        lang: &str,
        on_token: &mut dyn FnMut(&str),
        abort: &AtomicBool,
    ) -> Result<(), String> {
        if abort.load(Ordering::Relaxed) {
            return Ok(());
        }

        let system = build_chat_system_prompt(context, lang);
        let body = Self::build_messages_body(&self.model, &system, question, true);

        let client = Self::client()?;
        let resp = client
            .post(format!("{}/messages", self.base))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .json(&body)
            .send()
            .map_err(|e| format!("sin conexión con Anthropic: {e}"))?;

        if !resp.status().is_success() {
            return Err(status_to_err(resp.status(), PROVIDER));
        }

        // Stream SSE tokens. Anthropic does NOT send [DONE]; stream ends when
        // connection closes. Abort mid-stream is a clean stop (not an error).
        read_sse(resp, |payload| {
            if abort.load(Ordering::Relaxed) {
                return ControlFlow::Break(());
            }
            if let Some(tok) = extract_anthropic_delta(payload) {
                on_token(&tok);
            }
            ControlFlow::Continue(())
        })
        .map_err(|e| format!("error leyendo stream de Anthropic: {e}"))?;

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests (tiny_http mock — no real network)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};

    use super::*;

    fn provider_for(port: u16) -> AnthropicProvider {
        AnthropicProvider {
            api_key: "test-key".to_string(),
            model: "claude-3-5-sonnet-20241022".to_string(),
            base: format!("http://127.0.0.1:{port}/v1"),
        }
    }

    fn dummy_input() -> AnalysisInput {
        AnalysisInput {
            transcript: vec![
                (
                    0,
                    "Alice".to_string(),
                    "We decided to launch next Monday.".to_string(),
                ),
                (30, "Bob".to_string(), "Blocker: CI is failing.".to_string()),
            ],
            template_sections: vec!["Resumen".to_string(), "Decisiones".to_string()],
            lang: "es".to_string(),
        }
    }

    // ------------------------------------------------------------------
    // extract_anthropic_delta — unit tests (no network)
    // ------------------------------------------------------------------
    #[test]
    fn delta_extracts_text_from_content_block_delta() {
        let payload = r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hola"}}"#;
        assert_eq!(extract_anthropic_delta(payload), Some("Hola".to_string()));
    }

    #[test]
    fn delta_returns_none_for_non_delta_events() {
        let message_stop = r#"{"type":"message_stop"}"#;
        assert_eq!(extract_anthropic_delta(message_stop), None);

        let message_start = r#"{"type":"message_start","message":{"id":"msg_01"}}"#;
        assert_eq!(extract_anthropic_delta(message_start), None);

        let ping = r#"{"type":"ping"}"#;
        assert_eq!(extract_anthropic_delta(ping), None);
    }

    #[test]
    fn delta_ignores_non_text_delta_types() {
        // tool-use streams carry input_json_delta inside content_block_delta;
        // these are NOT answer tokens and must be ignored.
        let input_json_delta = r#"{"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"{\"k\""}}"#;
        assert_eq!(extract_anthropic_delta(input_json_delta), None);
    }

    // ------------------------------------------------------------------
    // analyze — happy path
    // ------------------------------------------------------------------
    #[test]
    fn analyze_parses_summary_and_decisions() {
        let content_json =
            r#"{"summary":"Resumen.","decisions":[{"text":"D1"}],"blockers":[],"actions":[]}"#;
        let body = serde_json::json!({
            "content": [{"type": "text", "text": content_json}]
        })
        .to_string();

        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let port = server.server_addr().to_ip().unwrap().port();
        let prov = provider_for(port);

        let body_clone = body.clone();
        std::thread::spawn(move || {
            let req = server.recv().unwrap();
            let resp = tiny_http::Response::from_string(body_clone).with_header(
                tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..])
                    .unwrap(),
            );
            req.respond(resp).ok();
        });

        let abort = AtomicBool::new(false);
        let analysis = prov
            .analyze(&dummy_input(), &mut |_| {}, &abort)
            .expect("analyze should succeed");

        assert_eq!(analysis.summary.es, "Resumen.");
        assert_eq!(analysis.decisions[0].text, "D1");
    }

    // ------------------------------------------------------------------
    // analyze — retry path: first response is bad JSON, second is valid
    // ------------------------------------------------------------------
    #[test]
    fn analyze_retries_on_bad_json() {
        let bad_body = serde_json::json!({
            "content": [{"type": "text", "text": "no json here"}]
        })
        .to_string();
        let good_content = r#"{"summary":"Retry ok.","decisions":[],"blockers":[],"actions":[]}"#;
        let good_body = serde_json::json!({
            "content": [{"type": "text", "text": good_content}]
        })
        .to_string();

        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let port = server.server_addr().to_ip().unwrap().port();
        let prov = provider_for(port);

        let (b1, b2) = (bad_body.clone(), good_body.clone());
        std::thread::spawn(move || {
            for body in [b1, b2] {
                if let Ok(req) = server.recv() {
                    let resp = tiny_http::Response::from_string(body).with_header(
                        tiny_http::Header::from_bytes(
                            &b"Content-Type"[..],
                            &b"application/json"[..],
                        )
                        .unwrap(),
                    );
                    req.respond(resp).ok();
                }
            }
        });

        let abort = AtomicBool::new(false);
        let analysis = prov
            .analyze(&dummy_input(), &mut |_| {}, &abort)
            .expect("should succeed after retry");
        assert_eq!(analysis.summary.es, "Retry ok.");
    }

    // ------------------------------------------------------------------
    // answer — SSE streaming, non-text events are ignored
    // ------------------------------------------------------------------
    #[test]
    fn answer_collects_streamed_tokens_and_ignores_non_text_events() {
        // Note: Anthropic does NOT send [DONE]; stream ends when connection closes.
        let sse_body = concat!(
            "data: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_01\"}}\n",
            "data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n",
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hola\"}}\n",
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\" mundo\"}}\n",
            "data: {\"type\":\"message_stop\"}\n",
        );

        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let port = server.server_addr().to_ip().unwrap().port();
        let prov = provider_for(port);

        let sse = sse_body.to_string();
        std::thread::spawn(move || {
            let req = server.recv().unwrap();
            let resp = tiny_http::Response::from_string(sse).with_header(
                tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"text/event-stream"[..])
                    .unwrap(),
            );
            req.respond(resp).ok();
        });

        let chunks = vec![Chunk {
            idx: 0,
            text: "contexto de prueba".to_string(),
            vector: vec![],
        }];

        let abort = AtomicBool::new(false);
        let mut tokens = String::new();
        prov.answer(
            "¿Qué decidimos?",
            &chunks,
            "es",
            &mut |tok| tokens.push_str(tok),
            &abort,
        )
        .expect("answer should succeed");

        assert_eq!(tokens, "Hola mundo");
    }

    // ------------------------------------------------------------------
    // answer — abort before stream starts
    // ------------------------------------------------------------------
    #[test]
    fn answer_aborts_before_network_when_flag_set() {
        // No mock server needed: abort is set before any HTTP call.
        let prov = AnthropicProvider {
            api_key: "key".to_string(),
            model: "claude-3-5-sonnet-20241022".to_string(),
            base: "http://127.0.0.1:19998/v1".to_string(), // nothing listening
        };
        let abort = AtomicBool::new(true); // pre-set
        let mut tokens = String::new();
        let result = prov.answer("q", &[], "es", &mut |t| tokens.push_str(t), &abort);
        // Should return Ok(()) cleanly, no tokens, no network error.
        assert!(result.is_ok());
        assert!(tokens.is_empty());
    }

    // ------------------------------------------------------------------
    // answer — abort honored mid-stream
    // ------------------------------------------------------------------
    #[test]
    fn answer_stops_when_aborted_mid_stream() {
        // Mock serves 3 tokens. The on_token callback flips the abort flag after
        // the FIRST token, so read_sse's callback returns Break before "dos"/"tres".
        let sse_body = concat!(
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"uno\"}}\n",
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"dos\"}}\n",
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"tres\"}}\n",
        );

        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let port = server.server_addr().to_ip().unwrap().port();
        let prov = provider_for(port);

        let sse = sse_body.to_string();
        std::thread::spawn(move || {
            let req = server.recv().unwrap();
            let resp = tiny_http::Response::from_string(sse).with_header(
                tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"text/event-stream"[..])
                    .unwrap(),
            );
            req.respond(resp).ok();
        });

        let abort = AtomicBool::new(false);
        let mut tokens = String::new();
        let result = prov.answer(
            "pregunta",
            &[],
            "es",
            &mut |tok| {
                tokens.push_str(tok);
                // Request cancel right after the first token arrives.
                abort.store(true, Ordering::Relaxed);
            },
            &abort,
        );

        assert!(result.is_ok());
        // Only the first token is emitted; the abort breaks before "dos"/"tres".
        assert_eq!(tokens, "uno");
    }

    // ------------------------------------------------------------------
    // embed — always returns ANTHROPIC_NO_EMBEDDINGS sentinel
    // ------------------------------------------------------------------
    #[test]
    fn embed_returns_no_embeddings_sentinel() {
        let prov = AnthropicProvider {
            api_key: "key".to_string(),
            model: "claude-3-5-sonnet-20241022".to_string(),
            base: "http://127.0.0.1:19998/v1".to_string(), // nothing listening
        };
        let result = prov.embed(&["x".to_string()]);
        assert_eq!(result, Err(ANTHROPIC_NO_EMBEDDINGS.to_string()));
    }

    #[test]
    fn embed_sentinel_value_is_stable() {
        // B5 imports this exact string — verify it matches.
        assert_eq!(ANTHROPIC_NO_EMBEDDINGS, "anthropic-no-embeddings");
    }
}
