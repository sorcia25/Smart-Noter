use std::ops::ControlFlow;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde_json::Value;
use smart_noter_core::models::ai::{Chunk, MeetingAnalysis};
use smart_noter_core::traits::{AnalysisInput, ChatEngine, Summarizer};

use crate::http_common::{
    build_chat_body, build_chat_system_prompt, build_embed_body, extract_delta,
    extract_message_content, parse_embed_response, status_to_err, EMBED_MODEL,
};
use crate::sse::read_sse;

/// Human-facing provider name used in error messages.
const PROVIDER: &str = "OpenAI";

// ---------------------------------------------------------------------------
// Public struct
// ---------------------------------------------------------------------------

pub struct OpenAiProvider {
    pub api_key: String,
    pub model: String,
    /// Base URL, e.g. "https://api.openai.com/v1". Injectable for tests.
    pub base: String,
}

impl OpenAiProvider {
    /// Production constructor — defaults base to the real OpenAI API.
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            api_key,
            model,
            base: "https://api.openai.com/v1".to_string(),
        }
    }

    fn client() -> Result<reqwest::blocking::Client, String> {
        reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|e| format!("error construyendo cliente HTTP: {e}"))
    }
}

// ---------------------------------------------------------------------------
// Summarizer impl
// ---------------------------------------------------------------------------

impl Summarizer for OpenAiProvider {
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
        let body = build_chat_body(&self.model, &system, &user, false);

        let resp = client
            .post(format!("{}/chat/completions", self.base))
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .map_err(|e| format!("sin conexión con OpenAI: {e}"))?;

        if !resp.status().is_success() {
            return Err(status_to_err(resp.status(), PROVIDER));
        }

        let val: Value = resp
            .json()
            .map_err(|e| format!("error parseando respuesta de OpenAI: {e}"))?;

        let content = extract_message_content(&val)
            .ok_or_else(|| "respuesta de OpenAI sin contenido".to_string())?;

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

                // --- attempt 2 (strict prompt) ---
                let (system2, user2) = smart_noter_core::ai_prompt::build_messages(input, true);
                let body2 = build_chat_body(&self.model, &system2, &user2, false);

                let resp2 = client
                    .post(format!("{}/chat/completions", self.base))
                    .bearer_auth(&self.api_key)
                    .json(&body2)
                    .send()
                    .map_err(|e| format!("sin conexión con OpenAI: {e}"))?;

                if !resp2.status().is_success() {
                    return Err(status_to_err(resp2.status(), PROVIDER));
                }

                let val2: Value = resp2
                    .json()
                    .map_err(|e| format!("error parseando respuesta de OpenAI: {e}"))?;

                let content2 = extract_message_content(&val2)
                    .ok_or_else(|| "respuesta de OpenAI sin contenido".to_string())?;

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

impl ChatEngine for OpenAiProvider {
    fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let client = Self::client()?;
        let body = build_embed_body(EMBED_MODEL, texts);

        let resp = client
            .post(format!("{}/embeddings", self.base))
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .map_err(|e| format!("sin conexión con OpenAI: {e}"))?;

        if !resp.status().is_success() {
            return Err(status_to_err(resp.status(), PROVIDER));
        }

        let val: Value = resp
            .json()
            .map_err(|e| format!("error parseando respuesta de embeddings: {e}"))?;

        parse_embed_response(&val)
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
        let body = build_chat_body(&self.model, &system, question, true);

        let client = Self::client()?;
        let resp = client
            .post(format!("{}/chat/completions", self.base))
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .map_err(|e| format!("sin conexión con OpenAI: {e}"))?;

        if !resp.status().is_success() {
            return Err(status_to_err(resp.status(), PROVIDER));
        }

        // Stream SSE tokens. Abort mid-stream is a clean stop (not an error).
        read_sse(resp, |payload| {
            if abort.load(Ordering::Relaxed) {
                return ControlFlow::Break(());
            }
            if let Some(tok) = extract_delta(payload) {
                on_token(&tok);
            }
            ControlFlow::Continue(())
        })
        .map_err(|e| format!("error leyendo stream de OpenAI: {e}"))?;

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

    fn provider_for(port: u16) -> OpenAiProvider {
        OpenAiProvider {
            api_key: "test-key".to_string(),
            model: "gpt-4o".to_string(),
            base: format!("http://127.0.0.1:{port}/v1"),
        }
    }

    fn dummy_input() -> AnalysisInput {
        AnalysisInput {
            transcript: vec![
                (
                    "Alice".to_string(),
                    "We decided to launch next Monday.".to_string(),
                ),
                ("Bob".to_string(), "Blocker: CI is failing.".to_string()),
            ],
            template_sections: vec!["Resumen".to_string(), "Decisiones".to_string()],
            lang: "es".to_string(),
        }
    }

