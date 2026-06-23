# Sub-4 — Persistence (completion) — Design

**Date:** 2026-06-22
**Status:** Approved (design)
**Roadmap item:** 4 — *Persistence* (size S originally; rescoped to M after discovery)

## Context

The roadmap describes Sub-4 as *"SQLite schema with migrations, CRUD for
meetings/participants/actions, audio file storage."* Discovery showed the
**core persistence is already done**, built incrementally inside Sub-2/3a/3b
because recording and transcription required durable storage:

| Area | State | Evidence |
|------|-------|----------|
| Schema + migrations (0001–0003) | DONE | FK on, CASCADE, indexes; `connection.rs` |
| Record → persist `meetings` + `meeting_assets` (audio) | DONE | `audio.rs` `finalize_recording`, transactional |
| Transcription → `transcript_lines` + `participants` + `talk_pct` | DONE | `transcript_repo.rs` `replace_lines` |
| `list_meetings` / `get_meeting` from real DB | DONE | RTK Query, no mock; `meetings.rs` |
| Participant CRUD (rename/merge/reassign/create) | DONE | from Sub-3b |

Sub-4 therefore = **closing the remaining CRUD and data-management gaps**, at
the **Complete + extras** scope chosen by the user:

1. Soft-delete (trash) for meetings, with disk cleanup on purge.
2. Full CRUD for actions / decisions / blockers.
3. Functional export (MP3 / Markdown / PDF) — wiring the existing `ExportModal`.
4. Full-text search (FTS5) over titles + summaries + transcripts.

Out of scope (belongs to other Subs): AI summary generation/editing (Sub-5,
which owns `summary_es/en`), API-key storage (Sub-6).

## Decisions (locked with user)

- **Export "audio" = real MP3 transcode** (not a copy of the WAV/FLAC).
- **Export delivery = native "Save as" dialog** (Tauri dialog plugin).
- **FTS scope = titles + summaries + transcript bodies** (a phrase spoken in a
  meeting must be findable). Requires a new search bar in the meetings list.
- **Delete = soft delete (trash)** with restore; hard purge removes the row
  (CASCADE) and the audio file from disk.

## New dependencies (3)

| Need | Choice | Why / rejected alternative |
|------|--------|----------------------------|
| MP3 encode | `mp3lame-encoder` (libmp3lame, vendored, builds with the C toolchain already required by sherpa/whisper) | No mature pure-Rust MP3 encoder; embedded ffmpeg is too heavy |
| PDF | `genpdf` (pure-Rust, over `printpdf`) | Headless-Chrome HTML→PDF is heavy and fragile |
| Save-as dialog | `tauri-plugin-dialog` (official) | Avoid hand-rolling a native picker |

The project already vendors native libs (sherpa/onnx DLLs) and compiles C via
its established LLVM/cmake preamble, so `mp3lame-encoder` does not introduce a
new class of build risk — but it is the highest-risk new dep and is sequenced
accordingly.

---

## Module A — Soft delete (trash)

**Schema.** Migration `0004_meeting_soft_delete.sql`:

```sql
ALTER TABLE meetings ADD COLUMN deleted_at TEXT;  -- nullable; NULL = active
CREATE INDEX idx_meetings_deleted ON meetings(deleted_at);
```

**Repo (`meetings_repo`).**
- `list_summaries` gains `WHERE deleted_at IS NULL` (active only).
- New `list_trashed` → trashed summaries, ordered by `deleted_at DESC`.
- `soft_delete(id)` → set `deleted_at = datetime('now')`.
- `restore(id)` → set `deleted_at = NULL`.
- `purge(id)` → hard `DELETE` (CASCADE wipes participants/actions/decisions/
  blockers/transcript_lines/meeting_assets/FTS row). Returns the audio asset
  path(s) so the command can delete the file(s) from disk.
- `list_purgeable(older_than)` → trashed rows with `deleted_at` older than a
  cutoff, for automatic purge.

**Commands.** `delete_meeting(id)`, `restore_meeting(id)`, `purge_meeting(id)`,
`list_trashed_meetings()`. `purge_meeting` deletes the row, then unlinks the
audio file(s) on disk (best-effort; a missing file is not an error).

**Startup purge.** A 30-day auto-purge sweep at boot, reusing the
`sweep_orphan_tmp_files` pattern in `lib.rs`: for each `list_purgeable(30d)`,
call the same purge path. Logged, non-fatal.

**Frontend.** "Delete" action on a meeting → confirm modal → `delete_meeting`
(moves to trash, list invalidates). New **Trash** view listing trashed meetings
with **Restore** and **Delete permanently** (calls `purge_meeting`, with its own
confirm).

**Why soft delete:** protects against accidental loss of an
expensive-to-reproduce recording; the audio file survives until explicit purge.

---

## Module B — CRUD for actions / decisions / blockers

Today only `toggle_action` exists; actions/decisions/blockers are otherwise
seed-only.

**Repos.**
- `actions_repo`: add `create`, `update` (text_es/text_en, owner_participant_id,
  due), `delete`. Keep existing `toggle` and `list_by_meeting`.
- New `decisions_repo` and `blockers_repo`: `list_by_meeting`, `create`,
  `update` (text_es/text_en), `delete`. (decisions/blockers tables already
  exist from `0001_init`.)

**Commands (≈9 new).**
- Actions: `create_action`, `update_action`, `delete_action` (+ existing
  `toggle_action`).
- Decisions: `create_decision`, `update_decision`, `delete_decision`.
- Blockers: `create_blocker`, `update_blocker`, `delete_blocker`.

