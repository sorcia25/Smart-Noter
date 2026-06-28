# Sub-6: Cloud Providers (LLM + STT) Design

> **Status:** DESIGN — approved in brainstorming 2026-06-27. Roadmap item 6.

**Goal:** Add cloud provider options (OpenAI, Anthropic, Azure) for both
transcription (STT) and AI (summary + chat), alongside the existing local-first
engines, with DPAPI-encrypted API keys and per-domain selection.

**Architecture:** Per-domain provider selection in settings. A backend *factory*
returns a local or cloud implementation of the existing `Summarizer` /
`ChatEngine` traits and a new `Transcriber` trait. Cloud adapters live in
`crates/providers`. API keys are encrypted with Windows DPAPI and stored in a new
`provider_secrets` table; they never reach the frontend.

**Tech stack:** `reqwest` (HTTPS + SSE streaming), `windows` crate (DPAPI
`CryptProtectData`/`CryptUnprotectData`), `serde_json`, existing `whisper`/`llm`
crates for the local fallbacks.

---

## Goals

- Cloud **LLM** (summary + chat) via OpenAI, Anthropic, Azure — same traits, streamed.
- Cloud **STT** (transcription) via OpenAI, Azure — behind a new `Transcriber` trait.
- **DPAPI-encrypted** API keys; **per-domain** provider+model selection; provider-config UI.
- **Cloud embeddings** (OpenAI/Azure) with **local fallback** for RAG.
- **Local diarization** (sherpa) aligned to cloud-STT text by timestamps.

## Non-goals (out of scope)

- Arbitrary/custom OpenAI-compatible endpoints — only the 3 named providers.
- Cloud sync or storage of meetings — meetings are always local.
- Automatic retries/backoff beyond surfacing one clear error.
- Cost tracking, budgets, or usage metering.
- Streaming STT (cloud STT is batch file upload only).
- Replacing the local engines — local stays the default and the fallback.

## Locked decisions (from brainstorming 2026-06-27)

| Decision | Choice |
|----------|--------|
| Scope | **Both** LLM + STT cloud |
| Providers | **OpenAI + Anthropic + Azure** (named adapters each) |
| Key encryption | **Windows DPAPI** (per architecture; not OS keychain) |
| Embeddings | **Cloud when available, local fallback** (Anthropic → local) |
| Selection granularity | **Per-domain** (`transcription_provider`, `ai_provider`) — replaces global `run_local` |
| Abstraction | **Per-domain traits + one adapter per provider** |
| Decomposition | **3 modules**: A infra → B LLM → C STT |
| Cloud-STT diarization | **Local sherpa, aligned to cloud text by timestamps** |

---

## Architecture

### Domain-based selection (replaces `run_local`)

`settings.run_local` (global) is superseded by per-domain provider fields:

- `transcription_provider`: `"local" | "openai" | "azure"` (already exists; today only `"local"`).
- `ai_provider`: `"local" | "openai" | "anthropic" | "azure"` (NEW).
- `transcription_model` (exists) + `ai_model` (NEW) — the model id per domain.

`run_local` is kept only for backward-compatible deserialization of old blobs and
is no longer read by the factory (a migration on load maps `run_local=false` →
leave providers as set; default stays `"local"`).

### The factory

A small `provider_factory` module (in the binary, `src/commands/`) exposes:

```rust
fn summarizer<'a>(settings, secrets, llm_guard) -> Result<Box<dyn Summarizer + 'a>, AppError>;
fn chat_engine<'a>(settings, secrets, llm_guard) -> Result<Box<dyn ChatEngine + 'a>, AppError>;
fn transcriber(settings, secrets) -> Result<Box<dyn Transcriber>, AppError>;
```

Each reads the domain provider from settings; if `"local"`, returns the existing
local impl (Sub-5 `LocalSummarizer`/`LocalChat`, Sub-3a Whisper); otherwise
decrypts the provider key and returns the matching cloud adapter, erroring with a
clear "no API key configured" if the key is absent. `run_summary` / `ask_meeting`
/ the transcription job call the factory instead of constructing `LocalSummarizer`
/`LocalChat` directly (today at `ai.rs:282`, `ai.rs:903`), and otherwise work
unchanged because they operate against the traits.

### Traits

- `Summarizer`, `ChatEngine` — already in `core/traits.rs` (Sub-5), reused as-is.
- `Transcriber` — NEW in `core/traits.rs`:

