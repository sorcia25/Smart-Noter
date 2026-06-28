# Sub-6 Module B — Cloud LLM (summary + chat + embeddings) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement OpenAI / Anthropic / Azure adapters of the existing `Summarizer` + `ChatEngine` traits (cloud summary, streamed RAG chat, cloud embeddings with local fallback), and a backend factory that picks local vs cloud per `ai_provider`. Module A (DPAPI key store + provider config + settings) is already merged.

**Architecture:** A new set of adapters in `crates/providers` (`openai.rs`/`anthropic.rs`/`azure.rs`) implement `Summarizer`+`ChatEngine` over HTTPS via `reqwest`. A shared `sse.rs` parses streaming deltas. The summary prompt is refactored so the logical `(system, user)` content is produced once (`build_messages`) and each engine formats it (local → ChatML; cloud → provider `messages[]`). A `provider_factory` in the binary reads `settings.ai_provider` + the decrypted key and returns `Box<dyn Summarizer>` / `Box<dyn ChatEngine>`; `run_summary`/`ask_meeting` call the factory instead of constructing `LocalSummarizer`/`LocalChat` directly (today `ai.rs:282`, `ai.rs:903`). Embeddings: OpenAI/Azure call their embeddings API; Anthropic (no embeddings API) and any cloud error fall back to the local `llm` embedder — and because the local `LocalLlm` is a process singleton in `AppState`, **the fallback is orchestrated in the binary (`ai.rs`), not inside the providers crate**.

**Tech Stack:** `reqwest` (HTTPS + SSE), `serde_json`, the `core` traits, `eventsource`-style manual SSE parsing.

**Conventions (verified):** providers crate depends ONLY on `smart-noter-core` + `reqwest` + `serde`/`serde_json` (NOT `llm` — no singleton there). Trait error type is `String`. `cargo fmt` from `src-tauri/`. Env preamble (PATH+LIBCLANG_PATH) on every cargo/git command. lefthook fmt+clippy — never `--no-verify`. The `Summarizer`/`ChatEngine` traits are SYNCHRONOUS (spawned in a thread) — so the adapters must run their async reqwest calls on a blocking bridge (use `reqwest::blocking` OR `tokio::runtime::Handle::current().block_on(...)` — prefer `reqwest::blocking::Client` since the trait methods are sync and called from a `std::thread`).

**Decisions made for this plan (sensible defaults; revisit if needed):**
- Default models: OpenAI `gpt-4o-mini`, Anthropic `claude-3-5-sonnet-latest`, Azure = the user's deployment name (no default). Embeddings: OpenAI/Azure `text-embedding-3-small`.
- Azure needs a base URL (resource endpoint). Add `azure_endpoint` to settings (Module B) since the AI domain uses it; STT (Module C) reuses it.
- `reqwest::blocking` (the trait methods are sync, invoked from worker threads — no async runtime conflict).

---

## File Structure

| File | Responsibility |
|------|----------------|
| `crates/llm/src/summarize.rs` (modify) | extract `pub fn build_messages(input, strict) -> (String, String)`; `build_prompt` = `chatml(build_messages(..))` |
| `crates/providers/Cargo.toml` (modify) | deps: core, reqwest(blocking, json, rustls), serde, serde_json |
| `crates/providers/src/lib.rs` (modify) | re-export adapters + a small `ProviderError`→String helper |
| `crates/providers/src/sse.rs` (create) | parse an SSE byte stream into delta strings (provider-agnostic line reader) |
| `crates/providers/src/openai.rs` (create) | `OpenAiProvider` impl Summarizer + ChatEngine |
| `crates/providers/src/anthropic.rs` (create) | `AnthropicProvider` impl Summarizer + ChatEngine (embed → Err) |
| `crates/providers/src/azure.rs` (create) | `AzureProvider` (OpenAI-shaped, base-URL + deployment) |
| `crates/core/src/models/settings.rs` (modify) | add `azure_endpoint: String` |
| `src/commands/provider_factory.rs` (create) | `summarizer()`/`chat_engine()` pick local vs cloud; embed-fallback helper |
| `src/commands/ai.rs` (modify) | call the factory; embed fallback in run_summary + ask_meeting |

---

## Task B1: Prompt content/format refactor + providers crate scaffold + SSE parser

**Files:** `crates/llm/src/summarize.rs`, `crates/providers/{Cargo.toml,src/lib.rs,src/sse.rs}`.

