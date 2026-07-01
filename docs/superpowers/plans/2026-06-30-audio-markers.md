# Audio Markers Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give each meeting a typed, timestamped list of audio markers — AI-generated (decisions/actions/blockers anchored to when they were said, plus "highlights") produced with the summary, and manual ones the user drops at the current playback position — clickable to seek the recording.

**Architecture:** One `markers` table (kind + source), populated by the summary analysis (source=ai) and by manual commands (source=manual). The AI flow extends the existing shared `core::ai_prompt` so local + all cloud providers get it in one change. The Audio tab reads a single `list_markers` command.

**Tech Stack:** Rust/Tauri 2, sqlx/SQLite, specta bindings, React/TS + RTK Query, vitest.

Spec: `docs/superpowers/specs/2026-06-30-audio-markers-design.md`.

**Conventions to follow:**
- Backend commands run from `src-tauri/`; env preamble on every cargo command: `export PATH="$HOME/.cargo/bin:/c/Program Files (x86)/Microsoft Visual Studio/2022/BuildTools/Common7/IDE/CommonExtensions/Microsoft/CMake/CMake/bin:$PATH"` and `export LIBCLANG_PATH="C:/Program Files/LLVM/bin"`.
- Regenerate bindings from **inside `src-tauri/`**: `cargo run --bin specta-export` (path is CWD-relative). bindings.ts is gitignored.
- After a new DB table, `crates/db/tests/migration.rs` exact-list assert MUST include it (alphabetical).

---

## File Structure

- `crates/db/migrations/0008_markers.sql` — new `markers` table.
- `crates/core/src/models/marker.rs` — `Marker` IPC type + `MarkerKind`/source as strings. (new)
- `crates/core/src/models/ai.rs` — extend `ExtractedAction` (+`t_seconds`), `MeetingAnalysis` (decisions/blockers → `MarkedItem`, +`highlights`), add `MarkedItem`, `Highlight`.
- `crates/core/src/traits.rs` — `AnalysisInput.transcript` gains the timestamp.
- `crates/core/src/ai_prompt.rs` — prompt text + `RawAnalysis`/`RawAction` (+`t`, +`highlights`) + `parse_analysis`.
- `crates/db/src/repos/markers_repo.rs` — CRUD + `replace_ai`. (new)
- `crates/db/src/repos/transcript_lines_repo.rs` — add `list_with_time` (or reuse). Check if the repo exists; else add a query in ai.rs.
- `src/commands/markers.rs` — `list_markers`/`create_marker`/`update_marker`/`delete_marker`. (new)
- `src/commands/ai.rs` — build timestamped transcript; after persisting items, populate markers.
- `src/lib.rs` — register 4 marker commands + `pub mod markers`.
- `src/store/api/markers.api.ts` — RTK Query. (new)
- `src/features/meeting-detail/tabs/AudioTab.tsx` (+ `.module.css`) — Markers section.

---

## Task 1: markers table + migration test

**Files:**
- Create: `src-tauri/crates/db/migrations/0008_markers.sql`
- Modify: `src-tauri/crates/db/tests/migration.rs:17-31`

- [ ] **Step 1: Write the migration**

```sql
-- Audio markers (manual + AI-anchored). kind: decision|action|blocker|highlight|manual
CREATE TABLE markers (
    id         TEXT PRIMARY KEY,
    meeting_id TEXT NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
    t_seconds  INTEGER NOT NULL,
    kind       TEXT NOT NULL,
    label      TEXT NOT NULL,
    source     TEXT NOT NULL,   -- 'ai' | 'manual'
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_markers_meeting ON markers(meeting_id);
```

- [ ] **Step 2: Add `"markers"` to the expected-tables assert** (alphabetical, between `"decisions"` and `"meeting_assets"`) in `migration.rs`.

- [ ] **Step 3: Run the migration test — expect PASS**

Run (from `src-tauri/`): `cargo test -p smart-noter-db migration_creates_expected_tables`
Expected: PASS (table list now matches).