```rust
pub struct TranscribeInput {
    pub wav_path: PathBuf,
    pub lang: Option<String>,      // hint; None = auto
}
pub struct TranscribedLine { pub start_ms: u64, pub end_ms: u64, pub text: String }

pub trait Transcriber: Send + Sync {
    fn transcribe(
        &self,
        input: &TranscribeInput,
        progress: &mut dyn FnMut(u32),
        abort: &AtomicBool,
    ) -> Result<Vec<TranscribedLine>, String>;
}
```

The local Whisper engine is refactored to implement `Transcriber`; diarization
(sherpa) stays a separate step layered on top of any transcriber's lines.

---

## Module A — Infrastructure (foundation, build first)

**Files:**
- Create: `src-tauri/src/secrets.rs` (DPAPI wrapper) — binary-local (Windows-only API).
- Create: `src-tauri/crates/db/migrations/0007_provider_secrets.sql`.
- Create: `src-tauri/crates/db/src/repos/secrets_repo.rs`.
- Modify: `src-tauri/crates/core/src/models/settings.rs` (add `ai_provider`, `ai_model`).
- Create: `src-tauri/src/commands/providers.rs` (the 3 commands).
- Create: `src/features/settings/ProviderPanel.tsx` + RTK `providers.api.ts`.

### DPAPI secret store

`secrets.rs` wraps DPAPI:

```rust
pub fn encrypt(plaintext: &str) -> Result<Vec<u8>, String>;   // CryptProtectData
pub fn decrypt(ciphertext: &[u8]) -> Result<String, String>;  // CryptUnprotectData
```

Tied to the current Windows user profile (no extra entropy in MVP). The ciphertext
is opaque; plaintext keys never persist and never leave the backend.

### Migration 0007 + repo

