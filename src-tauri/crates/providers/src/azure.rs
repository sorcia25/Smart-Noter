use std::ops::ControlFlow;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde_json::Value;
use smart_noter_core::models::ai::{Chunk, MeetingAnalysis};
use smart_noter_core::traits::{AnalysisInput, ChatEngine, Summarizer};

use crate::http_common::{
    build_chat_body, build_chat_system_prompt, extract_delta, extract_message_content,
    status_to_err,
};
use crate::sse::read_sse;

/// Azure REST API version for Chat Completions.
const API_VERSION: &str = "2024-06-01";

/// Human-facing provider name used in error messages.
const PROVIDER: &str = "Azure";

// ---------------------------------------------------------------------------
// Public struct
// ---------------------------------------------------------------------------

pub struct AzureProvider {
    /// Resource endpoint, e.g. "https://my-res.openai.azure.com"
    pub endpoint: String,
    /// Azure deployment name (used where other providers use `model`).
    pub deployment: String,
    pub api_key: String,
}

impl AzureProvider {
    pub fn new(endpoint: String, deployment: String, api_key: String) -> Self {
        Self {
            // Normalize once: the Azure portal displays the endpoint WITH a trailing
            // slash, but `chat_url()` already inserts `/openai/...`. Stripping it here
            // prevents a `...azure.com//openai/...` double slash (which Azure 404s).
            endpoint: endpoint.trim_end_matches('/').to_string(),
            deployment,
            api_key,
        }
    }

