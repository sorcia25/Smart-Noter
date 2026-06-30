//! Cloud-provider configuration commands. API keys are DPAPI-encrypted in
//! `provider_secrets`; the full key NEVER crosses to the frontend — only
//! `configured` + the last 4 chars.

use crate::error::from_db;
use crate::secrets;
use crate::state::AppState;
use smart_noter_core::models::ai::ProviderConfig;
use smart_noter_core::AppError;
use smart_noter_db::repos::{secrets_repo, settings_repo};
use tauri::State;

/// The cloud providers we support (local is implicit, has no key).
const CLOUD_PROVIDERS: &[&str] = &["openai", "anthropic", "azure"];

/// Last 4 chars of a decrypted key, for display ("••••1234").
fn last4(key: &str) -> String {
    let n = key.chars().count();
    key.chars().skip(n.saturating_sub(4)).collect()
}

#[tauri::command]
#[specta::specta]
pub async fn get_provider_config(
    state: State<'_, AppState>,
) -> Result<Vec<ProviderConfig>, AppError> {
    let settings = settings_repo::get(&state.pool).await.map_err(from_db)?;
    let mut out = Vec::new();
    for &p in CLOUD_PROVIDERS {
        let ct = secrets_repo::get(&state.pool, p).await.map_err(from_db)?;
        let key_last4 = ct
            .and_then(|c| secrets::decrypt(&c).ok())
            .map(|k| last4(&k));
        out.push(ProviderConfig {
            domain: "ai".into(),
            provider: p.into(),
            configured: key_last4.is_some(),
            key_last4,
            model: settings.model_for(p),
        });
    }
    Ok(out)
}

/// (method, url, auth header name, auth header value) for a lightweight key-validation
/// request per provider. Pure — unit-testable without network.
fn validation_request(
    provider: &str,
    key: &str,
) -> Result<(&'static str, String, String, String), AppError> {
    match provider {
        "openai" => Ok((
            "GET",
            "https://api.openai.com/v1/models".into(),
            "Authorization".into(),
            format!("Bearer {key}"),
        )),
        "anthropic" => Ok((
            "GET",
            "https://api.anthropic.com/v1/models".into(),
            "x-api-key".into(),
            key.to_string(),
        )),
        "azure" => Err(AppError::Internal(
            "Azure se valida al configurar su endpoint (Módulo B/C)".into(),
        )),
        other => Err(AppError::Internal(format!(
            "proveedor desconocido: {other}"
        ))),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn update_provider_config(
    state: State<'_, AppState>,
    provider: String,
    key: Option<String>,
    model: Option<String>,
) -> Result<(), AppError> {
    if let Some(k) = key.as_deref().filter(|k| !k.trim().is_empty()) {
        let ct = secrets::encrypt(k).map_err(AppError::Internal)?;
        secrets_repo::upsert(&state.pool, &provider, &ct)
            .await
            .map_err(from_db)?;
    }
    // Always persist the active provider — even when only a key (no model) is saved.
    // The backend is the source of truth for `ai_provider`, independent of FE call order.
    let mut s = settings_repo::get(&state.pool).await.map_err(from_db)?;
    s.ai_provider = provider.clone();
    if let Some(m) = model {
        s.provider_models.insert(provider.clone(), m);
    }
    settings_repo::upsert(&state.pool, &s)
        .await
        .map_err(from_db)?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn test_api_key(state: State<'_, AppState>, provider: String) -> Result<(), AppError> {
    let ct = secrets_repo::get(&state.pool, &provider)
        .await
        .map_err(from_db)?
        .ok_or_else(|| AppError::Internal("no hay API key configurada".into()))?;
    let key = secrets::decrypt(&ct).map_err(AppError::Internal)?;
    let (_method, url, hname, hval) = validation_request(&provider, &key)?;

    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .header(hname.as_str(), hval.as_str())
        .header("anthropic-version", "2023-06-01")
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("sin conexión con el proveedor: {e}")))?;

    if resp.status().is_success() {
        Ok(())
    } else if resp.status().as_u16() == 401 || resp.status().as_u16() == 403 {
        Err(AppError::Internal("API key inválida o sin permiso".into()))
    } else {
        Err(AppError::Internal(format!(
            "el proveedor respondió {}",
            resp.status()
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn last4_handles_short_and_unicode() {
        assert_eq!(last4("sk-1234567"), "4567");
        assert_eq!(last4("ab"), "ab");
        assert_eq!(last4(""), "");
    }

    #[test]
    fn validation_request_shapes() {
        let (m, url, h, v) = validation_request("openai", "K").unwrap();
        assert_eq!(m, "GET");
        assert!(url.starts_with("https://api.openai.com"));
        assert_eq!(h, "Authorization");
        assert_eq!(v, "Bearer K");

        let (_, _, h2, v2) = validation_request("anthropic", "K").unwrap();
        assert_eq!(h2, "x-api-key");
        assert_eq!(v2, "K");

        assert!(validation_request("azure", "K").is_err());
        assert!(validation_request("nope", "K").is_err());
    }
}
