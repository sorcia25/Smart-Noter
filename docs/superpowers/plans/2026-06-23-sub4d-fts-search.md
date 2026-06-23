# Sub-4D — Full-Text Search (FTS5) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let a user search their meetings by title, summary, and transcript content from the meetings-list search bar, getting matching meetings with a highlighted snippet of where the term appeared.

**Architecture:** A standalone FTS5 virtual table `meeting_search(meeting_id UNINDEXED, title, summary, body)` with one row per meeting (`body` = that meeting's transcript lines concatenated). A `search_repo` upserts/deletes rows and runs `MATCH` queries with `snippet()`. The index is maintained explicitly: upserted after `transcribe_meeting` persists lines and after `update_meeting_title`, deleted on `purge_meeting`, and backfilled for existing meetings at startup. Search results filter out trashed meetings (`deleted_at IS NULL`) and respect the active template chip. The bar runs backend search (debounced) when there's a query and falls back to the normal list when empty.

**Tech Stack:** Rust + sqlx (SQLite, FTS5 confirmed available + `snippet()`), Tauri commands + specta, React + RTK Query, vitest/RTL.

**Conventions (same as Sub-4A/B):**
- UNCHECKED sqlx queries only for new code (no `query!`). Build/test with the LLVM/cmake preamble.
- Run `cargo fmt` AND `npx biome format --write` on touched files before each commit (lefthook blocks on format).
- The `no-hardcoded-strings` hook false-positives on `=> Promise<…>` arrow types — use method-signature syntax in interfaces.
- Generated `bindings.ts` / `keys.ts` are gitignored — regenerate, never commit.
- FTS5 is confirmed compiled into the bundled sqlite (verified 2026-06-23). No `LIKE` fallback needed.

---

## Phase 1 — Schema + search repo

### Task 1: Migration 0005 — `meeting_search` FTS5 table

**Files:**
- Create: `src-tauri/crates/db/migrations/0005_meeting_fts.sql`
- Test: `src-tauri/crates/db/tests/migration.rs`

- [ ] **Step 1: Write the failing test**

Add to `migration.rs`:

```rust
#[tokio::test]
async fn migration_0005_creates_fts_table() {
    let pool = init_pool_in_memory().await.expect("pool");
    // The FTS5 virtual table is usable: insert + MATCH must work.
    sqlx::query("INSERT INTO meeting_search (meeting_id, title, summary, body) VALUES ('m1','Hola','S','cuerpo buscable')")
        .execute(&pool).await.expect("insert into fts");
    let n: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM meeting_search WHERE meeting_search MATCH 'buscable'")
        .fetch_one(&pool).await.expect("match query");
    assert_eq!(n.0, 1);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p smart-noter-db migration_0005_creates_fts_table`
Expected: FAIL — `no such table: meeting_search`.

- [ ] **Step 3: Create the migration**

Create `src-tauri/crates/db/migrations/0005_meeting_fts.sql`:

```sql
-- Full-text search over meetings: one row per meeting. `meeting_id` is stored
-- but not indexed; title/summary/body are tokenized. `body` holds the meeting's
-- transcript lines concatenated. Maintained explicitly by search_repo (after
-- transcription, title edits) and deleted on purge; trashed meetings stay
-- indexed but are filtered out of results by joining on meetings.deleted_at.
CREATE VIRTUAL TABLE meeting_search USING fts5(
    meeting_id UNINDEXED,
    title,
    summary,
    body
);
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p smart-noter-db migration_0005_creates_fts_table`
Expected: PASS.

> NOTE: the existing `migration_creates_expected_tables` test asserts an exact list of `type='table'` names. An FTS5 virtual table also creates shadow tables (`meeting_search_data`, `_idx`, `_content`, `_docsize`, `_config`) AND the virtual table itself shows as `type='table'`. Update that test's expected vector to include `meeting_search` (and exclude the shadow tables: extend its `WHERE name NOT LIKE 'meeting_search_%'`). Adjust the query+vector so it stays green.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/crates/db/migrations/0005_meeting_fts.sql src-tauri/crates/db/tests/migration.rs
git commit -m "feat(db): migration 0005 meeting_search FTS5 table"
```

---

### Task 2: `search_repo` — upsert / delete / search / backfill

**Files:**
- Create: `src-tauri/crates/db/src/repos/search_repo.rs`
- Modify: `src-tauri/crates/db/src/repos/mod.rs` (`pub mod search_repo;`)
- Modify: `src-tauri/crates/core/src/models/` — add a `SearchHit` model (see Step 3)

- [ ] **Step 1: Add the `SearchHit` core model**

Create `src-tauri/crates/core/src/models/search.rs`:

```rust
use super::MeetingSummary;
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Type, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub meeting: MeetingSummary,
    /// Snippet of the best-matching column, with matches wrapped between the
    /// markers \u{2068} (start) and \u{2069} (end) for the frontend to highlight.
    pub snippet: String,
}
```

Export it in `src-tauri/crates/core/src/models/mod.rs`: `pub mod search;` + `pub use search::SearchHit;`.

- [ ] **Step 2: Write the failing tests**

Create `src-tauri/crates/db/src/repos/search_repo.rs` with impl + tests (new file, impl+test together):

```rust
use crate::repos::meetings_repo;
use crate::DbError;
use smart_noter_core::models::SearchHit;
use sqlx::SqlitePool;

