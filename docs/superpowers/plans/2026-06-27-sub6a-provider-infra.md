# Sub-6 Module A — Provider Infrastructure Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the secure, DPAPI-encrypted API-key store + per-domain provider settings + the 3 provider-config commands + the Configuración UI — the foundation every cloud provider (LLM in Module B, STT in Module C) plugs into. No real LLM/STT adapter yet; `test_api_key` makes the first real validation call.

**Architecture:** API keys are encrypted with Windows DPAPI (`secrets.rs`, binary-local) and stored as ciphertext in a new `provider_secrets` table (migration 0007, `secrets_repo`). Per-domain provider/model lives in `AppSettings` (`ai_provider`/`ai_model` added; `transcription_provider`/`transcription_model` already exist). Three Tauri commands (`get_provider_config`, `update_provider_config`, `test_api_key`) expose config to the UI, never returning the full key. A `ProviderPanel.tsx` in Configuración drives them.

**Tech Stack:** `windows` crate (DPAPI), `reqwest` (key validation), `sqlx`, `tauri`/`specta`, RTK Query.

**Conventions to follow (verified in this codebase):**
- Migrations: `sqlx::migrate!("./migrations")` auto-runs every file in `crates/db/migrations/` at `init_pool`. Tests use `init_pool_in_memory()`.
- Repos: free functions in `crates/db/src/repos/<name>.rs`, `Result<_, DbError>`, `#[tokio::test]` with `init_pool_in_memory()`.
- Commands: `#[tauri::command] #[specta::specta] pub async fn ... (state: State<'_, AppState>, ...) -> Result<_, AppError>`, map DB errors with `crate::error::from_db`. Register in `src/lib.rs` `collect_commands![...]`.
- Bilingual/i18n: user-facing strings in `.tsx` go through `t('key')`; add keys to `src/i18n/locales/{es,en}.json`. The `no-hardcoded-strings` hook blocks literals.
- Format/lint gates (lefthook): run `cargo fmt` from inside `src-tauri/`, `npx biome format --write` + `npx biome check` on changed TS. Never `--no-verify`.
- Env preamble for cargo/git (LLVM/cmake on PATH) — needed because the workspace links whisper/llm:
  ```bash
  export PATH="$HOME/.cargo/bin:/c/Program Files (x86)/Microsoft Visual Studio/2022/BuildTools/Common7/IDE/CommonExtensions/Microsoft/CMake/CMake/bin:$PATH"
  export LIBCLANG_PATH="C:/Program Files/LLVM/bin"
  ```
- Regenerate bindings after adding/altering commands or `specta::Type` types: `cargo run --bin specta-export` (writes `src/ipc/bindings.ts`). Copy the whisper/llm/sherpa DLLs next to `specta-export` if it fails with STATUS_DLL_NOT_FOUND (known trap).

---

## File Structure

| File | Responsibility |
|------|----------------|
| `src-tauri/src/secrets.rs` (create) | DPAPI `encrypt`/`decrypt` (binary-local, Windows API) |
| `src-tauri/crates/db/migrations/0007_provider_secrets.sql` (create) | `provider_secrets` table |
| `src-tauri/crates/db/src/repos/secrets_repo.rs` (create) | `upsert`/`get`/`delete`/`list_providers` of ciphertext |
| `src-tauri/crates/db/src/repos/mod.rs` (modify) | export `secrets_repo` |
| `src-tauri/crates/core/src/models/settings.rs` (modify) | add `ai_provider`, `ai_model` |
| `src-tauri/crates/core/src/models/ai.rs` (modify) | add `ProviderConfig` type |
| `src-tauri/src/commands/providers.rs` (create) | `get_provider_config`, `update_provider_config`, `test_api_key` |
| `src-tauri/src/commands/mod.rs` (modify) | declare `pub mod providers;` |
| `src-tauri/src/lib.rs` (modify) | register the 3 commands |
| `src-tauri/Cargo.toml` (modify) | add `windows`, `reqwest` |
| `src/store/api/providers.api.ts` (create) | RTK endpoints |
| `src/features/settings/ProviderPanel.tsx` (create) | provider config UI |
| `src/features/settings/SettingsPage.tsx` (modify) | mount `ProviderPanel` |
| `src/i18n/locales/{es,en}.json` (modify) | new keys |

---

## Task A1: DPAPI secrets module

**Files:**
- Modify: `src-tauri/Cargo.toml` (add `windows`)
- Create: `src-tauri/src/secrets.rs`
- Modify: `src-tauri/src/lib.rs` (add `pub mod secrets;`)

