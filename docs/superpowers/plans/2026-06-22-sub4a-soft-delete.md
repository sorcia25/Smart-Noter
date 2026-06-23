# Sub-4A — Soft Delete (Trash) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let a user delete a meeting into a recoverable Trash, restore it, or permanently purge it (removing the row via CASCADE and the audio file from disk), with a 30-day auto-purge at startup.

**Architecture:** Additive migration adds a nullable `meetings.deleted_at`. The active list filters it out; a new trash list shows it. Repo gains soft_delete / restore / purge / list_trashed / list_purgeable. Purge returns the meeting's audio asset paths so the Tauri command (which owns the filesystem) unlinks the files. A boot sweep purges trash older than 30 days, reusing the existing orphan-sweep pattern. Frontend adds delete-with-confirm on the list and a Trash page.

**Tech Stack:** Rust + sqlx (SQLite, offline `.sqlx` cache), Tauri commands + specta bindings, React + RTK Query + react-router, vitest/RTL.

**Conventions for this plan:**
- All NEW queries use the **unchecked** form `sqlx::query(...)` / `sqlx::query_as(...)` (already the dominant pattern in `participants_repo.rs`). This avoids regenerating the `.sqlx` offline cache. Do **not** use the checked `sqlx::query!` macro for new queries here, or CI's offline build will fail without a `cargo sqlx prepare` step.
- Build/test the db crate from repo root with the project's standard env preamble (LLVM/libclang + cmake PATH) — see `reference_whisper_build_toolchain`. Commands below assume that preamble is active.
- Run rust tests with `cargo test -p smart-noter-db`.

---

### Task 1: Migration 0004 — `meetings.deleted_at`

**Files:**
- Create: `src-tauri/crates/db/migrations/0004_meeting_soft_delete.sql`
- Test: `src-tauri/crates/db/tests/migration.rs`

- [ ] **Step 1: Write the failing test**

Add to `src-tauri/crates/db/tests/migration.rs`:

```rust
#[tokio::test]
async fn migration_0004_adds_deleted_at_column() {
    let pool = init_pool_in_memory().await.expect("pool");
    let cols: Vec<String> =
        sqlx::query_scalar("SELECT name FROM pragma_table_info('meetings')")
            .fetch_all(&pool)
            .await
            .expect("query");
    assert!(
        cols.iter().any(|c| c == "deleted_at"),
        "got columns: {cols:?}"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p smart-noter-db migration_0004_adds_deleted_at_column`
Expected: FAIL — `deleted_at` not present (migration file does not exist yet).

- [ ] **Step 3: Create the migration**

Create `src-tauri/crates/db/migrations/0004_meeting_soft_delete.sql`:

```sql
-- Soft delete: NULL deleted_at = active meeting; non-NULL = in Trash.
ALTER TABLE meetings ADD COLUMN deleted_at TEXT;
CREATE INDEX idx_meetings_deleted ON meetings(deleted_at);
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p smart-noter-db migration_0004_adds_deleted_at_column`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/crates/db/migrations/0004_meeting_soft_delete.sql src-tauri/crates/db/tests/migration.rs
git commit -m "feat(db): migration 0004 add meetings.deleted_at for soft delete"
```

---

### Task 2: Repo — filter active list + `list_trashed`

**Files:**
- Modify: `src-tauri/crates/db/src/repos/meetings_repo.rs`

`list_summaries` currently uses the checked `query!` macro. To keep this task self-contained (no `.sqlx` regen), refactor it and `list_trashed` onto a shared private helper that uses an **unchecked** query.

- [ ] **Step 1: Write the failing tests**

Add to the `mod tests` block in `meetings_repo.rs`:

```rust
async fn insert_meeting(pool: &SqlitePool, id: &str, deleted_at: Option<&str>) {
    sqlx::query(
        r#"INSERT INTO meetings (id, title_es, template_id, date, duration_sec, deleted_at)
           VALUES (?, ?, 'tecnica', '2026-06-01T00:00:00Z', 10, ?)"#,
    )
    .bind(id)
    .bind(format!("M {id}"))
    .bind(deleted_at)
    .execute(pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn list_summaries_excludes_trashed() {
    let pool = init_pool_in_memory().await.unwrap();
    insert_meeting(&pool, "m-active", None).await;
    insert_meeting(&pool, "m-trashed", Some("2026-06-02T00:00:00Z")).await;

    let active = list_summaries(&pool).await.unwrap();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].id, "m-active");
}