    // ------------------------------------------------------------------
    // analyze — happy path
    // ------------------------------------------------------------------
    #[test]
    fn analyze_parses_summary_and_decisions() {
        let content_json =
            r#"{"summary":"Resumen.","decisions":["D1"],"blockers":[],"actions":[]}"#;
        let body = serde_json::json!({
            "choices": [{"message": {"content": content_json}}]
        })
        .to_string();

        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let port = server.server_addr().to_ip().unwrap().port();
        let prov = provider_for(port);

        // Spawn server thread: answer one request.
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
        assert_eq!(analysis.decisions, vec!["D1"]);
    }

    // ------------------------------------------------------------------
    // analyze — retry path: first response is bad JSON, second is valid
    // ------------------------------------------------------------------
    #[test]
    fn analyze_retries_on_bad_json() {
        let bad_body = serde_json::json!({
            "choices": [{"message": {"content": "no json here"}}]
        })
        .to_string();
        let good_content = r#"{"summary":"Retry ok.","decisions":[],"blockers":[],"actions":[]}"#;
        let good_body = serde_json::json!({
            "choices": [{"message": {"content": good_content}}]
        })
        .to_string();

        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let port = server.server_addr().to_ip().unwrap().port();
        let prov = provider_for(port);

        // Serve two requests sequentially.
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
    // answer — SSE streaming
    // ------------------------------------------------------------------
    #[test]
    fn answer_collects_streamed_tokens() {
        let sse_body = concat!(
            "data: {\"choices\":[{\"delta\":{\"content\":\"Hola\"}}]}\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\" mundo\"}}]}\n",
            "data: [DONE]\n",
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
        let prov = OpenAiProvider {
            api_key: "key".to_string(),
            model: "gpt-4o".to_string(),
            base: "http://127.0.0.1:19999/v1".to_string(), // nothing listening
        };
        let abort = AtomicBool::new(true); // pre-set
        let mut tokens = String::new();
        let result = prov.answer("q", &[], "es", &mut |t| tokens.push_str(t), &abort);
        // Should return Ok(()) cleanly, no tokens, no network error.
        assert!(result.is_ok());
        assert!(tokens.is_empty());
    }

    // ------------------------------------------------------------------
    // embed — two texts
    // ------------------------------------------------------------------
    #[test]
    fn embed_returns_vectors_in_order() {
        let resp_body = serde_json::json!({
            "data": [
                {"index": 0, "embedding": [0.1_f64, 0.2_f64]},
                {"index": 1, "embedding": [0.3_f64, 0.4_f64]}
            ]
        })
        .to_string();

        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let port = server.server_addr().to_ip().unwrap().port();
        let prov = provider_for(port);

        let body_clone = resp_body.clone();
        std::thread::spawn(move || {
            let req = server.recv().unwrap();
            let resp = tiny_http::Response::from_string(body_clone).with_header(
                tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..])
                    .unwrap(),
            );
            req.respond(resp).ok();
        });

        let texts = vec!["alpha".to_string(), "beta".to_string()];
        let result = prov.embed(&texts).expect("embed should succeed");

        assert_eq!(result.len(), 2);
        assert!((result[0][0] - 0.1_f32).abs() < 1e-5);
        assert!((result[0][1] - 0.2_f32).abs() < 1e-5);
        assert!((result[1][0] - 0.3_f32).abs() < 1e-5);
        assert!((result[1][1] - 0.4_f32).abs() < 1e-5);
    }

    // ------------------------------------------------------------------
    // embed — empty input (no network)
    // ------------------------------------------------------------------
    #[test]
    fn embed_empty_returns_ok_without_network() {
        let prov = OpenAiProvider {
            api_key: "key".to_string(),
            model: "gpt-4o".to_string(),
            base: "http://127.0.0.1:19999/v1".to_string(), // nothing listening
        };
        let result = prov.embed(&[]).expect("empty embed should return Ok");
        assert!(result.is_empty());
    }

    // ------------------------------------------------------------------
    // answer — abort honored mid-stream (proves answer→read_sse→abort wiring)
    // ------------------------------------------------------------------
    #[test]
    fn answer_stops_when_aborted_mid_stream() {
        // Mock serves a multi-token SSE body. The user's on_token callback flips
        // the abort flag after the FIRST token, so read_sse's callback returns
        // Break before emitting the second. This deterministically exercises the
        // answer→read_sse→abort path (no sleeps/races). Only "uno" lands.
        let sse_body = concat!(
            "data: {\"choices\":[{\"delta\":{\"content\":\"uno\"}}]}\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"dos\"}}]}\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"tres\"}}]}\n",
            "data: [DONE]\n",
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
}