- [ ] **Step 1: Add the `windows` dependency**

In `src-tauri/Cargo.toml` `[dependencies]`, add:
```toml
windows = { version = "0.58", features = ["Win32_Security_Cryptography", "Win32_Foundation"] }
```

- [ ] **Step 2: Write the failing round-trip test**

Create `src-tauri/src/secrets.rs`:
```rust
//! Windows DPAPI wrapper: encrypt/decrypt small secrets (API keys) bound to the
//! current Windows user profile. Ciphertext is opaque and only decryptable by the
//! same user on the same machine. Plaintext never persists.

use windows::Win32::Foundation::{LocalFree, HLOCAL};
use windows::Win32::Security::Cryptography::{
    CryptProtectData, CryptUnprotectData, CRYPT_INTEGER_BLOB,
};

/// Encrypt `plaintext` with DPAPI. Returns opaque ciphertext bytes.
pub fn encrypt(plaintext: &str) -> Result<Vec<u8>, String> {
    let mut bytes = plaintext.as_bytes().to_vec();
    let mut in_blob = CRYPT_INTEGER_BLOB {
        cbData: bytes.len() as u32,
        pbData: bytes.as_mut_ptr(),
    };
    let mut out_blob = CRYPT_INTEGER_BLOB::default();
    unsafe {
        CryptProtectData(&mut in_blob, None, None, None, None, 0, &mut out_blob)
            .map_err(|e| format!("DPAPI encrypt failed: {e}"))?;
        let slice = std::slice::from_raw_parts(out_blob.pbData, out_blob.cbData as usize);
        let result = slice.to_vec();
        let _ = LocalFree(Some(HLOCAL(out_blob.pbData as *mut core::ffi::c_void)));
        Ok(result)
    }
}

/// Decrypt DPAPI `ciphertext` back to the original UTF-8 string.
pub fn decrypt(ciphertext: &[u8]) -> Result<String, String> {
    let mut data = ciphertext.to_vec();
    let mut in_blob = CRYPT_INTEGER_BLOB {
        cbData: data.len() as u32,
        pbData: data.as_mut_ptr(),
    };
    let mut out_blob = CRYPT_INTEGER_BLOB::default();
    unsafe {
        CryptUnprotectData(&mut in_blob, None, None, None, None, 0, &mut out_blob)
            .map_err(|e| format!("DPAPI decrypt failed: {e}"))?;
        let slice = std::slice::from_raw_parts(out_blob.pbData, out_blob.cbData as usize);
        let result = String::from_utf8(slice.to_vec()).map_err(|e| e.to_string());
        let _ = LocalFree(Some(HLOCAL(out_blob.pbData as *mut core::ffi::c_void)));
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypt_decrypt_round_trip() {
        let secret = "sk-test-ABC123-áé"; // includes multibyte to prove UTF-8 safety
        let ct = encrypt(secret).unwrap();
        assert_ne!(ct.as_slice(), secret.as_bytes(), "ciphertext must differ from plaintext");
        assert_eq!(decrypt(&ct).unwrap(), secret);
    }

    #[test]
    fn decrypt_garbage_errors() {
        assert!(decrypt(&[0u8, 1, 2, 3, 4]).is_err());
    }
}
```
Add `pub mod secrets;` to `src-tauri/src/lib.rs` (near the other `pub mod` lines).

- [ ] **Step 3: Run the test (must fail to compile first, then pass once the dep resolves)**

Run (env preamble first):
```bash
cargo test -p smart-noter --manifest-path src-tauri/Cargo.toml secrets:: -- --nocapture
```
Expected: `encrypt_decrypt_round_trip` and `decrypt_garbage_errors` PASS. If the `windows` API signatures differ in 0.58 (e.g. `CryptProtectData` return type), adjust per the compiler — the shape (in-blob → out-blob, `LocalFree` the out buffer) is correct; verify against the installed crate at `~/.cargo/registry/src/*/windows-0.58*`.

- [ ] **Step 4: fmt + commit**

```bash
cd src-tauri && cargo fmt && cd ..
git add src-tauri/Cargo.toml src-tauri/src/secrets.rs src-tauri/src/lib.rs
git commit -m "feat(sub6a): DPAPI encrypt/decrypt for API keys"
```

---

## Task A2: Migration 0007 + secrets_repo

**Files:**
- Create: `src-tauri/crates/db/migrations/0007_provider_secrets.sql`
- Create: `src-tauri/crates/db/src/repos/secrets_repo.rs`
- Modify: `src-tauri/crates/db/src/repos/mod.rs`