- [ ] **Step 4: Commit**

```bash
git add src-tauri/crates/db/migrations/0008_markers.sql src-tauri/crates/db/tests/migration.rs
git commit -m "feat(db): markers table (migration 0008)"
```

---

## Task 2: Marker core model

**Files:**
- Create: `src-tauri/crates/core/src/models/marker.rs`
- Modify: `src-tauri/crates/core/src/models/mod.rs` (add `pub mod marker;` + re-export)

- [ ] **Step 1: Write the model**

```rust
use serde::{Deserialize, Serialize};
use specta::Type;

/// An audio marker: a timestamped, typed point in a meeting's recording.
/// `kind` = decision|action|blocker|highlight|manual; `source` = ai|manual.
#[derive(Debug, Clone, Type, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Marker {
    pub id: String,
    pub meeting_id: String,
    pub t_seconds: i64,
    pub kind: String,
    pub label: String,
    pub source: String,
    pub created_at: String,
}
```

- [ ] **Step 2: Export it** — add `pub mod marker;` and `pub use marker::Marker;` in `models/mod.rs` following the existing pattern (check how `MeetingAsset` is re-exported).

- [ ] **Step 3: Compile** — `cargo check -p smart-noter-core`. Expected: OK.

- [ ] **Step 4: Commit** — `git commit -am "feat(core): Marker model"`

---

## Task 3: markers_repo (CRUD + replace_ai)

**Files:**
- Create: `src-tauri/crates/db/src/repos/markers_repo.rs`
- Modify: `src-tauri/crates/db/src/repos/mod.rs` (add `pub mod markers_repo;`)
- Test: same file, `#[cfg(test)] mod tests`

- [ ] **Step 1: Write the failing tests** (mirror `meeting_assets_repo.rs` test setup — `in_memory_pool` + `ensure_schema` + a `seed_meeting` helper).

```rust
#[tokio::test]
async fn create_list_and_delete() {
    let pool = in_memory_pool().await.unwrap();
    ensure_schema(&pool).await.unwrap();
    seed_meeting(&pool, "m-1").await;
    let repo = MarkersRepo(&pool);
    let m = repo.create("m-1", 84, "manual", "Nota", "manual").await.unwrap();
    let all = repo.list_by_meeting("m-1").await.unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].t_seconds, 84);
    repo.delete(&m.id).await.unwrap();
    assert!(repo.list_by_meeting("m-1").await.unwrap().is_empty());
}

#[tokio::test]
async fn replace_ai_preserves_manual() {
    let pool = in_memory_pool().await.unwrap();
    ensure_schema(&pool).await.unwrap();
    seed_meeting(&pool, "m-1").await;
    let repo = MarkersRepo(&pool);
    repo.create("m-1", 10, "manual", "mine", "manual").await.unwrap();
    repo.replace_ai("m-1", &[(20, "decision".into(), "D1".into())]).await.unwrap();
    let all = repo.list_by_meeting("m-1").await.unwrap();
    assert_eq!(all.len(), 2); // manual kept + 1 ai
    // replacing ai again drops the old ai, keeps manual
    repo.replace_ai("m-1", &[]).await.unwrap();
    let all = repo.list_by_meeting("m-1").await.unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].source, "manual");
}

#[tokio::test]
async fn list_orders_by_time() {
    let pool = in_memory_pool().await.unwrap();
    ensure_schema(&pool).await.unwrap();
    seed_meeting(&pool, "m-1").await;
    let repo = MarkersRepo(&pool);
    repo.create("m-1", 50, "manual", "b", "manual").await.unwrap();
    repo.create("m-1", 10, "manual", "a", "manual").await.unwrap();
    let all = repo.list_by_meeting("m-1").await.unwrap();
    assert_eq!(all[0].t_seconds, 10);
    assert_eq!(all[1].t_seconds, 50);
}
```

- [ ] **Step 2: Run — expect FAIL** (`MarkersRepo` undefined).

