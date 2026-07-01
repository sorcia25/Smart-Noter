# Audio Markers — Design Spec

**Date:** 2026-06-30
**Status:** Approved (pending spec review)

## Goal

Give each meeting a list of **audio markers** — timestamped points the user can click to jump to that moment in the recording. Markers come from two sources: **AI** (generated with the summary — decisions, actions, blockers, and "highlights", each anchored to when it was said) and **manual** (the user drops a marker at the current playback position). Every marker carries a **type label** so the UI can distinguish them.

This replaces the fabricated, hardcoded markers that were removed from the Audio tab.

## Non-goals (YAGNI)

- No separate "detect markers" analysis pass — markers are produced by the existing summary analysis.
- No editing of AI markers' text as first-class (they are regenerated); only manual markers are editable. Any marker can be deleted.
- No sub-second precision. AI timestamps are approximate (navigation, not surgery).
- No backfill of existing meetings — old summaries get markers only when their summary is regenerated.

## 1. Data model

New table (migration **0008**):

```sql
CREATE TABLE markers (
    id         TEXT PRIMARY KEY,
    meeting_id TEXT NOT NULL,
    t_seconds  INTEGER NOT NULL,
    kind       TEXT NOT NULL,   -- decision | action | blocker | highlight | manual
    label      TEXT NOT NULL,
    source     TEXT NOT NULL,   -- ai | manual
    created_at TEXT NOT NULL,
    FOREIGN KEY (meeting_id) REFERENCES meetings(id) ON DELETE CASCADE
);
CREATE INDEX idx_markers_meeting ON markers(meeting_id);
```

- Core model `Marker { id, meeting_id, t_seconds, kind, label, source, created_at }` (`camelCase` in bindings).
- **`crates/db/tests/migration.rs` exact-table-list assertion MUST add `markers`** (alphabetical) — the known trap when adding a table.
- The existing `decisions/blockers/actions` tables are unchanged (they still feed the Summary). Markers of those kinds **replicate** the text + timestamp for audio navigation. Chosen over adding `t_seconds` to the 3 tables so the Audio tab reads **one** source and every marker has a `kind`. The duplication is acceptable: Summary and Audio are distinct views, regenerated together.

## 2. AI flow (extend the existing analysis)

**Input.** `AnalysisInput.transcript` changes from `Vec<(String, String)>` to carry the timestamp — `Vec<(u32, String, String)>` (`t_seconds, speaker, text`). `build_messages` renders each line as `[mm:ss] speaker: text`. `ai.rs::run_summary` already reads `TranscriptLine` (which has the time) when building the input.

**Prompt.** The system prompt is extended to require a `t` (integer seconds, taken from the transcript timestamps) on each decision/blocker/action, plus a new `highlights` array (3–5 key moments not already covered by a decision/action/blocker):

```json
{
  "summary": "…",
  "decisions": [{"text": "…", "t": 84}],
  "blockers":  [{"text": "…", "t": 102}],
  "actions":   [{"text": "…", "owner": "…"|null, "due": "…"|null, "t": 185}],
  "highlights":[{"label": "…", "t": 45}]
}
```

Note: `decisions`/`blockers` go from `[string]` to `[{text, t}]`. Only new analyses use the new schema; already-persisted summaries are not re-parsed.

**Parsing.** `parse_analysis` captures the `t` on each item and the `highlights`. `MeetingAnalysis` gains the per-item `t_seconds: Option<u32>` and `highlights: Vec<Highlight { label, t_seconds }>`. `RawAnalysis`/`RawAction` gain `t` (with `#[serde(default)]` so a missing `t` parses).

**Persistence.** After `analyze()`, in addition to writing decisions/blockers/actions to their tables (using `.text` as today), populate `markers`:
1. `DELETE FROM markers WHERE meeting_id = ? AND source = 'ai'` (preserves manual markers).
2. For each decision/blocker/action **with a valid `t`** → insert a marker (`kind` = decision/action/blocker, `label` = text, `source='ai'`).
3. For each highlight → insert a marker (`kind='highlight'`).

Shared by **local + all 3 cloud adapters** via `core::ai_prompt` — one change covers every provider.

## 3. Manual markers

Commands:
- `list_markers(meeting_id) -> Vec<Marker>` — ordered by `t_seconds`.
- `create_marker(meeting_id, t_seconds, label) -> Marker` — `source='manual'`, `kind='manual'`.
- `update_marker(id, label)` — manual only.
- `delete_marker(id)` — any marker (AI markers reappear on regenerate).

## 4. UI (Audio tab)

Restore the **Markers** section below the player, now real:
- List from `list_markers`, ordered by time. Each row: a **type chip** (color + icon per kind), the **time** (click → `seekToFraction`/set `currentTime`), the label, and a delete button (edit for manual).
- A **"Marcar aquí"** button captures the player's current second → `create_marker` with an inline-editable label (default empty / "Marcador").
- Empty state when there are no markers.

## 5. Reliability / edge cases

- Backend clamps each AI `t` to `[0, duration_sec]`; an item with a missing/invalid `t` produces **no** marker (it still appears in the Summary).
- Regenerating a summary replaces `source='ai'` markers and preserves `source='manual'`.
- Old meetings show only manual markers until their summary is regenerated.

## 6. Files touched

- **Migration** `crates/db/migrations/0008_markers.sql`; `crates/db/tests/migration.rs` (table list).
- **Core** `models/marker.rs` (new), `models/ai.rs` (`MeetingAnalysis`, `Highlight`, `ExtractedAction.t_seconds`), `ai_prompt.rs` (prompt + `RawAnalysis`/parse), `traits.rs` (`AnalysisInput.transcript`).
- **DB** `repos/markers_repo.rs` (new: list/create/update/delete + `replace_ai(meeting_id, markers)`).
- **Backend** `commands/markers.rs` (new commands), `commands/ai.rs` (build timestamped input + populate markers after analyze), `lib.rs` (register commands).
- **Frontend** `features/meeting-detail/tabs/AudioTab.tsx` (+ `.module.css`), `store/api/markers.api.ts` (new), bindings regenerated.

## 7. Testing

- `ai_prompt::parse_analysis`: item with `t`, missing `t`, invalid `t`, highlights present/absent, back-compat of the tolerant JSON extraction.
- `markers_repo`: CRUD, `replace_ai` preserves manual, cascade delete with meeting.
- `list_markers`: ordering by time.
- FE (vitest): AudioTab renders markers with type chips, "Marcar aquí" creates one, clicking a time seeks, delete removes.