- [ ] **Step 1: Write the migration**

Create `src-tauri/crates/db/migrations/0007_provider_secrets.sql`:
```sql
-- Cloud provider API keys (Sub-6). Ciphertext is DPAPI-encrypted; plaintext is
-- never stored. One row per provider id.
CREATE TABLE provider_secrets (
    provider   TEXT PRIMARY KEY,       -- 'openai' | 'anthropic' | 'azure'
    ciphertext BLOB NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

- [ ] **Step 2: Write the failing repo test**

Create `src-tauri/crates/db/src/repos/secrets_repo.rs`:
```rust
use crate::DbError;
use sqlx::SqlitePool;

/// Insert or replace the ciphertext for `provider`.
pub async fn upsert(pool: &SqlitePool, provider: &str, ciphertext: &[u8]) -> Result<(), DbError> {
    sqlx::query(
        "INSERT INTO provider_secrets (provider, ciphertext, updated_at) \
         VALUES (?, ?, datetime('now')) \
         ON CONFLICT(provider) DO UPDATE SET ciphertext = excluded.ciphertext, updated_at = datetime('now')",
    )
    .bind(provider)
    .bind(ciphertext)
    .execute(pool)
    .await?;
    Ok(())
}

/// Get the stored ciphertext for `provider`, if any.
pub async fn get(pool: &SqlitePool, provider: &str) -> Result<Option<Vec<u8>>, DbError> {
    let row: Option<(Vec<u8>,)> =
        sqlx::query_as("SELECT ciphertext FROM provider_secrets WHERE provider = ?")
            .bind(provider)
            .fetch_optional(pool)
            .await?;
    Ok(row.map(|(c,)| c))
}

/// Remove the stored key for `provider` (no error if absent).
pub async fn delete(pool: &SqlitePool, provider: &str) -> Result<(), DbError> {
    sqlx::query("DELETE FROM provider_secrets WHERE provider = ?")
        .bind(provider)
        .execute(pool)
        .await?;
    Ok(())
}

/// List provider ids that currently have a stored key.
pub async fn list_providers(pool: &SqlitePool) -> Result<Vec<String>, DbError> {
    let rows: Vec<(String,)> =
        sqlx::query_as("SELECT provider FROM provider_secrets ORDER BY provider")
            .fetch_all(pool)
            .await?;
    Ok(rows.into_iter().map(|(p,)| p).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::init_pool_in_memory;

    #[tokio::test]
    async fn upsert_get_delete_round_trip() {
        let pool = init_pool_in_memory().await.unwrap();
        assert!(get(&pool, "openai").await.unwrap().is_none());

        upsert(&pool, "openai", &[1, 2, 3]).await.unwrap();
        assert_eq!(get(&pool, "openai").await.unwrap().unwrap(), vec![1, 2, 3]);

        // upsert replaces, not duplicates
        upsert(&pool, "openai", &[9, 9]).await.unwrap();
        assert_eq!(get(&pool, "openai").await.unwrap().unwrap(), vec![9, 9]);

        upsert(&pool, "anthropic", &[7]).await.unwrap();
        assert_eq!(list_providers(&pool).await.unwrap(), vec!["anthropic", "openai"]);

        delete(&pool, "openai").await.unwrap();
        assert!(get(&pool, "openai").await.unwrap().is_none());
        assert_eq!(list_providers(&pool).await.unwrap(), vec!["anthropic"]);
    }
}
```
Add `pub mod secrets_repo;` to `src-tauri/crates/db/src/repos/mod.rs`.

- [ ] **Step 3: Run the test**

```bash
cargo test -p smart-noter-db --manifest-path src-tauri/Cargo.toml secrets_repo
```
Expected: `upsert_get_delete_round_trip` PASS.

- [ ] **Step 4: fmt + commit**

```bash
cd src-tauri && cargo fmt && cd ..
git add src-tauri/crates/db/migrations/0007_provider_secrets.sql src-tauri/crates/db/src/repos/secrets_repo.rs src-tauri/crates/db/src/repos/mod.rs
git commit -m "feat(sub6a): migration 0007 provider_secrets + secrets_repo"
```

---

## Task A3: Settings fields (ai_provider, ai_model)

**Files:**
- Modify: `src-tauri/crates/core/src/models/settings.rs`

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `settings.rs`:
```rust
#[test]
fn defaults_include_ai_provider_fields() {
    let d = AppSettings::default();
    assert_eq!(d.ai_provider, "local");
    assert_eq!(d.ai_model, "qwen2.5-3b-instruct-q4");
}

#[test]
fn legacy_blob_without_ai_provider_uses_defaults() {
    // A persisted blob from Sub-5 (no aiProvider/aiModel). Must deserialize + fill.
    let json = r##"{
        "theme":"light","accent":"#10b981","language":"es","avatarStyle":"circle",
        "aiChatVisible":true,"captureMode":"system","defaultDevice":"system-loopback",
        "recordingQuality":"WAV 48k","runLocal":true,"autoDeleteAudio":false,
        "transcriptionProvider":"local","transcriptionModel":"large-v3",
        "autoTranscribe":true,"nativeLanguage":"es","defaultTemplate":"tecnica",
        "identifySpeakers":true,"diarizationModel":"default","autoGenerateSummary":true
    }"##;
    let parsed: AppSettings = serde_json::from_str(json).expect("legacy blob must deserialize");
    assert_eq!(parsed.ai_provider, "local");
    assert_eq!(parsed.ai_model, "qwen2.5-3b-instruct-q4");
}
```

- [ ] **Step 2: Run to verify it fails**

```bash
cargo test -p smart-noter-core --manifest-path src-tauri/Cargo.toml settings
```
Expected: FAIL (no field `ai_provider`).

- [ ] **Step 3: Add the fields**

In `AppSettings` (after `auto_generate_summary`), add:
```rust
    #[serde(default = "default_ai_provider")]
    pub ai_provider: String, // "local" | "openai" | "anthropic" | "azure"
    #[serde(default = "default_ai_model")]
    pub ai_model: String,