    /// Build the Azure Chat Completions URL for the configured deployment.
    ///
    /// Format: `{endpoint}/openai/deployments/{deployment}/chat/completions?api-version={API_VERSION}`
    fn chat_url(&self) -> String {
        format!(
            "{}/openai/deployments/{}/chat/completions?api-version={}",
            self.endpoint, self.deployment, API_VERSION
        )
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

impl Summarizer for AzureProvider {
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
        // Azure body is OpenAI-shaped; the `model` field carries the deployment name —
        // Azure accepts (and ignores) it in practice.
        let body = build_chat_body(&self.deployment, &system, &user, false);

        let resp = client
            .post(self.chat_url())
            .header("api-key", &self.api_key)
            .json(&body)
            .send()
            .map_err(|e| format!("sin conexión con Azure: {e}"))?;

        if !resp.status().is_success() {
            return Err(status_to_err(resp.status(), PROVIDER));
        }

        let val: Value = resp
            .json()
            .map_err(|e| format!("error parseando respuesta de Azure: {e}"))?;

        let content = extract_message_content(&val)
            .ok_or_else(|| "respuesta de Azure sin contenido".to_string())?;

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
                let body2 = build_chat_body(&self.deployment, &system2, &user2, false);

                let resp2 = client
                    .post(self.chat_url())
                    .header("api-key", &self.api_key)
                    .json(&body2)
                    .send()
                    .map_err(|e| format!("sin conexión con Azure: {e}"))?;

                if !resp2.status().is_success() {
                    return Err(status_to_err(resp2.status(), PROVIDER));
                }

                let val2: Value = resp2
                    .json()
                    .map_err(|e| format!("error parseando respuesta de Azure: {e}"))?;

                let content2 = extract_message_content(&val2)
                    .ok_or_else(|| "respuesta de Azure sin contenido".to_string())?;

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

impl ChatEngine for AzureProvider {
    /// Azure embeddings are not wired in this MVP — each deployment in Azure handles
    /// a specific model, and we would need a separate embeddings-deployment setting
    /// (distinct from the chat deployment) to call the embeddings API. To keep the
    /// configuration surface minimal, the factory (Task B5) detects this Err return
    /// and falls back to the local embedder when the active provider is Azure.
    fn embed(&self, _texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
        Err("Azure embeddings no configurados en esta versión (se usa el modelo local)".to_string())
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
        let body = build_chat_body(&self.deployment, &system, question, true);

        let client = Self::client()?;
        let resp = client
            .post(self.chat_url())
            .header("api-key", &self.api_key)
            .json(&body)
            .send()
            .map_err(|e| format!("sin conexión con Azure: {e}"))?;

        if !resp.status().is_success() {
            return Err(status_to_err(resp.status(), PROVIDER));
        }

        // Stream SSE tokens. Azure SSE deltas are OpenAI-shaped. Abort mid-stream
        // is a clean stop (not an error).
        read_sse(resp, |payload| {
            if abort.load(Ordering::Relaxed) {
                return ControlFlow::Break(());
            }
            if let Some(tok) = extract_delta(payload) {
                on_token(&tok);
            }
            ControlFlow::Continue(())
        })
        .map_err(|e| format!("error leyendo stream de Azure: {e}"))?;

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests (tiny_http mock — no real network)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicBool;

    use super::*;

    /// Build a provider pointing at a local mock server.
    /// `endpoint` has no trailing slash; `chat_url()` adds `/openai/...`.
    fn provider_for(port: u16) -> AzureProvider {
        AzureProvider {
            endpoint: format!("http://127.0.0.1:{port}"),
            deployment: "gpt-4o-deploy".to_string(),
            api_key: "test-azure-key".to_string(),
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
    // analyze — happy path + URL shape assertion
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

        // Capture the request URL so we can assert the Azure-specific path shape.
        let body_clone = body.clone();
        let deployment = prov.deployment.clone();
        std::thread::spawn(move || {
            let req = server.recv().unwrap();
            // Assert the URL contains the Azure-specific path components.
            let url = req.url().to_string();
            assert!(
                url.contains(&format!(
                    "/openai/deployments/{deployment}/chat/completions"
                )),
                "URL should contain Azure deployment path, got: {url}"
            );
            assert!(
                url.contains(&format!("api-version={API_VERSION}")),
                "URL should contain api-version, got: {url}"
            );
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
    // answer — SSE streaming + URL shape assertion
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

        let deployment = prov.deployment.clone();
        let sse = sse_body.to_string();
        std::thread::spawn(move || {
            let req = server.recv().unwrap();
            let url = req.url().to_string();
            assert!(
                url.contains(&format!(
                    "/openai/deployments/{deployment}/chat/completions"
                )),
                "SSE URL should contain Azure deployment path, got: {url}"
            );
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
        let prov = AzureProvider {
            endpoint: "http://127.0.0.1:19998".to_string(), // nothing listening
            deployment: "gpt-4o-deploy".to_string(),
            api_key: "key".to_string(),
        };
        let abort = AtomicBool::new(true); // pre-set
        let mut tokens = String::new();
        let result = prov.answer("q", &[], "es", &mut |t| tokens.push_str(t), &abort);
        // Should return Ok(()) cleanly, no tokens, no network error.
        assert!(result.is_ok());
        assert!(tokens.is_empty());
    }

    // ------------------------------------------------------------------
    // embed — always returns Err (local fallback in MVP)
    // ------------------------------------------------------------------
    #[test]
    fn embed_returns_err_for_azure_local_fallback() {
        let prov = AzureProvider {
            endpoint: "http://127.0.0.1:19998".to_string(),
            deployment: "gpt-4o-deploy".to_string(),
            api_key: "key".to_string(),
        };
        let result = prov.embed(&["texto de prueba".to_string()]);
        assert!(
            result.is_err(),
            "Azure embed should return Err (local fallback)"
        );
        let msg = result.unwrap_err();
        assert!(
            msg.contains("Azure embeddings"),
            "Error message should mention Azure embeddings, got: {msg}"
        );
    }

    // ------------------------------------------------------------------
    // new() — trailing slash on the endpoint is normalized away
    // ------------------------------------------------------------------
    #[test]
    fn new_normalizes_trailing_slash_in_url() {
        // The Azure portal shows the endpoint with a trailing slash. `chat_url()`
        // inserts its own `/openai/...`, so the slash must be stripped at construction
        // to avoid a `//openai` double slash (which Azure 404s).
        let prov = AzureProvider::new("http://x/".into(), "dep".into(), "k".into());
        let url = prov.chat_url();
        assert!(
            url.contains("x/openai/deployments/dep/chat/completions"),
            "URL should have a single slash before /openai, got: {url}"
        );
        assert!(
            !url.contains("x//openai"),
            "URL must not contain a double slash, got: {url}"
        );
    }
}