- [ ] **Step 1 — Extract `build_messages`.** In `summarize.rs`, refactor `build_prompt` so the system/user strings come from a reusable pub fn:
```rust
/// The logical (system, user) content for a summary request, provider-agnostic.
/// Local wraps this in ChatML; cloud adapters send it as messages[].
pub fn build_messages(input: &AnalysisInput, strict: bool) -> (String, String) {
    let body: String = input.transcript.iter().map(|(s, t)| format!("{s}: {t}")).collect::<Vec<_>>().join("\n");
    let sections = input.template_sections.join(", ");
    let lang = &input.lang;
    let strict_prefix = if strict {
        "IMPORTANTE: responde ÚNICAMENTE el JSON empezando por {. Sin texto adicional.\n"
    } else { "" };
    let system = format!(
        "{strict_prefix}Eres un asistente que resume reuniones. Plantilla con secciones: [{sections}].\n\
         Devuelve SOLO un objeto JSON válido con las claves exactas: \"summary\" (string, en {lang}),\n\
         \"decisions\" (array de strings), \"blockers\" (array de strings), \"actions\"\n\
         (array de objetos {{\"text\":..,\"owner\":..|null,\"due\":..|null}}). No añadas texto fuera del JSON."
    );
    let user = format!("Transcripción:\n{body}");
    (system, user)
}

pub fn build_prompt(input: &AnalysisInput, strict: bool) -> String {
    let (system, user) = build_messages(input, strict);
    chatml(&system, &user)
}
```
Keep all existing `parse_analysis` + summarize tests green: `cargo test -p smart-noter-llm`.

- [ ] **Step 2 — providers Cargo.toml.** Set `crates/providers/Cargo.toml` `[dependencies]`:
```toml
smart-noter-core = { path = "../core" }
reqwest = { version = "0.12", default-features = false, features = ["blocking", "json", "rustls-tls"] }
serde = { workspace = true }
serde_json = { workspace = true }

[dev-dependencies]
# a tiny local mock server for adapter tests:
tiny_http = "0.12"
```

- [ ] **Step 3 — SSE parser.** Create `crates/providers/src/sse.rs`: a function that, given a `reqwest::blocking::Response`, iterates lines and yields the payload of each `data:` line (skipping `data: [DONE]`), invoking a callback. Write it so the per-provider delta extraction (JSON-decode each payload) is done by the caller. Include unit tests over a fixed byte buffer (use `&[u8]` reader, not a real response): multi-line events, `[DONE]`, blank lines.
```rust
use std::io::{BufRead, BufReader, Read};
/// Read an SSE stream, calling `on_data(payload)` for each `data: <payload>` line
/// (excluding the literal `[DONE]`). Returns Ok when the stream ends.
pub fn read_sse<R: Read>(reader: R, mut on_data: impl FnMut(&str)) -> std::io::Result<()> {
    let buf = BufReader::new(reader);
    for line in buf.lines() {
        let line = line?;
        if let Some(rest) = line.strip_prefix("data:") {
            let payload = rest.trim();
            if payload.is_empty() || payload == "[DONE]" { continue; }
            on_data(payload);
        }
    }
    Ok(())
}
```
Tests assert the right payloads are surfaced. Add `pub mod sse;` + re-exports to `lib.rs`.

- [ ] **Step 4 — Build + test + commit.**
```bash
cargo test -p smart-noter-llm -p smart-noter-providers --manifest-path src-tauri/Cargo.toml
cd src-tauri && cargo fmt && cd ..
git add -A && git commit -m "feat(sub6b): build_messages refactor + providers scaffold + SSE parser"
```

---

## Task B2: OpenAI adapter (the template the others mirror)

**Files:** `crates/providers/src/openai.rs`, `lib.rs`.

`OpenAiProvider { api_key: String, model: String }`. Implement BOTH traits.