```
In the `Default` impl (after `auto_generate_summary: true,`):
```rust
            ai_provider: "local".into(),
            ai_model: "qwen2.5-3b-instruct-q4".into(),
```
Add the default fns near `default_diar_model`:
```rust
fn default_ai_provider() -> String {
    "local".into()
}
fn default_ai_model() -> String {
    "qwen2.5-3b-instruct-q4".into()
}
```

- [ ] **Step 4: Run to verify it passes**

```bash
cargo test -p smart-noter-core --manifest-path src-tauri/Cargo.toml settings
```
Expected: PASS.

- [ ] **Step 5: fmt + commit**

```bash
cd src-tauri && cargo fmt && cd ..
git add src-tauri/crates/core/src/models/settings.rs
git commit -m "feat(sub6a): ai_provider + ai_model settings (per-domain selection)"
```

---

## Task A4: ProviderConfig type + get_provider_config command

**Files:**
- Modify: `src-tauri/crates/core/src/models/ai.rs`
- Create: `src-tauri/src/commands/providers.rs`
- Modify: `src-tauri/src/commands/mod.rs`, `src-tauri/src/lib.rs`

- [ ] **Step 1: Add the `ProviderConfig` type**

In `src-tauri/crates/core/src/models/ai.rs`, add (it must derive `Type` + `Serialize`, camelCase, like the other IPC types there):
```rust
/// One provider's config as the UI sees it. NEVER contains the full key.
#[derive(Debug, Clone, specta::Type, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConfig {
    pub domain: String,    // "ai" | "transcription"
    pub provider: String,  // "local" | "openai" | "anthropic" | "azure"
    pub configured: bool,  // a key is stored for this provider
    pub key_last4: Option<String>,
    pub model: String,     // selected model id for this domain
}
```
(Confirm `specta`/`serde` are deps of `core` — they are, used by `settings.rs`.)

- [ ] **Step 2: Write `get_provider_config` (no unit test — thin IPC wrapper; covered by the smoke)**

Create `src-tauri/src/commands/providers.rs`:
```rust
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
pub async fn get_provider_config(state: State<'_, AppState>) -> Result<Vec<ProviderConfig>, AppError> {
    let settings = settings_repo::get(&state.pool).await.map_err(from_db)?;
    let mut out = Vec::new();
    // AI domain: the selected provider + model, plus per-cloud-provider configured flags.
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
```
> Design note: `configured`/`keyLast4` are per *provider* (the key is shared across domains for the same provider); `model` reflects the AI domain's selection. The transcription domain reuses the same keys and is surfaced the same way in Module C. The UI keys off `provider` + `configured`.

Add `pub mod providers;` to `src-tauri/src/commands/mod.rs`, and register in `src/lib.rs` `collect_commands![...]`:
```rust
        commands::providers::get_provider_config,
```

- [ ] **Step 3: Build + regenerate bindings**

```bash
cargo build -p smart-noter --manifest-path src-tauri/Cargo.toml
cargo run --bin specta-export --manifest-path src-tauri/Cargo.toml
```
Expected: builds; `src/ipc/bindings.ts` now has `getProviderConfig` + `ProviderConfig`.

- [ ] **Step 4: fmt + commit**

```bash
cd src-tauri && cargo fmt && cd ..
git add -A
git commit -m "feat(sub6a): ProviderConfig type + get_provider_config command"
```

---

## Task A5: update_provider_config + test_api_key

**Files:**
- Modify: `src-tauri/Cargo.toml` (add `reqwest`)
- Modify: `src-tauri/src/commands/providers.rs`, `src-tauri/src/lib.rs`

- [ ] **Step 1: Add `reqwest`**

In `src-tauri/Cargo.toml` `[dependencies]`:
```toml
reqwest = { version = "0.12", default-features = false, features = ["rustls-tls", "json"] }
```

- [ ] **Step 2: Write the validation-endpoint helper + its test**

Append to `src-tauri/src/commands/providers.rs`:
```rust
/// (method, url, auth header name, auth header value) for a lightweight key-validation
/// request per provider. Pure — unit-testable without network.
fn validation_request(provider: &str, key: &str) -> Result<(&'static str, String, String, String), AppError> {
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
        // Azure validation needs the resource endpoint; deferred to Module C/B where the
        // Azure base URL setting exists. For now reject with a clear message.
        "azure" => Err(AppError::Internal(
            "Azure se valida al configurar su endpoint (Módulo B/C)".into(),
        )),
        other => Err(AppError::Internal(format!("proveedor desconocido: {other}"))),
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
```

- [ ] **Step 3: Run the unit test**

```bash
cargo test -p smart-noter --manifest-path src-tauri/Cargo.toml providers::tests
```
Expected: `last4_handles_short_and_unicode`, `validation_request_shapes` PASS.

- [ ] **Step 4: Add the two commands**

Append to `providers.rs`:
```rust
#[tauri::command]
#[specta::specta]
pub async fn update_provider_config(
    state: State<'_, AppState>,
    provider: String,
    key: Option<String>,
    model: Option<String>,
) -> Result<(), AppError> {
    // Store the key (encrypted) if provided and non-empty.
    if let Some(k) = key.as_deref().filter(|k| !k.trim().is_empty()) {
        let ct = secrets::encrypt(k).map_err(AppError::Internal)?;
        secrets_repo::upsert(&state.pool, &provider, &ct)
            .await
            .map_err(from_db)?;
    }
    // Persist the AI-domain selection (provider + model) when a model is given.
    if let Some(m) = model {
        let mut s = settings_repo::get(&state.pool).await.map_err(from_db)?;
        s.ai_provider = provider.clone();
        s.ai_model = m;
        settings_repo::upsert(&state.pool, &s).await.map_err(from_db)?;
    }
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
        .header(hname, hval)
        .header("anthropic-version", "2023-06-01") // ignored by OpenAI; required by Anthropic
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("sin conexión con el proveedor: {e}")))?;

    if resp.status().is_success() {
        Ok(())
    } else if resp.status().as_u16() == 401 || resp.status().as_u16() == 403 {
        Err(AppError::Internal("API key inválida o sin permiso".into()))
    } else {
        Err(AppError::Internal(format!("el proveedor respondió {}", resp.status())))
    }
}
```
Register both in `src/lib.rs` `collect_commands![...]`:
```rust
        commands::providers::update_provider_config,
        commands::providers::test_api_key,
