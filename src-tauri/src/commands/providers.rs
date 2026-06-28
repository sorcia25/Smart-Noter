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
            model: settings.ai_model.clone(),
        });
    }
    Ok(out)
}
