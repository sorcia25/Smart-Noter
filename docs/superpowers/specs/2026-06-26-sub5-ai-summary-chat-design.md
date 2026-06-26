# Sub-5 — AI Summary + Chat (Design)

> **Status:** DESIGN — approved in brainstorming 2026-06-26. Roadmap item 5
> ("AI Summary + Chat", size L). Built on top of Sub-4 (Persistence, complete).

## 1. Goal & Scope

Give every transcribed meeting an **AI-generated, template-aware summary** plus
**extracted decisions / blockers / actions**, and a **RAG chat** ("Pregúntale a
la reunión") grounded in the transcript — all running on a **local LLM by
default** (llama.cpp), with the door open for cloud providers in Sub-6.

**In scope (one cohesive Sub-5):**
1. Local LLM infrastructure (new `crates/llm`, llama.cpp, GGUF model download/management).
2. Template-based summary generation → writes `meetings.summary_es/en`.
3. Extraction of decisions / blockers / actions from the transcript (tagged `source='ai'`).
4. Editable summary (user can hand-edit the generated text; the roadmap's "summary edit").
5. RAG chat over the transcript: embeddings → cosine retrieval → streamed LLM answer, with persisted history.
6. Frontend wiring: SummaryTab (real + editable + regenerate), AiChatPanel (real, streaming), Configuración (auto toggle + LLM model management).

**Out of scope (deferred):**
- **Cloud LLM providers + encrypted API keys → Sub-6.** The `Summarizer`/`ChatEngine` traits are introduced now so Sub-6 only adds a second impl.
- Vector-store engines (faiss/qdrant) — transcripts are short-to-medium; cosine-in-Rust is enough.
- Multi-meeting / cross-meeting chat.
- Streaming the *summary* generation token-by-token (summary just emits coarse progress; only **chat** streams tokens).

## 2. Locked Decisions (from brainstorming)

| Decision | Choice |
|---|---|
| Engine | **Trait + local-first.** New `Summarizer` + `ChatEngine` traits in `core`; local impl now, cloud impl plugs in at Sub-6. |
| Scope | **All three** (summary + extraction + chat) in one spec; the plan sequences them by risk. |
| Trigger | **Auto after transcription (configurable via a Settings toggle) + manual "Regenerar" button.** |
| LLM library | **llama.cpp** (GGUF, GPU optional), mirroring how `whisper-rs` wraps whisper.cpp. |
| LLM model | **Qwen2.5-3B-Instruct** GGUF Q4 (~2 GB), strong Spanish; downloadable like Whisper models. |
| Embeddings | A small dedicated embeddings model (**multilingual-e5-small / bge-small**, ~150 MB GGUF) run via llama.cpp embedding mode. |
| Vector store | **None.** Persist chunk embeddings in SQLite; cosine similarity + top-k in Rust. |
| Chat | **Streamed** answer (Tauri token events) + persisted history. |
| AI vs manual items | New **`source ('ai'|'manual')`** column on decisions/blockers/actions; **regenerate replaces only `ai` rows**, preserving manual edits. |

## 3. Verified Integration Facts (checked 2026-06-26 — trust these)

- **No Rust traits exist yet.** Whisper is a free fn: `whisper::transcribe(pcm, model_path, opts, progress: impl FnMut(u32), abort: Arc<AtomicBool>) -> Result<Vec<Segment>>` (`crates/whisper/src/transcribe.rs:87`). `Summarizer`/`ChatEngine` will be the first traits; their **execution style mirrors this** (spawned thread, progress/token closures, `Arc<AtomicBool>` abort).
- **Transcription job** (`src-tauri/src/commands/transcription.rs`): `transcribe_meeting(state, app, meeting_id, speaker_count_hint)` spawns a **`std::thread`** (not tokio), uses `tauri::async_runtime::block_on` for DB. Emits camelCase events: `transcription:progress {meetingId,pct}`, `:completed {meetingId,lineCount,wordCount}` (line 341), `:failed {meetingId,code,message}`, `:cancelled {meetingId}`. **Chaining hook: after the FTS upsert (~line 326) / before `transcription:completed`** is where auto-summary runs. Cancel via `Arc<AtomicBool>` (`cancel_transcription`, line 370).
- **Whisper model mgmt** (`crates/whisper/src/models.rs`): `ModelSpec {id, display_name, size_mb, sha256, url}`, `ModelStatus {..., downloaded}`, `download(app_data, id, progress: impl FnMut(u32,u64,u64), is_cancelled) -> Result<()>` (streams via `ureq`, SHA-256 verify, atomic rename). Stored at `app_data/models/ggml-{id}.bin`. Events `whisper-download:{progress,completed,failed}`. **Reuse this whole pattern for GGUF LLM models.**
- **Repos** (`crates/db/src/repos/`): `decisions_repo::create(pool, meeting_id, text_es) -> i64`; `blockers_repo::create(...) -> i64`; `actions_repo::create(pool, meeting_id, text_es, owner_participant_id: Option<&str>, due: Option<&str>) -> String`. Tables have only `text_es/text_en` (no `source`). `ALTER TABLE … ADD COLUMN source TEXT DEFAULT 'manual'` is a clean SQLite migration (no backfill).
- **Summary write is missing.** `get_detail` reads `summary_es/en → Option<Bilingual>` (`meetings_repo.rs:147`); `create_with_asset` is insert-only. **Add `meetings_repo::update_summary(pool, id, &Bilingual)`** mirroring `update_title` (line 172).
- **Settings** are a single JSON blob (`settings` table, key=`app`): `settings_repo::get/upsert`, commands `get_settings`/`update_settings`, struct `AppSettings` (`core/src/models/settings.rs`, camelCase, has `run_local`, `auto_transcribe`, `identify_speakers`). **Add `auto_generate_summary: bool` with `#[serde(default = "default_true")]` — no migration** (schemaless JSON).
- **Frontend event subscribe** (`src/features/meeting-detail/useTranscription.ts`): a `sub<T>(ev, cb)` helper over `listen<T>` from `@tauri-apps/api/event`, cleaned up on unmount, re-hydrated via a `get_*_state` command. **Copy this for `useAiSummary` and `useChatStream`.**
- **Native build**: `whisper-rs = "0.16"` drives CMake automatically; no custom `build.rs`, no GPU feature flags declared. `crates/llm` mirrors this with a llama.cpp binding crate; declare optional GPU features (`cuda`, `vulkan`) there. Needs the LLVM/cmake env preamble (same as whisper/sherpa — see [[reference_whisper_build_toolchain]]).

## 4. Architecture

```
core/                     # + traits + AI types (FIRST traits in the project)
  models/                 #   MeetingAnalysis, ExtractedItem, ChatAnswer, AppSettings(+auto_generate_summary)
  traits/                 #   Summarizer, ChatEngine        ← new
crates/llm/  (NEW)        # local llama.cpp impl of both traits + embeddings + GGUF model mgmt
crates/providers/         # cloud impl of the same traits   ← Sub-6 (untouched now)
src-tauri/src/commands/
  ai.rs       (NEW)       # generate_summary, cancel_summary, ask_meeting, update_summary_text,
                          # list_llm_models, download_llm_model, delete_llm_model, get_summary_state
```

**Trait shape (conceptual — execution mirrors `whisper::transcribe`):**
```rust
// core/traits
pub trait Summarizer: Send + Sync {
    fn analyze(&self, input: &AnalysisInput, progress: &mut dyn FnMut(u32),
               abort: &AtomicBool) -> Result<MeetingAnalysis, AiError>;
}
pub trait ChatEngine: Send + Sync {
    fn answer(&self, q: &str, context: &[Chunk], on_token: &mut dyn FnMut(&str),
              abort: &AtomicBool) -> Result<(), AiError>;
    fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, AiError>;
}
```
- `AnalysisInput` = transcript lines + the meeting's template `sections` + target language.
- `MeetingAnalysis` = `{ summary: Bilingual, decisions: Vec<String>, blockers: Vec<String>, actions: Vec<ExtractedAction> }`, parsed from the model's **structured JSON** output (with a tolerant parser + one retry on malformed JSON).
- The local impl (`crates/llm`) loads the GGUF once, held in `AppState` behind a `Mutex<Option<LocalLlm>>` (lazy-loaded on first use). A small **backend factory** picks the impl: Sub-5 always returns local; Sub-6 returns cloud when `run_local=false` + key present.

## 5. Module Breakdown (the plan will sequence these)

**M1 — LLM infra (`crates/llm`):** llama.cpp binding + CMake build (prove it compiles, like Whisper Task 1 did). GGUF model catalog/download/list/delete mirroring `whisper::models` (events `llm-download:*`, stored in `app_data/models/`). Embeddings via llama.cpp embedding mode. A second `DownloadHandle` slot in `AppState`.

**M2 — Summarizer (local) + generation flow:** `Summarizer` trait + local impl. A shared core fn `run_summary(pool, app, meeting_id, llm)` that: loads transcript → builds a template-aware prompt → runs the model (emitting `summary:progress`) → parses JSON → `update_summary` + replaces `source='ai'` decisions/blockers/actions (preserving `manual`) → emits `summary:completed`. Called by **both** the `generate_summary` command (Regenerar) **and** the transcription thread when `auto_generate_summary` is on. Cancel via `cancel_summary` (`Arc<AtomicBool>`).

**M3 — Extraction wiring:** migration adds `source` to decisions/blockers/actions; repos gain a `source` param (or `create_ai` variants) and a `delete_ai_items(meeting_id)` used before re-inserting. Frontend EditableItems already render these (Sub-4B) — items just gain a source tag; manual CRUD unchanged.

**M4 — RAG chat:** migration `transcript_embeddings (meeting_id, chunk_idx, text, vector BLOB)` + `chat_messages (id, meeting_id, role, content, created_at)`. On summary generation (or first chat), chunk the transcript and embed → persist. `ask_meeting(meeting_id, question)`: embed question → cosine top-k vs chunks → prompt with context → stream tokens (`chat:token` / `chat:done` / `chat:error`) → persist both messages. `embeddings_repo`, `chat_repo`.

**M5 — Persistence glue:** migration 0006 (the two new tables + `source` columns + `meetings.summarized_at`). `meetings_repo::update_summary`. Settings `auto_generate_summary` (+ `default_true`).

**M6 — Frontend:** `SummaryTab` — real summary, **editable textarea** persisting via `update_summary_text`, **Regenerar** button (confirm if manual edits exist), empty state "Generar resumen", generation spinner; drop the `FAKE_*` placeholders. `AiChatPanel` — wire the input → `ask_meeting`, render streamed tokens + persisted history, suggested-question chips fire real questions. `Configuración` — "Generar resumen automáticamente" toggle + "Modelo de IA" download/delete section (reuse the Whisper-models UI). Hooks `useAiSummary` / `useChatStream` (copy `sub<T>` from `useTranscription`). i18n (es/en); regenerate `bindings.ts`/`keys.ts`.

**M7 — Verify + smoke:** crate tests (JSON parse, chunking, cosine top-k — no model needed; one `#[ignore]` real-model integration test). Command tests with **mock** `Summarizer`/`ChatEngine` (the trait makes this trivial). FE tests. Real-app smoke: transcribe → auto summary → edit → regenerate (manual preserved) → chat with streaming → model download UI. Back up + restore the `%APPDATA%` DB.

## 6. Data Flow

```
Transcription thread ──(completed, FTS upsert)──▶ if auto_generate_summary:
    run_summary(): load transcript → prompt(template sections) → LLM (summary:progress)
                 → parse JSON → update_summary + replace source='ai' items
                 → chunk + embed transcript → persist embeddings → summary:completed
Regenerar button ─────────────────────────────▶ generate_summary command → run_summary()
User asks in chat ────────────────────────────▶ ask_meeting: embed q → cosine top-k chunks
                 → prompt(context) → stream chat:token… → chat:done → persist chat_messages
```

## 7. Risks & Mitigations

- **llama.cpp native build** is the highest risk (C/CMake, like whisper/sherpa). M1 builds it before any feature code — same front-load strategy that worked for Sub-4C's `mp3lame-encoder`. GPU features optional; CPU fallback always.
- **Generation latency** (a 3B model on CPU can take 10–60 s for a long transcript). Mitigation: coarse `summary:progress`, cancelable, auto is configurable-off, and it runs off the UI thread (spawned thread like transcription).
- **Malformed JSON from the LLM.** Mitigation: a tolerant extractor (find the JSON block) + one re-prompt; on persistent failure, save the raw summary text and leave extraction empty (non-fatal), surfacing a soft warning.
- **Model size / first-run UX.** ~2 GB LLM + ~150 MB embeddings download. Mitigation: reuse the Whisper download UX (progress, resumable-ish, SHA verify); summary/chat features show an empty state + "Descargar modelo" until present.
- **Bilingual.** Whisper transcribes in one language; the summary is generated in the transcript's language (fills `summary_es` for Spanish meetings; `en` left null unless the transcript is English). Bilingual generation is not a goal for Sub-5.

## 8. Decomposition note

Sub-5 is large (L). This spec is intentionally one cohesive design, but the
**implementation plan will phase it M1→M7 by build/risk order** (LLM infra first,
chat RAG last), each phase green before the next — the same approach that carried
Sub-4C. If M4 (RAG) proves heavier than estimated during planning, it can split
into its own follow-up plan without redesign.