- [ ] **Step 1 — Summarizer (non-streamed chat completion → parse JSON).**
```rust
// POST https://api.openai.com/v1/chat/completions
// body: {"model":model,"messages":[{"role":"system","content":system},{"role":"user","content":user}],
//        "temperature":0.3,"response_format":{"type":"json_object"}}
// auth: Authorization: Bearer {api_key}
// resp: {"choices":[{"message":{"content":"<json string>"}}]}
```
`analyze(input, progress, abort)`: `progress(10)`; build `(system,user)` via `smart_noter_llm`? NO — providers must not depend on `llm`. Instead the FACTORY passes the prompt pieces. **Design:** the cloud `Summarizer::analyze` re-derives the messages from `AnalysisInput` using a copy of `build_messages` logic OR `core` exposes it. CLEANEST: move `build_messages` + `parse_analysis` into `core` (they're pure, no llama dep) so BOTH `llm` and `providers` reuse them. Do that in B1 Step 1 instead — put `build_messages`/`parse_analysis` in `core::ai_prompt`, and have `llm/summarize.rs` re-export/use them. Then here: `let (system,user)=core::ai_prompt::build_messages(input,false); ... ; let analysis = core::ai_prompt::parse_analysis(&content, &input.lang)?;` with the strict retry on parse failure.
- POST via `reqwest::blocking::Client`, map non-2xx to `Err(String)` (401→"API key inválida", 429→"límite de uso"). `progress(100)`.

- [ ] **Step 2 — ChatEngine::answer (SSE streamed).**
```rust
// POST .../chat/completions with "stream":true ; messages:[{system: "Responde en {lang} usando SOLO el contexto..."},{user: "Contexto:\n{ctx}\n\nPregunta: {question}"}]
// SSE deltas: data: {"choices":[{"delta":{"content":"..."}}]}
```
Use `read_sse(resp, |payload| { let v: serde_json::Value = ...; if let Some(tok)=v["choices"][0]["delta"]["content"].as_str() { on_token(tok); } })`. Check `abort` between deltas (read in a loop you control, or check inside the closure and stop). Return `Ok(())`.

- [ ] **Step 3 — ChatEngine::embed.**
```rust
// POST https://api.openai.com/v1/embeddings  body: {"model":"text-embedding-3-small","input":[texts...]}
// resp: {"data":[{"embedding":[f32...]}, ...]}  (same order)
```
Return `Vec<Vec<f32>>`. Map errors to `Err(String)`.

- [ ] **Step 4 — Tests** against a `tiny_http` mock server bound to `127.0.0.1:0` (canned JSON for chat, SSE for stream, embeddings). Build a provider pointed at the mock's base URL — so make the base URL a field (`base: String`, default `https://api.openai.com/v1`) to allow injection. Assert: analyze parses, answer streams tokens in order, embed returns vectors. Commit.
```bash
cargo test -p smart-noter-providers --manifest-path src-tauri/Cargo.toml
git add -A && git commit -m "feat(sub6b): OpenAI adapter (summary + streamed chat + embeddings)"
```

---

## Task B3: Anthropic adapter

**Files:** `crates/providers/src/anthropic.rs`.

Mirror B2 with Anthropic's API deltas:
- Auth header `x-api-key: {key}` + `anthropic-version: 2023-06-01`.
- Endpoint `POST https://api.anthropic.com/v1/messages`. Body: `{"model":model,"max_tokens":1024,"system":system,"messages":[{"role":"user","content":user}]}`. For chat, add `"stream":true`.
- Non-streamed summary response: `{"content":[{"type":"text","text":"<json>"}]}` → take `content[0].text` → `parse_analysis`.
- SSE deltas: events `content_block_delta` with `{"delta":{"type":"text_delta","text":"..."}}` — parse each `data:` payload, read `["delta"]["text"]`.
- `embed`: Anthropic has NO embeddings API → return `Err("anthropic-no-embeddings")` (a sentinel the factory recognizes for local fallback).
- Tests with `tiny_http` mock (injectable base URL). Commit `feat(sub6b): Anthropic adapter (messages + streamed chat; embed unsupported)`.

---

## Task B4: Azure adapter

**Files:** `crates/providers/src/azure.rs`, `crates/core/src/models/settings.rs` (add `azure_endpoint`).

Azure OpenAI is OpenAI-shaped but URL = `{azure_endpoint}/openai/deployments/{deployment}/chat/completions?api-version=2024-06-01`, auth header `api-key: {key}`. `AzureProvider { endpoint, deployment, api_key }` (deployment = the model field). Reuse the OpenAI request/response shapes (extract the OpenAI body-build + SSE-delta-extract into shared helpers in `openai.rs` or `lib.rs` to avoid copy-paste — DRY). Embeddings: `{endpoint}/openai/deployments/{embed_deployment}/embeddings?api-version=...` — for MVP use the same deployment field is wrong; add an `azure_embed_deployment` setting OR document that Azure embeddings fall back to local in MVP (simpler — pick this and note it). Add `azure_endpoint: String` (default "") to settings with a serde default. Tests with mock. Commit.

---

## Task B5: Factory + ai.rs integration + embed fallback

**Files:** `src/commands/provider_factory.rs` (create), `src/commands/ai.rs` (modify), `src/commands/mod.rs`.

- [ ] **Step 1 — Factory.** `provider_factory.rs`:
```rust
// summarizer(settings, pool, llm_guard) -> Result<Box<dyn Summarizer + '_>, AppError>
//   match settings.ai_provider:
//     "local" => Box::new(LocalSummarizer { llm }),  // needs the llm guard
//     "openai" => decrypt key from secrets_repo; Box::new(OpenAiProvider{ api_key, model: settings.ai_model, base: default })
//     "anthropic" => ...; "azure" => ...(endpoint from settings)
//     missing key => Err(AppError::Internal("configura la API key del proveedor"))
// chat_engine(...) similarly returns Box<dyn ChatEngine>.
```
Because the local impls borrow the `llm` MutexGuard and the cloud impls own their data, the boxed trait object lifetime differs — return an enum `Engine::Local(LocalSummarizer)` / `Engine::Cloud(Box<dyn Summarizer>)` OR keep the guard alive in the caller. Simplest: the factory is a function the caller invokes WHILE holding the llm guard for the local branch; for cloud it ignores the guard. Implement as two helper fns the caller uses inside the existing locked scope.

- [ ] **Step 2 — Wire run_summary.** Replace `let summarizer = LocalSummarizer { llm };` (ai.rs ~282) with a factory call selecting local/cloud by `settings.ai_provider`. The rest (analyze → persist) is unchanged.

- [ ] **Step 3 — Wire ask_meeting + embed fallback.** Replace `let chat_engine = LocalChat { llm };` (~903). For the embed step: call `chat_engine.embed(texts)`; if it returns `Err` containing the no-embeddings sentinel OR `ai_provider` is anthropic, fall back to the local `LocalLlm` embedder (the singleton already in scope). Keep the existing chunk/persist logic.

- [ ] **Step 4 — Build + the existing AI tests + commit.**
```bash
cargo build -p smart-noter --manifest-path src-tauri/Cargo.toml
cargo test -p smart-noter-core -p smart-noter-db -p smart-noter -p smart-noter-providers --manifest-path src-tauri/Cargo.toml
cargo run --bin specta-export --manifest-path src-tauri/Cargo.toml   # azure_endpoint added → regen bindings
git add -A && git commit -m "feat(sub6b): provider factory + cloud summary/chat wired into ai.rs"
```

---

## Task B6: Frontend model defaults + real-app smoke

**Files:** `src/features/settings/ProviderPanel.tsx` (default-model hints), i18n.

- [ ] **Step 1 — Default-model hints.** In `ProviderPanel`, when a provider is selected and no model is set, prefill the model input's placeholder with the provider's default (`gpt-4o-mini`/`claude-3-5-sonnet-latest`/deployment-name) + an Azure endpoint field bound to `azure_endpoint`. Strings via `t()`.

- [ ] **Step 2 — tsc + vitest + commit.**
```bash
npx tsc --noEmit && npx vitest run
git add -A && git commit -m "feat(sub6b): provider model defaults + Azure endpoint field"
```

- [ ] **Step 3 — Real-app smoke (manual, controller-run, like Sub-5).** Back up the %APPDATA% DB first. In the dev app: set AI provider = OpenAI, paste a real key (user does this), Probar conexión → ✓; generate a summary on a meeting → confirms cloud summary + parse; ask a question → confirms SSE streaming + embed (cloud or fallback). Repeat for Anthropic (embed falls back to local). Restore the DB after. Document results.

---

## Self-Review

**Spec coverage:** ✅ OpenAI/Anthropic/Azure adapters of Summarizer+ChatEngine (B2-B4), prompt content/format split (B1), SSE streaming → chat:token (B2-B4 via the existing event plumbing in ai.rs), cloud embeddings + local fallback (B2-B3 + B5 Step 3), the factory choosing per `ai_provider` (B5). Matches the Sub-6 spec's Module B.

**Type consistency:** `build_messages`/`parse_analysis` move to `core::ai_prompt` (B1) and are reused by `llm` + all 3 adapters. The no-embeddings sentinel string is produced by Anthropic (B3) and recognized by the factory (B5 Step 3). `azure_endpoint` added in B4, read in B5/B6.

**Open follow-ups (note for execution):** (1) the sync-trait-over-async-HTTP bridge uses `reqwest::blocking` — verify it coexists with the Tauri tokio runtime when called from the worker `std::thread` (it should, since blocking client spins its own). (2) `abort` mid-stream: the SSE reader loop must check `abort` and break. (3) Azure embeddings fall back to local in MVP (no separate embed-deployment setting) — documented in B4. (4) Module A's `test_api_key` is GET-only + `ai_model` is global — fine for B; revisit if per-provider model storage is wanted.