#[tokio::test]
async fn list_trashed_returns_only_trashed() {
    let pool = init_pool_in_memory().await.unwrap();
    insert_meeting(&pool, "m-active", None).await;
    insert_meeting(&pool, "m-trashed", Some("2026-06-02T00:00:00Z")).await;

    let trashed = list_trashed(&pool).await.unwrap();
    assert_eq!(trashed.len(), 1);
    assert_eq!(trashed[0].id, "m-trashed");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p smart-noter-db list_trashed_returns_only_trashed list_summaries_excludes_trashed`
Expected: FAIL — `list_trashed` undefined; `list_summaries` returns trashed too.

- [ ] **Step 3: Refactor `list_summaries` + add `list_trashed`**

Replace the existing `list_summaries` function (lines ~9-34) with:

```rust
/// Shared builder: runs `sql` (which must SELECT the standard summary columns)
/// and hydrates each row's participants.
async fn summaries_from_sql(pool: &SqlitePool, sql: &str) -> Result<Vec<MeetingSummary>, DbError> {
    let rows = sqlx::query_as::<_, (String, String, Option<String>, String, String, i64, i64)>(sql)
        .fetch_all(pool)
        .await?;

    let mut out = Vec::with_capacity(rows.len());
    for (id, title_es, title_en, template, date, duration_sec, word_count) in rows {
        let participants = participants_repo::list_by_meeting(pool, &id).await?;
        out.push(MeetingSummary {
            id,
            title: Bilingual { es: title_es, en: title_en },
            template,
            date,
            duration_sec,
            participants,
            word_count,
        });
    }
    Ok(out)
}

pub async fn list_summaries(pool: &SqlitePool) -> Result<Vec<MeetingSummary>, DbError> {
    summaries_from_sql(
        pool,
        r#"SELECT id, title_es, title_en, template_id, date, duration_sec, word_count
           FROM meetings WHERE deleted_at IS NULL ORDER BY date DESC"#,
    )
    .await
}

pub async fn list_trashed(pool: &SqlitePool) -> Result<Vec<MeetingSummary>, DbError> {
    summaries_from_sql(
        pool,
        r#"SELECT id, title_es, title_en, template_id, date, duration_sec, word_count
           FROM meetings WHERE deleted_at IS NOT NULL ORDER BY deleted_at DESC"#,
    )
    .await
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p smart-noter-db list_trashed_returns_only_trashed list_summaries_excludes_trashed list_summaries_empty_on_fresh_db`
Expected: PASS (all three).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/crates/db/src/repos/meetings_repo.rs
git commit -m "feat(db): filter trashed from list_summaries + add list_trashed"
```

---

### Task 3: Repo — `soft_delete` + `restore`

**Files:**
- Modify: `src-tauri/crates/db/src/repos/meetings_repo.rs`

- [ ] **Step 1: Write the failing test**

Add to the `mod tests` block:

```rust
#[tokio::test]
async fn soft_delete_then_restore_round_trips() {
    let pool = init_pool_in_memory().await.unwrap();
    insert_meeting(&pool, "m1", None).await;

    soft_delete(&pool, "m1").await.unwrap();
    assert_eq!(list_summaries(&pool).await.unwrap().len(), 0);
    assert_eq!(list_trashed(&pool).await.unwrap().len(), 1);

    restore(&pool, "m1").await.unwrap();
    assert_eq!(list_summaries(&pool).await.unwrap().len(), 1);
    assert_eq!(list_trashed(&pool).await.unwrap().len(), 0);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p smart-noter-db soft_delete_then_restore_round_trips`
Expected: FAIL — `soft_delete` / `restore` undefined.

- [ ] **Step 3: Implement `soft_delete` + `restore`**

Add to `meetings_repo.rs` (after `update_title`):

```rust
pub async fn soft_delete(pool: &SqlitePool, id: &str) -> Result<(), DbError> {
    sqlx::query("UPDATE meetings SET deleted_at = datetime('now') WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn restore(pool: &SqlitePool, id: &str) -> Result<(), DbError> {
    sqlx::query("UPDATE meetings SET deleted_at = NULL WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p smart-noter-db soft_delete_then_restore_round_trips`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/crates/db/src/repos/meetings_repo.rs
git commit -m "feat(db): soft_delete + restore meeting"
```

---

### Task 4: Repo — `purge` (returns audio paths) + `list_purgeable`

**Files:**
- Modify: `src-tauri/crates/db/src/repos/meetings_repo.rs`

- [ ] **Step 1: Write the failing tests**

Add to the `mod tests` block:

```rust
#[tokio::test]
async fn purge_deletes_row_and_returns_audio_paths() {
    use crate::repos::MeetingsRepo;
    use smart_noter_core::{Bilingual, MeetingAsset, MeetingDetail};

    let pool = init_pool_in_memory().await.unwrap();
    let meeting = MeetingDetail {
        id: "m-purge".into(),
        title: Bilingual { es: "P".into(), en: None },
        template: "tecnica".into(),
        date: "2026-06-01T00:00:00Z".into(),
        duration_sec: 5,
        device_used: None,
        word_count: 0,
        summary: None,
        participants: vec![],
        actions: vec![],
        decisions: vec![],
        blockers: vec![],
        transcript: vec![],
    };
    let asset = MeetingAsset {
        id: "a-purge".into(),
        meeting_id: "m-purge".into(),
        kind: "audio".into(),
        path: "C:/audio/m-purge.wav".into(),
        bytes: 1,
        mime_type: Some("audio/wav".into()),
        created_at: "2026-06-01T00:00:00Z".into(),
    };
    MeetingsRepo(&pool).create_with_asset(&meeting, &asset).await.unwrap();
    soft_delete(&pool, "m-purge").await.unwrap();

    let paths = purge(&pool, "m-purge").await.unwrap();
    assert_eq!(paths, vec!["C:/audio/m-purge.wav".to_string()]);
    assert_eq!(count(&pool).await.unwrap(), 0);
}

#[tokio::test]
async fn list_purgeable_returns_old_trash_only() {
    let pool = init_pool_in_memory().await.unwrap();
    // 40 days ago -> purgeable; just-now trash -> not.
    insert_meeting(&pool, "m-old", Some("2026-01-01T00:00:00Z")).await;
    sqlx::query("UPDATE meetings SET deleted_at = datetime('now','-40 days') WHERE id = 'm-old'")
        .execute(&pool).await.unwrap();
    insert_meeting(&pool, "m-fresh", None).await;
    soft_delete(&pool, "m-fresh").await.unwrap();

    let ids = list_purgeable(&pool).await.unwrap();
    assert_eq!(ids, vec!["m-old".to_string()]);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p smart-noter-db purge_deletes_row_and_returns_audio_paths list_purgeable_returns_old_trash_only`
Expected: FAIL — `purge` / `list_purgeable` undefined.

- [ ] **Step 3: Implement `purge` + `list_purgeable`**

Add to `meetings_repo.rs`:

```rust
/// Hard-deletes the meeting (CASCADE wipes participants/actions/decisions/
/// blockers/transcript_lines/meeting_assets) and returns the audio asset
/// path(s) so the caller can unlink the files from disk.
pub async fn purge(pool: &SqlitePool, id: &str) -> Result<Vec<String>, DbError> {
    let paths: Vec<String> = sqlx::query_scalar(
        "SELECT path FROM meeting_assets WHERE meeting_id = ? AND kind = 'audio'",
    )
    .bind(id)
    .fetch_all(pool)
    .await?;

    sqlx::query("DELETE FROM meetings WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(paths)
}

/// IDs of meetings trashed more than 30 days ago.
pub async fn list_purgeable(pool: &SqlitePool) -> Result<Vec<String>, DbError> {
    let ids: Vec<String> = sqlx::query_scalar(
        "SELECT id FROM meetings WHERE deleted_at IS NOT NULL \
         AND deleted_at < datetime('now','-30 days')",
    )
    .fetch_all(pool)
    .await?;
    Ok(ids)
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p smart-noter-db purge_deletes_row_and_returns_audio_paths list_purgeable_returns_old_trash_only`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/crates/db/src/repos/meetings_repo.rs
git commit -m "feat(db): purge meeting (returns audio paths) + list_purgeable"
```

---

### Task 5: Tauri commands + registration + disk unlink

**Files:**
- Modify: `src-tauri/src/commands/meetings.rs`
- Modify: `src-tauri/src/lib.rs` (register in `collect_commands!`)

- [ ] **Step 1: Add the four commands**

Append to `src-tauri/src/commands/meetings.rs`:

```rust
#[tauri::command]
#[specta::specta]
pub async fn delete_meeting(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    meetings_repo::soft_delete(&state.pool, &id)
        .await
        .map_err(from_db)
}

#[tauri::command]
#[specta::specta]
pub async fn restore_meeting(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    meetings_repo::restore(&state.pool, &id)
        .await
        .map_err(from_db)
}

#[tauri::command]
#[specta::specta]
pub async fn list_trashed_meetings(
    state: State<'_, AppState>,
) -> Result<Vec<MeetingSummary>, AppError> {
    meetings_repo::list_trashed(&state.pool)
        .await
        .map_err(from_db)
}

#[tauri::command]
#[specta::specta]
pub async fn purge_meeting(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    let paths = meetings_repo::purge(&state.pool, &id)
        .await
        .map_err(from_db)?;
    for p in paths {
        if let Err(e) = std::fs::remove_file(&p) {
            tracing::warn!("purge_meeting: could not delete audio file {p}: {e}");
        }
    }
    Ok(())
}
```

- [ ] **Step 2: Register the commands**

In `src-tauri/src/lib.rs`, add inside `collect_commands![ ... ]` after `commands::meetings::create_speaker,`:

```rust
        commands::meetings::delete_meeting,
        commands::meetings::restore_meeting,
        commands::meetings::list_trashed_meetings,
        commands::meetings::purge_meeting,
```

- [ ] **Step 3: Build to verify it compiles**

Run: `cargo build -p smart-noter` (or the workspace bin)
Expected: compiles; specta macros accept the new commands.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands/meetings.rs src-tauri/src/lib.rs
git commit -m "feat(commands): delete/restore/purge/list_trashed meeting commands"
```

---

### Task 6: Startup 30-day auto-purge

**Files:**
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Add the boot sweep**

In `src-tauri/src/lib.rs`, inside the `block_on` async block, **after** `seed_if_empty(...)` succeeds and **before** `app_handle.manage(...)`:

```rust
                // Auto-purge meetings trashed > 30 days ago (best-effort).
                if let Ok(ids) =
                    smart_noter_db::repos::meetings_repo::list_purgeable(&pool).await
                {
                    for id in ids {
                        match smart_noter_db::repos::meetings_repo::purge(&pool, &id).await {
                            Ok(paths) => {
                                for p in paths {
                                    let _ = std::fs::remove_file(&p);
                                }
                                tracing::info!("auto-purged trashed meeting {id}");
                            }
                            Err(e) => tracing::warn!("auto-purge failed for {id}: {e}"),
                        }
                    }
                }
```

- [ ] **Step 2: Build to verify it compiles**

Run: `cargo build -p smart-noter`
Expected: compiles.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat(startup): auto-purge meetings trashed over 30 days"
```

---

### Task 7: Regenerate bindings + add i18n keys

**Files:**
- Modify (generated): `src/ipc/bindings.ts`
- Modify: `src/i18n/locales/es.json`, `src/i18n/locales/en.json`
- Modify (generated): i18n key types via `generate:i18n-keys`

- [ ] **Step 1: Regenerate IPC bindings**

Run: `npm run generate:bindings`
Expected: `src/ipc/bindings.ts` now contains `deleteMeeting`, `restoreMeeting`, `listTrashedMeetings`, `purgeMeeting` typed commands.

- [ ] **Step 2: Add i18n keys**

Add to `src/i18n/locales/es.json`:

```json
"trashTitle": "Papelera",
"trashSub": "Reuniones eliminadas. Se borran definitivamente tras 30 días.",
"trashEmpty": "La papelera está vacía.",
"navTrash": "Papelera",
"deleteMeeting": "Eliminar",
"restoreMeeting": "Restaurar",
"deletePermanently": "Eliminar definitivamente",
"confirmDeleteTitle": "¿Mover a la papelera?",
"confirmDeleteBody": "Podrás restaurarla desde la papelera durante 30 días.",
"confirmPurgeTitle": "¿Eliminar definitivamente?",
"confirmPurgeBody": "Esta acción no se puede deshacer. Se borrará la reunión y su audio."
```

Add the matching keys to `src/i18n/locales/en.json`:

```json
"trashTitle": "Trash",
"trashSub": "Deleted meetings. Permanently removed after 30 days.",
"trashEmpty": "Trash is empty.",
"navTrash": "Trash",
"deleteMeeting": "Delete",
"restoreMeeting": "Restore",
"deletePermanently": "Delete permanently",
"confirmDeleteTitle": "Move to Trash?",
"confirmDeleteBody": "You can restore it from Trash for 30 days.",
"confirmPurgeTitle": "Delete permanently?",
"confirmPurgeBody": "This cannot be undone. The meeting and its audio will be erased."
```

- [ ] **Step 3: Regenerate i18n key types**

Run: `npm run generate:i18n-keys`
Expected: `src/i18n/keys.ts` includes the new keys; `t('trashTitle')` etc. typecheck.

- [ ] **Step 4: Commit**

```bash
git add src/ipc/bindings.ts src/i18n/locales/es.json src/i18n/locales/en.json src/i18n/keys.ts
git commit -m "chore(ipc,i18n): bindings + trash/delete keys for soft delete"
```

---

### Task 8: Frontend RTK Query endpoints

**Files:**
- Modify: `src/store/api/meetings.api.ts`

- [ ] **Step 1: Write the failing test**

Create `src/store/api/meetings.trash.test.ts`:

```ts
import { describe, expect, it, vi, beforeEach } from 'vitest';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a: unknown[]) => invoke(...a) }));

import { store } from '@/store/store';
import { meetingsApi } from './meetings.api';

describe('trash endpoints', () => {
  beforeEach(() => invoke.mockReset());

  it('deleteMeeting invokes delete_meeting with id', async () => {
    invoke.mockResolvedValueOnce(undefined);
    await store.dispatch(meetingsApi.endpoints.deleteMeeting.initiate('m1')).unwrap();
    expect(invoke).toHaveBeenCalledWith('delete_meeting', { id: 'm1' });
  });

  it('listTrashedMeetings invokes list_trashed_meetings', async () => {
    invoke.mockResolvedValueOnce([]);
    await store.dispatch(meetingsApi.endpoints.listTrashedMeetings.initiate()).unwrap();
    expect(invoke).toHaveBeenCalledWith('list_trashed_meetings', {});
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npm run test -- meetings.trash`
Expected: FAIL — `deleteMeeting` / `listTrashedMeetings` endpoints don't exist.

- [ ] **Step 3: Add the endpoints**

In `src/store/api/meetings.api.ts`, add inside `endpoints: (b) => ({ ... })`:

```ts
    listTrashedMeetings: b.query<MeetingSummary[], void>({
      query: () => ({ cmd: 'list_trashed_meetings' }),
      providesTags: ['Trash'],
    }),
    deleteMeeting: b.mutation<void, string>({
      query: (id) => ({ cmd: 'delete_meeting', args: { id } }),
      invalidatesTags: ['Meeting', 'Trash'],
    }),
    restoreMeeting: b.mutation<void, string>({
      query: (id) => ({ cmd: 'restore_meeting', args: { id } }),
      invalidatesTags: ['Meeting', 'Trash'],
    }),
    purgeMeeting: b.mutation<void, string>({
      query: (id) => ({ cmd: 'purge_meeting', args: { id } }),
      invalidatesTags: ['Trash'],
    }),
```

And add to the exported hooks block:

```ts
  useListTrashedMeetingsQuery,
  useDeleteMeetingMutation,
  useRestoreMeetingMutation,
  usePurgeMeetingMutation,
```

In `src/store/api/base.ts`, add `'Trash'` to `tagTypes`:

```ts
  tagTypes: ['Meeting', 'Template', 'AudioDevice', 'Settings', 'Trash'],
```

- [ ] **Step 4: Run test to verify it passes**

Run: `npm run test -- meetings.trash`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/store/api/meetings.api.ts src/store/api/base.ts src/store/api/meetings.trash.test.ts
git commit -m "feat(fe): trash/delete/restore/purge RTK Query endpoints"
```

---

### Task 9: Delete-with-confirm on the meetings list

**Files:**
- Modify: `src/features/meetings-list/MeetingsListPage.tsx`
- Test: `src/features/meetings-list/MeetingsListPage.test.tsx`

The `MeetingRow` is currently the whole clickable row. Add a Delete affordance that opens a confirm modal and calls `deleteMeeting`. Reuse the existing `Modal` primitive (same one `ExportModal` uses) and `Button`.

- [ ] **Step 1: Write the failing test**

Add to `src/features/meetings-list/MeetingsListPage.test.tsx` (follow the file's existing render/mock setup):

```tsx
it('deleting a meeting calls delete_meeting after confirm', async () => {
  // existing helpers in this file mock list_meetings to return one meeting m1
  renderPage();
  await screen.findByText(/M 1/i);

  fireEvent.click(screen.getByLabelText('delete-m1'));      // trash icon on the row
  fireEvent.click(screen.getByText(/Mover a la papelera|Move to Trash/i)); // confirm button

  await waitFor(() =>
    expect(invoke).toHaveBeenCalledWith('delete_meeting', { id: 'm1' }),
  );
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npm run test -- MeetingsListPage`
Expected: FAIL — no delete control / confirm wired.

- [ ] **Step 3: Implement delete + confirm**

In `MeetingsListPage.tsx`:

1. Import the modal + mutation + state:

```tsx
import { Modal } from '@/components/primitives/Modal/Modal';
import { useDeleteMeetingMutation } from '@/store/api/meetings.api';
```

2. Inside the component, add:

```tsx
  const [deleteMeeting] = useDeleteMeetingMutation();
  const [pendingDelete, setPendingDelete] = useState<string | null>(null);
```

3. Give each row a delete button. Wrap the `MeetingRow` so a trash icon sits beside it:

```tsx
            {filtered.map((m) => (
              <div key={m.id} className={styles.rowWrap}>
                <MeetingRow meeting={m} onClick={() => navigate(Paths.MeetingDetail(m.id))} />
                <button
                  type="button"
                  aria-label={`delete-${m.id}`}
                  className={styles.deleteBtn}
                  onClick={() => setPendingDelete(m.id)}
                >
                  <Icon name="trash" size={16} />
                </button>
              </div>
            ))}
```

4. Render the confirm modal at the end of the returned JSX:

```tsx
      <Modal
        open={pendingDelete !== null}
        onClose={() => setPendingDelete(null)}
        title={t('confirmDeleteTitle')}
        subtitle={t('confirmDeleteBody')}
        footer={
          <>
            <Button variant="ghost" onClick={() => setPendingDelete(null)}>
              {t('cancel')}
            </Button>
            <Button
              variant="primary"
              onClick={async () => {
                if (pendingDelete) await deleteMeeting(pendingDelete).unwrap();
                setPendingDelete(null);
              }}
            >
              {t('confirmDeleteTitle')}
            </Button>
          </>
        }
      >
        <div />
      </Modal>
```

5. Add `.rowWrap` / `.deleteBtn` rules to `MeetingsListPage.module.css`: `.rowWrap { display: flex; align-items: center; gap: 8px; }` with `.deleteBtn` right-aligned and muted until hover. Use existing color vars (`var(--muted)`, `var(--danger)` if present; otherwise `var(--accent)`).

> If `Icon name="trash"` doesn't exist in the icon set, check `src/components/primitives/Icon` for the available names and use the closest (e.g. `delete`); update the test's expectation accordingly.

- [ ] **Step 4: Run test to verify it passes**

Run: `npm run test -- MeetingsListPage`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/features/meetings-list/
git commit -m "feat(fe): delete meeting (to trash) with confirm on list"
```

---

### Task 10: Trash page + route + access

**Files:**
- Create: `src/features/trash/TrashPage.tsx`
- Create: `src/features/trash/TrashPage.test.tsx`
- Modify: `src/router/paths.ts`, `src/router/routes.tsx`
- Modify: the nav/sidebar that lists `navMeetings` (find it; likely `src/components/.../Sidebar`)

- [ ] **Step 1: Write the failing test**

Create `src/features/trash/TrashPage.test.tsx`:

```tsx
import { fireEvent, screen, waitFor } from '@testing-library/react';
import { describe, expect, it, vi, beforeEach } from 'vitest';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a: unknown[]) => invoke(...a) }));

import { renderWithProviders } from '@/test/renderWithProviders'; // use the repo's existing helper
import TrashPage from './TrashPage';

describe('TrashPage', () => {
  beforeEach(() => invoke.mockReset());

  it('restores a trashed meeting', async () => {
    invoke.mockImplementation((cmd: string) =>
      cmd === 'list_trashed_meetings'
        ? Promise.resolve([{ id: 'm1', title: { es: 'M 1', en: null }, template: 'tecnica', date: '2026-06-01T00:00:00Z', durationSec: 10, participants: [], wordCount: 0 }])
        : Promise.resolve(undefined),
    );
    renderWithProviders(<TrashPage />);
    await screen.findByText(/M 1/i);
    fireEvent.click(screen.getByLabelText('restore-m1'));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith('restore_meeting', { id: 'm1' }));
  });
});
```

> Use whatever provider/render helper the other feature tests use (check an existing `*.test.tsx`); replace `renderWithProviders` import accordingly.

- [ ] **Step 2: Run test to verify it fails**

Run: `npm run test -- TrashPage`
Expected: FAIL — `TrashPage` doesn't exist.

- [ ] **Step 3: Add path + route**

In `src/router/paths.ts`, add to the `Paths` object:

```ts
  Trash: '/trash',
```

In `src/router/routes.tsx`:

```tsx
const Trash = lazy(() => import('@/features/trash/TrashPage'));
```

and add to the `routes` array (before the `'*'` catch-all):

```tsx
  { path: Paths.Trash, element: <Trash /> },
```

- [ ] **Step 4: Implement `TrashPage`**

Create `src/features/trash/TrashPage.tsx`:

```tsx
import { Button } from '@/components/primitives/Button/Button';
import { Modal } from '@/components/primitives/Modal/Modal';
import { useT } from '@/i18n/useT';
import {
  useListTrashedMeetingsQuery,
  usePurgeMeetingMutation,
  useRestoreMeetingMutation,
} from '@/store/api/meetings.api';
import { pickL } from '@/utils/format';
import { useState } from 'react';
import styles from './TrashPage.module.css';

export default function TrashPage() {
  const { t, lang } = useT();
  const { data: trashed = [] } = useListTrashedMeetingsQuery();
  const [restoreMeeting] = useRestoreMeetingMutation();
  const [purgeMeeting] = usePurgeMeetingMutation();
  const [pendingPurge, setPendingPurge] = useState<string | null>(null);

  return (
    <div className={styles.page} data-screen-label="Trash">
      <h1 className={styles.title}>{t('trashTitle')}</h1>
      <div className={styles.sub}>{t('trashSub')}</div>

      {trashed.length === 0 ? (
        <div className={styles.empty}>{t('trashEmpty')}</div>
      ) : (
        <div className={styles.list}>
          {trashed.map((m) => (
            <div key={m.id} className={styles.row}>
              <span className={styles.rowTitle}>{pickL(m.title, lang)}</span>
              <div className={styles.rowActions}>
                <Button
                  variant="ghost"
                  aria-label={`restore-${m.id}`}
                  onClick={() => restoreMeeting(m.id)}
                >
                  {t('restoreMeeting')}
                </Button>
                <Button
                  variant="ghost"
                  aria-label={`purge-${m.id}`}
                  onClick={() => setPendingPurge(m.id)}
                >
                  {t('deletePermanently')}
                </Button>
              </div>
            </div>
          ))}
        </div>
      )}

      <Modal
        open={pendingPurge !== null}
        onClose={() => setPendingPurge(null)}
        title={t('confirmPurgeTitle')}
        subtitle={t('confirmPurgeBody')}
        footer={
          <>
            <Button variant="ghost" onClick={() => setPendingPurge(null)}>
              {t('cancel')}
            </Button>
            <Button
              variant="primary"
              onClick={async () => {
                if (pendingPurge) await purgeMeeting(pendingPurge).unwrap();
                setPendingPurge(null);
              }}
            >
              {t('deletePermanently')}
            </Button>
          </>
        }
      >
        <div />
      </Modal>
    </div>
  );
}
```

Create `src/features/trash/TrashPage.module.css` with `.page/.title/.sub/.empty/.list/.row/.rowTitle/.rowActions` rules mirroring `MeetingsListPage.module.css` spacing/typography (reuse the same CSS vars).

- [ ] **Step 5: Add a nav entry to Trash**

Find the sidebar component rendering `t('navMeetings')` (search `navMeetings` under `src/components`). Add a link to `Paths.Trash` labelled `t('navTrash')`, following the existing nav-item markup exactly.

- [ ] **Step 6: Run test to verify it passes**

Run: `npm run test -- TrashPage`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add src/features/trash/ src/router/paths.ts src/router/routes.tsx src/components/
git commit -m "feat(fe): Trash page with restore + permanent delete, route + nav"
```

---

### Task 11: Full verification

- [ ] **Step 1: Backend tests**

Run: `cargo test -p smart-noter-db`
Expected: all pass, including the new migration/repo tests.

- [ ] **Step 2: Backend build (commands + startup)**

Run: `cargo build -p smart-noter`
Expected: compiles clean.

- [ ] **Step 3: Frontend tests + typecheck + lint**

Run: `npm run test && npm run typecheck && npm run lint`
Expected: all green. (If script names differ, use the ones in `package.json`.)

- [ ] **Step 4: Manual smoke (release)**

Per `project_sub3b_diarization_state`, run the **release** build (dev-debug has a CRT-assert bug). Verify: delete a meeting → it leaves the list and appears in Trash; restore → it returns; permanent delete → it's gone and its `.wav`/`.flac` is removed from `%APPDATA%/com.smartnoter.app/audio/`.

- [ ] **Step 5: Final commit (if any fixups)**

```bash
git add -A
git commit -m "test(sub4a): verify soft-delete trash end-to-end"
```

---

## Notes for the executor

- **sqlx offline cache:** this plan deliberately uses unchecked queries to avoid `.sqlx` regen. If you convert any new query to the `query!` macro, run `cargo sqlx prepare` (with a `DATABASE_URL` pointing at a migrated SQLite file) before committing, or CI's offline build breaks.
- **Bindings + i18n regen** (Task 7) must happen before the frontend tasks typecheck — `bindings.ts` and `keys.ts` are generated, never hand-edited.
- **Icon/render-helper unknowns** (Tasks 9–10): the plan flags the two spots where a name may differ in this codebase (`Icon name="trash"`, the test render helper). Resolve by inspecting the existing primitives/tests, not by guessing.
- **Binding field casing** (Task 10 mock object): the `MeetingSummary` field names in the test mock (`durationSec`, `wordCount`) must match exactly what `src/ipc/bindings.ts` emits after Task 7. tauri-specta may render them snake_case (`duration_sec`, `word_count`); copy the real field names from `bindings.ts` so the test typechecks.
- This is **Module A of Sub-4**. Modules B (CRUD actions/decisions/blockers), D (FTS5), C (export) get their own plans after A integrates.
```