- [ ] **Step 3: Implement the repo** (id via `uuid::Uuid::now_v7` string; created_at via `chrono::Utc::now().to_rfc3339()`).

```rust
use smart_noter_core::{AppError, Marker};
use sqlx::SqlitePool;

pub struct MarkersRepo<'a>(pub &'a SqlitePool);

impl MarkersRepo<'_> {
    pub async fn list_by_meeting(&self, meeting_id: &str) -> Result<Vec<Marker>, AppError> {
        let rows = sqlx::query_as::<_, (String, String, i64, String, String, String, String)>(
            "SELECT id, meeting_id, t_seconds, kind, label, source, created_at
             FROM markers WHERE meeting_id = ? ORDER BY t_seconds",
        )
        .bind(meeting_id)
        .fetch_all(self.0)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(rows.into_iter().map(|(id, meeting_id, t_seconds, kind, label, source, created_at)| Marker {
            id, meeting_id, t_seconds, kind, label, source, created_at,
        }).collect())
    }

    pub async fn create(&self, meeting_id: &str, t_seconds: i64, kind: &str, label: &str, source: &str) -> Result<Marker, AppError> {
        let id = uuid::Uuid::now_v7().to_string();
        let created_at = chrono::Utc::now().to_rfc3339();
        sqlx::query("INSERT INTO markers (id, meeting_id, t_seconds, kind, label, source, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)")
            .bind(&id).bind(meeting_id).bind(t_seconds).bind(kind).bind(label).bind(source).bind(&created_at)
            .execute(self.0).await.map_err(|e| AppError::Database(e.to_string()))?;
        Ok(Marker { id, meeting_id: meeting_id.into(), t_seconds, kind: kind.into(), label: label.into(), source: source.into(), created_at })
    }

    pub async fn update_label(&self, id: &str, label: &str) -> Result<(), AppError> {
        sqlx::query("UPDATE markers SET label = ? WHERE id = ?")
            .bind(label).bind(id).execute(self.0).await.map_err(|e| AppError::Database(e.to_string()))?;
        Ok(())
    }

    pub async fn delete(&self, id: &str) -> Result<(), AppError> {
        sqlx::query("DELETE FROM markers WHERE id = ?").bind(id).execute(self.0).await.map_err(|e| AppError::Database(e.to_string()))?;
        Ok(())
    }

    /// Replace all source='ai' markers for a meeting with the given (t, kind, label) triples.
    pub async fn replace_ai(&self, meeting_id: &str, items: &[(i64, String, String)]) -> Result<(), AppError> {
        sqlx::query("DELETE FROM markers WHERE meeting_id = ? AND source = 'ai'")
            .bind(meeting_id).execute(self.0).await.map_err(|e| AppError::Database(e.to_string()))?;
        for (t, kind, label) in items {
            self.create(meeting_id, *t, kind, label, "ai").await?;
        }
        Ok(())
    }
}
```

- [ ] **Step 4: Run — expect PASS.** `cargo test -p smart-noter-db markers_repo`

- [ ] **Step 5: Commit** — `git commit -am "feat(db): markers_repo (CRUD + replace_ai)"`

---

## Task 4: Manual marker commands + registration

**Files:**
- Create: `src-tauri/src/commands/markers.rs`
- Modify: `src-tauri/src/commands/mod.rs` (`pub mod markers;`), `src-tauri/src/lib.rs` (register 4 commands)

- [ ] **Step 1: Write the commands** (follow `commands/settings.rs` for `State<AppState>` + `from_db` mapping).