```

- [ ] **Step 5: Build + regenerate bindings + commit**

```bash
cargo build -p smart-noter --manifest-path src-tauri/Cargo.toml
cargo run --bin specta-export --manifest-path src-tauri/Cargo.toml
cd src-tauri && cargo fmt && cd ..
git add -A
git commit -m "feat(sub6a): update_provider_config + test_api_key commands"
```

---

## Task A6: Frontend — RTK api + ProviderPanel + Configuración

**Files:**
- Create: `src/store/api/providers.api.ts`
- Create: `src/features/settings/ProviderPanel.tsx`
- Modify: `src/features/settings/SettingsPage.tsx`
- Modify: `src/i18n/locales/es.json`, `src/i18n/locales/en.json`

- [ ] **Step 1: RTK endpoints**

Create `src/store/api/providers.api.ts` (mirror `ai.api.ts`'s `createApi`/`invoke` pattern):
```ts
import { baseApi } from './base';
import { commands } from '@/ipc/bindings';
import type { ProviderConfig } from '@/ipc/bindings';

export const providersApi = baseApi.injectEndpoints({
  endpoints: (build) => ({
    getProviderConfig: build.query<ProviderConfig[], void>({
      queryFn: async () => ({ data: await commands.getProviderConfig() }),
      providesTags: ['ProviderConfig'],
    }),
    updateProviderConfig: build.mutation<
      void,
      { provider: string; key?: string | null; model?: string | null }
    >({
      queryFn: async ({ provider, key, model }) => {
        await commands.updateProviderConfig(provider, key ?? null, model ?? null);
        return { data: undefined };
      },
      invalidatesTags: ['ProviderConfig'],
    }),
    testApiKey: build.mutation<string | null, { provider: string }>({
      // resolves to null on success, throws the error string on failure
      queryFn: async ({ provider }) => {
        const res = await commands.testApiKey(provider);
        if (res.status === 'error') return { error: res.error };
        return { data: null };
      },
    }),
  }),
});