const MARK_START: char = '\u{2068}';
const MARK_END: char = '\u{2069}';

/// Turn a user's raw query into a safe FTS5 MATCH expression: each
/// whitespace-separated token becomes a quoted prefix term, so `arq sist`
/// matches `arquitectura sistema`. Quoting neutralizes FTS5 operators
/// (AND/OR/NEAR/"/* etc.) so arbitrary input can't be a syntax error.
fn to_match_expr(query: &str) -> String {
    query
        .split_whitespace()
        .map(|tok| {
            let escaped = tok.replace('"', "");
            format!("\"{escaped}\"*")
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Rebuild the FTS row for one meeting from its current title/summary/transcript.
pub async fn upsert_meeting(pool: &SqlitePool, meeting_id: &str) -> Result<(), DbError> {
    let meta: Option<(String, Option<String>, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT title_es, title_en, summary_es, summary_en FROM meetings WHERE id = ?",
    )
    .bind(meeting_id)
    .fetch_optional(pool)
    .await?;
    let Some((title_es, title_en, summary_es, summary_en)) = meta else {
        return Ok(());
    };

    let lines: Vec<(String, Option<String>)> =
        sqlx::query_as("SELECT text_es, text_en FROM transcript_lines WHERE meeting_id = ? ORDER BY t_seconds")
            .bind(meeting_id)
            .fetch_all(pool)
            .await?;

    let title = [Some(title_es), title_en].into_iter().flatten().collect::<Vec<_>>().join(" ");
    let summary = [summary_es, summary_en].into_iter().flatten().collect::<Vec<_>>().join(" ");
    let body = lines
        .into_iter()
        .flat_map(|(es, en)| [Some(es), en])
        .flatten()
        .collect::<Vec<_>>()
        .join(" ");

    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM meeting_search WHERE meeting_id = ?")
        .bind(meeting_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO meeting_search (meeting_id, title, summary, body) VALUES (?, ?, ?, ?)")
        .bind(meeting_id)
        .bind(&title)
        .bind(&summary)
        .bind(&body)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

pub async fn delete_meeting(pool: &SqlitePool, meeting_id: &str) -> Result<(), DbError> {
    sqlx::query("DELETE FROM meeting_search WHERE meeting_id = ?")
        .bind(meeting_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Search non-trashed meetings, optionally filtered to a template.
pub async fn search(
    pool: &SqlitePool,
    query: &str,
    template: Option<&str>,
) -> Result<Vec<SearchHit>, DbError> {
    if query.trim().is_empty() {
        return Ok(vec![]);
    }
    let expr = to_match_expr(query);
    let mark_start = MARK_START.to_string();
    let mark_end = MARK_END.to_string();

    // Pull matching meeting ids + snippet, newest first, filtering trashed +
    // (optionally) template. `rank` could order by relevance; date keeps it
    // consistent with the normal list.
    // Numbered params (?1..?4) so the template bind (?4) can be referenced twice
    // without binding it twice, and so it can't collide with the MATCH expr.
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT ms.meeting_id, snippet(meeting_search, -1, ?1, ?2, '…', 12) \
         FROM meeting_search ms \
         JOIN meetings m ON m.id = ms.meeting_id \
         WHERE meeting_search MATCH ?3 AND m.deleted_at IS NULL \
           AND (?4 IS NULL OR m.template_id = ?4) \
         ORDER BY m.date DESC",
    )
    .bind(&mark_start)
    .bind(&mark_end)
    .bind(&expr)
    .bind(template)
    .fetch_all(pool)
    .await?;

    let mut hits = Vec::with_capacity(rows.len());
    for (meeting_id, snippet) in rows {
        let meeting = meetings_repo::summary_by_id(pool, &meeting_id).await?;
        if let Some(meeting) = meeting {
            hits.push(SearchHit { meeting, snippet });
        }
    }
    Ok(hits)
}

/// Populate the index for every meeting that has no FTS row yet (existing rows
/// from before this feature). Idempotent; best-effort per meeting.
pub async fn backfill(pool: &SqlitePool) -> Result<(), DbError> {
    let ids: Vec<String> = sqlx::query_scalar(
        "SELECT id FROM meetings WHERE id NOT IN (SELECT meeting_id FROM meeting_search)",
    )
    .fetch_all(pool)
    .await?;
    for id in ids {
        upsert_meeting(pool, &id).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::init_pool_in_memory;

    async fn seed_meeting(pool: &SqlitePool, id: &str, title: &str) {
        sqlx::query("INSERT INTO meetings (id, title_es, template_id, date, duration_sec) VALUES (?, ?, 'tecnica', '2026-06-01', 1)")
            .bind(id).bind(title).execute(pool).await.unwrap();
    }

    #[tokio::test]
    async fn upsert_then_search_finds_by_title_and_body() {
        let pool = init_pool_in_memory().await.unwrap();
        seed_meeting(&pool, "m1", "Revisión de arquitectura").await;
        sqlx::query("INSERT INTO transcript_lines (meeting_id, t_seconds, t_display, text_es) VALUES ('m1', 0, '0:00', 'hablamos del despliegue en kubernetes')")
            .execute(&pool).await.unwrap();
        upsert_meeting(&pool, "m1").await.unwrap();

        let by_title = search(&pool, "arquitectura", None).await.unwrap();
        assert_eq!(by_title.len(), 1);
        assert_eq!(by_title[0].meeting.id, "m1");

        let by_body = search(&pool, "kubernetes", None).await.unwrap();
        assert_eq!(by_body.len(), 1);
        assert!(by_body[0].snippet.contains('\u{2068}'), "snippet should mark the match");

        let prefix = search(&pool, "kube", None).await.unwrap();
        assert_eq!(prefix.len(), 1, "prefix search should match");
    }

    #[tokio::test]
    async fn search_excludes_trashed_and_respects_template() {
        let pool = init_pool_in_memory().await.unwrap();
        seed_meeting(&pool, "m1", "alpha tecnica").await;
        sqlx::query("INSERT INTO meetings (id, title_es, template_id, date, duration_sec) VALUES ('m2','alpha ejecutiva','ejecutiva','2026-06-01',1)")
            .execute(&pool).await.unwrap();
        upsert_meeting(&pool, "m1").await.unwrap();
        upsert_meeting(&pool, "m2").await.unwrap();

        assert_eq!(search(&pool, "alpha", None).await.unwrap().len(), 2);
        assert_eq!(search(&pool, "alpha", Some("tecnica")).await.unwrap().len(), 1);

        // Trash m1 -> drops out of results.
        sqlx::query("UPDATE meetings SET deleted_at = datetime('now') WHERE id = 'm1'")
            .execute(&pool).await.unwrap();
        assert_eq!(search(&pool, "alpha", None).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn weird_query_is_not_a_syntax_error() {
        let pool = init_pool_in_memory().await.unwrap();
        seed_meeting(&pool, "m1", "normal title").await;
        upsert_meeting(&pool, "m1").await.unwrap();
        // FTS5 operators / quotes in raw input must not error.
        assert!(search(&pool, "a\"b OR (", None).await.is_ok());
    }

    #[tokio::test]
    async fn delete_removes_from_index() {
        let pool = init_pool_in_memory().await.unwrap();
        seed_meeting(&pool, "m1", "deletable").await;
        upsert_meeting(&pool, "m1").await.unwrap();
        delete_meeting(&pool, "m1").await.unwrap();
        assert_eq!(search(&pool, "deletable", None).await.unwrap().len(), 0);
    }
}
```

Add `pub mod search_repo;` to `repos/mod.rs`.

- [ ] **Step 3: Add `meetings_repo::summary_by_id` (helper used by search)**

`search` hydrates each hit to a `MeetingSummary`. Add to `meetings_repo.rs` a single-row variant of the summary builder:

```rust
pub async fn summary_by_id(
    pool: &SqlitePool,
    id: &str,
) -> Result<Option<MeetingSummary>, DbError> {
    let row = sqlx::query_as::<_, (String, String, Option<String>, String, String, i64, i64)>(
        "SELECT id, title_es, title_en, template_id, date, duration_sec, word_count \
         FROM meetings WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    let Some((id, title_es, title_en, template, date, duration_sec, word_count)) = row else {
        return Ok(None);
    };
    let participants = participants_repo::list_by_meeting(pool, &id).await?;
    Ok(Some(MeetingSummary {
        id,
        title: Bilingual { es: title_es, en: title_en },
        template,
        date,
        duration_sec,
        participants,
        word_count,
    }))
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p smart-noter-db search_repo summary_by_id`
Expected: PASS (all search_repo tests). If `snippet(meeting_search, -1, …)` errors on the `-1` column index, use the `body` column index `3` instead (`snippet(meeting_search, 3, …)`).

- [ ] **Step 5: Commit**

```bash
cargo fmt  # from src-tauri
git add src-tauri/crates/core/src/models src-tauri/crates/db/src/repos/search_repo.rs src-tauri/crates/db/src/repos/mod.rs src-tauri/crates/db/src/repos/meetings_repo.rs
git commit -m "feat(db): search_repo (FTS upsert/delete/search/backfill) + SearchHit"
```

---

## Phase 2 — Maintain the index

### Task 3: Wire upsert/delete/backfill into the app

**Files:**
- Modify: `src-tauri/src/commands/transcription.rs` (after `replace_lines` Ok)
- Modify: `src-tauri/src/commands/meetings.rs` (`update_meeting_title`, `purge_meeting`)
- Modify: `src-tauri/src/lib.rs` (startup backfill)

- [ ] **Step 1: Upsert after transcription persists**

In `transcription.rs`, in the `Ok(())` arm right after `replace_lines` succeeds (the block that emits `transcription:segment`/`completed`), add a best-effort FTS upsert (it runs in the same `block_on` thread context — wrap a `block_on`):

```rust
            Ok(()) => {
                // Refresh the search index for this meeting (best-effort).
                if let Err(e) = tauri::async_runtime::block_on(
                    smart_noter_db::repos::search_repo::upsert_meeting(&pool, &mid),
                ) {
                    tracing::warn!("fts upsert after transcription failed for {mid}: {e}");
                }
                for l in &lines {
                    // ... existing segment emits ...
```
(Keep the existing segment/completed emits exactly as they are, after the upsert.)

- [ ] **Step 2: Upsert on title edit, delete on purge**

In `meetings.rs`:
- `update_meeting_title`: after the repo call succeeds, upsert the FTS row so the new title is searchable. Change it to:

```rust
pub async fn update_meeting_title(
    state: State<'_, AppState>,
    id: String,
    title_es: String,
    title_en: Option<String>,
) -> Result<(), AppError> {
    meetings_repo::update_title(&state.pool, &id, &title_es, title_en.as_deref())
        .await
        .map_err(from_db)?;
    if let Err(e) = search_repo::upsert_meeting(&state.pool, &id).await {
        tracing::warn!("fts upsert after title edit failed for {id}: {e}");
    }
    Ok(())
}
```

- `purge_meeting`: after purging, delete the FTS row (FTS5 virtual tables are NOT covered by FK CASCADE):

```rust
pub async fn purge_meeting(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    let paths = meetings_repo::purge(&state.pool, &id).await.map_err(from_db)?;
    let _ = search_repo::delete_meeting(&state.pool, &id).await;
    for p in paths {
        if let Err(e) = std::fs::remove_file(&p) {
            tracing::warn!("purge_meeting: could not delete audio file {p}: {e}");
        }
    }
    Ok(())
}
```

Add `search_repo` to the `use smart_noter_db::repos::{...}` import in `meetings.rs`.

- [ ] **Step 3: Startup backfill**

In `lib.rs`, inside the `block_on` async block AFTER `seed_if_empty(...)` and the 30-day auto-purge, before `app_handle.manage(...)`:

```rust
                // Backfill the search index for meetings created before this feature.
                if let Err(e) = smart_noter_db::repos::search_repo::backfill(&pool).await {
                    tracing::warn!("fts backfill failed: {e}");
                }
```

- [ ] **Step 4: Build**

Run: `cargo build -p smart-noter`
Expected: compiles.

- [ ] **Step 5: Commit**

```bash
cargo fmt
git add src-tauri/src/commands/transcription.rs src-tauri/src/commands/meetings.rs src-tauri/src/lib.rs
git commit -m "feat(app): maintain FTS index (transcription, title edit, purge, startup backfill)"
```

---

## Phase 3 — Command

### Task 4: `search_meetings` command

**Files:**
- Modify: `src-tauri/src/commands/meetings.rs`
- Modify: `src-tauri/src/lib.rs` (register)

- [ ] **Step 1: Add the command**

In `meetings.rs` (import `SearchHit`):

```rust
#[tauri::command]
#[specta::specta]
pub async fn search_meetings(
    state: State<'_, AppState>,
    query: String,
    template: Option<String>,
) -> Result<Vec<SearchHit>, AppError> {
    search_repo::search(&state.pool, &query, template.as_deref())
        .await
        .map_err(from_db)
}
```
Add `SearchHit` to the `use smart_noter_core::{... models::{...}}` import.

- [ ] **Step 2: Register**

In `lib.rs` `collect_commands![...]`, after the CRUD commands: `commands::meetings::search_meetings,`.

- [ ] **Step 3: Build + commit**

Run: `cargo build -p smart-noter` (compiles), then:
```bash
git add src-tauri/src/commands/meetings.rs src-tauri/src/lib.rs
git commit -m "feat(commands): search_meetings FTS command"
```

---

## Phase 4 — bindings + i18n

### Task 5: Regenerate bindings + i18n keys

- [ ] **Step 1: Regenerate bindings**

Run `npm run generate:bindings` (DLL-copy workaround if it crashes). Confirm `searchMeetings` and `SearchHit` appear in `bindings.ts`.

- [ ] **Step 2: Add i18n keys**

Add to `es.json`: `"searchNoResults": "Sin resultados para tu búsqueda.", "searchingHint": "Buscando…"`. To `en.json`: `"searchNoResults": "No results for your search.", "searchingHint": "Searching…"`. (The search placeholder `searchMeetings` already exists.)

- [ ] **Step 3: Regenerate keys + validate JSON + commit**

```bash
npm run generate:i18n-keys
node -e "JSON.parse(require('fs').readFileSync('src/i18n/locales/es.json','utf8'));JSON.parse(require('fs').readFileSync('src/i18n/locales/en.json','utf8'));console.log('OK')"
git add src/i18n/locales/es.json src/i18n/locales/en.json
git commit -m "feat(i18n): search result strings"
```

---

## Phase 5 — Frontend

### Task 6: RTK search endpoint + wire the list

**Files:**
- Modify: `src/store/api/meetings.api.ts`
- Modify: `src/features/meetings-list/MeetingsListPage.tsx`
- Create: `src/features/meetings-list/components/SearchSnippet/SearchSnippet.tsx` (+ `.module.css`)
- Test: `src/store/api/meetings.search.test.ts`, and extend `MeetingsListPage.test.tsx`

- [ ] **Step 1: Write the failing endpoint test**

Create `src/store/api/meetings.search.test.ts` (hoisted-mock pattern):

```ts
import { beforeEach, describe, expect, it, vi } from 'vitest';
const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a: unknown[]) => invoke(...a) }));
import { store } from '@/store';
import { meetingsApi } from './meetings.api';

describe('search endpoint', () => {
  beforeEach(() => invoke.mockReset());
  it('searchMeetings invokes search_meetings with query + template', async () => {
    invoke.mockResolvedValueOnce([]);
    await store.dispatch(
      meetingsApi.endpoints.searchMeetings.initiate({ query: 'arq', template: 'tecnica' }),
    ).unwrap();
    expect(invoke).toHaveBeenCalledWith('search_meetings', { query: 'arq', template: 'tecnica' });
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npm run test:run -- meetings.search`
Expected: FAIL — endpoint undefined.

- [ ] **Step 3: Add the endpoint**

In `meetings.api.ts` (import `SearchHit` type from bindings), add a query endpoint:

```ts
    searchMeetings: b.query<SearchHit[], { query: string; template: string | null }>({
      query: (args) => ({ cmd: 'search_meetings', args }),
    }),
```
Add `useSearchMeetingsQuery` (and the lazy variant `useLazySearchMeetingsQuery` if preferred) to the exported hooks. No tags needed (search is read-only and re-runs on arg change).

- [ ] **Step 4: SearchSnippet component (highlight markers → <mark>)**

Create `src/features/meetings-list/components/SearchSnippet/SearchSnippet.tsx`:

```tsx
import styles from './SearchSnippet.module.css';

const START = '⁨';
const END = '⁩';

/** Render an FTS snippet, converting the ⁨…⁩ marker pairs to <mark>. */
export function SearchSnippet({ text }: { text: string }) {
  // Split on the markers and alternate normal / highlighted segments without
  // dangerouslySetInnerHTML (XSS-safe).
  const parts = text.split(START).flatMap((chunk, i) => {
    if (i === 0) return [{ hit: false, s: chunk }];
    const [hit, ...rest] = chunk.split(END);
    return [
      { hit: true, s: hit },
      { hit: false, s: rest.join(END) },
    ];
  });
  return (
    <span className={styles.snippet}>
      {parts.map((p, i) =>
        p.hit ? (
          <mark key={i} className={styles.mark}>
            {p.s}
          </mark>
        ) : (
          <span key={i}>{p.s}</span>
        ),
      )}
    </span>
  );
}
```
`SearchSnippet.module.css`: `.snippet { font-size: 12px; color: var(--text-muted); }` and `.mark { background: var(--accent-soft, rgba(99,102,241,0.18)); color: var(--text); border-radius: 3px; padding: 0 2px; }`.

- [ ] **Step 5: Wire MeetingsListPage**

In `MeetingsListPage.tsx`:
1. Debounce `search` into `debouncedSearch` (300ms) via a small `useEffect` + `setTimeout` (or an existing debounce util if the repo has one — check `src/utils`).
2. `const isSearching = debouncedSearch.trim().length > 0;`
3. `const { data: hits = [], isFetching } = useSearchMeetingsQuery({ query: debouncedSearch, template: filter === 'all' ? null : filter }, { skip: !isSearching });`
4. When `isSearching`: render `hits` — each as the existing `MeetingRow` for `hit.meeting` PLUS a `<SearchSnippet text={hit.snippet} />` under it; empty → `t('searchNoResults')`. When not searching: keep the current `filtered` list behavior (template chip + full list), but DROP the old client-side title `.includes` filter (search now owns the query path).
5. The template chip still drives both: it filters `filtered` when not searching, and is passed as `template` when searching.

- [ ] **Step 6: Extend MeetingsListPage test**

Add a test: typing in the search box (after debounce) calls `invoke('search_meetings', ...)` and renders a hit's snippet. Use `vi.useFakeTimers()` or `waitFor` for the debounce. Keep existing tests green (the empty-state text assertions may need `isSearching=false` path intact).

- [ ] **Step 7: Verify + commit**

Run: `npm run test:run -- meetings.search MeetingsListPage` then `npx tsc --noEmit` then `npm run lint` (run `npx biome format --write` on touched files first).
Expected: green.
```bash
git add src/store/api/meetings.api.ts src/store/api/meetings.search.test.ts src/features/meetings-list/
git commit -m "feat(fe): backend FTS search with highlighted snippets in the meetings list"
```

---

## Phase 6 — Verification

### Task 7: Full verification + smoke

- [ ] **Step 1: Backend** — `cargo test -p smart-noter-db` + `cargo build -p smart-noter` (green).
- [ ] **Step 2: Frontend** — `npm run test:run && npx tsc --noEmit && npm run lint` (green).
- [ ] **Step 3: Manual smoke** — launch the app (back up the DB first — it points at real `%APPDATA%` data; migration 0005 is additive but restore after). Verify: at startup the backfill populates the index (search an old meeting's title → it appears); type a word that appears in a transcript → the meeting shows with a highlighted snippet; select a template chip → results narrow; clear the box → normal list returns; trashed meetings don't show in results. Restore the DB after.
- [ ] **Step 4: Final commit (if fixups).**

---

## Notes for the executor

- **FTS5 confirmed available** (verified 2026-06-23 via a throwaway test: virtual table + MATCH + snippet all work). No LIKE fallback.
- **Query safety:** `to_match_expr` quotes every token and appends `*`, so arbitrary user input (operators, quotes, parens) can't be an FTS5 syntax error. There's a test for this (`weird_query_is_not_a_syntax_error`) — keep it.
- **Migration test gotcha:** the FTS5 virtual table creates shadow tables; Task 1 Step 4 adjusts `migration_creates_expected_tables` so it stays green. Don't skip that.
- **FTS5 is not FK-covered:** purge must delete the FTS row explicitly (Task 3); CASCADE won't.
- **Snippet highlighting is XSS-safe:** markers are `⁨`/`⁩` (isolate chars unlikely in transcripts), converted to `<mark>` by splitting — never `dangerouslySetInnerHTML`.
- **Backfill is idempotent** (only meetings with no FTS row); safe to run every startup.
- This is **Module D of Sub-4**. Module C (export MP3/MD/PDF) is the last one.
```