```rust
use crate::error::from_db;
use crate::state::AppState;
use smart_noter_core::{AppError, Marker};
use smart_noter_db::repos::markers_repo::MarkersRepo;
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn list_markers(state: State<'_, AppState>, meeting_id: String) -> Result<Vec<Marker>, AppError> {
    MarkersRepo(&state.pool).list_by_meeting(&meeting_id).await.map_err(from_db_app)
}

#[tauri::command]
#[specta::specta]
pub async fn create_marker(state: State<'_, AppState>, meeting_id: String, t_seconds: i64, label: String) -> Result<Marker, AppError> {
    MarkersRepo(&state.pool).create(&meeting_id, t_seconds, "manual", &label, "manual").await.map_err(from_db_app)
}

#[tauri::command]
#[specta::specta]
pub async fn update_marker(state: State<'_, AppState>, id: String, label: String) -> Result<(), AppError> {
    MarkersRepo(&state.pool).update_label(&id, &label).await.map_err(from_db_app)
}

#[tauri::command]
#[specta::specta]
pub async fn delete_marker(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    MarkersRepo(&state.pool).delete(&id).await.map_err(from_db_app)
}
```

Note: `MarkersRepo` already returns `AppError`, so no `from_db` needed — drop the `.map_err` and return directly (verify: `MarkersRepo` methods return `Result<_, AppError>`). Use the plain form:
```rust
pub async fn list_markers(state: State<'_, AppState>, meeting_id: String) -> Result<Vec<Marker>, AppError> {
    MarkersRepo(&state.pool).list_by_meeting(&meeting_id).await
}
```
(and likewise for the others).

- [ ] **Step 2: Register** — add `pub mod markers;` in `commands/mod.rs` and these 4 lines in `lib.rs` `collect_commands!` (after the `settings` group):

```rust
        commands::markers::list_markers,
        commands::markers::create_marker,
        commands::markers::update_marker,
        commands::markers::delete_marker,
```

- [ ] **Step 3: Compile** — `cargo check -p smart-noter`. Expected: OK.

- [ ] **Step 4: Commit** — `git commit -am "feat(markers): manual marker commands"`

---

## Task 5: Extend the analysis model (core)

**Files:**
- Modify: `src-tauri/crates/core/src/models/ai.rs`

- [ ] **Step 1: Add `t_seconds` to `ExtractedAction`; add `MarkedItem` + `Highlight`; extend `MeetingAnalysis`.**

```rust
#[derive(Debug, Clone, Type, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractedAction {
    pub text: String,
    pub owner_hint: Option<String>,
    pub due: Option<String>,
    pub t_seconds: Option<u32>,   // NEW: audio anchor, None if the LLM didn't give one
}

/// A summary item (decision/blocker) with an optional audio anchor.
#[derive(Debug, Clone)]
pub struct MarkedItem {
    pub text: String,
    pub t_seconds: Option<u32>,
}

/// A key moment the LLM flagged that isn't already a decision/action/blocker.
#[derive(Debug, Clone)]
pub struct Highlight {
    pub label: String,
    pub t_seconds: u32,
}

#[derive(Debug, Clone)]
pub struct MeetingAnalysis {
    pub summary: Bilingual,
    pub decisions: Vec<MarkedItem>,
    pub blockers: Vec<MarkedItem>,
    pub actions: Vec<ExtractedAction>,
    pub highlights: Vec<Highlight>,
}
```
Update `Default` for `MeetingAnalysis` (add `highlights: Vec::new()`; decisions/blockers stay `Vec::new()`).

- [ ] **Step 2: Compile** — `cargo check -p smart-noter-core`. Expect errors in `ai_prompt.rs` (fixed in Task 6) — that's fine; this task only defines the types. Do NOT commit yet (broken build); Task 6 completes it.

Note: combine Tasks 5+6 into one commit since the build only compiles after both.

---

## Task 6: Prompt + parse_analysis (t + highlights)

**Files:**
- Modify: `src-tauri/crates/core/src/ai_prompt.rs`