export const {
  useGetProviderConfigQuery,
  useUpdateProviderConfigMutation,
  useTestApiKeyMutation,
} = providersApi;
```
Add `'ProviderConfig'` to the `tagTypes` array in `src/store/api/base.ts`.
> Note: `commands.*` here uses the tauri-specta `Result` wrapper (`{status:'ok'|'error'}`) — match how `ai.api.ts` unwraps it; if `ai.api.ts` calls the raw `@tauri-apps/api/core` `invoke` instead, follow that exact pattern rather than `commands.*`.

- [ ] **Step 2: ProviderPanel component**

Create `src/features/settings/ProviderPanel.tsx`: a panel listing the AI-domain providers (`openai`, `anthropic`, `azure` + `local`), with a `<select>` for the active `ai_provider`, and for the selected cloud provider: a password input for the key (placeholder shows `••••${keyLast4}` when `configured`), a model input, a **Probar conexión** button (calls `useTestApiKeyMutation`, shows ✓/✗ + the error string), and a **Guardar** button (calls `useUpdateProviderConfigMutation`). All user-facing strings via `t(...)`. The key input is write-only — never pre-filled from state. Show the privacy disclaimer (`t('cloudPrivacyDisclaimer')`) when a cloud provider is selected. Follow the structure/styles of `AiModelPanel.tsx`.

- [ ] **Step 3: Mount in SettingsPage**

In `src/features/settings/SettingsPage.tsx`, add a "Proveedores de IA" section rendering `<ProviderPanel />` (next to the existing AI model section).

- [ ] **Step 4: i18n keys**

Add to `src/i18n/locales/es.json` and `en.json` (es shown; translate for en): `providersTitle` ("Proveedores de IA"), `providerLocal` ("Local"), `apiKey` ("API key"), `apiKeyConfigured` ("Configurada"), `testConnection` ("Probar conexión"), `connectionOk` ("Conexión correcta"), `model` ("Modelo"), `save` (exists?), `cloudPrivacyDisclaimer` ("En modo nube, el audio o la transcripción salen de tu dispositivo hacia el proveedor.").

- [ ] **Step 5: Verify + commit**

```bash
npx tsc --noEmit
npx vitest run
npx biome check src/store/api/providers.api.ts src/features/settings/ProviderPanel.tsx
cd src-tauri && cargo fmt && cd ..
git add -A
git commit -m "feat(sub6a): provider config UI (RTK + ProviderPanel + Configuración)"
```

---

## Self-Review

**Spec coverage:** ✅ DPAPI key store (A1), `provider_secrets` table (A2), per-domain settings (A3), `get_provider_config`/`update_provider_config`/`test_api_key` returning configured+last4 never the key (A4/A5), provider-config UI + privacy disclaimer + "keys never in Redux" (A6). Module A's spec scope is fully covered. (Modules B/C — adapters, factory, STT — are separate plans.)

**Type consistency:** `ProviderConfig` (A4) is consumed verbatim in `providers.api.ts` (A6). `ai_provider`/`ai_model` (A3) are written by `update_provider_config` (A5). `secrets::encrypt`/`decrypt` (A1) used by `secrets_repo` callers in commands (A4/A5). `validation_request` (A5) ↔ its test (A5).

**Known follow-ups (not Module A):** Azure key validation needs its resource endpoint (added in B/C alongside the Azure base-URL setting). `transcription_provider` UI surfacing lands in Module C. The factory + real adapters are Modules B/C.