```sql
CREATE TABLE provider_secrets (
  provider   TEXT PRIMARY KEY,   -- "openai" | "anthropic" | "azure"
  ciphertext BLOB NOT NULL,
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

`secrets_repo`: `upsert(provider, ciphertext)`, `get(provider) -> Option<Vec<u8>>`,
`delete(provider)`, `list_providers() -> Vec<String>` (which providers have a key).

### Commands (keys never cross to the frontend)

- `get_provider_config() -> Vec<ProviderConfig>` where
  `ProviderConfig { domain, provider, configured: bool, keyLast4: Option<String>, model }`.
  `keyLast4` is computed by decrypting + taking the last 4 chars; the full key is
  never returned.
- `update_provider_config(provider, key: Option<String>, model: Option<String>)` —
  if `key` is `Some`, encrypt + upsert into `provider_secrets`; persist `model`/
  provider selection into settings. `key: None` = update only the model/selection.
- `test_api_key(provider) -> Result<(), String>` — a lightweight authenticated call
  (e.g. OpenAI `GET /v1/models`, Anthropic a 1-token messages call, Azure a
  deployment list) to validate the stored key; returns ok or a clear error.

### Settings UI (Configuración)

Per domain (Transcripción, IA): a provider `<select>`, an API-key field
(`type=password`; after save shows `configurada ••••1234` and is **not** re-fetched
— the key is write-only from the UI's perspective), a model `<select>`, and a
**"Probar conexión"** button calling `test_api_key`. Per the architecture rule,
**API keys never enter Redux** — the slice holds only `configured` + `keyLast4`.
A **privacy disclaimer** appears when any cloud provider is selected (audio/
transcript leaves the device).

> Security note: the API-key field is for the **user** to paste their own key into
> their own app; it is encrypted immediately and never returns to the frontend or
> logs. (Claude does not enter keys — only designs the flow.)

---

## Module B — Cloud LLM (summary + chat + embeddings)

**Files:**
- Create: `src-tauri/crates/providers/src/{openai.rs, anthropic.rs, azure.rs, sse.rs, lib.rs}`.
- Modify: `src-tauri/crates/llm/src/summarize.rs` (split prompt content from format).
- Modify: `src-tauri/src/commands/ai.rs` (use the factory).
- `providers/Cargo.toml`: add `reqwest` (rustls, stream), `serde_json`, `smart-noter-core`.

### Prompt refactor (content vs format)

`build_prompt` is split so the **logical** content (system instruction + the
"Transcripción: …" user content) is produced once and each engine formats it:

- Local (`llm`): wraps in Qwen ChatML (unchanged behavior).
- Cloud: OpenAI/Azure → `messages: [{role:system}, {role:user}]`; Anthropic →
  top-level `system` + `messages: [{role:user}]`.

`parse_analysis` (extract the first balanced JSON object) is reused verbatim —
cloud models emit cleaner JSON than Qwen.

### Streaming (SSE)

Each cloud adapter's `ChatEngine::answer` opens a streaming HTTPS request via
`reqwest` and parses Server-Sent Events (`sse.rs`):

- OpenAI/Azure: `data: {choices:[{delta:{content}}]}` lines, terminated by `data: [DONE]`.
- Anthropic: `event: content_block_delta` with `{delta:{text}}`.

Each delta → `on_token(piece)` → the existing `chat:token` / `chat:done` /
`chat:error` events. The chat UI is unchanged. `Summarizer::analyze` uses the same
client non-streamed (collect the full text, then `parse_analysis`).

### Embeddings

`ChatEngine::embed`:
- OpenAI/Azure adapters → embeddings API (`text-embedding-3-small`).
- Anthropic adapter (no embeddings API) and any cloud failure → **local fallback**
  (the `llm` crate's embedder; requires the local model present — surfaced as a
  setup hint if missing).

---

## Module C — Cloud STT (transcription)

**Files:**
- Modify: `src-tauri/crates/core/src/traits.rs` (`Transcriber` trait + types).
- Modify: `src-tauri/crates/whisper` (impl `Transcriber` for the local engine).
- Create: `src-tauri/crates/providers/src/stt.rs` (OpenAI + Azure STT adapters).
- Modify: `src-tauri/src/commands/transcription.rs` (use the factory; align diarization).

### Adapters

- OpenAI: `POST /v1/audio/transcriptions` (multipart: the WAV + `model=whisper-1` +
  `response_format=verbose_json` for segment timestamps).
- Azure: the Azure Speech / Whisper deployment equivalent (multipart, timestamps).

Both return `Vec<TranscribedLine>` (start/end ms + text), mapped to the existing
`transcript_lines` shape.

### Diarization alignment

When `identify_speakers` is ON, the sherpa diarizer still runs **locally** on the
WAV (independent of the transcriber) producing speaker segments `(start_ms,
end_ms, speaker)`. The transcription job assigns each `TranscribedLine` the speaker
whose segment overlaps its midpoint — the same alignment the local pipeline already
does between Whisper lines and sherpa segments, now applied to cloud lines. With
`identify_speakers` OFF, lines get a single default speaker.

---

## Errors, security, testing

### Error handling

Cloud calls surface one clear error (no auto-retry in MVP), mapped from HTTP
status: 401 → "API key inválida o sin permiso", 429 → "límite de uso alcanzado",
quota/billing → "cuota agotada", network/timeout → "sin conexión con el proveedor".
Emitted via the existing `summary:failed` / `chat:error` / `transcription:failed`
events with the provider name. The local fallback path stays available.

### Security & privacy

- Keys encrypted with DPAPI; stored only as ciphertext; never in Redux, never logged.
- HTTPS only; `keyLast4` is the maximum the frontend ever sees.
- Clear cloud privacy disclaimer in settings (data leaves the device in cloud mode).

### Testing

- `secrets.rs`: DPAPI encrypt→decrypt round-trip (Windows test).
- Adapters: against a local mock HTTP server (canned OpenAI/Anthropic/Azure
  responses + SSE streams) — no real network or keys in tests.
- `sse.rs`: parser unit tests (partial chunks, `[DONE]`, Anthropic events).
- `provider_factory`: returns the right impl for each settings+key combination,
  errors cleanly when a cloud provider is selected with no key.
- Diarization alignment: lines + segments → correct speaker assignment.

---

## Module / plan order

1. **A — Infrastructure** — DPAPI store, migration 0007, secrets repo, settings
   fields, the 3 commands, provider-config UI. No real provider wired yet
   (`test_api_key` does the first real call). Ships a usable, secure key store.
2. **B — Cloud LLM** — adapters + prompt refactor + SSE + embeddings + factory for
   AI. Reuses the Sub-5 trait seam; highest user value.
3. **C — Cloud STT** — `Transcriber` trait + adapters + diarization alignment +
   factory for STT.

Each module is its own implementation plan and ships independently (local stays
the default throughout).