- [ ] **Step 1: Write the failing tests** (replace the existing `parse_analysis` tests' expectations for the new shape; add timestamp/highlight cases).

```rust
#[test]
fn parses_items_with_timestamps_and_highlights() {
    let raw = r#"{"summary":"S.","decisions":[{"text":"D1","t":84}],
      "blockers":[],"actions":[{"text":"A1","owner":"Ana","due":null,"t":185}],
      "highlights":[{"label":"Arranque","t":12}]}"#;
    let a = parse_analysis(raw, "es").unwrap();
    assert_eq!(a.decisions[0].text, "D1");
    assert_eq!(a.decisions[0].t_seconds, Some(84));
    assert_eq!(a.actions[0].t_seconds, Some(185));
    assert_eq!(a.highlights[0].label, "Arranque");
    assert_eq!(a.highlights[0].t_seconds, 12);
}

#[test]
fn tolerates_missing_t_and_highlights() {
    let raw = r#"{"summary":"S.","decisions":[{"text":"D1"}],"actions":[{"text":"A1"}]}"#;
    let a = parse_analysis(raw, "es").unwrap();
    assert_eq!(a.decisions[0].t_seconds, None);
    assert_eq!(a.actions[0].t_seconds, None);
    assert!(a.highlights.is_empty());
}
```

- [ ] **Step 2: Run — expect FAIL** (compile errors / shape mismatch).

- [ ] **Step 3: Update the serde types, the prompt, and the mapping.**

`RawAnalysis`/`RawAction` (decisions/blockers become objects; add `t`, `highlights`):
```rust
#[derive(Deserialize)]
struct RawAnalysis {
    summary: String,
    #[serde(default)] decisions: Vec<RawItem>,
    #[serde(default)] blockers: Vec<RawItem>,
    #[serde(default)] actions: Vec<RawAction>,
    #[serde(default)] highlights: Vec<RawHighlight>,
}
#[derive(Deserialize)]
struct RawItem { text: String, #[serde(default)] t: Option<u32> }
#[derive(Deserialize)]
struct RawAction { text: String, #[serde(default)] owner: Option<String>, #[serde(default)] due: Option<String>, #[serde(default)] t: Option<u32> }
#[derive(Deserialize)]
struct RawHighlight { label: String, t: u32 }
```

`build_messages` — render timestamps and ask for `t` + `highlights`:
```rust
let body: String = input
    .transcript
    .iter()
    .map(|(t, s, txt)| format!("[{:02}:{:02}] {s}: {txt}", t / 60, t % 60))
    .collect::<Vec<_>>()
    .join("\n");
```
System prompt: extend the keys description to
`"decisions"/"blockers" (array de objetos {{"text":..,"t":<segundos>}}), "actions" (array de objetos {{"text":..,"owner":..|null,"due":..|null,"t":<segundos>}}), "highlights" (array de 3-5 objetos {{"label":..,"t":<segundos>}} con momentos clave no cubiertos por lo anterior). El campo "t" es el segundo del audio donde ocurre, tomado de los [mm:ss] de la transcripción.`

`parse_analysis` mapping:
```rust
Ok(MeetingAnalysis {
    summary,
    decisions: r.decisions.into_iter().map(|i| MarkedItem { text: i.text, t_seconds: i.t }).collect(),
    blockers: r.blockers.into_iter().map(|i| MarkedItem { text: i.text, t_seconds: i.t }).collect(),
    actions: r.actions.into_iter().map(|a| ExtractedAction { text: a.text, owner_hint: a.owner, due: a.due, t_seconds: a.t }).collect(),
    highlights: r.highlights.into_iter().map(|h| Highlight { label: h.label, t_seconds: h.t }).collect(),
})
```
Import `MarkedItem`, `Highlight` from `crate::models::ai`.

- [ ] **Step 4: Fix `AnalysisInput.transcript` type** in `traits.rs`: `pub transcript: Vec<(u32, String, String)>` (t_seconds, speaker, text). Update its doc comment.

- [ ] **Step 5: Run — expect PASS.** `cargo test -p smart-noter-core`

- [ ] **Step 6: Commit (Tasks 5+6 together)** — `git commit -am "feat(core): timestamped analysis items + highlights in the AI prompt"`

---

## Task 7: ai.rs — timestamped input + populate markers

**Files:**
- Modify: `src-tauri/src/commands/ai.rs` (transcript build ~231-242; input ~267; persist ~334-362)

- [ ] **Step 1: Build the transcript with timestamps.** Replace the `transcript_pairs` build so each entry is `(t_seconds, label, text)`. Source the seconds from `line` — the `TranscriptLine` exposes only a display `t`; instead query `transcript_lines` directly for `(t_seconds, speaker_id, text_es)` and map speaker labels through `label_map` (same as now). Add a small query near the top of the persist section, or a `transcript_lines_repo::list_with_time(&pool, &meeting_id) -> Vec<(u32, String, String)>`. Then:

```rust
let transcript_pairs: Vec<(u32, String, String)> = lines_with_time
    .into_iter()
    .map(|(t, sid, text)| {
        let label = label_map.get(&sid).cloned().unwrap_or(sid);
        (t, label, text)
    })
    .collect();
```
`AnalysisInput { transcript: transcript_pairs, template_sections, lang }` (type now matches).

- [ ] **Step 2: Populate markers after persisting items.** In the `persist_result` block, after the actions loop, add (clamp `t` to the meeting duration; `detail.duration_sec` is available):

```rust
use smart_noter_db::repos::markers_repo::MarkersRepo;
let dur = detail.duration_sec.max(0) as u32;
let clamp = |t: u32| t.min(dur) as i64;
let mut mk: Vec<(i64, String, String)> = Vec::new();
for d in &analysis.decisions { if let Some(t) = d.t_seconds { mk.push((clamp(t), "decision".into(), d.text.clone())); } }
for b in &analysis.blockers  { if let Some(t) = b.t_seconds { mk.push((clamp(t), "blocker".into(),  b.text.clone())); } }
for a in &analysis.actions   { if let Some(t) = a.t_seconds { mk.push((clamp(t), "action".into(),   a.text.clone())); } }
for h in &analysis.highlights { mk.push((clamp(h.t_seconds), "highlight".into(), h.label.clone())); }
MarkersRepo(&pool).replace_ai(&meeting_id, &mk).await?;
```
Keep the existing decisions/blockers/actions persistence using `.text` (they now come off `MarkedItem`/`ExtractedAction`, so `for d in &analysis.decisions { decisions_repo::create_with_source(&pool, &meeting_id, &d.text, "ai").await?; }`).

- [ ] **Step 3: Update `LocalSummarizer` construction of any `MeetingAnalysis`/`AnalysisInput` in tests** (search `crates/llm` + `crates/providers` for `AnalysisInput {` and `MeetingAnalysis {` literals; add the new fields). Run `cargo build --workspace --tests` and fix each compile error.

- [ ] **Step 4: Run backend suite — expect PASS.** `cargo test --workspace`

- [ ] **Step 5: Commit** — `git commit -am "feat(markers): populate AI markers from the summary analysis"`

---

## Task 8: Frontend markers API

**Files:**
- Create: `src/store/api/markers.api.ts`

- [ ] **Step 1: Write the RTK Query slice** (mirror `templates.api.ts` — `baseApi.injectEndpoints`, `tauriBaseQuery` shape `{ cmd, args }`, tag `'Marker'`; register `'Marker'` in the base api `tagTypes` if not present).

```ts
import type { Marker } from '@/ipc/bindings';
import { baseApi } from './base';

export const markersApi = baseApi.injectEndpoints({
  endpoints: (b) => ({
    listMarkers: b.query<Marker[], string>({
      query: (meetingId) => ({ cmd: 'list_markers', args: { meetingId } }),
      providesTags: ['Marker'],
    }),
    createMarker: b.mutation<Marker, { meetingId: string; tSeconds: number; label: string }>({
      query: (a) => ({ cmd: 'create_marker', args: a }),
      invalidatesTags: ['Marker'],
    }),
    updateMarker: b.mutation<void, { id: string; label: string }>({
      query: (a) => ({ cmd: 'update_marker', args: a }),
      invalidatesTags: ['Marker'],
    }),
    deleteMarker: b.mutation<void, string>({
      query: (id) => ({ cmd: 'delete_marker', args: { id } }),
      invalidatesTags: ['Marker'],
    }),
  }),
});

export const { useListMarkersQuery, useCreateMarkerMutation, useUpdateMarkerMutation, useDeleteMarkerMutation } = markersApi;
```

- [ ] **Step 2: tsc** — `pnpm exec tsc -b --noEmit`. Expected: OK (needs bindings from Task 7's regen — run `cargo run --bin specta-export` from `src-tauri/` first).

- [ ] **Step 3: Commit** — `git commit -am "feat(markers): frontend RTK API"`

---

## Task 9: AudioTab Markers section

**Files:**
- Modify: `src/features/meeting-detail/tabs/AudioTab.tsx` (+ `.module.css`)

- [ ] **Step 1: Add the tests** (`AudioTab.test.tsx` — new file; mock `useListMarkersQuery` to return two markers, assert both render with their type label; mock `invoke('get_meeting_audio')`).

```tsx
// renders markers with type chips, sorted; "Marcar aquí" calls createMarker; clicking a time seeks.
```
(Write concrete assertions: `screen.getByText('Decisión')`, `screen.getByText('D1')`, a click on the time row calls the audio ref's currentTime setter — spy on it.)

- [ ] **Step 2: Run — expect FAIL.**

- [ ] **Step 3: Implement the Markers section** below the player card. Use `useListMarkersQuery(meeting.id)`, `useCreateMarkerMutation`, `useDeleteMarkerMutation`. A `KIND_META` map gives `{ label, icon, color }` per kind (decision/action/blocker/highlight/manual). Each row: chip + `<button>` time (calls the same seek helper used by the waveform, seeking to `t_seconds`), the label, delete button. Header row has a **"Marcar aquí"** button that reads the `<audio>` current time (`Math.floor(audioRef.current?.currentTime ?? 0)`) and calls `createMarker({ meetingId: meeting.id, tSeconds, label: '' })`. Empty state when the list is empty.

- [ ] **Step 4: Run — expect PASS.** `pnpm exec vitest run AudioTab`

- [ ] **Step 5: Commit** — `git commit -am "feat(markers): Audio tab markers UI"`

---

## Task 10: Regenerate bindings, full verify, smoke

- [ ] **Step 1:** From `src-tauri/`: `cargo run --bin specta-export` (regenerates bindings with `Marker` + the 4 commands).
- [ ] **Step 2:** `cargo test --workspace` (backend) and `pnpm exec vitest run` + `pnpm exec tsc -b --noEmit` (frontend) — all green.
- [ ] **Step 3:** `cargo clippy --workspace --all-targets -- -D warnings` — clean.
- [ ] **Step 4: Real-app smoke** (release or with `identifySpeakers`/`autoTranscribe` off to dodge the debug whisper/sherpa crash): regenerate a summary on a transcribed meeting → confirm AI markers appear in the Audio tab with correct type chips and that clicking one seeks the player. Drop a manual marker with "Marcar aquí", edit/delete it. Regenerate the summary again → manual marker survives, AI ones refresh.
- [ ] **Step 5: Commit** any smoke fixes.

---

## Self-review notes (author)

- **Spec coverage:** table (T1), model (T2), repo+replace_ai (T3), manual commands (T4), analysis model (T5), prompt+parse (T6), populate + timestamped input (T7), FE api (T8), UI (T9), verify (T10) — all spec sections mapped.
- **Timestamp source:** the `TranscriptLine` IPC type only has a display `t`; T7 sources seconds from `transcript_lines` directly (do not parse the display string).
- **Build ordering:** T5 leaves the build red on purpose; T6 finishes it — commit them together (noted in T5/T6).
- **Type consistency:** `MarkedItem { text, t_seconds }`, `Highlight { label, t_seconds }`, `ExtractedAction.t_seconds: Option<u32>`, `AnalysisInput.transcript: Vec<(u32,String,String)>`, `Marker { …, t_seconds: i64, kind, source }`, commands `list_markers/create_marker(meetingId,tSeconds,label)/update_marker(id,label)/delete_marker(id)` — consistent across tasks and the FE api.
