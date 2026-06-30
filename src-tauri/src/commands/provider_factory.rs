//! Provider factory: builds cloud `Summarizer` / `ChatEngine` trait objects from
//! the persisted `AppSettings` + a decrypted API key, and routes embeddings with a
//! LOCAL FALLBACK.
//!
//! This module is intentionally Tauri-State-free so it can be unit-tested without
//! an app handle. The caller (commands/ai.rs) loads `AppSettings` + the decrypted
//! key and passes them in.
//!
//! ## Lock discipline
//! Cloud providers are OWNED (`Box<dyn ...>`) and perform their own blocking HTTP;
//! they never touch the local LLM lock. Only the LOCAL branch of `embed_texts`
//! locks `llm_arc`, and only for the duration of the embed call. This keeps the
//! cloud HTTP path off the LLM mutex entirely.

use parking_lot::Mutex;
use smart_noter_core::models::AppSettings;
use smart_noter_core::traits::{ChatEngine, Summarizer};
use smart_noter_db::repos::{secrets_repo, settings_repo};
use smart_noter_llm::engine::LocalLlm;
use smart_noter_providers::{AnthropicProvider, AzureProvider, OpenAiProvider};
use sqlx::SqlitePool;
use std::sync::Arc;

/// Resolve `(provider, settings, decrypted_key)` from persisted config.
///
/// `key` is `""` for the local provider. This is the SINGLE source of provider/key
/// resolution shared by the AI commands (run_summary, ask_meeting) and any future
/// cloud module (e.g. Module C cloud STT). Errors carry a user-facing Spanish
/// message; the caller maps it onto its own event vocabulary.
pub async fn resolve_provider(pool: &SqlitePool) -> Result<(String, AppSettings, String), String> {
    let settings = settings_repo::get(pool)
        .await
        .map_err(|e| format!("settings: {e}"))?;
    let provider = settings.ai_provider.clone();
    let key = if provider == "local" {
        String::new()
    } else {
        match secrets_repo::get(pool, &provider)
            .await
            .map_err(|e| format!("secrets: {e}"))?
        {
            Some(ct) => crate::secrets::decrypt(&ct)
                .map_err(|e| format!("no se pudo leer la API key: {e}"))?,
            None => return Err("configura la API key del proveedor en Configuración".to_string()),
        }
    };
    Ok((provider, settings, key))
}

/// Build a cloud `Summarizer` for a non-"local" provider.
///
/// `key` is the decrypted API key. Errors if the provider is unknown or required
/// configuration (e.g. the Azure endpoint) is missing.
pub fn cloud_summarizer(
    provider: &str,
    settings: &AppSettings,
    key: &str,
) -> Result<Box<dyn Summarizer>, String> {
    match provider {
        "openai" => Ok(Box::new(OpenAiProvider::new(
            key.to_string(),
            settings.model_for(provider),
        ))),
        "anthropic" => Ok(Box::new(AnthropicProvider::new(
            key.to_string(),
            settings.model_for(provider),
        ))),
        "azure" => {
            if settings.azure_endpoint.trim().is_empty() {
                return Err("configura el endpoint de Azure en Configuración".to_string());
            }
            let deployment = settings.model_for(provider);
            if deployment.is_empty() {
                return Err(
                    "configura el nombre de deployment (modelo) de Azure en Configuración"
                        .to_string(),
                );
            }
            Ok(Box::new(AzureProvider::new(
                settings.azure_endpoint.clone(),
                deployment,
                key.to_string(),
            )))
        }
        other => Err(format!("proveedor de IA desconocido: {other}")),
    }
}

/// Build a cloud `ChatEngine` for a non-"local" provider. Same matching as
/// [`cloud_summarizer`].
pub fn cloud_chat_engine(
    provider: &str,
    settings: &AppSettings,
    key: &str,
) -> Result<Box<dyn ChatEngine>, String> {
    match provider {
        "openai" => Ok(Box::new(OpenAiProvider::new(
            key.to_string(),
            settings.model_for(provider),
        ))),
        "anthropic" => Ok(Box::new(AnthropicProvider::new(
            key.to_string(),
            settings.model_for(provider),
        ))),
        "azure" => {
            if settings.azure_endpoint.trim().is_empty() {
                return Err("configura el endpoint de Azure en Configuración".to_string());
            }
            let deployment = settings.model_for(provider);
            if deployment.is_empty() {
                return Err(
                    "configura el nombre de deployment (modelo) de Azure en Configuración"
                        .to_string(),
                );
            }
            Ok(Box::new(AzureProvider::new(
                settings.azure_endpoint.clone(),
                deployment,
                key.to_string(),
            )))
        }
        other => Err(format!("proveedor de IA desconocido: {other}")),
    }
}

/// Embed `texts` honoring the active provider, with LOCAL FALLBACK:
/// - `"openai"`: cloud embeddings; on ANY error, fall back to the local embedder.
/// - `"anthropic"` / `"azure"`: local embeddings (MVP — no cloud embeddings wired
///   for these; their `ChatEngine::embed` returns a sentinel `Err`).
/// - anything else (`"local"`): local embeddings.
///
/// The local branch locks `llm_arc` internally (and only then), so the cloud
/// path never holds the LLM lock during HTTP.
pub fn embed_texts(
    provider: &str,
    settings: &AppSettings,
    key: &str,
    texts: &[String],
    llm_arc: &Arc<Mutex<Option<LocalLlm>>>,
) -> Result<Vec<Vec<f32>>, String> {
    match provider {
        "openai" => {
            let engine = OpenAiProvider::new(key.to_string(), settings.model_for(provider));
            match engine.embed(texts) {
                Ok(v) => Ok(v),
                Err(e) => {
                    tracing::warn!(error = %e, "cloud embeddings failed; falling back to local embedder");
                    local_embed(llm_arc, texts)
                }
            }
        }
        // Anthropic + Azure have no cloud embeddings in this MVP; go local directly.
        "anthropic" | "azure" => local_embed(llm_arc, texts),
        _ => local_embed(llm_arc, texts),
    }
}

