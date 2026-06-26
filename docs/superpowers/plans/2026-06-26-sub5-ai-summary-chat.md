# Sub-5 — AI Summary + Chat Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Generate a template-aware AI summary + extracted decisions/blockers/actions for each transcribed meeting, let the user edit/regenerate it, and answer questions about the meeting via a streamed RAG chat — all on a local LLM (llama.cpp), behind traits so cloud (Sub-6) plugs in later.

**Architecture:** New pure-ish crate `smart-noter-llm` wraps llama.cpp (text-gen + embeddings) and owns GGUF model download/management (mirroring `crates/whisper`). New `Summarizer`/`ChatEngine` traits in `core` (the project's first traits) with a local impl in `crates/llm`. A `commands/ai.rs` orchestrates: load transcript → run model → persist summary + `source='ai'` items → chunk+embed for RAG → stream chat answers. Frontend wires the existing SummaryTab + AiChatPanel + a Configuración model-manager.

**Tech Stack:** Rust (`llama-cpp-2` for llama.cpp, `ureq`+`sha2` for downloads, `sqlx` SQLite), Tauri v2 commands + events + specta, React/TS + RTK Query + Tauri event listeners, vitest/RTL.

**Spec:** `docs/superpowers/specs/2026-06-26-sub5-ai-summary-chat-design.md` (read it; it has the verified integration facts this plan relies on).

---

## Conventions (same toolchain as Sub-3a/3b/4)

- **Env preamble on EVERY `cargo`/`git` command** (the pre-commit clippy hook rebuilds native crates; llama.cpp vendors+compiles C via CMake, like whisper/sherpa):
  ```bash
  export PATH="$HOME/.cargo/bin:/c/Program Files (x86)/Microsoft Visual Studio/2022/BuildTools/Common7/IDE/CommonExtensions/Microsoft/CMake/CMake/bin:$PATH"
  export LIBCLANG_PATH="C:/Program Files/LLVM/bin"
  ```
  Run cargo with `--manifest-path "src-tauri/Cargo.toml"`. **Use a LONG timeout (600000 ms) on the first build of each new native dep.**
- Run `cargo fmt` **from inside `src-tauri/`** (the lefthook hook formats from there; `--manifest-path` differs slightly and the hook will reject it). Run `npx biome format --write` on touched FE files before each FE commit.
- Generated `bindings.ts` / `i18n/keys.ts` are gitignored — regenerate (`npm run generate:bindings`, `npm run generate:i18n-keys`), never commit. If `generate:bindings` crashes with `STATUS_DLL_NOT_FOUND`, copy the sherpa `*.dll` into `src-tauri/target/debug/` next to `specta-export.exe`.
- DB migrations: this plan adds migration `0006`. UNCHECKED sqlx (`sqlx::query(...)` not `query!`) — the project uses runtime-checked queries in repos.
- `db` crate uuid uses feature `v7`: `Uuid::now_v7()` (not v4).
- `no-hardcoded-strings` FE hook false-positives on `=> Promise<…>` arrow types → use method-signature syntax. All user-facing FE strings go through `t(...)`.
- Branch: `sub-5-ai-summary-chat` (already created; the spec commit is its first commit).

## File Structure

| Path | Responsibility |
|---|---|
| `src-tauri/crates/llm/Cargo.toml` + `src/lib.rs` | crate root: re-exports, `AiError` |
| `src-tauri/crates/llm/src/models.rs` | GGUF model catalog + download/list/delete (mirror `whisper/src/models.rs`) |
| `src-tauri/crates/llm/src/engine.rs` | `LocalLlm`: load GGUF, `generate()` (text), `embed()` (vectors) via llama-cpp-2 |
| `src-tauri/crates/llm/src/summarize.rs` | `LocalSummarizer` impl: prompt build + JSON parse → `MeetingAnalysis` |
| `src-tauri/crates/llm/src/chat.rs` | `LocalChat` impl: chunking, prompt build, streamed `answer()` |
| `src-tauri/crates/core/src/traits.rs` | `Summarizer`, `ChatEngine` traits |
| `src-tauri/crates/core/src/models/ai.rs` | `MeetingAnalysis`, `ExtractedAction`, `Chunk`, `ChatMessage` |
| `src-tauri/crates/db/migrations/0006_ai.sql` | `chat_messages`, `transcript_embeddings`, `source` cols, `summarized_at` |
| `src-tauri/crates/db/src/repos/{chat_repo,embeddings_repo}.rs` | chat history + embeddings persistence |
| `src-tauri/crates/db/src/repos/meetings_repo.rs` (modify) | + `update_summary` |
| `src-tauri/crates/db/src/repos/{decisions,blockers,actions}_repo.rs` (modify) | + `source` param, `delete_ai(meeting_id)` |
| `src-tauri/src/commands/ai.rs` | `generate_summary`, `cancel_summary`, `update_summary_text`, `ask_meeting`, `list_llm_models`, `download_llm_model`, `delete_llm_model`, `get_summary_state` |
| `src-tauri/src/commands/transcription.rs` (modify) | chain auto-summary after `transcription:completed` |
| `src-tauri/src/state.rs` (modify) | hold `Mutex<Option<LocalLlm>>` + a summary/llm-download handle slot |
| `src/features/meeting-detail/tabs/SummaryTab.tsx` (modify) | real + editable summary, Regenerar, empty/loading states |
| `src/features/meeting-detail/side/SidePanel.tsx` (modify) | wire AiChatPanel to streamed `ask_meeting` |
| `src/features/meeting-detail/useAiSummary.ts` + `useChatStream.ts` | event hooks (copy `useTranscription`'s `sub<T>`) |
| `src/features/settings/...` | auto-summary toggle + "Modelo de IA" download UI (mirror Whisper models UI) |
| `src/store/api/ai.api.ts` (+ tests) | RTK mutations for the AI commands |

---

# Phase 1 — Local LLM infrastructure (M1)

### Task 1: Scaffold `smart-noter-llm` crate + prove llama.cpp builds

**Files:** Create `src-tauri/crates/llm/Cargo.toml`, `src/lib.rs`; Modify `src-tauri/Cargo.toml` (workspace members).

- [ ] **Step 1: Add to workspace.** In `src-tauri/Cargo.toml` `[workspace].members`, add `"crates/llm"`.

- [ ] **Step 2: Create `src-tauri/crates/llm/Cargo.toml`.**
```toml
[package]
name = "smart-noter-llm"
version.workspace = true
edition.workspace = true

[dependencies]
smart-noter-core = { path = "../core" }
thiserror.workspace = true
llama-cpp-2 = "0.1"      # llama.cpp binding; drives CMake like whisper-rs. VERIFY latest 0.1.x at impl time.
ureq = "2.10"
sha2 = "0.10"
serde = { workspace = true }
serde_json = { workspace = true }

[dev-dependencies]
tempfile = "3"

[features]
cuda = ["llama-cpp-2/cuda"]
vulkan = ["llama-cpp-2/vulkan"]
```
> **API NOTE:** `llama-cpp-2` is the most-maintained binding. Confirm the crate name/version and feature flag names against crates.io at implementation time; if it won't build on this toolchain, the fallback is `llama_cpp` (the `rustformers`/`utilityai` alternative). The crate MUST compile before any feature code — this is the front-loaded build risk (same strategy as Sub-4C Task 1).

- [ ] **Step 3: Create `src-tauri/crates/llm/src/lib.rs`.**
```rust
//! Local LLM (llama.cpp) for summary generation, extraction, and RAG chat.
//! Owns GGUF model management. Implements core's Summarizer/ChatEngine traits.
pub mod chat;
pub mod engine;
pub mod models;
pub mod summarize;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AiError {
    #[error("model not found: {0}")]
    ModelMissing(String),
    #[error("llm load failed: {0}")]
    Load(String),
    #[error("inference failed: {0}")]
    Inference(String),
    #[error("download failed: {0}")]
    Download(String),
    #[error("bad model output: {0}")]
    Parse(String),
}
```

- [ ] **Step 4: Stub the four modules** so it compiles (each with a `use` + a minimal item; real bodies land in later tasks). e.g. `engine.rs`:
```rust
use crate::AiError;
pub struct LocalLlm;
impl LocalLlm {
    pub fn placeholder() -> Result<(), AiError> { Ok(()) }
}
```
(Similar trivial stubs for `models.rs`, `summarize.rs`, `chat.rs` so `cargo build -p smart-noter-llm` compiles.)

- [ ] **Step 5: Build + commit.**
```bash
# (env preamble)
cargo build -p smart-noter-llm --manifest-path "src-tauri/Cargo.toml"   # LONG timeout: first llama.cpp CMake build
cargo fmt   # from src-tauri/
git add src-tauri/Cargo.toml src-tauri/crates/llm
git commit -m "feat(llm): scaffold smart-noter-llm crate; prove llama.cpp builds"
```
Expected: compiles. **If llama-cpp-2 fails (CMake/cc/linker), STOP and report BLOCKED with the full error** — that's the toolchain risk surfacing; resolve before continuing (do not swap to a fake LLM).

---

### Task 2: GGUF model catalog + download/list/delete

**Files:** Modify `src-tauri/crates/llm/src/models.rs`. **Read `src-tauri/crates/whisper/src/models.rs` first and mirror it exactly** — same `ModelSpec`/`ModelStatus` shapes, same `download()` (ureq stream → tmp → SHA-256 verify → atomic rename), same `models_dir`/`list`/`delete`.

- [ ] **Step 1: Define the catalog.** Mirror whisper's `ModelSpec { id, display_name, size_mb, sha256, url }` and a `CATALOG: &[ModelSpec]` with TWO entries:
  - `qwen2.5-3b-instruct-q4` — Qwen2.5-3B-Instruct GGUF Q4_K_M (HuggingFace direct URL; fill the real URL + SHA-256 at impl time).
  - `e5-small-embed` — multilingual-e5-small GGUF (embeddings model).
  Store under `app_data/models/llm-{id}.gguf` (note the `llm-` prefix to avoid colliding with whisper's `ggml-` files).

- [ ] **Step 2: Mirror `download(app_data, id, progress: impl FnMut(u32,u64,u64), is_cancelled: impl Fn() -> bool) -> Result<(), AiError>`** from whisper's `models::download` (ureq streaming, sha2 verify, atomic rename). Same for `list(app_data) -> Vec<ModelStatus>`, `delete(app_data, id)`, `model_path(app_data, id) -> PathBuf`.

- [ ] **Step 3: Unit test** the path + catalog (no network): `model_path` returns `app_data/models/llm-qwen2.5-3b-instruct-q4.gguf`; `CATALOG` has the embeddings + LLM entries; `list` reports `downloaded=false` for a temp empty dir.
```rust
#[test]
fn catalog_and_paths() {
    let dir = tempfile::tempdir().unwrap();
    assert!(CATALOG.iter().any(|m| m.id == "e5-small-embed"));
    assert!(model_path(dir.path(), "qwen2.5-3b-instruct-q4").ends_with("llm-qwen2.5-3b-instruct-q4.gguf"));
    assert!(list(dir.path()).iter().all(|m| !m.downloaded));
}
```

- [ ] **Step 4: Run + commit.**
```bash
cargo test -p smart-noter-llm --manifest-path "src-tauri/Cargo.toml" models
cargo fmt   # from src-tauri/
git add src-tauri/crates/llm/src/models.rs
git commit -m "feat(llm): GGUF model catalog + download/list/delete (mirrors whisper models)"
```

---

### Task 3: `LocalLlm` engine — load, generate, embed (TDD-light; real-model test ignored)

**Files:** Modify `src-tauri/crates/llm/src/engine.rs`.

- [ ] **Step 1: Implement `LocalLlm`** holding a loaded llama.cpp model + context. Methods:
```rust
use crate::AiError;
use std::path::Path;
use std::sync::atomic::AtomicBool;

pub struct LocalLlm { /* llama_cpp_2 model + backend handle */ }

impl LocalLlm {
    /// Load a GGUF text-gen model. `n_gpu_layers` 0 = CPU.
    pub fn load(path: &Path, n_gpu_layers: u32) -> Result<Self, AiError> { /* ... */ }

    /// Greedy/sampled generation of up to `max_tokens`, calling `on_token` per
    /// decoded token and checking `abort` between tokens. Returns the full text.
    pub fn generate(&self, prompt: &str, max_tokens: usize,
                    on_token: &mut dyn FnMut(&str), abort: &AtomicBool) -> Result<String, AiError> { /* ... */ }

    /// Embed each text (mean-pooled last hidden state) using an embeddings-mode
    /// context. For the dedicated e5 model, prefix queries with "query: " and
    /// docs with "passage: " per e5 conventions.
    pub fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, AiError> { /* ... */ }
}
```
> **API NOTE (llama-cpp-2):** the real API is roughly: `LlamaBackend::init()`, `LlamaModel::load_from_file(&backend, path, &params)`, `model.new_context(&backend, ctx_params)`, build a batch from `model.str_to_token(...)`, `ctx.decode(&mut batch)`, sample with `ctx.candidates_ith(...)` / a sampler, `model.token_to_str(token)`. Embeddings need `ctx_params.with_embeddings(true)` and `ctx.embeddings_seq_ith(...)`. **Verify every call against the installed crate source at `~/.cargo/registry/src/*/llama-cpp-2-*/` before trusting these names.** The two invariants the tests pin: `generate` returns non-empty text and calls `on_token`; `embed` returns one `Vec<f32>` per input of consistent dimension.

- [ ] **Step 2: Tests.** A pure test of any helper (e.g. e5 prefixing) runs always. The real-model test is `#[ignore]` (needs the GGUF; too heavy for CI):
```rust
#[test]
#[ignore = "requires a downloaded GGUF model; run manually"]
fn generate_and_embed_smoke() {
    let m = LocalLlm::load(std::path::Path::new(&std::env::var("LLM_GGUF").unwrap()), 0).unwrap();
    let mut toks = 0;
    let out = m.generate("Responde solo: hola", 16, &mut |_| toks += 1, &std::sync::atomic::AtomicBool::new(false)).unwrap();
    assert!(!out.is_empty() && toks > 0);
    let e = m.embed(&["hola".into(), "adiós".into()]).unwrap();
    assert_eq!(e.len(), 2);
    assert_eq!(e[0].len(), e[1].len());
}
```

- [ ] **Step 3: Build + commit.**
```bash
cargo build -p smart-noter-llm --manifest-path "src-tauri/Cargo.toml"
cargo fmt   # from src-tauri/
git add src-tauri/crates/llm/src/engine.rs
git commit -m "feat(llm): LocalLlm engine — load GGUF, generate (token stream), embed"
```

---

# Phase 2 — Traits + summary generation + extraction (M2/M3)

### Task 4: Core traits + AI types

**Files:** Create `src-tauri/crates/core/src/traits.rs`, `src-tauri/crates/core/src/models/ai.rs`; Modify `core/src/lib.rs` + `core/src/models/mod.rs` to export them.

- [ ] **Step 1: `models/ai.rs`** (read an existing `core/src/models/*.rs` for the derive/serde style — `Type, Serialize, Deserialize`, `#[serde(rename_all = "camelCase")]`):
```rust
use crate::Bilingual;
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Type, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractedAction { pub text: String, pub owner_hint: Option<String>, pub due: Option<String> }

/// The structured result of analyzing one meeting transcript.
#[derive(Debug, Clone, Default)]
pub struct MeetingAnalysis {
    pub summary: Bilingual,
    pub decisions: Vec<String>,
    pub blockers: Vec<String>,
    pub actions: Vec<ExtractedAction>,
}

/// One retrieval chunk of a transcript + its embedding.
#[derive(Debug, Clone)]
pub struct Chunk { pub idx: i64, pub text: String, pub vector: Vec<f32> }

#[derive(Debug, Clone, Type, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage { pub id: i64, pub role: String, pub content: String, pub created_at: String }
```

- [ ] **Step 2: `traits.rs`** (execution mirrors `whisper::transcribe` — sync, thread-run, closures + `Arc<AtomicBool>` abort):
```rust
use crate::models::ai::{Chunk, MeetingAnalysis};
use std::sync::atomic::AtomicBool;

pub struct AnalysisInput {
    pub transcript: Vec<(String, String)>, // (speaker_label, text)
    pub template_sections: Vec<String>,    // the meeting's template `sections`
    pub lang: String,                      // "es" | "en"
}

pub trait Summarizer: Send + Sync {
    fn analyze(&self, input: &AnalysisInput, progress: &mut dyn FnMut(u32),
               abort: &AtomicBool) -> Result<MeetingAnalysis, String>;
}

pub trait ChatEngine: Send + Sync {
    fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, String>;
    fn answer(&self, question: &str, context: &[Chunk], lang: &str,
              on_token: &mut dyn FnMut(&str), abort: &AtomicBool) -> Result<(), String>;
}
```
> Error type is `String` here to keep `core` dependency-free; impls map their `AiError` to string. (Matches how the project already stringly-maps errors at the command boundary.)

- [ ] **Step 3: Export** from `core/src/lib.rs` (`pub mod traits;`) and `models/mod.rs` (`pub mod ai;`). Build.

- [ ] **Step 4: Commit.**
```bash
cargo build -p smart-noter-core --manifest-path "src-tauri/Cargo.toml"
cargo fmt   # from src-tauri/
git add src-tauri/crates/core/src
git commit -m "feat(core): Summarizer/ChatEngine traits + AI types (first traits)"
```

---

### Task 5: `LocalSummarizer` — prompt build + tolerant JSON parse (TDD)

**Files:** Modify `src-tauri/crates/llm/src/summarize.rs`. The **JSON parsing is the testable contract** (no model needed); the prompt+generate call is exercised by the ignored integration test.

- [ ] **Step 1: Failing test for the JSON extractor** (the LLM returns prose around a JSON block; we extract+parse tolerantly):
```rust
#[test]
fn parses_json_block_amid_prose() {
    let raw = r#"Claro, aquí está:
    {"summary":"Resumen.","decisions":["D1"],"blockers":[],"actions":[{"text":"Hacer X","owner":"Ana","due":"2026-07-01"}]}
    Espero que ayude."#;
    let a = parse_analysis(raw, "es").unwrap();
    assert_eq!(a.summary.es, "Resumen.");
    assert_eq!(a.decisions, vec!["D1"]);
    assert!(a.blockers.is_empty());
    assert_eq!(a.actions[0].text, "Hacer X");
    assert_eq!(a.actions[0].owner_hint.as_deref(), Some("Ana"));
}

#[test]
fn errors_on_no_json() { assert!(parse_analysis("no json here", "es").is_err()); }
```

- [ ] **Step 2: Run → FAIL.** `cargo test -p smart-noter-llm summarize`.

- [ ] **Step 3: Implement `parse_analysis` + `build_prompt` + the `Summarizer` impl.**
```rust
use crate::{engine::LocalLlm, AiError};
use serde::Deserialize;
use smart_noter_core::models::ai::{ExtractedAction, MeetingAnalysis};
use smart_noter_core::traits::{AnalysisInput, Summarizer};
use smart_noter_core::Bilingual;
use std::sync::atomic::AtomicBool;

#[derive(Deserialize)]
struct RawAnalysis {
    summary: String,
    #[serde(default)] decisions: Vec<String>,
    #[serde(default)] blockers: Vec<String>,
    #[serde(default)] actions: Vec<RawAction>,
}
#[derive(Deserialize)]
struct RawAction { text: String, #[serde(default)] owner: Option<String>, #[serde(default)] due: Option<String> }

/// Find the first balanced `{...}` block and parse it; tolerant to prose around it.
pub fn parse_analysis(raw: &str, lang: &str) -> Result<MeetingAnalysis, AiError> {
    let start = raw.find('{').ok_or_else(|| AiError::Parse("no JSON".into()))?;
    let end = raw.rfind('}').ok_or_else(|| AiError::Parse("no JSON".into()))?;
    if end <= start { return Err(AiError::Parse("no JSON".into())); }
    let r: RawAnalysis = serde_json::from_str(&raw[start..=end]).map_err(|e| AiError::Parse(e.to_string()))?;
    let summary = if lang == "en" { Bilingual { es: String::new(), en: Some(r.summary) } }
                  else { Bilingual { es: r.summary, en: None } };
    Ok(MeetingAnalysis {
        summary,
        decisions: r.decisions, blockers: r.blockers,
        actions: r.actions.into_iter().map(|a| ExtractedAction { text: a.text, owner_hint: a.owner, due: a.due }).collect(),
    })
}

/// Build a template-aware instruction that forces a single JSON object.
pub fn build_prompt(input: &AnalysisInput) -> String {
    let body: String = input.transcript.iter().map(|(s, t)| format!("{s}: {t}")).collect::<Vec<_>>().join("\n");
    let sections = input.template_sections.join(", ");
    format!(
"Eres un asistente que resume reuniones. Plantilla con secciones: [{sections}].
Devuelve SOLO un objeto JSON válido con las claves exactas: \"summary\" (string, en {lang}),
\"decisions\" (array de strings), \"blockers\" (array de strings), \"actions\"
(array de objetos {{\"text\":..,\"owner\":..|null,\"due\":..|null}}). No añadas texto fuera del JSON.

Transcripción:
{body}",
        sections = sections, lang = input.lang, body = body)
}

pub struct LocalSummarizer<'a> { pub llm: &'a LocalLlm }

impl Summarizer for LocalSummarizer<'_> {
    fn analyze(&self, input: &AnalysisInput, progress: &mut dyn FnMut(u32), abort: &AtomicBool)
        -> Result<MeetingAnalysis, String> {
        progress(5);
        let prompt = build_prompt(input);
        // One generate; retry once with a stricter suffix if JSON parse fails.
        let mut sink = |_: &str| {};
        let raw = self.llm.generate(&prompt, 1024, &mut sink, abort).map_err(|e| e.to_string())?;
        progress(80);
        let analysis = parse_analysis(&raw, &input.lang).or_else(|_| {
            let strict = format!("{prompt}\n\nIMPORTANTE: responde ÚNICAMENTE el JSON, empezando por {{.");
            let raw2 = self.llm.generate(&strict, 1024, &mut |_| {}, abort).map_err(|e| e.to_string())?;
            parse_analysis(&raw2, &input.lang).map_err(|e| e.to_string())
        })?;
        progress(100);
        Ok(analysis)
    }
}
```

- [ ] **Step 4: Run → PASS.** `cargo test -p smart-noter-llm summarize` (2 parse tests green).

- [ ] **Step 5: Commit.**
```bash
cargo fmt   # from src-tauri/
git add src-tauri/crates/llm/src/summarize.rs
git commit -m "feat(llm): LocalSummarizer — template prompt + tolerant JSON parse (TDD)"
```

---

### Task 6: Migration 0006 + repo changes (summary write, source columns)

**Files:** Create `src-tauri/crates/db/migrations/0006_ai.sql`; Modify `meetings_repo.rs`, `decisions_repo.rs`, `blockers_repo.rs`, `actions_repo.rs`.

- [ ] **Step 1: `0006_ai.sql`.**
```sql
-- AI summary + chat (Sub-5).
ALTER TABLE meetings ADD COLUMN summarized_at TEXT;            -- NULL = never summarized
ALTER TABLE decisions ADD COLUMN source TEXT NOT NULL DEFAULT 'manual';  -- 'ai' | 'manual'
ALTER TABLE blockers  ADD COLUMN source TEXT NOT NULL DEFAULT 'manual';
ALTER TABLE actions   ADD COLUMN source TEXT NOT NULL DEFAULT 'manual';

CREATE TABLE chat_messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    meeting_id TEXT NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
    role TEXT NOT NULL,            -- 'user' | 'assistant'
    content TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_chat_meeting ON chat_messages(meeting_id);

CREATE TABLE transcript_embeddings (
    meeting_id TEXT NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
    chunk_idx INTEGER NOT NULL,
    text TEXT NOT NULL,
    vector BLOB NOT NULL,         -- f32 little-endian, length = dim
    PRIMARY KEY (meeting_id, chunk_idx)
);
```

- [ ] **Step 2: `meetings_repo::update_summary`** (mirror `update_title`, `meetings_repo.rs:172`):
```rust
pub async fn update_summary(pool: &SqlitePool, id: &str, summary: &Bilingual) -> Result<(), DbError> {
    sqlx::query("UPDATE meetings SET summary_es = ?, summary_en = ?, summarized_at = datetime('now') WHERE id = ?")
        .bind(&summary.es).bind(summary.en.as_deref()).bind(id)
        .execute(pool).await.map_err(DbError::from)?;
    Ok(())
}
```

- [ ] **Step 3: Add `source` to the create fns + a delete-AI fn** in `decisions_repo`/`blockers_repo`/`actions_repo`. For decisions (mirror for blockers; actions analogous with its extra cols):
```rust
pub async fn create_with_source(pool: &SqlitePool, meeting_id: &str, text_es: &str, source: &str) -> Result<i64, DbError> {
    let row = sqlx::query("INSERT INTO decisions (meeting_id, text_es, source) VALUES (?, ?, ?) RETURNING id")
        .bind(meeting_id).bind(text_es).bind(source).fetch_one(pool).await.map_err(DbError::from)?;
    Ok(row.get::<i64, _>("id"))
}
pub async fn delete_ai(pool: &SqlitePool, meeting_id: &str) -> Result<(), DbError> {
    sqlx::query("DELETE FROM decisions WHERE meeting_id = ? AND source = 'ai'")
        .bind(meeting_id).execute(pool).await.map_err(DbError::from)?;
    Ok(())
}
```
Keep the existing `create(...)` (defaults `source='manual'`) for the manual-CRUD path unchanged.

- [ ] **Step 4: Test** (the db crate has repo tests with a temp pool — follow the existing pattern in `decisions_repo.rs` tests): insert one `ai` + one `manual` decision, `delete_ai`, assert only the manual remains; `update_summary` then `get_detail` returns the new summary + non-null `summarized_at`.

- [ ] **Step 5: Run + commit.**
```bash
cargo test -p smart-noter-db --manifest-path "src-tauri/Cargo.toml"
cargo fmt   # from src-tauri/
git add src-tauri/crates/db
git commit -m "feat(db): migration 0006 (summarized_at, source cols, chat+embeddings tables) + update_summary/source repos"
```

---

### Task 7: `commands/ai.rs` — `run_summary` core + `generate_summary`/`cancel_summary`/`update_summary_text` + state

**Files:** Create `src-tauri/src/commands/ai.rs`; Modify `src-tauri/src/commands/mod.rs`, `src-tauri/src/lib.rs` (register), `src-tauri/src/state.rs` (LLM + handle slots). **Read `commands/transcription.rs` for the thread+emit+abort+`block_on` pattern and copy it.**

- [ ] **Step 1: `AppState`** gains: `pub llm: Mutex<Option<smart_noter_llm::engine::LocalLlm>>` (lazy) and `pub summary: Mutex<Option<SummaryHandle>>` where `SummaryHandle { meeting_id: String, abort: Arc<AtomicBool> }` (busy-guard like `transcription`). Add an `llm_download` handle slot too (mirror the whisper download slot).

- [ ] **Step 2: A helper to get/lazy-load the LLM** from the downloaded GGUF (errors with a clear "model not downloaded" if absent), reading `n_gpu_layers` from settings (default a conservative value).

- [ ] **Step 3: `run_summary(pool, app, meeting_id, llm)`** (shared by the command and the transcription chain). It: loads the transcript (`get_detail`) + template sections (`templates_repo`), builds `AnalysisInput`, runs `LocalSummarizer.analyze` (emitting `summary:progress {meetingId,pct}`), then `update_summary` + `delete_ai` + re-inserts decisions/blockers/actions with `source='ai'`, then chunks+embeds the transcript and persists (Task 10 provides the chunk/embed helpers — until then, leave a `// TODO(Task10): embed` and emit completed). Emits `summary:completed {meetingId}` or `summary:failed {meetingId,code,message}`. Runs on a `std::thread` with `block_on` for DB, exactly like transcription.

- [ ] **Step 4: Commands.**
```rust
#[tauri::command] #[specta::specta]
pub async fn generate_summary(state: State<'_, AppState>, app: AppHandle, meeting_id: String) -> Result<(), AppError> { /* busy-guard; spawn thread → run_summary */ }
#[tauri::command] #[specta::specta]
pub fn cancel_summary(state: State<'_, AppState>, meeting_id: String) -> Result<(), AppError> { /* set abort */ }
#[tauri::command] #[specta::specta]
pub async fn update_summary_text(state: State<'_, AppState>, meeting_id: String, summary: smart_noter_core::Bilingual) -> Result<(), AppError> { meetings_repo::update_summary(&state.pool, &meeting_id, &summary).await.map_err(from_db) }
#[tauri::command] #[specta::specta]
pub fn get_summary_state(state: State<'_, AppState>) -> Result<Option<String>, AppError> { /* Some(meeting_id) if a job is running, for FE re-hydration */ }
```

- [ ] **Step 5: Register** `pub mod ai;` in `mod.rs`; add all `commands::ai::*` to `collect_commands![...]` in `lib.rs`. Build the binary.

- [ ] **Step 6: Commit.**
```bash
cargo build -p smart-noter --manifest-path "src-tauri/Cargo.toml"   # LONG timeout
cargo fmt   # from src-tauri/
git add src-tauri/src/commands/ai.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs src-tauri/src/state.rs
git commit -m "feat(commands): ai.rs — run_summary core, generate/cancel/update-text/state commands"
```

---

### Task 8: Chain auto-summary after transcription

**Files:** Modify `src-tauri/src/commands/transcription.rs` (~line 326, after FTS upsert, before `transcription:completed`).

- [ ] **Step 1:** After the transcript is persisted + FTS upserted, read `settings_repo::get(&pool).await?.auto_generate_summary`; if true AND the LLM model is downloaded, call the same `run_summary(&pool, &app, &mid, llm)` used by the command (reuse, do not duplicate). Wrap in best-effort logging — a summary failure must NOT fail the transcription (log + emit `summary:failed`, but still emit `transcription:completed`).

- [ ] **Step 2:** Build the binary; commit.
```bash
cargo build -p smart-noter --manifest-path "src-tauri/Cargo.toml"
cargo fmt   # from src-tauri/
git add src-tauri/src/commands/transcription.rs
git commit -m "feat(transcription): auto-generate summary after transcription when enabled"
```

---

# Phase 3 — RAG chat (M4)

### Task 9: Chunking + cosine retrieval (pure, TDD)

**Files:** Modify `src-tauri/crates/llm/src/chat.rs`.

- [ ] **Step 1: Failing tests** for `chunk_transcript` and `top_k`:
```rust
#[test]
fn chunks_by_window() {
    let lines: Vec<(String,String)> = (0..10).map(|i| ("S1".into(), format!("línea {i}"))).collect();
    let chunks = chunk_transcript(&lines, 3); // 3 lines per chunk
    assert_eq!(chunks.len(), 4); // 3+3+3+1
    assert!(chunks[0].contains("línea 0") && chunks[0].contains("línea 2"));
}
#[test]
fn cosine_top_k_orders_by_similarity() {
    let q = vec![1.0, 0.0];
    let chunks = vec![
        Chunk { idx: 0, text: "a".into(), vector: vec![0.0, 1.0] },   // orthogonal
        Chunk { idx: 1, text: "b".into(), vector: vec![1.0, 0.0] },   // identical
    ];
    let top = top_k(&q, &chunks, 1);
    assert_eq!(top[0].idx, 1);
}
```

- [ ] **Step 2: Run → FAIL.**

- [ ] **Step 3: Implement.**
```rust
use smart_noter_core::models::ai::Chunk;

pub fn chunk_transcript(lines: &[(String, String)], per_chunk: usize) -> Vec<String> {
    lines.chunks(per_chunk.max(1))
        .map(|w| w.iter().map(|(s, t)| format!("{s}: {t}")).collect::<Vec<_>>().join("\n"))
        .collect()
}
fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if na == 0.0 || nb == 0.0 { 0.0 } else { dot / (na * nb) }
}
pub fn top_k<'a>(query: &[f32], chunks: &'a [Chunk], k: usize) -> Vec<&'a Chunk> {
    let mut scored: Vec<_> = chunks.iter().map(|c| (cosine(query, &c.vector), c)).collect();
    scored.sort_by(|a, b| b.0.total_cmp(&a.0));
    scored.into_iter().take(k).map(|(_, c)| c).collect()
}
```

- [ ] **Step 4: Run → PASS; commit.**
```bash
cargo test -p smart-noter-llm --manifest-path "src-tauri/Cargo.toml" chat
cargo fmt   # from src-tauri/
git add src-tauri/crates/llm/src/chat.rs
git commit -m "feat(llm): transcript chunking + cosine top-k retrieval (TDD)"
```

---

### Task 10: `LocalChat` + embeddings persistence wiring

**Files:** Modify `src-tauri/crates/llm/src/chat.rs`; Create `src-tauri/crates/db/src/repos/embeddings_repo.rs`, `chat_repo.rs`; fill the `// TODO(Task10)` in `run_summary`.

- [ ] **Step 1: `embeddings_repo`** — `upsert(pool, meeting_id, chunks: &[(i64, String, Vec<f32>)])` (encode `Vec<f32>` as little-endian bytes for the BLOB) and `load(pool, meeting_id) -> Vec<Chunk>` (decode). `chat_repo` — `insert(pool, meeting_id, role, content)` and `list(pool, meeting_id) -> Vec<ChatMessage>`.

- [ ] **Step 2: `LocalChat`** implements `ChatEngine`: `embed` delegates to `LocalLlm::embed`; `answer` builds a prompt from the question + retrieved chunks (`top_k`), generates with the `on_token` streaming closure (checking `abort`).
```rust
pub struct LocalChat<'a> { pub llm: &'a LocalLlm }
impl ChatEngine for LocalChat<'_> {
    fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, String> { self.llm.embed(texts).map_err(|e| e.to_string()) }
    fn answer(&self, q: &str, context: &[Chunk], lang: &str, on_token: &mut dyn FnMut(&str), abort: &AtomicBool) -> Result<(), String> {
        let ctx = context.iter().map(|c| c.text.as_str()).collect::<Vec<_>>().join("\n---\n");
        let prompt = format!(
"Responde en {lang} usando SOLO el contexto de la reunión. Si no está en el contexto, dilo.

Contexto:
{ctx}

Pregunta: {q}
Respuesta:");
        self.llm.generate(&prompt, 512, on_token, abort).map(|_| ()).map_err(|e| e.to_string())
    }
}
```

- [ ] **Step 3: Fill `run_summary`'s embed step** (Task 7): after persisting the summary, `chunk_transcript` → `LocalLlm::embed` → `embeddings_repo::upsert`.

- [ ] **Step 4: Test** `embeddings_repo` round-trip (encode/decode f32 BLOB) + `chat_repo` insert/list with a temp pool.

- [ ] **Step 5: Build + commit.**
```bash
cargo test -p smart-noter-db --manifest-path "src-tauri/Cargo.toml"
cargo build -p smart-noter-llm --manifest-path "src-tauri/Cargo.toml"
cargo fmt   # from src-tauri/
git add src-tauri/crates/db src-tauri/crates/llm/src/chat.rs src-tauri/src/commands/ai.rs
git commit -m "feat(llm,db): LocalChat (RAG answer) + embeddings/chat repos; wire embed into run_summary"
```

---

### Task 11: `ask_meeting` command with streamed answer

**Files:** Modify `src-tauri/src/commands/ai.rs`.

- [ ] **Step 1:** `ask_meeting(state, app, meeting_id, question)`: persist the user message (`chat_repo::insert`), load embeddings (`embeddings_repo::load`), embed the question, `top_k` (k=4), spawn a thread running `LocalChat::answer` with an `on_token` closure that `app.emit("chat:token", { meetingId, token })`; on completion emit `chat:done { meetingId }` and persist the assistant message; on error `chat:error { meetingId, message }`. If embeddings are empty (summary never generated), embed-on-demand first. Cancelable via an abort flag in a chat handle slot (reuse the pattern).

- [ ] **Step 1b:** Add `list_chat(state, meeting_id) -> Result<Vec<ChatMessage>, AppError>` (delegates to `chat_repo::list`) for the frontend to hydrate prior chat history.

- [ ] **Step 2:** Register both commands (`ask_meeting`, `list_chat`) in `collect_commands!`. Build + commit.
```bash
cargo build -p smart-noter --manifest-path "src-tauri/Cargo.toml"
cargo fmt   # from src-tauri/
git add src-tauri/src/commands/ai.rs src-tauri/src/lib.rs
git commit -m "feat(commands): ask_meeting — RAG chat with streamed tokens + persisted history"
```

---

# Phase 4 — Settings + Frontend (M5/M6)

### Task 12: `auto_generate_summary` setting

**Files:** Modify `src-tauri/crates/core/src/models/settings.rs` (read it first — copy the `identify_speakers` `default_true` pattern).

- [ ] **Step 1:** Add `#[serde(default = "default_true")] pub auto_generate_summary: bool` to `AppSettings`. No migration (JSON blob). Build the core crate + binary.
- [ ] **Step 2:** Commit.
```bash
cargo build -p smart-noter --manifest-path "src-tauri/Cargo.toml"
cargo fmt   # from src-tauri/
git add src-tauri/crates/core/src/models/settings.rs
git commit -m "feat(settings): auto_generate_summary toggle (default on)"
```

---

### Task 13: Regenerate bindings + i18n + RTK AI api

**Files:** regenerate `bindings.ts`; Modify `src/i18n/locales/{es,en}.json`; Create `src/store/api/ai.api.ts` (+ `ai.api.test.ts`).

- [ ] **Step 1:** `npm run generate:bindings` (DLL workaround if needed). Confirm `generateSummary`, `askMeeting`, `updateSummaryText`, `listLlmModels`, `downloadLlmModel`, `deleteLlmModel`, `getSummaryState` appear in `src/ipc/bindings.ts`.
- [ ] **Step 2:** Add i18n keys (es + en) for: `generateSummary`, `regenerate`, `summarizing`, `summaryEmpty`, `summaryFailed`, `editSummary`, `aiModel`, `downloadModel`, `autoSummary`, `chatPlaceholder`, `chatThinking`, `chatError`. `npm run generate:i18n-keys`; validate JSON.
- [ ] **Step 3:** RTK mutations in `ai.api.ts` (mirror `meetings.export.test.ts` hoisted-mock pattern for the test): `generateSummary({meetingId})`, `updateSummaryText({meetingId, summary})`, `askMeeting({meetingId, question})`, and the model commands. Test asserts `invoke('generate_summary', {meetingId})` etc.
- [ ] **Step 4:** `npm run test:run -- ai.api`; `npx tsc --noEmit`; commit (NOT bindings/keys — gitignored).
```bash
git add src/i18n/locales/es.json src/i18n/locales/en.json src/store/api/ai.api.ts src/store/api/ai.api.test.ts
git commit -m "feat(fe): AI RTK api + i18n keys for summary/chat/models"
```

---

### Task 14: SummaryTab — real, editable, regenerate

**Files:** Modify `src/features/meeting-detail/tabs/SummaryTab.tsx`; Create `src/features/meeting-detail/useAiSummary.ts` (copy `sub<T>` from `useTranscription.ts`).

- [ ] **Step 1:** `useAiSummary(meetingId)` hook subscribing to `summary:progress`/`completed`/`failed`, exposing `{ status, pct }`, re-hydrated via `getSummaryState`.
- [ ] **Step 2:** In `SummaryTab`, remove the `FAKE_*` constants. Render the real `meeting.summary` (already wired) with: an **edit** affordance (textarea → `updateSummaryText` on blur/save), a **Regenerar** button (calls `generateSummary`; if any decision/blocker/action has `source==='manual'`, confirm first), a **spinner** while `useAiSummary().status==='running'` (with pct), and an **empty state** ("Generar resumen" button) when `meeting.summary` is null. Decisions/blockers sections already render via `EditableItems` (Sub-4B) — unchanged.
- [ ] **Step 3:** `npx biome format --write`; `npm run test:run` + `npx tsc --noEmit`; commit.
```bash
git add src/features/meeting-detail/tabs/SummaryTab.tsx src/features/meeting-detail/useAiSummary.ts
git commit -m "feat(fe): SummaryTab — real/editable summary, Regenerar, empty+loading states"
```

---

### Task 15: AiChatPanel — wire to streamed `ask_meeting`

**Files:** Modify `src/features/meeting-detail/side/SidePanel.tsx`; Create `src/features/meeting-detail/useChatStream.ts`.

- [ ] **Step 1:** `useChatStream(meetingId)`: subscribes to `chat:token` (append to the in-progress assistant message), `chat:done` (finalize), `chat:error`; exposes `{ messages, ask(question), busy }` where `ask` calls `askMeeting` and seeds a user + empty assistant message; loads prior history via a `getChatHistory`/RTK query (add a small `list_chat` command, or reuse `get_meeting` if cheap — prefer a dedicated `list_chat(meetingId)` command).
- [ ] **Step 2:** In `SidePanel`'s chat block: enable the input + send button, render `messages` (replacing the hardcoded greeting/example), wire the suggested-question chips to `ask(t('suggestedQ1'))`, show a "thinking" indicator while `busy`.
- [ ] **Step 3:** biome/tsc/test; commit.
```bash
git add src/features/meeting-detail/side/SidePanel.tsx src/features/meeting-detail/useChatStream.ts
git commit -m "feat(fe): AiChatPanel — streamed RAG chat wired to ask_meeting"
```

> Note: this needs a `list_chat(meeting_id) -> Vec<ChatMessage>` command + binding. Add it to `commands/ai.rs` in Task 11's module (and re-run bindings in this task).

---

### Task 16: Configuración — auto toggle + AI model manager

**Files:** Modify the settings feature (find it: `src/features/settings/` — grep for where `auto_transcribe`/`autoTranscribe` and the Whisper models UI live; **mirror that Whisper-models component** for the LLM models).

- [ ] **Step 1:** Add a "Generar resumen automáticamente" toggle bound to `auto_generate_summary` via the existing settings get/update flow.
- [ ] **Step 2:** Add a "Modelo de IA" section listing `listLlmModels()` with download/delete + a progress bar driven by `llm-download:progress` events (copy the Whisper model-manager component + its event hook; swap command/event names to the `llm` ones).
- [ ] **Step 3:** biome/tsc/test; commit.
```bash
git add src/features/settings
git commit -m "feat(fe): Configuración — auto-summary toggle + AI (GGUF) model manager"
```

---

# Phase 5 — Verification + smoke (M7)

### Task 17: Full verification + real-app smoke

- [ ] **Step 1 — Backend:** `cargo test -p smart-noter-llm -p smart-noter-db -p smart-noter-core --manifest-path "src-tauri/Cargo.toml"` (all green; the `#[ignore]` model test stays ignored) + `cargo build -p smart-noter` (green).
- [ ] **Step 2 — Frontend:** `npm run test:run && npx tsc --noEmit && npm run lint` (`src/` clean).
- [ ] **Step 3 — Manual smoke** (`npm run tauri:dev` with the preamble; back up the `%APPDATA%` DB first):
  1. Configuración → download the Qwen + e5 models (progress works).
  2. Open a meeting with a transcript → **Generar resumen**: summary appears, decisions/blockers/actions populate (tagged `ai`), `summarized_at` set.
  3. **Edit** the summary text → persists. Add a **manual** decision → **Regenerar** → confirm dialog → manual decision preserved, `ai` ones replaced.
  4. Toggle **auto** on → transcribe a (short) meeting → summary auto-generates after completion.
  5. **Chat**: ask a question grounded in the transcript → answer **streams** token-by-token, history persists across reopen.
  6. Cancel mid-generation and mid-chat → neutral state, no crash.
  Restore the DB after.
- [ ] **Step 4:** Final fixup commit if needed.

---

## Notes for the executor

- **Build risk is front-loaded** (Task 1: llama.cpp via llama-cpp-2). If it won't compile, STOP — that's the gating risk, same as `mp3lame-encoder` in Sub-4C. Resolve the C/CMake toolchain (the env preamble provides cmake; CUDA/Vulkan features are optional — CPU build must work).
- **External-API drift:** the `llama-cpp-2` snippets (load/generate/embed) reflect the documented API but WILL need adjustment — verify against the installed crate source. The TESTS (JSON parse, chunking, cosine, repo round-trips) are the stable contracts; adapt the llama.cpp calls under them.
- **Reuse, don't duplicate:** model management mirrors `whisper::models`; the job thread/emit/abort mirrors `transcription.rs`; the FE event hooks copy `useTranscription`'s `sub<T>`; the model-manager UI copies the Whisper one. Read those first.
- **Summary failure is non-fatal to transcription** (Task 8) — always emit `transcription:completed`.
- **This is one large plan (M1→M7).** Ship the whole of Sub-5 on this branch; when it lands, the AI Summary + Chat roadmap item is complete and Sub-6 only adds cloud impls of the two traits.
```