IDs: actions use TEXT ids (match existing); decisions/blockers use the existing
`INTEGER PRIMARY KEY AUTOINCREMENT` — create commands return the new id.

**Frontend.** Inline editing in the meeting-detail view: add / edit / delete an
item per list; actions additionally edit owner (participant picker) and due
date. RTK Query mutations invalidate `get_meeting`.

---

## Module C — Export (MP3 / Markdown / PDF)

**Command.** `export_meeting(meeting_id, formats: Fmt[], opts)` where
`opts = { timestamps: bool, bilingual: bool, file_name: string }` and
`Fmt = 'mp3' | 'md' | 'pdf'`.

**Delivery.** Native save dialog (`tauri-plugin-dialog`):
- Single format → "Save as" pre-filled with `file_name` + extension.
- Multiple formats → "Select folder"; files written as `file_name.{ext}`.

**Generators** (new `export` module/crate, one function per format, each
independently testable):
- **Markdown** — template: title, date, participants, summary, decisions,
  blockers, actions, then transcript. `timestamps` toggles the `[mm:ss]` prefix
  per line; `bilingual` emits the `_en` text alongside `_es` when present.
- **PDF** — `genpdf`, same content model as Markdown rendered to a paginated
  document (heading styles, a participants table, transcript paragraphs).
- **MP3** — read the source `.wav`/`.flac` (path from `meeting_assets`,
  kind=audio) via `hound`/`symphonia` into PCM, encode to MP3 with
  `mp3lame-encoder` (fixed sensible bitrate, e.g. 128 kbps mono/stereo per
  source channels). `timestamps`/`bilingual` do not apply to audio.

**Frontend.** Wire the existing `ExportModal`: replace the "Próximamente"
no-op with a call to `export_meeting`; relabel the audio row to the real MP3
output; show progress/success/error.

**Data source.** All text comes from `get_meeting` content already in the DB —
no recomputation.

---

## Module D — Full-text search (FTS5)

**Schema.** Migration `0005_meeting_fts.sql`:

```sql
CREATE VIRTUAL TABLE meeting_search USING fts5(
    meeting_id UNINDEXED,
    title,
    summary,
    body                         -- transcript lines concatenated
);
```

One row per meeting. `body` is the concatenation of that meeting's
`transcript_lines.text_es` (and `_en` when present).

**Maintenance** (explicit repo calls, not triggers — writes are already
batched and centralized):
- After `transcribe_meeting` persists lines → upsert the meeting's FTS row.
- On `update_meeting_title` and any future summary write → refresh title/summary.
- On `purge_meeting` → delete the FTS row. (Soft-deleted meetings stay indexed
  but are filtered out of results by joining on `deleted_at IS NULL`.)
- A one-time backfill in migration/bootstrap populates rows for existing
  meetings (covers seeded data and any pre-existing recordings).

**Command.** `search_meetings(query)` → list of `{ meeting_summary, snippet }`,
where `snippet` is FTS5 `snippet()` highlighting the match. Results filtered to
active (non-trashed) meetings.

**Frontend.** New search bar in the meetings list; typing queries
`search_meetings` (debounced) and renders matches with the highlighted snippet;
empty query falls back to `list_meetings`.

**Risk to verify in the plan:** confirm the bundled `sqlx`/`libsqlite3-sys`
build has FTS5 compiled in (`SQLITE_ENABLE_FTS5`). If not, enable the feature
or fall back to a `LIKE`-based search over a denormalized text column.

---

## Cross-cutting concerns

**Migrations.** Two new, additive, backward-compatible: `0004` (soft delete)
and `0005` (FTS). CRUD for actions/decisions/blockers and Export need no
migration (their tables already exist). Both run via the existing migration
runner; `migration.rs` test extended to assert the new objects exist.

**Bindings + i18n.** Per the project's session rule, regenerate `bindings.ts`
(specta) after the new commands, and add the i18n keys for new UI strings
(trash view, search placeholder, export labels, CRUD controls).

**Testing.**
- Repo-level: soft-delete/restore/purge transitions; CRUD round-trips for
  actions/decisions/blockers; FTS upsert + query returns expected meetings;
  purge deletes the audio file.
- Export: each generator produces a non-empty, well-formed artifact for a
  fixture meeting; MP3 decodes back to ~same duration.
- Migration test covers `0004`/`0005` objects.
- Frontend: trash flow, inline CRUD, export modal call, search bar.

**Error handling.** All commands return `AppError`. Purge file-unlink is
best-effort (missing file logged, not fatal). Export surfaces dialog-cancelled
as a no-op (not an error). FTS query errors degrade to the plain list.

## Sequencing (for the plan)

Lowest-risk first, native-build risk isolated:

1. **A — Soft delete** (schema + repo + commands + trash UI).
2. **B — CRUD actions/decisions/blockers** (repos + commands + inline UI).
3. **D — FTS5** (verify FTS5 availability early; migration + maintenance + search UI).
4. **C — Export** (MD first, then PDF, then MP3 — MP3 last because it carries
   the `mp3lame-encoder` native-build risk).

Each phase ends green (tests + bindings + i18n) before the next.

## Non-goals

- AI summary generation or editing (Sub-5).
- Cloud sync / providers / API-key storage (Sub-6).
- Bundling DLLs into the MSI (deferred to Sub-8; tracked separately).
- Per-line "jump to transcript match" from search results (FTS returns the
  meeting + snippet; deep-linking to the exact line is a later nicety).