/// Lock the local LLM slot and embed with the on-device embedder.
/// Errors if the local model is not loaded.
fn local_embed(
    llm_arc: &Arc<Mutex<Option<LocalLlm>>>,
    texts: &[String],
) -> Result<Vec<Vec<f32>>, String> {
    let guard = llm_arc.lock();
    let llm = guard
        .as_ref()
        .ok_or_else(|| "el modelo local no está cargado".to_string())?;
    llm.embed(texts).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use smart_noter_core::models::AppSettings;

    #[test]
    fn cloud_summarizer_ok_for_openai_and_anthropic() {
        let s = AppSettings::default();
        assert!(cloud_summarizer("openai", &s, "k").is_ok());
        assert!(cloud_summarizer("anthropic", &s, "k").is_ok());
    }

    #[test]
    fn cloud_summarizer_azure_requires_endpoint() {
        let mut s = AppSettings::default();
        // default azure_endpoint is empty. `Box<dyn Summarizer>` isn't Debug, so we
        // can't use `unwrap_err()`; match to extract the error string instead.
        match cloud_summarizer("azure", &s, "k") {
            Ok(_) => panic!("expected Err for azure with empty endpoint"),
            Err(err) => assert!(err.contains("endpoint de Azure"), "unexpected error: {err}"),
        }

        // Endpoint set + a deployment (model) configured → Ok.
        s.azure_endpoint = "https://my-res.openai.azure.com".to_string();
        s.provider_models.insert("azure".into(), "gpt-4o".into());
        assert!(cloud_summarizer("azure", &s, "k").is_ok());
    }

    #[test]
    fn cloud_summarizer_azure_requires_deployment() {
        // Endpoint set but NO deployment configured → Err mentioning "deployment".
        // Guards against AzureProvider::new("", ...) building a malformed URL.
        let mut s = AppSettings {
            azure_endpoint: "https://my-res.openai.azure.com".to_string(),
            ..Default::default()
        };
        match cloud_summarizer("azure", &s, "k") {
            Ok(_) => panic!("expected Err for azure with empty deployment"),
            Err(err) => assert!(err.contains("deployment"), "unexpected error: {err}"),
        }

        // An empty stored deployment also fails (treated as absent).
        s.provider_models.insert("azure".into(), "".into());
        assert!(cloud_summarizer("azure", &s, "k").is_err());

        // Once a deployment is set → Ok.
        s.provider_models.insert("azure".into(), "gpt-4o".into());
        assert!(cloud_summarizer("azure", &s, "k").is_ok());
    }

    #[test]
    fn cloud_summarizer_unknown_provider_errors() {
        let s = AppSettings::default();
        match cloud_summarizer("nope", &s, "k") {
            Ok(_) => panic!("expected Err for unknown provider"),
            Err(err) => assert!(err.contains("desconocido"), "unexpected error: {err}"),
        }
    }

    #[test]
    fn cloud_chat_engine_ok_for_openai_and_anthropic() {
        let s = AppSettings::default();
        assert!(cloud_chat_engine("openai", &s, "k").is_ok());
        assert!(cloud_chat_engine("anthropic", &s, "k").is_ok());
    }

    #[test]
    fn cloud_chat_engine_azure_requires_endpoint() {
        let mut s = AppSettings::default();
        assert!(cloud_chat_engine("azure", &s, "k").is_err());
        // Endpoint alone is not enough — a deployment is still required.
        s.azure_endpoint = "https://my-res.openai.azure.com".to_string();
        assert!(cloud_chat_engine("azure", &s, "k").is_err());
        // Endpoint + deployment → Ok.
        s.provider_models.insert("azure".into(), "gpt-4o".into());
        assert!(cloud_chat_engine("azure", &s, "k").is_ok());
    }

    #[test]
    fn cloud_chat_engine_unknown_provider_errors() {
        let s = AppSettings::default();
        assert!(cloud_chat_engine("zzz", &s, "k").is_err());
    }

    // ------------------------------------------------------------------
    // embed_texts routing — anthropic/azure/local all route to local_embed,
    // which errors with the "not loaded" message when the slot is None. This
    // guards the local-fallback contract without needing a network or a model.
    // ------------------------------------------------------------------
    #[test]
    fn embed_texts_anthropic_routes_local_and_errors_without_model() {
        let s = AppSettings::default();
        let llm: Arc<Mutex<Option<LocalLlm>>> = Arc::new(Mutex::new(None));
        let err = embed_texts("anthropic", &s, "k", &["hi".into()], &llm).unwrap_err();
        assert!(err.contains("no está cargado"), "unexpected error: {err}");
    }

    #[test]
    fn embed_texts_azure_routes_local_and_errors_without_model() {
        let s = AppSettings::default();
        let llm: Arc<Mutex<Option<LocalLlm>>> = Arc::new(Mutex::new(None));
        let err = embed_texts("azure", &s, "k", &["hi".into()], &llm).unwrap_err();
        assert!(err.contains("no está cargado"), "unexpected error: {err}");
    }

    #[test]
    fn embed_texts_local_routes_local_and_errors_without_model() {
        let s = AppSettings::default();
        let llm: Arc<Mutex<Option<LocalLlm>>> = Arc::new(Mutex::new(None));
        let err = embed_texts("local", &s, "k", &["hi".into()], &llm).unwrap_err();
        assert!(err.contains("no está cargado"), "unexpected error: {err}");
    }
}
