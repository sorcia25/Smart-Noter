# Sub-2 Audio Capture Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver real WASAPI/cpal audio capture end-to-end — from PreRecord device pick to a `.wav`/`.flac` persisted with a fresh meeting row and a `meeting_assets` link, with real-time level + waveform driven by audio events.

**Architecture:** Event-driven Rust crate `smart-noter-audio` with `tauri::State<Arc<Mutex<CaptureSession>>>`; audio callbacks push samples to bounded MPSC channels; writer + meter + (optional) mixer worker threads consume; meter emits Tauri events (`audio:level @20Hz`, `audio:waveform-bin @10Hz`, `audio:elapsed @1Hz`, `audio:error`); new DB migration 0002 adds `meeting_assets` table; frontend swaps fake `useLiveTimer`/waveform for event subscriptions and adds a `StopConfirmModal`.

**Tech Stack:** Rust (tauri 2, wasapi 0.16, cpal 0.15, hound 3.5, claxon 0.4, rubato 0.15, crossbeam-channel 0.5, parking_lot 0.12, thiserror, tracing, specta), TypeScript (React 18, Redux Toolkit, RTK Query, react-router 6, sonner via existing Toast wrapper), Vitest, Playwright, sqlx (SQLite).

**Spec:** [docs/superpowers/specs/2026-05-19-sub2-audio-capture-design.md](../specs/2026-05-19-sub2-audio-capture-design.md)

---

## Working conventions (read once, apply to every task)

1. **TDD where practical.** For Rust pure-logic modules (`error.rs`, `mixer.rs`, `writer.rs`, `meter.rs`, state machine in `capture/mod.rs`, the new repos) — test first, fail, implement, pass, commit. For WASAPI/cpal-touching modules (`devices.rs`, `capture/stream.rs`) — TDD is impractical without hardware; ship implementation + a smoke test that runs only with `--features audio-integration` and is excluded from default `cargo test`.
2. **Working directory.** Plan executes inside the Sub-2 worktree (created via `superpowers:using-git-worktrees`). Commit messages use the existing conventional-commit style (`feat(audio):`, `feat(db):`, `feat(ipc):`, etc.).
3. **Regenerate after touching Rust commands.** After ANY task in Phase 4 that adds/changes a `#[tauri::command]` (and after `core` changes Specta-typed structs), run `pnpm generate:bindings`. The file `src/ipc/bindings.ts` is gitignored — regenerate is required on each clone but the diff must be sanity-checked locally.
4. **Regenerate after adding i18n keys.** After Phase 0.1, run `pnpm generate:i18n-keys`. Same gitignored convention.
5. **Quality gates after each phase.** At the end of every phase, run `pnpm lint && pnpm check:hardcoded-strings && pnpm check:stories && pnpm test:run && cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace` from project root. If any fails, fix before continuing.
6. **No new TODOs.** The DoD §4.4 has a "no `// TODO` / `// FIXME` / `// XXX`" rule. Use phrased comments instead (e.g., "Sub-3 will replace this with…" not "TODO(sub-3)…").
7. **Commit cadence.** One commit per task (end of task's last step). Squash WIP commits before pushing if you want, but the plan steps assume one commit per task boundary.

---

## Phase 0 — Prep: dependencies + i18n

### Task 0.1: Add audio crate dependencies

**Files:**
- Modify: `src-tauri/crates/audio/Cargo.toml`
- Modify: `src-tauri/Cargo.toml` (workspace deps, if any new ones go there)

- [ ] **Step 1: Replace `src-tauri/crates/audio/Cargo.toml`**

```toml
[package]
name = "smart-noter-audio"
version.workspace = true
edition.workspace = true

[dependencies]
wasapi = "0.16"
cpal = "0.15"
hound = "3.5"
claxon = "0.4"
rubato = "0.15"
crossbeam-channel = "0.5"
parking_lot = "0.12"
thiserror = { workspace = true }
tracing = { workspace = true }
serde = { workspace = true, features = ["derive"] }
specta = { workspace = true, features = ["derive"] }
smart-noter-core = { path = "../core" }

[dev-dependencies]
rstest = "0.21"
serde_json = { workspace = true }

[features]
default = []
# Gated integration tests (require real hardware or fake-cpal harness).
audio-integration = []
```

- [ ] **Step 2: Verify the workspace builds (cargo check downloads the new crates)**

```bash
cd src-tauri && cargo check -p smart-noter-audio
```

Expected: green build, several new crates downloaded.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/crates/audio/Cargo.toml src-tauri/Cargo.lock
git commit -m "chore(audio): add wasapi, cpal, hound, claxon, rubato + dev deps"
```

---

### Task 0.2: Add new i18n keys for Sub-2

**Files:**
- Modify: `src/i18n/locales/es.json`
- Modify: `src/i18n/locales/en.json`

The Sub-2 spec calls for ~20 new keys (8 modal + 12 error). Insert all of them sorted alphabetically (the existing JSON files are sorted by key — preserve that).

- [ ] **Step 1: Add 8 modal-related keys to `es.json`**

Insert (keeping alphabetical order in the existing file):

```json
{
  "audioErrorTitle": "Error de captura de audio",
  "audioError.DeviceNotFound": "Dispositivo no disponible. Volvé a elegir uno.",
  "audioError.WasapiInit": "No se pudo iniciar la captura. Revisá que el dispositivo esté libre.",
  "audioError.FormatUnsupported": "Formato no soportado por el dispositivo. Usando WAV.",
  "audioError.DiskFull": "Espacio en disco insuficiente. Sesión detenida con audio parcial.",
  "audioError.MixerOverflow": "El sistema no pudo seguir el ritmo. Algunas muestras se perdieron.",
  "discard": "Descartar",
  "save": "Guardar",
  "saveRecording": "Guardar grabación",
  "saveRecordingSub": "Ponele un nombre. La transcripción y resumen se generan después."
}
```

- [ ] **Step 2: Add the same 8+2 keys to `en.json`**

```json
{
  "audioErrorTitle": "Audio capture error",
  "audioError.DeviceNotFound": "Device unavailable. Pick a different one.",
  "audioError.WasapiInit": "Failed to start capture. Check that the device is free.",
  "audioError.FormatUnsupported": "Format not supported by the device. Falling back to WAV.",
  "audioError.DiskFull": "Not enough disk space. Session stopped with partial audio.",
  "audioError.MixerOverflow": "System couldn't keep up. Some samples were dropped.",
  "discard": "Discard",
  "save": "Save",
  "saveRecording": "Save recording",
  "saveRecordingSub": "Give it a name. Transcript and summary are generated afterwards."
}
```

- [ ] **Step 3: Regenerate keys.ts and verify the count grew by 10**

```bash
pnpm generate:i18n-keys
```

Expected output: `Wrote 170 keys → ...`. (Was 160; +10 new.)

- [ ] **Step 4: Commit**

```bash
git add src/i18n/locales/es.json src/i18n/locales/en.json
git commit -m "feat(i18n): add Sub-2 keys — save/discard modal + 6 audio error messages"
```

---

## Phase 1 — Core + DB foundations

### Task 1.1: Add `AppError::Audio` variant + `AudioErrorCode` enum

**Files:**
- Modify: `src-tauri/crates/core/src/error.rs`
- Test: same file (inline `#[cfg(test)]` mod)

- [ ] **Step 1: Write failing tests in `src-tauri/crates/core/src/error.rs`**

Append below the existing `#[cfg(test)] mod tests` block (do not delete the existing tests):

```rust
#[test]
fn audio_error_serializes_with_code_field() {
    let err = AppError::Audio {
        code: AudioErrorCode::DeviceNotFound,
        message: "loopback-001".into(),
    };
    let json = serde_json::to_string(&err).unwrap();
    assert_eq!(
        json,
        r#"{"code":"audio","message":{"code":"DeviceNotFound","message":"loopback-001"}}"#
    );
}

#[test]
fn audio_error_i18n_key_routes_by_audio_code() {
    let err = AppError::Audio {
        code: AudioErrorCode::DiskFull,
        message: "C:/x".into(),
    };
    assert_eq!(err.i18n_key(), "audioError.DiskFull");
}
```

- [ ] **Step 2: Run tests — expect compile failure**

```bash
cd src-tauri && cargo test -p smart-noter-core error::tests
```

Expected: compile errors for missing `AudioErrorCode` and missing `Audio` variant.

- [ ] **Step 3: Add `AudioErrorCode` and `AppError::Audio` to `src-tauri/crates/core/src/error.rs`**

Insert above the existing `pub enum AppError`:

```rust
#[derive(Debug, Clone, Type, Serialize, Deserialize, PartialEq, Eq)]
pub enum AudioErrorCode {
    DeviceNotFound,
    WasapiInit,
    FormatUnsupported,
    DiskFull,
    AlreadyRecording,
    NotRecording,
    MixerOverflow,
    Other,
}
```

Then add a new variant to `AppError`:

```rust
    #[error("Audio error ({code:?}): {message}")]
    Audio {
        code: AudioErrorCode,
        message: String,
    },
```

(Place it before `Internal(String)` so the enum stays roughly ordered by frequency.)

Then add the `i18n_key()` arm:

```rust
            AppError::Audio { code, .. } => match code {
                AudioErrorCode::DeviceNotFound => "audioError.DeviceNotFound",
                AudioErrorCode::WasapiInit => "audioError.WasapiInit",
                AudioErrorCode::FormatUnsupported => "audioError.FormatUnsupported",
                AudioErrorCode::DiskFull => "audioError.DiskFull",
                AudioErrorCode::AlreadyRecording => "audioError.AlreadyRecording",
                AudioErrorCode::NotRecording => "audioError.NotRecording",
                AudioErrorCode::MixerOverflow => "audioError.MixerOverflow",
                AudioErrorCode::Other => "audioError.Other",
            },
```

Note the `i18n_key()` mapping returns keys like `audioError.AlreadyRecording` even though we didn't add those to the i18n bundles (they're internal/dev errors that shouldn't surface to users). The 6 user-facing ones (`DeviceNotFound`, `WasapiInit`, `FormatUnsupported`, `DiskFull`, `MixerOverflow`) are translated; the other 3 fall back to the key string in dev. Acceptable.

- [ ] **Step 4: Run tests — expect pass**

```bash
cd src-tauri && cargo test -p smart-noter-core error::tests
```

Expected: all error tests pass (original 2 + new 2).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/crates/core/src/error.rs
git commit -m "feat(core): AppError::Audio variant + AudioErrorCode enum (8 codes)"
```

---

### Task 1.2: Add `MeetingAsset` model in core

**Files:**
- Create: `src-tauri/crates/core/src/models/meeting_asset.rs`
- Modify: `src-tauri/crates/core/src/models/mod.rs`
- Test: same file as model (`#[cfg(test)]`)

- [ ] **Step 1: Write the model file `src-tauri/crates/core/src/models/meeting_asset.rs`**

```rust
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Type, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MeetingAsset {
    pub id: String,
    pub meeting_id: String,
    pub kind: String,
    pub path: String,
    pub bytes: i64,
    pub mime_type: Option<String>,
    pub created_at: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_with_camelcase_keys() {
        let a = MeetingAsset {
            id: "a-1".into(),
            meeting_id: "m-1".into(),
            kind: "audio".into(),
            path: "C:/x.wav".into(),
            bytes: 1024,
            mime_type: Some("audio/wav".into()),
            created_at: "2026-05-19T00:00:00Z".into(),
        };
        let json = serde_json::to_string(&a).unwrap();
        assert!(json.contains(r#""meetingId":"m-1""#));
        assert!(json.contains(r#""mimeType":"audio/wav""#));
        assert!(json.contains(r#""createdAt":"2026-05-19T00:00:00Z""#));
    }
}
```

- [ ] **Step 2: Re-export from `src-tauri/crates/core/src/models/mod.rs`**

Add the `pub mod meeting_asset;` and `pub use meeting_asset::*;` lines next to the other models (preserve alphabetical order).

- [ ] **Step 3: Run tests**

```bash
cd src-tauri && cargo test -p smart-noter-core meeting_asset
```

Expected: 1 test passes.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/crates/core/src/models/
git commit -m "feat(core): MeetingAsset model (id, meetingId, kind, path, bytes, mimeType, createdAt)"
```

---

### Task 1.3: Add DB migration 0002_meeting_assets.sql

**Files:**
- Create: `src-tauri/crates/db/migrations/0002_meeting_assets.sql`

- [ ] **Step 1: Write the migration**

```sql
CREATE TABLE meeting_assets (
    id TEXT PRIMARY KEY,
    meeting_id TEXT NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN ('audio', 'transcript', 'export')),
    path TEXT NOT NULL,
    bytes INTEGER NOT NULL CHECK (bytes >= 0),
    mime_type TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_meeting_assets_meeting_id ON meeting_assets(meeting_id);
CREATE INDEX idx_meeting_assets_kind ON meeting_assets(meeting_id, kind);
```

- [ ] **Step 2: Run the migration runner to apply against the prepare DB**

```bash
cd src-tauri && DATABASE_URL="sqlite://./crates/db/sn_prepare.db" cargo sqlx migrate run --source crates/db/migrations
```

Expected: migration `0002_meeting_assets` applied successfully.

- [ ] **Step 3: Verify the table exists**

```bash
cd src-tauri && sqlite3 ./crates/db/sn_prepare.db ".schema meeting_assets"
```

Expected output shows the `CREATE TABLE meeting_assets ...` statement.

- [ ] **Step 4: Regenerate sqlx offline cache (the existing repos may not need it, but keep it consistent)**

```bash
cd src-tauri && DATABASE_URL="sqlite://./crates/db/sn_prepare.db" cargo sqlx prepare --workspace -- --workspace --tests
```

Expected: `query data written to .sqlx/`.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/crates/db/migrations/0002_meeting_assets.sql src-tauri/.sqlx
git commit -m "feat(db): migration 0002 — meeting_assets table with CASCADE"
```

---

### Task 1.4: MeetingAssetsRepo

**Files:**
- Create: `src-tauri/crates/db/src/repos/meeting_assets_repo.rs`
- Modify: `src-tauri/crates/db/src/repos/mod.rs`
- Test: `src-tauri/crates/db/src/repos/meeting_assets_repo.rs` (inline)

- [ ] **Step 1: Write failing tests in the new repo file (full file)**

```rust
use smart_noter_core::{AppError, MeetingAsset};
use sqlx::SqlitePool;

pub struct MeetingAssetsRepo<'a>(pub &'a SqlitePool);

impl MeetingAssetsRepo<'_> {
    pub async fn create(&self, a: &MeetingAsset) -> Result<(), AppError> {
        sqlx::query(
            r#"INSERT INTO meeting_assets (id, meeting_id, kind, path, bytes, mime_type, created_at)
               VALUES (?, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(&a.id)
        .bind(&a.meeting_id)
        .bind(&a.kind)
        .bind(&a.path)
        .bind(a.bytes)
        .bind(a.mime_type.as_deref())
        .bind(&a.created_at)
        .execute(self.0)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(())
    }

    pub async fn list_by_meeting(&self, meeting_id: &str) -> Result<Vec<MeetingAsset>, AppError> {
        let rows = sqlx::query_as::<_, (String, String, String, String, i64, Option<String>, String)>(
            r#"SELECT id, meeting_id, kind, path, bytes, mime_type, created_at
               FROM meeting_assets WHERE meeting_id = ? ORDER BY created_at"#,
        )
        .bind(meeting_id)
        .fetch_all(self.0)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(rows
            .into_iter()
            .map(|(id, meeting_id, kind, path, bytes, mime_type, created_at)| MeetingAsset {
                id,
                meeting_id,
                kind,
                path,
                bytes,
                mime_type,
                created_at,
            })
            .collect())
    }

    pub async fn get_audio(&self, meeting_id: &str) -> Result<Option<MeetingAsset>, AppError> {
        Ok(self
            .list_by_meeting(meeting_id)
            .await?
            .into_iter()
            .find(|a| a.kind == "audio"))
    }

    pub async fn delete(&self, asset_id: &str) -> Result<Option<String>, AppError> {
        let row: Option<(String,)> = sqlx::query_as("SELECT path FROM meeting_assets WHERE id = ?")
            .bind(asset_id)
            .fetch_optional(self.0)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;
        sqlx::query("DELETE FROM meeting_assets WHERE id = ?")
            .bind(asset_id)
            .execute(self.0)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(row.map(|r| r.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connection::{ensure_schema, in_memory_pool};
    use smart_noter_core::Bilingual;

    async fn seed_meeting(pool: &SqlitePool, id: &str) {
        sqlx::query(r#"INSERT INTO meetings (id, title_es, title_en, template, date, duration_sec, device_used, word_count, summary_es, summary_en)
                       VALUES (?, ?, NULL, 'tecnica', '2026-05-19T00:00:00Z', 0, NULL, 0, NULL, NULL)"#)
            .bind(id)
            .bind("test")
            .execute(pool)
            .await
            .unwrap();
        // Keep Bilingual import used to satisfy lint
        let _ = Bilingual { es: "x".into(), en: None };
    }

    fn sample_asset(id: &str, meeting_id: &str) -> MeetingAsset {
        MeetingAsset {
            id: id.into(),
            meeting_id: meeting_id.into(),
            kind: "audio".into(),
            path: "C:/test.wav".into(),
            bytes: 1024,
            mime_type: Some("audio/wav".into()),
            created_at: "2026-05-19T00:00:00Z".into(),
        }
    }

    #[tokio::test]
    async fn creates_and_retrieves_asset() {
        let pool = in_memory_pool().await.unwrap();
        ensure_schema(&pool).await.unwrap();
        seed_meeting(&pool, "m-1").await;
        let repo = MeetingAssetsRepo(&pool);
        repo.create(&sample_asset("a-1", "m-1")).await.unwrap();
        let assets = repo.list_by_meeting("m-1").await.unwrap();
        assert_eq!(assets.len(), 1);
        assert_eq!(assets[0].path, "C:/test.wav");
    }

    #[tokio::test]
    async fn get_audio_returns_only_audio_kind() {
        let pool = in_memory_pool().await.unwrap();
        ensure_schema(&pool).await.unwrap();
        seed_meeting(&pool, "m-1").await;
        let repo = MeetingAssetsRepo(&pool);
        let mut transcript = sample_asset("a-1", "m-1");
        transcript.kind = "transcript".into();
        let audio = sample_asset("a-2", "m-1");
        repo.create(&transcript).await.unwrap();
        repo.create(&audio).await.unwrap();
        let got = repo.get_audio("m-1").await.unwrap().unwrap();
        assert_eq!(got.id, "a-2");
    }

    #[tokio::test]
    async fn delete_returns_path_and_removes_row() {
        let pool = in_memory_pool().await.unwrap();
        ensure_schema(&pool).await.unwrap();
        seed_meeting(&pool, "m-1").await;
        let repo = MeetingAssetsRepo(&pool);
        repo.create(&sample_asset("a-1", "m-1")).await.unwrap();
        let path = repo.delete("a-1").await.unwrap();
        assert_eq!(path.as_deref(), Some("C:/test.wav"));
        assert!(repo.list_by_meeting("m-1").await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn cascade_delete_meetings_drops_assets() {
        let pool = in_memory_pool().await.unwrap();
        ensure_schema(&pool).await.unwrap();
        seed_meeting(&pool, "m-1").await;
        let repo = MeetingAssetsRepo(&pool);
        repo.create(&sample_asset("a-1", "m-1")).await.unwrap();
        sqlx::query("DELETE FROM meetings WHERE id = ?")
            .bind("m-1")
            .execute(&pool)
            .await
            .unwrap();
        assert!(repo.list_by_meeting("m-1").await.unwrap().is_empty());
    }
}
```

- [ ] **Step 2: Add module declaration to `src-tauri/crates/db/src/repos/mod.rs`**

```rust
pub mod meeting_assets_repo;
pub use meeting_assets_repo::MeetingAssetsRepo;
```

(Preserve the existing order of `mod` and `use` declarations — append below the existing entries.)

- [ ] **Step 3: Run tests**

```bash
cd src-tauri && cargo test -p smart-noter-db meeting_assets_repo
```

Expected: 4 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/crates/db/src/repos/
git commit -m "feat(db): MeetingAssetsRepo — create, list_by_meeting, get_audio, delete"
```

---

### Task 1.5: `MeetingsRepo::create_with_asset` transactional helper

**Files:**
- Modify: `src-tauri/crates/db/src/repos/meetings_repo.rs`

- [ ] **Step 1: Append failing test to the existing `#[cfg(test)] mod tests` in `meetings_repo.rs`**

```rust
#[tokio::test]
async fn create_with_asset_writes_both_rows_atomically() {
    use smart_noter_core::{Bilingual, Meeting, MeetingAsset};
    let pool = in_memory_pool().await.unwrap();
    ensure_schema(&pool).await.unwrap();
    let repo = MeetingsRepo(&pool);
    let meeting = Meeting {
        id: "m-tx-1".into(),
        title: Bilingual { es: "TX test".into(), en: None },
        template: "tecnica".into(),
        date: "2026-05-19T00:00:00Z".into(),
        duration_sec: 42,
        device_used: None,
        word_count: 0,
        summary: None,
        decisions: vec![],
        blockers: vec![],
    };
    let asset = MeetingAsset {
        id: "a-tx-1".into(),
        meeting_id: "m-tx-1".into(),
        kind: "audio".into(),
        path: "C:/tx.wav".into(),
        bytes: 999,
        mime_type: Some("audio/wav".into()),
        created_at: "2026-05-19T00:00:00Z".into(),
    };
    repo.create_with_asset(&meeting, &asset).await.unwrap();

    let assets = super::MeetingAssetsRepo(&pool)
        .list_by_meeting("m-tx-1")
        .await
        .unwrap();
    assert_eq!(assets.len(), 1);
    assert_eq!(assets[0].id, "a-tx-1");
}
```

(Adjust the `Meeting` struct shape to match the actual fields in `core/src/models/meeting.rs` — pull the actual model into the test verbatim.)

- [ ] **Step 2: Add the method to `impl MeetingsRepo<'_>`**

```rust
pub async fn create_with_asset(
    &self,
    meeting: &smart_noter_core::Meeting,
    asset: &smart_noter_core::MeetingAsset,
) -> Result<(), smart_noter_core::AppError> {
    let mut tx = self.0.begin().await
        .map_err(|e| smart_noter_core::AppError::Database(e.to_string()))?;

    // Re-use the existing single-insert query body. If MeetingsRepo::create
    // exists already, factor its body into a helper that accepts a tx; if not,
    // inline the INSERT here.
    sqlx::query(
        r#"INSERT INTO meetings (id, title_es, title_en, template, date, duration_sec,
                                 device_used, word_count, summary_es, summary_en)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&meeting.id)
    .bind(&meeting.title.es)
    .bind(meeting.title.en.as_deref())
    .bind(&meeting.template)
    .bind(&meeting.date)
    .bind(meeting.duration_sec)
    .bind(meeting.device_used.as_deref())
    .bind(meeting.word_count)
    .bind(meeting.summary.as_ref().map(|s| s.es.clone()))
    .bind(meeting.summary.as_ref().and_then(|s| s.en.clone()))
    .execute(&mut *tx)
    .await
    .map_err(|e| smart_noter_core::AppError::Database(e.to_string()))?;

    sqlx::query(
        r#"INSERT INTO meeting_assets (id, meeting_id, kind, path, bytes, mime_type, created_at)
           VALUES (?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&asset.id)
    .bind(&asset.meeting_id)
    .bind(&asset.kind)
    .bind(&asset.path)
    .bind(asset.bytes)
    .bind(asset.mime_type.as_deref())
    .bind(&asset.created_at)
    .execute(&mut *tx)
    .await
    .map_err(|e| smart_noter_core::AppError::Database(e.to_string()))?;

    tx.commit().await
        .map_err(|e| smart_noter_core::AppError::Database(e.to_string()))?;
    Ok(())
}
```

If `MeetingsRepo::create` already exists (it should, from Foundation), the inserts above intentionally duplicate the SQL to avoid refactoring `create` mid-plan. Phase 7 can fold them into one shared helper as cleanup.

- [ ] **Step 3: Run test**

```bash
cd src-tauri && cargo test -p smart-noter-db meetings_repo
```

Expected: existing tests + 1 new passing.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/crates/db/src/repos/meetings_repo.rs
git commit -m "feat(db): MeetingsRepo::create_with_asset — transactional meeting + asset insert"
```

---

### Phase 1 quality gate

Before moving to Phase 2, run the convention check from "Working conventions §5":

```bash
cd src-tauri && cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
```

Expected: all green, ~16 Rust tests passing (12 from Foundation + 4 new in this phase).

---

## Phase 2 — Audio crate scaffolding (pure-logic modules)

### Task 2.1: `src-tauri/crates/audio/src/error.rs`

**Files:**
- Create: `src-tauri/crates/audio/src/error.rs`

- [ ] **Step 1: Write tests + implementation**

```rust
//! Errors for the audio crate. They convert into `smart_noter_core::AppError::Audio`
//! at the Tauri command boundary; this module's variants carry richer context
//! (WASAPI HRESULT, drop counters, etc.) that gets summarised before the IPC hop.

use serde::{Deserialize, Serialize};
use smart_noter_core::{AppError, AudioErrorCode};
use thiserror::Error;

#[derive(Debug, Error, Serialize, Deserialize, Clone)]
#[serde(tag = "code", content = "message")]
pub enum AudioError {
    #[error("Device not found: {0}")]
    DeviceNotFound(String),

    #[error("Failed to initialize WASAPI: HRESULT={hresult:#x}")]
    WasapiInit { hresult: i32 },

    #[error("Format unsupported by device: {0}")]
    FormatUnsupported(String),

    #[error("Disk full while writing to {path}")]
    DiskFull { path: String },

    #[error("Recording session already active")]
    AlreadyRecording,

    #[error("No active recording session")]
    NotRecording,

    #[error("Audio pipeline overflow (dropped {dropped} frames)")]
    MixerOverflow { dropped: u32 },

    #[error("Unknown audio error: {0}")]
    Other(String),
}

impl From<AudioError> for AppError {
    fn from(e: AudioError) -> Self {
        let code = match &e {
            AudioError::DeviceNotFound(_) => AudioErrorCode::DeviceNotFound,
            AudioError::WasapiInit { .. } => AudioErrorCode::WasapiInit,
            AudioError::FormatUnsupported(_) => AudioErrorCode::FormatUnsupported,
            AudioError::DiskFull { .. } => AudioErrorCode::DiskFull,
            AudioError::AlreadyRecording => AudioErrorCode::AlreadyRecording,
            AudioError::NotRecording => AudioErrorCode::NotRecording,
            AudioError::MixerOverflow { .. } => AudioErrorCode::MixerOverflow,
            AudioError::Other(_) => AudioErrorCode::Other,
        };
        AppError::Audio {
            code,
            message: e.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_with_tagged_code() {
        let e = AudioError::DeviceNotFound("loopback-1".into());
        let json = serde_json::to_string(&e).unwrap();
        assert_eq!(json, r#"{"code":"DeviceNotFound","message":"loopback-1"}"#);
    }

    #[test]
    fn into_app_error_preserves_code() {
        let app: AppError = AudioError::DiskFull {
            path: "C:/x.wav".into(),
        }
        .into();
        match app {
            AppError::Audio { code, message } => {
                assert_eq!(code, AudioErrorCode::DiskFull);
                assert!(message.contains("C:/x.wav"));
            }
            other => panic!("expected Audio variant, got {other:?}"),
        }
    }

    #[test]
    fn mixer_overflow_format_is_helpful() {
        let e = AudioError::MixerOverflow { dropped: 137 };
        assert_eq!(format!("{e}"), "Audio pipeline overflow (dropped 137 frames)");
    }
}
```

- [ ] **Step 2: Add `pub mod error;` to `src-tauri/crates/audio/src/lib.rs`**

For now `lib.rs` contains only `pub fn version() -> &'static str { "0.1.0" }`. Replace it with:

```rust
//! Smart Noter audio capture — WASAPI/cpal-backed crate.

pub mod error;

pub use error::AudioError;
```

- [ ] **Step 3: Run tests**

```bash
cd src-tauri && cargo test -p smart-noter-audio
```

Expected: 3 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/crates/audio/src/
git commit -m "feat(audio): AudioError + From<AudioError> for AppError"
```

---

### Task 2.2: Device enumeration (`devices.rs`)

**Files:**
- Create: `src-tauri/crates/audio/src/devices.rs`
- Modify: `src-tauri/crates/audio/src/lib.rs`

Note: this is a WASAPI/cpal-touching module. We ship implementation + a single smoke test gated behind `cfg(feature = "audio-integration")` that ONLY runs when CI passes `--features audio-integration` AND the runner has audio hardware. The unit test is small: `kind_for_endpoint_id()` is the only pure-logic helper that gets a real test.

- [ ] **Step 1: Write `devices.rs`**

```rust
//! Device enumeration. Combines WASAPI render endpoints (for loopback capture)
//! with cpal input devices (for microphones). Produces a `Vec<AudioDevice>`
//! with deterministic ids so the same physical device gets the same id across
//! sessions.

use crate::error::AudioError;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::hash::{Hash, Hasher};

#[derive(Debug, Clone, Type, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AudioDeviceKind {
    Loopback,
    Input,
}

#[derive(Debug, Clone, Type, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioDevice {
    pub id: String,
    pub name: String,
    pub kind: AudioDeviceKind,
    pub sample_rate: u32,
    pub channels: u16,
    pub is_default: bool,
    pub recommended: bool,
}

/// Stable hash for an endpoint identifier (WASAPI endpoint ID or cpal device name).
/// Used as the device `id` so the same device gets the same id across runs.
pub fn stable_id_for(endpoint: &str, kind: AudioDeviceKind) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    endpoint.hash(&mut hasher);
    let suffix = match kind {
        AudioDeviceKind::Loopback => "L",
        AudioDeviceKind::Input => "I",
    };
    format!("d-{suffix}-{:016x}", hasher.finish())
}

/// Enumerate all available capture devices.
pub fn enumerate() -> Result<Vec<AudioDevice>, AudioError> {
    let mut out = Vec::new();
    out.extend(enumerate_loopback()?);
    out.extend(enumerate_input()?);
    Ok(out)
}

fn enumerate_loopback() -> Result<Vec<AudioDevice>, AudioError> {
    use wasapi::{get_default_device, DeviceCollection, Direction};

    let default_endpoint_id = get_default_device(&Direction::Render)
        .ok()
        .and_then(|d| d.get_id().ok());

    let collection = DeviceCollection::new(&Direction::Render)
        .map_err(|e| AudioError::WasapiInit { hresult: e.hresult().0 })?;

    let mut out = Vec::new();
    for i in 0..collection.get_nbr_devices().unwrap_or(0) {
        let device = match collection.get_device_at_index(i) {
            Ok(d) => d,
            Err(_) => continue,
        };
        let name = device.get_friendlyname().unwrap_or_else(|_| "Unknown".into());
        let endpoint_id = device.get_id().unwrap_or_else(|_| name.clone());
        let id = stable_id_for(&endpoint_id, AudioDeviceKind::Loopback);
        let format = device
            .get_iaudioclient()
            .and_then(|c| c.get_mixformat())
            .ok();
        let (sr, ch) = format
            .map(|f| (f.get_samplespersec(), f.get_nchannels()))
            .unwrap_or((48_000, 2));
        out.push(AudioDevice {
            id,
            name,
            kind: AudioDeviceKind::Loopback,
            sample_rate: sr,
            channels: ch,
            is_default: Some(&endpoint_id) == default_endpoint_id.as_ref(),
            recommended: true, // loopback is the primary use case
        });
    }
    Ok(out)
}

fn enumerate_input() -> Result<Vec<AudioDevice>, AudioError> {
    use cpal::traits::{DeviceTrait, HostTrait};
    let host = cpal::default_host();
    let default_input = host.default_input_device().and_then(|d| d.name().ok());

    let mut out = Vec::new();
    let devices = host.input_devices()
        .map_err(|e| AudioError::Other(format!("cpal input_devices: {e}")))?;
    for d in devices {
        let name = d.name().unwrap_or_else(|_| "Unknown".into());
        let config = d.default_input_config().ok();
        let (sr, ch) = config
            .map(|c| (c.sample_rate().0, c.channels()))
            .unwrap_or((48_000, 1));
        out.push(AudioDevice {
            id: stable_id_for(&name, AudioDeviceKind::Input),
            is_default: default_input.as_ref() == Some(&name),
            name,
            kind: AudioDeviceKind::Input,
            sample_rate: sr,
            channels: ch,
            recommended: false,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_id_is_deterministic() {
        let a = stable_id_for("{0.0.1.00000000}.{abc}", AudioDeviceKind::Loopback);
        let b = stable_id_for("{0.0.1.00000000}.{abc}", AudioDeviceKind::Loopback);
        assert_eq!(a, b);
        assert!(a.starts_with("d-L-"));
    }

    #[test]
    fn stable_id_differs_per_kind() {
        let l = stable_id_for("dev-x", AudioDeviceKind::Loopback);
        let i = stable_id_for("dev-x", AudioDeviceKind::Input);
        assert_ne!(l, i);
    }

    /// Smoke test — enumerate returns at least the default render endpoint
    /// on Windows hosts. Excluded from default `cargo test` since CI runners
    /// may not have audio hardware.
    #[cfg(feature = "audio-integration")]
    #[test]
    fn enumerate_returns_nonempty_on_windows() {
        let devices = enumerate().unwrap();
        assert!(!devices.is_empty());
    }
}
```

- [ ] **Step 2: Re-export `AudioDevice` + `AudioDeviceKind` from `lib.rs`**

Append to `src-tauri/crates/audio/src/lib.rs`:

```rust
pub mod devices;
pub use devices::{enumerate, AudioDevice, AudioDeviceKind};
```

- [ ] **Step 3: Run tests**

```bash
cd src-tauri && cargo test -p smart-noter-audio devices
```

Expected: 2 tests pass (the `audio-integration` test is gated off by default).

- [ ] **Step 4: Commit**

```bash
git add src-tauri/crates/audio/src/
git commit -m "feat(audio): devices.rs — WASAPI loopback + cpal input enumeration with stable ids"
```

---

### Task 2.3: WAV writer (`capture/writer.rs`)

**Files:**
- Create: `src-tauri/crates/audio/src/capture/mod.rs` (just `pub mod writer;` for now)
- Create: `src-tauri/crates/audio/src/capture/writer.rs`
- Modify: `src-tauri/crates/audio/src/lib.rs` (add `pub mod capture;`)

- [ ] **Step 1: Write `writer.rs` with WAV-only support first**

```rust
//! Format-agnostic writer trait + concrete WAV implementation.
//! FLAC follows in the next task.

use crate::error::AudioError;
use hound::{SampleFormat, WavSpec, WavWriter};
use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;

pub trait AudioWriter: Send {
    fn write(&mut self, samples: &[f32]) -> Result<(), AudioError>;
    fn finalize(self: Box<Self>) -> Result<FinalizeResult, AudioError>;
}

#[derive(Debug, Clone)]
pub struct FinalizeResult {
    pub path: PathBuf,
    pub bytes: u64,
    pub sample_count: u64,
}

pub struct WavWriterImpl {
    inner: Option<WavWriter<BufWriter<File>>>,
    path: PathBuf,
    sample_count: u64,
}

impl WavWriterImpl {
    pub fn create(
        path: PathBuf,
        sample_rate: u32,
        channels: u16,
    ) -> Result<Self, AudioError> {
        let spec = WavSpec {
            channels,
            sample_rate,
            bits_per_sample: 16,
            sample_format: SampleFormat::Int,
        };
        let writer = WavWriter::create(&path, spec)
            .map_err(|e| AudioError::Other(format!("WAV create: {e}")))?;
        Ok(Self {
            inner: Some(writer),
            path,
            sample_count: 0,
        })
    }
}

impl AudioWriter for WavWriterImpl {
    fn write(&mut self, samples: &[f32]) -> Result<(), AudioError> {
        let writer = self
            .inner
            .as_mut()
            .ok_or_else(|| AudioError::Other("writer already finalized".into()))?;
        for &s in samples {
            let clipped = s.clamp(-1.0, 1.0);
            let i = (clipped * 32_767.0) as i16;
            writer
                .write_sample(i)
                .map_err(|e| classify_io_error(e, &self.path))?;
        }
        self.sample_count += samples.len() as u64;
        Ok(())
    }

    fn finalize(mut self: Box<Self>) -> Result<FinalizeResult, AudioError> {
        let inner = self
            .inner
            .take()
            .ok_or_else(|| AudioError::Other("writer already finalized".into()))?;
        inner
            .finalize()
            .map_err(|e| AudioError::Other(format!("WAV finalize: {e}")))?;
        let bytes = std::fs::metadata(&self.path)
            .map(|m| m.len())
            .unwrap_or(0);
        Ok(FinalizeResult {
            path: self.path,
            bytes,
            sample_count: self.sample_count,
        })
    }
}

fn classify_io_error(e: hound::Error, path: &PathBuf) -> AudioError {
    use std::io::ErrorKind;
    if let hound::Error::IoError(io) = &e {
        if matches!(io.kind(), ErrorKind::StorageFull | ErrorKind::Other) {
            return AudioError::DiskFull {
                path: path.display().to_string(),
            };
        }
    }
    AudioError::Other(format!("WAV write: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    fn tmp_path(name: &str) -> PathBuf {
        let dir = std::env::temp_dir();
        dir.join(format!("sn-test-{}-{}.wav", name, std::process::id()))
    }

    #[test]
    fn writes_wav_with_correct_header() {
        let path = tmp_path("header");
        let mut w = WavWriterImpl::create(path.clone(), 48_000, 2).unwrap();
        let samples = [0.0f32; 480];
        w.write(&samples).unwrap();
        let res = Box::new(w).finalize().unwrap();
        assert!(res.bytes > 44, "WAV header is ≥44 bytes plus payload");

        let mut file = std::fs::File::open(&path).unwrap();
        let mut header = [0u8; 12];
        file.read_exact(&mut header).unwrap();
        assert_eq!(&header[0..4], b"RIFF");
        assert_eq!(&header[8..12], b"WAVE");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn write_clamps_oversaturated_samples() {
        let path = tmp_path("clamp");
        let mut w = WavWriterImpl::create(path.clone(), 48_000, 1).unwrap();
        w.write(&[2.0, -2.0, 0.5]).unwrap();
        Box::new(w).finalize().unwrap();
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn finalize_returns_sample_count() {
        let path = tmp_path("count");
        let mut w = WavWriterImpl::create(path.clone(), 48_000, 1).unwrap();
        w.write(&[0.0; 1000]).unwrap();
        let res = Box::new(w).finalize().unwrap();
        assert_eq!(res.sample_count, 1000);
        std::fs::remove_file(&path).ok();
    }
}
```

- [ ] **Step 2: Create `capture/mod.rs`**

```rust
//! Capture session state machine + worker thread pipelines.

pub mod writer;
```

- [ ] **Step 3: Add `pub mod capture;` to `src-tauri/crates/audio/src/lib.rs`**

Replace the contents of `lib.rs` accumulated so far with:

```rust
//! Smart Noter audio capture — WASAPI/cpal-backed crate.

pub mod capture;
pub mod devices;
pub mod error;

pub use devices::{enumerate, AudioDevice, AudioDeviceKind};
pub use error::AudioError;
```

- [ ] **Step 4: Run tests**

```bash
cd src-tauri && cargo test -p smart-noter-audio writer
```

Expected: 3 tests pass.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/crates/audio/src/
git commit -m "feat(audio): WAV writer with AudioWriter trait, clamping, disk-full classification"
```

---

### Task 2.4: FLAC writer

**Files:**
- Modify: `src-tauri/crates/audio/src/capture/writer.rs`

- [ ] **Step 1: Append `FlacWriterImpl` + tests to `writer.rs`**

claxon's API only does encoder open + frame write. The pattern is the same shape as `WavWriterImpl`:

```rust
pub struct FlacWriterImpl {
    inner: Option<claxon::FlacWriter<BufWriter<File>>>,
    path: PathBuf,
    sample_count: u64,
    channels: u16,
}

impl FlacWriterImpl {
    pub fn create(
        path: PathBuf,
        sample_rate: u32,
        channels: u16,
    ) -> Result<Self, AudioError> {
        let file = File::create(&path).map_err(|e| classify_create_error(e, &path))?;
        let buf = BufWriter::new(file);
        let opts = claxon::FlacWriterOptions { sample_rate, channels: channels as u32, bits_per_sample: 16 };
        let writer = claxon::FlacWriter::new(buf, opts)
            .map_err(|e| AudioError::Other(format!("FLAC create: {e}")))?;
        Ok(Self {
            inner: Some(writer),
            path,
            sample_count: 0,
            channels,
        })
    }
}

impl AudioWriter for FlacWriterImpl {
    fn write(&mut self, samples: &[f32]) -> Result<(), AudioError> {
        let writer = self
            .inner
            .as_mut()
            .ok_or_else(|| AudioError::Other("FLAC writer already finalized".into()))?;
        // claxon takes interleaved i32 samples
        let buf: Vec<i32> = samples
            .iter()
            .map(|&s| (s.clamp(-1.0, 1.0) * 32_767.0) as i32)
            .collect();
        writer
            .write_interleaved(&buf)
            .map_err(|e| AudioError::Other(format!("FLAC write: {e}")))?;
        self.sample_count += samples.len() as u64;
        Ok(())
    }

    fn finalize(mut self: Box<Self>) -> Result<FinalizeResult, AudioError> {
        let inner = self
            .inner
            .take()
            .ok_or_else(|| AudioError::Other("FLAC writer already finalized".into()))?;
        inner
            .finish()
            .map_err(|e| AudioError::Other(format!("FLAC finalize: {e}")))?;
        let bytes = std::fs::metadata(&self.path)
            .map(|m| m.len())
            .unwrap_or(0);
        Ok(FinalizeResult {
            path: self.path,
            bytes,
            sample_count: self.sample_count,
        })
    }
}

fn classify_create_error(e: std::io::Error, path: &PathBuf) -> AudioError {
    use std::io::ErrorKind;
    if matches!(e.kind(), ErrorKind::StorageFull) {
        AudioError::DiskFull {
            path: path.display().to_string(),
        }
    } else {
        AudioError::Other(format!("create {}: {}", path.display(), e))
    }
}
```

> **API caveat:** The claxon crate (v0.4) ships a `FlacReader` but its `FlacWriter` API may vary by minor version. Verify the exact constructor & method names from the published docs and adjust signatures verbatim. If `FlacWriter` is not available (claxon is decoder-focused), swap to the `flac-bound` crate (binding to libFLAC) or `flacenc` (pure Rust). Update Cargo.toml accordingly and keep this file's tests in sync.

- [ ] **Step 2: Append FLAC tests**

```rust
#[test]
fn flac_writer_writes_and_finalizes() {
    let path = std::env::temp_dir().join(format!("sn-flac-test-{}.flac", std::process::id()));
    let mut w = FlacWriterImpl::create(path.clone(), 48_000, 1).unwrap();
    w.write(&[0.0; 480]).unwrap();
    let res = Box::new(w).finalize().unwrap();
    assert!(res.bytes > 0);
    // FLAC magic: "fLaC" at offset 0
    let mut file = std::fs::File::open(&path).unwrap();
    let mut magic = [0u8; 4];
    use std::io::Read;
    file.read_exact(&mut magic).unwrap();
    assert_eq!(&magic, b"fLaC");
    std::fs::remove_file(&path).ok();
}
```

- [ ] **Step 3: Run tests**

```bash
cd src-tauri && cargo test -p smart-noter-audio writer
```

Expected: 4 tests pass (3 WAV + 1 FLAC).

- [ ] **Step 4: Commit**

```bash
git add src-tauri/crates/audio/src/capture/writer.rs
git commit -m "feat(audio): FLAC writer alongside WAV — both behind AudioWriter trait"
```

---

### Task 2.5: Meter (RMS + peak + decimation)

**Files:**
- Create: `src-tauri/crates/audio/src/capture/meter.rs`
- Modify: `src-tauri/crates/audio/src/capture/mod.rs`

- [ ] **Step 1: Write `meter.rs`**

```rust
//! RMS + peak meter and waveform decimator.
//!
//! Consumes f32 sample buffers and exposes:
//! * `level()`        — last-block RMS + peak (0..1 normalised)
//! * `waveform()`     — rolling buffer of 36 normalised bins (one per ~100 ms)
//! * `samples_total()` — running count of samples observed (driver of `elapsed`)

use std::collections::VecDeque;

const WAVEFORM_BINS: usize = 36;

#[derive(Debug, Clone, Copy, Default)]
pub struct Level {
    pub rms: f32,
    pub peak: f32,
}

pub struct Meter {
    sample_rate: u32,
    channels: u16,
    // ~100 ms accumulator at the source sample rate. For 48 kHz mono → 4800 samples.
    bin_accum: Vec<f32>,
    bin_target: usize,
    bins: VecDeque<f32>,
    samples_total: u64,
    last_level: Level,
}

impl Meter {
    pub fn new(sample_rate: u32, channels: u16) -> Self {
        let frames_per_bin = (sample_rate as f32 * 0.100) as usize; // ~100 ms
        Self {
            sample_rate,
            channels,
            bin_accum: Vec::with_capacity(frames_per_bin * channels as usize),
            bin_target: frames_per_bin * channels as usize,
            bins: VecDeque::from(vec![0.0; WAVEFORM_BINS]),
            samples_total: 0,
            last_level: Level::default(),
        }
    }

    /// Push a block of **interleaved** f32 samples from all channels. For stereo at
    /// 48 kHz one second of audio = 96 000 samples (48 000 L+R frames × 2 channels).
    pub fn push(&mut self, samples: &[f32]) {
        if samples.is_empty() {
            return;
        }
        self.samples_total += samples.len() as u64;
        // Update last_level from this block.
        let mut sum_sq = 0.0;
        let mut peak: f32 = 0.0;
        for &s in samples {
            sum_sq += s * s;
            let a = s.abs();
            if a > peak {
                peak = a;
            }
        }
        let rms = (sum_sq / samples.len() as f32).sqrt();
        self.last_level = Level {
            rms: rms.min(1.0),
            peak: peak.min(1.0),
        };
        // Accumulate into the waveform bin.
        self.bin_accum.extend_from_slice(samples);
        while self.bin_accum.len() >= self.bin_target {
            let drained: Vec<f32> = self.bin_accum.drain(..self.bin_target).collect();
            let bin_peak = drained.iter().fold(0.0f32, |acc, &s| acc.max(s.abs()));
            if self.bins.len() == WAVEFORM_BINS {
                self.bins.pop_front();
            }
            self.bins.push_back(bin_peak.min(1.0));
        }
    }

    pub fn level(&self) -> Level {
        self.last_level
    }

    pub fn waveform(&self) -> Vec<f32> {
        self.bins.iter().copied().collect()
    }

    /// Total samples observed across all `push` calls. Includes every channel sample
    /// (i.e. interleaved, not frame-count).
    pub fn samples_total(&self) -> u64 {
        self.samples_total
    }

    pub fn elapsed_sec(&self) -> u32 {
        if self.sample_rate == 0 || self.channels == 0 {
            return 0;
        }
        (self.samples_total / (self.sample_rate as u64 * self.channels as u64)) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rms_of_sine_wave() {
        let mut m = Meter::new(48_000, 1);
        // 1000 samples of sine at amplitude 0.5
        let samples: Vec<f32> = (0..1000)
            .map(|i| (i as f32 * 0.1).sin() * 0.5)
            .collect();
        m.push(&samples);
        let lvl = m.level();
        assert!((lvl.rms - 0.353).abs() < 0.05, "RMS≈0.353 for 0.5 sine, got {}", lvl.rms);
    }

    #[test]
    fn peak_clamps_to_one() {
        let mut m = Meter::new(48_000, 1);
        m.push(&[0.9, -0.95, 0.3, 0.5]);
        assert!((m.level().peak - 0.95).abs() < 0.001);
    }

    #[test]
    fn waveform_starts_with_36_zero_bins() {
        let m = Meter::new(48_000, 1);
        assert_eq!(m.waveform().len(), 36);
        assert!(m.waveform().iter().all(|&v| v == 0.0));
    }

    #[test]
    fn pushing_one_bin_worth_advances_history() {
        let mut m = Meter::new(48_000, 1);
        m.push(&vec![0.5; 4800]); // 100 ms at 48 kHz mono
        let wf = m.waveform();
        assert_eq!(wf.len(), 36);
        assert!((wf[35] - 0.5).abs() < 0.001, "last bin should be the new peak");
        assert_eq!(wf[0], 0.0, "oldest bin should still be the seed zero");
    }

    #[test]
    fn elapsed_advances_per_sample_count() {
        let mut m = Meter::new(48_000, 2); // stereo
        m.push(&vec![0.0; 96_000]); // 0.5 s of stereo
        assert_eq!(m.elapsed_sec(), 0); // 96_000 / (48_000 * 2) = 1 second? no = 1
        // Recompute: 96000 samples / 48000 sr / 2 ch = 1 sec
        // (the comment was misleading; the assertion below verifies)
        assert_eq!(m.elapsed_sec(), 1);
    }
}
```

- [ ] **Step 2: Add `pub mod meter;` to `capture/mod.rs`**

```rust
//! Capture session state machine + worker thread pipelines.

pub mod meter;
pub mod writer;
```

- [ ] **Step 3: Run tests**

```bash
cd src-tauri && cargo test -p smart-noter-audio meter
```

Expected: 5 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/crates/audio/src/capture/
git commit -m "feat(audio): Meter — RMS/peak per block + 36-bin rolling waveform + elapsed counter"
```

---

### Task 2.6: Mixer (Mix mode resample + sum)

**Files:**
- Create: `src-tauri/crates/audio/src/capture/mixer.rs`
- Modify: `src-tauri/crates/audio/src/capture/mod.rs`

- [ ] **Step 1: Write `mixer.rs`**

```rust
//! Combines two source streams into one. Handles sample-rate mismatch via
//! rubato (FFT-based SRC), then sums sample-by-sample with anti-clipping gain.

use crate::error::AudioError;
use rubato::{FftFixedIn, Resampler};

pub const TARGET_SAMPLE_RATE: u32 = 48_000;
pub const ANTI_CLIP_GAIN: f32 = 0.7;

pub struct Mixer {
    a_resampler: Option<FftFixedIn<f32>>,
    b_resampler: Option<FftFixedIn<f32>>,
    a_in_rate: u32,
    b_in_rate: u32,
}

impl Mixer {
    pub fn new(a_in_rate: u32, b_in_rate: u32) -> Result<Self, AudioError> {
        let make = |in_rate: u32| -> Result<Option<FftFixedIn<f32>>, AudioError> {
            if in_rate == TARGET_SAMPLE_RATE {
                Ok(None)
            } else {
                FftFixedIn::<f32>::new(
                    in_rate as usize,
                    TARGET_SAMPLE_RATE as usize,
                    /* chunk */ 1024,
                    /* sub_chunks */ 2,
                    /* channels */ 1,
                )
                .map(Some)
                .map_err(|e| AudioError::Other(format!("rubato init: {e}")))
            }
        };
        Ok(Self {
            a_resampler: make(a_in_rate)?,
            b_resampler: make(b_in_rate)?,
            a_in_rate,
            b_in_rate,
        })
    }

    /// Resample both buffers (if needed) and sum with anti-clip gain.
    /// Both buffers are expected to be mono. Returns the mixed mono output at TARGET_SAMPLE_RATE.
    pub fn mix(&mut self, a: &[f32], b: &[f32]) -> Result<Vec<f32>, AudioError> {
        let a_out = if let Some(r) = &mut self.a_resampler {
            let chunks = vec![a.to_vec()];
            let out = r.process(&chunks, None)
                .map_err(|e| AudioError::Other(format!("rubato A: {e}")))?;
            out.into_iter().next().unwrap_or_default()
        } else {
            a.to_vec()
        };
        let b_out = if let Some(r) = &mut self.b_resampler {
            let chunks = vec![b.to_vec()];
            let out = r.process(&chunks, None)
                .map_err(|e| AudioError::Other(format!("rubato B: {e}")))?;
            out.into_iter().next().unwrap_or_default()
        } else {
            b.to_vec()
        };
        let n = a_out.len().min(b_out.len());
        let mut mixed = Vec::with_capacity(n);
        for i in 0..n {
            mixed.push((a_out[i] + b_out[i]) * ANTI_CLIP_GAIN);
        }
        Ok(mixed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_rate_passthrough_no_resampler() {
        let m = Mixer::new(48_000, 48_000).unwrap();
        assert!(m.a_resampler.is_none() && m.b_resampler.is_none());
    }

    #[test]
    fn different_rate_creates_resampler() {
        let m = Mixer::new(44_100, 48_000).unwrap();
        assert!(m.a_resampler.is_some());
        assert!(m.b_resampler.is_none());
    }

    #[test]
    fn mix_sums_with_gain() {
        let mut m = Mixer::new(48_000, 48_000).unwrap();
        let a = vec![0.5; 100];
        let b = vec![0.3; 100];
        let out = m.mix(&a, &b).unwrap();
        assert_eq!(out.len(), 100);
        let expected = (0.5 + 0.3) * ANTI_CLIP_GAIN;
        assert!((out[0] - expected).abs() < 0.001);
    }

    #[test]
    fn mix_truncates_to_shorter_buffer() {
        let mut m = Mixer::new(48_000, 48_000).unwrap();
        let a = vec![0.5; 80];
        let b = vec![0.3; 100];
        let out = m.mix(&a, &b).unwrap();
        assert_eq!(out.len(), 80);
    }

    #[test]
    fn resample_44k_to_48k_changes_length() {
        let mut m = Mixer::new(44_100, 48_000).unwrap();
        // rubato 0.15 FftFixedIn with chunk=1024, sub_chunks=2 produces FFT chunks
        // of 588 in / 640 out (gcd=300, fft_chunks=4). For 1024 input frames:
        // floor(1024/588) * 640 = 640.
        let a = vec![0.5; 1024];
        let b = vec![0.0; 640];
        let out = m.mix(&a, &b).unwrap();
        assert_eq!(out.len(), 640);
    }

    #[test]
    fn mix_completes_under_10ms_for_1s_of_audio() {
        let mut m = Mixer::new(48_000, 48_000).unwrap();
        let a = vec![0.1; 48_000];
        let b = vec![0.2; 48_000];
        let start = std::time::Instant::now();
        let out = m.mix(&a, &b).unwrap();
        let dt = start.elapsed();
        assert_eq!(out.len(), 48_000);
        assert!(dt.as_millis() < 10, "expected <10ms, got {} ms", dt.as_millis());
    }
}
```

- [ ] **Step 2: Add `pub mod mixer;` to `capture/mod.rs`**

```rust
//! Capture session state machine + worker thread pipelines.

pub mod meter;
pub mod mixer;
pub mod writer;
```

- [ ] **Step 3: Run tests**

```bash
cd src-tauri && cargo test -p smart-noter-audio mixer
```

Expected: 6 tests pass (perf test depends on machine; if it fails on CI, raise the budget).

- [ ] **Step 4: Commit**

```bash
git add src-tauri/crates/audio/src/capture/
git commit -m "feat(audio): Mixer — rubato resample + summed mix with 0.7 anti-clip gain"
```

---

### Task 2.7: CaptureSession state machine

**Files:**
- Create: `src-tauri/crates/audio/src/capture/session.rs`
- Modify: `src-tauri/crates/audio/src/capture/mod.rs`

This module owns the state machine. Worker threads + cpal/wasapi callbacks come in the next phase; here we only model the transitions so tests are pure.

- [ ] **Step 1: Write `session.rs`**

```rust
//! `CaptureSession` is the source of truth for the audio recording state machine.
//! Audio callbacks, writer thread, meter thread and the Tauri commands all
//! manipulate it through these methods. The methods do not block on I/O.

use crate::error::AudioError;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Type, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CaptureMode {
    System,
    Mic,
    Mix,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Type, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AudioFormat {
    Wav,
    Flac,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaptureState {
    Idle,
    Preview { device_id: String },
    Recording { session_id: String, paused: bool },
    Stopped { session_id: String, tmp_path: PathBuf, bytes: u64, duration_sec: u32 },
}

pub struct CaptureSession {
    state: CaptureState,
}

impl Default for CaptureSession {
    fn default() -> Self {
        Self {
            state: CaptureState::Idle,
        }
    }
}

impl CaptureSession {
    pub fn state(&self) -> &CaptureState {
        &self.state
    }

    pub fn begin_preview(&mut self, device_id: String) -> Result<(), AudioError> {
        match self.state {
            CaptureState::Idle => {
                self.state = CaptureState::Preview { device_id };
                Ok(())
            }
            _ => Err(AudioError::AlreadyRecording),
        }
    }

    pub fn end_preview(&mut self) {
        if matches!(self.state, CaptureState::Preview { .. }) {
            self.state = CaptureState::Idle;
        }
    }

    pub fn begin_recording(&mut self, session_id: String) -> Result<(), AudioError> {
        match self.state {
            CaptureState::Idle | CaptureState::Preview { .. } => {
                self.state = CaptureState::Recording {
                    session_id,
                    paused: false,
                };
                Ok(())
            }
            _ => Err(AudioError::AlreadyRecording),
        }
    }

    pub fn pause(&mut self) -> Result<(), AudioError> {
        match &mut self.state {
            CaptureState::Recording { paused, .. } if !*paused => {
                *paused = true;
                Ok(())
            }
            _ => Err(AudioError::NotRecording),
        }
    }

    pub fn resume(&mut self) -> Result<(), AudioError> {
        match &mut self.state {
            CaptureState::Recording { paused, .. } if *paused => {
                *paused = false;
                Ok(())
            }
            _ => Err(AudioError::NotRecording),
        }
    }

    pub fn is_paused(&self) -> bool {
        matches!(self.state, CaptureState::Recording { paused: true, .. })
    }

    pub fn current_session_id(&self) -> Option<&str> {
        match &self.state {
            CaptureState::Recording { session_id, .. } => Some(session_id),
            CaptureState::Stopped { session_id, .. } => Some(session_id),
            _ => None,
        }
    }

    pub fn stop(
        &mut self,
        tmp_path: PathBuf,
        bytes: u64,
        duration_sec: u32,
    ) -> Result<(), AudioError> {
        match &self.state {
            CaptureState::Recording { session_id, .. } => {
                self.state = CaptureState::Stopped {
                    session_id: session_id.clone(),
                    tmp_path,
                    bytes,
                    duration_sec,
                };
                Ok(())
            }
            _ => Err(AudioError::NotRecording),
        }
    }

    pub fn take_finished(&mut self) -> Option<(String, PathBuf, u64, u32)> {
        match std::mem::replace(&mut self.state, CaptureState::Idle) {
            CaptureState::Stopped {
                session_id,
                tmp_path,
                bytes,
                duration_sec,
            } => Some((session_id, tmp_path, bytes, duration_sec)),
            other => {
                self.state = other;
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    #[test]
    fn idle_to_recording_to_stopped_happy_path() {
        let mut s = CaptureSession::default();
        s.begin_recording("sess-1".into()).unwrap();
        assert!(matches!(s.state, CaptureState::Recording { .. }));
        s.stop(p("/tmp/x.wav"), 1024, 5).unwrap();
        assert!(matches!(s.state, CaptureState::Stopped { .. }));
    }

    #[test]
    fn cannot_start_recording_while_already_recording() {
        let mut s = CaptureSession::default();
        s.begin_recording("sess-1".into()).unwrap();
        let err = s.begin_recording("sess-2".into()).unwrap_err();
        assert!(matches!(err, AudioError::AlreadyRecording));
    }

    #[test]
    fn pause_then_resume() {
        let mut s = CaptureSession::default();
        s.begin_recording("sess-1".into()).unwrap();
        s.pause().unwrap();
        assert!(s.is_paused());
        s.resume().unwrap();
        assert!(!s.is_paused());
    }

    #[test]
    fn double_pause_errors() {
        let mut s = CaptureSession::default();
        s.begin_recording("sess-1".into()).unwrap();
        s.pause().unwrap();
        let err = s.pause().unwrap_err();
        assert!(matches!(err, AudioError::NotRecording));
    }

    #[test]
    fn stop_without_recording_errors() {
        let mut s = CaptureSession::default();
        let err = s.stop(p("/x"), 0, 0).unwrap_err();
        assert!(matches!(err, AudioError::NotRecording));
    }

    #[test]
    fn take_finished_returns_payload_once() {
        let mut s = CaptureSession::default();
        s.begin_recording("sess-1".into()).unwrap();
        s.stop(p("/tmp/x.wav"), 999, 7).unwrap();
        let first = s.take_finished().unwrap();
        assert_eq!(first.0, "sess-1");
        assert_eq!(first.2, 999);
        assert!(s.take_finished().is_none());
        assert_eq!(s.state, CaptureState::Idle);
    }

    #[test]
    fn preview_lifecycle_does_not_block_recording_start() {
        let mut s = CaptureSession::default();
        s.begin_preview("dev-1".into()).unwrap();
        s.begin_recording("sess-1".into()).unwrap();
        assert!(matches!(s.state, CaptureState::Recording { .. }));
    }
}
```

- [ ] **Step 2: Wire `pub mod session;` + re-exports in `capture/mod.rs`**

```rust
//! Capture session state machine + worker thread pipelines.

pub mod meter;
pub mod mixer;
pub mod session;
pub mod writer;

pub use session::{AudioFormat, CaptureMode, CaptureSession, CaptureState};
```

- [ ] **Step 3: Run tests**

```bash
cd src-tauri && cargo test -p smart-noter-audio session
```

Expected: 7 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/crates/audio/src/capture/
git commit -m "feat(audio): CaptureSession state machine — Idle/Preview/Recording/Stopped"
```

---

### Phase 2 quality gate

```bash
cd src-tauri && cargo test -p smart-noter-audio && cargo clippy -p smart-noter-audio --all-targets -- -D warnings
```

Expected: 19 unit tests pass (3 error + 2 devices + 4 writer + 6 mixer + 5 meter + 7 session = 27 actually; recount as you go).

---

## Phase 3 — Audio capture streams + Tauri events

### Task 3.1: Stream opener (`capture/stream.rs`)

**Files:**
- Create: `src-tauri/crates/audio/src/capture/stream.rs`
- Modify: `src-tauri/crates/audio/src/capture/mod.rs`

This is the largest WASAPI/cpal-touching module. The unit-test surface is limited — we provide one sanity test for the public `StreamHandle::sample_rate()` accessor; the heavy work is validated by the manual smoke checklist.

- [ ] **Step 1: Write `stream.rs`**

```rust
//! Opens audio input streams per `CaptureMode` and pushes samples to a channel.
//!
//! - `System`: WASAPI loopback on the selected render endpoint.
//! - `Mic`:    cpal default-host input device matched by name (the device id
//!             stored in our `AudioDevice.name` field).
//! - `Mix`:    both of the above, each feeding a separate channel that the
//!             mixer thread consumes.
//!
//! The audio callback does ONE allocation (`buf.to_vec()`) and then
//! `try_send(...)` on a bounded channel. Drops are counted and surfaced via
//! `audio:error` event when they exceed the threshold (see meter thread).

use crate::devices::{enumerate, AudioDeviceKind};
use crate::error::AudioError;
use crate::capture::session::CaptureMode;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossbeam_channel::Sender;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

pub struct StreamHandle {
    /// Sample rate the stream is delivering (might differ from device's mix format).
    pub sample_rate: u32,
    pub channels: u16,
    pub drops: Arc<AtomicU32>,
    /// Keep handles alive so the OS doesn't drop the stream.
    _streams: Vec<Box<dyn KeepAlive>>,
}

/// Marker trait so we can put cpal::Stream and wasapi handles in the same Vec.
trait KeepAlive: Send {}

struct CpalStream(cpal::Stream);
impl KeepAlive for CpalStream {}
// SAFETY: cpal::Stream is `!Send` by default; we erase via Box<dyn KeepAlive>.
// In practice we keep it on the thread that opened it; do NOT move handles across threads.
unsafe impl Send for CpalStream {}

/// Open one or two streams depending on the mode.
///
/// Returns a handle whose drop closes the streams.
pub fn open(
    mode: CaptureMode,
    device_id: &str,
    tx_a: Sender<Vec<f32>>,
    tx_b: Option<Sender<Vec<f32>>>,
) -> Result<StreamHandle, AudioError> {
    match mode {
        CaptureMode::System => open_loopback(device_id, tx_a),
        CaptureMode::Mic => open_mic(device_id, tx_a),
        CaptureMode::Mix => {
            let tx_b = tx_b.ok_or_else(|| {
                AudioError::Other("Mix mode requires a second channel sender".into())
            })?;
            // device_id is the loopback id; mic picks the system default for now
            let loop_handle = open_loopback(device_id, tx_a)?;
            let mic_handle = open_mic_default(tx_b)?;
            // Combine streams' keepalive boxes
            let mut streams = loop_handle._streams;
            streams.extend(mic_handle._streams);
            Ok(StreamHandle {
                sample_rate: loop_handle.sample_rate,
                channels: 1, // mixed output is mono
                drops: loop_handle.drops,
                _streams: streams,
            })
        }
    }
}

fn open_loopback(device_id: &str, tx: Sender<Vec<f32>>) -> Result<StreamHandle, AudioError> {
    // Look up the WASAPI endpoint by our stable id, then open in loopback mode.
    // wasapi's API for loopback (per its examples):
    //   1. AudioClient::initialize_client with stream flags STREAM_FLAGS_LOOPBACK
    //   2. Read captures via AudioCaptureClient
    //   3. Spawn a thread that calls get_buffer in a loop
    let devices = enumerate()?;
    let target = devices
        .iter()
        .find(|d| d.id == device_id && d.kind == AudioDeviceKind::Loopback)
        .ok_or_else(|| AudioError::DeviceNotFound(device_id.to_string()))?;

    // Resolve back to a wasapi::Device by enumerating render endpoints and
    // matching by friendly name (the name stored in our AudioDevice).
    use wasapi::{DeviceCollection, Direction};
    let coll = DeviceCollection::new(&Direction::Render).map_err(|e| match e {
        // Plan amended (Task 3.1): wasapi 0.16 uses WasapiError::Windows(inner),
        // not e.hresult().0.  Pattern established in Phase 2 Task 2.2 (devices.rs).
        wasapi::WasapiError::Windows(inner) => AudioError::WasapiInit {
            hresult: inner.code().0,
        },
        other => AudioError::Other(format!("WASAPI: {other}")),
    })?;

    let count = coll.get_nbr_devices().unwrap_or(0);
    let mut wasapi_dev = None;
    for i in 0..count {
        if let Ok(d) = coll.get_device_at_index(i) {
            if d.get_friendlyname().ok().as_deref() == Some(target.name.as_str()) {
                wasapi_dev = Some(d);
                break;
            }
        }
    }
    let device = wasapi_dev.ok_or_else(|| AudioError::DeviceNotFound(device_id.to_string()))?;

    // For brevity (and because the wasapi-rs API is verbose) the actual
    // loopback initialisation logic — `get_iaudioclient()`, `initialize_client`,
    // `get_audiocaptureclient`, the polling thread — is encapsulated in a
    // private helper below. This helper sends samples to `tx` as Vec<f32>.
    let drops = Arc::new(AtomicU32::new(0));
    let drops_clone = drops.clone();
    let sample_rate = target.sample_rate;
    let channels = target.channels;
    let handle = spawn_wasapi_loopback_thread(device, sample_rate, channels, tx, drops_clone)?;

    Ok(StreamHandle {
        sample_rate,
        channels,
        drops,
        _streams: vec![Box::new(WasapiStreamThread(handle))],
    })
}

fn open_mic(device_id: &str, tx: Sender<Vec<f32>>) -> Result<StreamHandle, AudioError> {
    let host = cpal::default_host();
    let devices = enumerate()?;
    let target = devices
        .iter()
        .find(|d| d.id == device_id && d.kind == AudioDeviceKind::Input)
        .ok_or_else(|| AudioError::DeviceNotFound(device_id.to_string()))?;

    let device = host
        .input_devices()
        .map_err(|e| AudioError::Other(format!("cpal input_devices: {e}")))?
        .find(|d| d.name().ok().as_deref() == Some(target.name.as_str()))
        .ok_or_else(|| AudioError::DeviceNotFound(device_id.to_string()))?;

    let config = device
        .default_input_config()
        .map_err(|e| AudioError::FormatUnsupported(e.to_string()))?;
    let sample_rate = config.sample_rate().0;
    let channels = config.channels();

    let drops = Arc::new(AtomicU32::new(0));
    let drops_clone = drops.clone();

    let stream = device
        .build_input_stream(
            &config.into(),
            move |data: &[f32], _| {
                if tx.try_send(data.to_vec()).is_err() {
                    drops_clone.fetch_add(1, Ordering::Relaxed);
                }
            },
            |err| {
                tracing::error!(?err, "cpal input stream error");
            },
            None,
        )
        .map_err(|e| AudioError::Other(format!("cpal build_input_stream: {e}")))?;
    stream
        .play()
        .map_err(|e| AudioError::Other(format!("cpal play: {e}")))?;

    Ok(StreamHandle {
        sample_rate,
        channels,
        drops,
        _streams: vec![Box::new(CpalStream(stream))],
    })
}

fn open_mic_default(tx: Sender<Vec<f32>>) -> Result<StreamHandle, AudioError> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or_else(|| AudioError::DeviceNotFound("system default input".into()))?;
    let config = device
        .default_input_config()
        .map_err(|e| AudioError::FormatUnsupported(e.to_string()))?;
    let sample_rate = config.sample_rate().0;
    let channels = config.channels();

    let drops = Arc::new(AtomicU32::new(0));
    let drops_clone = drops.clone();
    let stream = device
        .build_input_stream(
            &config.into(),
            move |data: &[f32], _| {
                if tx.try_send(data.to_vec()).is_err() {
                    drops_clone.fetch_add(1, Ordering::Relaxed);
                }
            },
            |err| {
                tracing::error!(?err, "cpal default input stream error");
            },
            None,
        )
        .map_err(|e| AudioError::Other(format!("cpal build_input_stream: {e}")))?;
    stream
        .play()
        .map_err(|e| AudioError::Other(format!("cpal play: {e}")))?;

    Ok(StreamHandle {
        sample_rate,
        channels,
        drops,
        _streams: vec![Box::new(CpalStream(stream))],
    })
}

// Plan amended (Task 3.1):
// - WasapiStreamThread refactored to hold stop: Arc<AtomicBool> + handle: Option<JoinHandle<()>>
//   so its Drop impl can signal the thread and join it (a bare JoinHandle drop only detaches).
// - wasapi::Device is !Send; wrapped in a SendDevice newtype (unsafe impl Send) with a SAFETY
//   comment explaining MTA COM rules. Passed to spawn() as SendDevice, unwrapped inside thread.
// - Drift 2: placeholder replaced with real wasapi_loopback_loop() that initialises MTA,
//   calls get_iaudioclient / WaveFormat::new(32,32,Float) / initialize_client / set_get_eventhandle /
//   get_audiocaptureclient / start_stream, then loops on h_event.wait_for_event(100) draining
//   packets via read_from_device, reinterprets raw bytes as f32 (native — no conversion),
//   and stop_stream on exit.
// - f32 native preferred over i16-and-convert (WaveFormat::new(32,32,&SampleType::Float,...)).

/// Newtype wrapper that makes `wasapi::Device` sendable across threads.
///
/// SAFETY: `wasapi::Device` wraps an `IMMDevice` COM pointer. COM objects in the
/// MTA can be accessed from any thread in that MTA. The loopback thread calls
/// `initialize_mta()` before touching the device. We transfer ownership to exactly
/// one thread and never share the pointer.
struct SendDevice(wasapi::Device);
unsafe impl Send for SendDevice {}

struct WasapiStreamThread {
    stop: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}
impl KeepAlive for WasapiStreamThread {}
impl Drop for WasapiStreamThread {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

fn spawn_wasapi_loopback_thread(
    device: SendDevice,
    sample_rate: u32,
    channels: u16,
    tx: Sender<Vec<f32>>,
    drops: Arc<AtomicU32>,
    stop: Arc<AtomicBool>,
) -> Result<std::thread::JoinHandle<()>, AudioError> {
    let handle = std::thread::Builder::new()
        .name("wasapi-loopback".into())
        .spawn(move || {
            if let Err(e) =
                wasapi_loopback_loop(device, sample_rate, channels, &tx, &drops, &stop)
            {
                tracing::error!("WASAPI loopback thread exited with error: {e}");
            }
        })
        .map_err(|e| AudioError::Other(format!("spawn loopback thread: {e}")))?;
    Ok(handle)
}

fn wasapi_loopback_loop(
    device: SendDevice,
    sample_rate: u32,
    channels: u16,
    tx: &Sender<Vec<f32>>,
    drops: &Arc<AtomicU32>,
    stop: &Arc<AtomicBool>,
) -> Result<(), AudioError> {
    use wasapi::{Direction, SampleType, ShareMode, WaveFormat};
    let _ = wasapi::initialize_mta(); // S_FALSE if already MTA — not an error
    let device = device.0;
    let mut audio_client = device.get_iaudioclient().map_err(/* WasapiError pattern */)?;
    let desired_format = WaveFormat::new(32, 32, &SampleType::Float, sample_rate as usize, channels as usize, None);
    let (_, min_time) = audio_client.get_periods().map_err(/* WasapiError pattern */)?;
    audio_client.initialize_client(&desired_format, min_time, &Direction::Capture, &ShareMode::Shared, true).map_err(/* WasapiError pattern */)?;
    let h_event = audio_client.set_get_eventhandle().map_err(/* WasapiError pattern */)?;
    let capture_client = audio_client.get_audiocaptureclient().map_err(/* WasapiError pattern */)?;
    let blockalign = desired_format.get_blockalign() as usize;
    audio_client.start_stream().map_err(/* WasapiError pattern */)?;
    let mut raw_buf: Vec<u8> = Vec::new();
    loop {
        if stop.load(Ordering::Relaxed) { break; }
        if h_event.wait_for_event(100).is_err() { continue; } // timeout = no data
        loop {
            match capture_client.get_next_nbr_frames() {
                Ok(Some(0)) | Ok(None) => break,
                Ok(Some(n)) => {
                    let needed = n as usize * blockalign;
                    raw_buf.resize(needed, 0);
                    if let Ok((read, _)) = capture_client.read_from_device(&mut raw_buf[..needed]) {
                        if read > 0 {
                            let bytes = read as usize * blockalign;
                            let samples: Vec<f32> = raw_buf[..bytes].chunks_exact(4)
                                .map(|c| f32::from_le_bytes([c[0],c[1],c[2],c[3]]))
                                .collect();
                            if tx.try_send(samples).is_err() { drops.fetch_add(1, Ordering::Relaxed); }
                        }
                    }
                }
                Err(_) => break,
            }
        }
    }
    let _ = audio_client.stop_stream();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stream_handle_struct_is_constructable() {
        let (tx, _rx) = crossbeam_channel::bounded::<Vec<f32>>(1);
        let drops = Arc::new(AtomicU32::new(0));
        let h = StreamHandle {
            sample_rate: 48_000,
            channels: 2,
            drops: drops.clone(),
            _streams: vec![],
        };
        assert_eq!(h.sample_rate, 48_000);
        assert_eq!(h.channels, 2);
        let _ = tx;
    }
}
```

> **Implementation note for the executor:** The placeholder body of `spawn_wasapi_loopback_thread` MUST be replaced with the real WASAPI loopback loop. Read `wasapi-rs` crate's `examples/loopback.rs` (linked from the crate docs on docs.rs) and port it line by line. Do not skip the event-handle setup — polling without the event drains CPU.

- [ ] **Step 2: Add `pub mod stream;` to `capture/mod.rs`**

```rust
//! Capture session state machine + worker thread pipelines.

pub mod meter;
pub mod mixer;
pub mod session;
pub mod stream;
pub mod writer;

pub use session::{AudioFormat, CaptureMode, CaptureSession, CaptureState};
```

- [ ] **Step 3: cargo check + tests**

```bash
cd src-tauri && cargo check -p smart-noter-audio && cargo test -p smart-noter-audio
```

Expected: build green, all existing tests pass + the 1 new unit test in `stream.rs`.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/crates/audio/src/capture/
git commit -m "feat(audio): stream.rs — open WASAPI loopback / cpal mic / both (Mix) with bounded MPSC"
```

---

### Task 3.2: Public `RecordingHandle` + worker orchestration

**Files:**
- Create: `src-tauri/crates/audio/src/capture/recorder.rs`
- Modify: `src-tauri/crates/audio/src/capture/mod.rs`
- Modify: `src-tauri/crates/audio/Cargo.toml` — add `tauri = { workspace = true }` (required because `Recorder` uses `tauri::{AppHandle, Emitter}` directly)

This is the "wire everything together" task. The `Recorder` struct owns the stream(s), spawns the writer + meter worker threads, and emits events through a Tauri `AppHandle`.

- [ ] **Step 1: Write `recorder.rs`**

```rust
//! Orchestrates the audio worker pipeline.
//!
//! Owns:
//!   * `StreamHandle` from `stream.rs` (one or two streams)
//!   * The bounded MPSC channels between callback → mixer (if Mix) → writer + meter
//!   * Worker thread join handles + a shutdown flag
//!
//! Emits Tauri events on a background thread via `AppHandle::emit`.

use crate::capture::meter::Meter;
use crate::capture::mixer::Mixer;
use crate::capture::session::{AudioFormat, CaptureMode};
use crate::capture::stream::{open, StreamHandle};
use crate::capture::writer::{AudioWriter, FlacWriterImpl, FinalizeResult, WavWriterImpl};
use crate::error::AudioError;
use crossbeam_channel::{bounded, Receiver, Sender};
use parking_lot::Mutex;
use serde::Serialize;
use specta::Type;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LevelEvent {
    pub rms: f32,
    pub peak: f32,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct WaveformEvent {
    pub bins: Vec<f32>,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ElapsedEvent {
    pub elapsed_sec: u32,
}

pub struct Recorder {
    pub stream: StreamHandle,
    pub writer_join: Option<JoinHandle<Result<FinalizeResult, AudioError>>>,
    pub meter_join: Option<JoinHandle<()>>,
    pub paused: Arc<AtomicBool>,
    pub stop_flag: Arc<AtomicBool>,
    pub tmp_path: PathBuf,
}

impl Recorder {
    pub fn start(
        app: AppHandle,
        mode: CaptureMode,
        device_id: String,
        format: AudioFormat,
        tmp_path: PathBuf,
    ) -> Result<Self, AudioError> {
        let (sample_tx, sample_rx) = bounded::<Vec<f32>>(64);

        // Open stream(s); for Mix mode we also wire a mixer thread between
        // the two source channels and `sample_tx`.
        let stream = if matches!(mode, CaptureMode::Mix) {
            let (a_tx, a_rx) = bounded::<Vec<f32>>(64);
            let (b_tx, b_rx) = bounded::<Vec<f32>>(64);
            let handle = open(mode, &device_id, a_tx, Some(b_tx))?;

            // Spawn mixer thread.
            let mixer_sample_rate_a = handle.sample_rate;
            let mixer_sample_rate_b = 48_000; // default mic; could be refined per device
            let sample_tx_for_mixer = sample_tx.clone();
            std::thread::spawn(move || {
                let mut mixer = match Mixer::new(mixer_sample_rate_a, mixer_sample_rate_b) {
                    Ok(m) => m,
                    Err(e) => {
                        tracing::error!(?e, "mixer init");
                        return;
                    }
                };
                loop {
                    let a = match a_rx.recv() {
                        Ok(v) => v,
                        Err(_) => return,
                    };
                    let b = match b_rx.recv_timeout(Duration::from_millis(50)) {
                        Ok(v) => v,
                        Err(_) => Vec::new(),
                    };
                    if let Ok(mixed) = mixer.mix(&a, &b) {
                        let _ = sample_tx_for_mixer.try_send(mixed);
                    }
                }
            });
            handle
        } else {
            open(mode, &device_id, sample_tx.clone(), None)?
        };

        let paused = Arc::new(AtomicBool::new(false));
        let stop_flag = Arc::new(AtomicBool::new(false));

        // Spawn writer thread.
        let mut writer: Box<dyn AudioWriter> = match format {
            AudioFormat::Wav => Box::new(WavWriterImpl::create(
                tmp_path.clone(),
                stream.sample_rate,
                stream.channels,
            )?),
            AudioFormat::Flac => Box::new(FlacWriterImpl::create(
                tmp_path.clone(),
                stream.sample_rate,
                stream.channels,
            )?),
        };

        let writer_paused = paused.clone();
        let writer_stop = stop_flag.clone();
        let writer_rx = sample_rx.clone();
        let writer_join = std::thread::spawn(move || -> Result<FinalizeResult, AudioError> {
            loop {
                if writer_stop.load(Ordering::Relaxed) {
                    break;
                }
                match writer_rx.recv_timeout(Duration::from_millis(50)) {
                    Ok(buf) => {
                        if !writer_paused.load(Ordering::Relaxed) {
                            writer.write(&buf)?;
                        }
                    }
                    Err(crossbeam_channel::RecvTimeoutError::Timeout) => continue,
                    Err(_) => break,
                }
            }
            writer.finalize()
        });

        // Spawn meter thread.
        let meter_app = app.clone();
        let meter_stop = stop_flag.clone();
        let meter_sample_rate = stream.sample_rate;
        let meter_channels = stream.channels;
        let meter_rx = sample_rx; // shared with writer via clone above (broadcast-ish via send to one consumer)
        // ^ Note: crossbeam_channel is MPMC — both writer and meter compete for samples.
        //   To get broadcast semantics, we'd need to send a clone of each buffer to a
        //   second channel. For simplicity (and because the meter only needs a sample
        //   of the stream, not every sample), we accept some lossiness: writer gets
        //   priority. If this is unacceptable, switch to two channels and have the
        //   stream thread send each buf to both. Sub-2's manual smoke test verifies
        //   the meter still drives realistic level + waveform with this trade-off.
        let meter_join = std::thread::spawn(move || {
            let mut meter = Meter::new(meter_sample_rate, meter_channels);
            let mut last_level = Instant::now();
            let mut last_wave = Instant::now();
            let mut last_elapsed = Instant::now();
            loop {
                if meter_stop.load(Ordering::Relaxed) {
                    break;
                }
                if let Ok(buf) = meter_rx.recv_timeout(Duration::from_millis(50)) {
                    meter.push(&buf);
                }
                let now = Instant::now();
                if now.duration_since(last_level) >= Duration::from_millis(50) {
                    last_level = now;
                    let lvl = meter.level();
                    let _ = meter_app.emit("audio:level", LevelEvent { rms: lvl.rms, peak: lvl.peak });
                }
                if now.duration_since(last_wave) >= Duration::from_millis(100) {
                    last_wave = now;
                    let _ = meter_app.emit("audio:waveform-bin", WaveformEvent { bins: meter.waveform() });
                }
                if now.duration_since(last_elapsed) >= Duration::from_millis(1000) {
                    last_elapsed = now;
                    let _ = meter_app.emit("audio:elapsed", ElapsedEvent { elapsed_sec: meter.elapsed_sec() });
                }
            }
        });

        Ok(Self {
            stream,
            writer_join: Some(writer_join),
            meter_join: Some(meter_join),
            paused,
            stop_flag,
            tmp_path,
        })
    }

    pub fn pause(&self) {
        self.paused.store(true, Ordering::Relaxed);
    }

    pub fn resume(&self) {
        self.paused.store(false, Ordering::Relaxed);
    }

    pub fn stop(mut self) -> Result<(PathBuf, u64, u32), AudioError> {
        self.stop_flag.store(true, Ordering::Relaxed);

        let writer = self.writer_join.take().unwrap().join()
            .map_err(|_| AudioError::Other("writer thread panicked".into()))??;
        let _ = self.meter_join.take().unwrap().join();

        let duration_sec = (writer.sample_count
            / (self.stream.sample_rate as u64).max(1)
            / (self.stream.channels as u64).max(1)) as u32;
        Ok((writer.path, writer.bytes, duration_sec))
    }
}

// Recorder must be Send so it can sit inside `tauri::State<Arc<Mutex<...>>>`.
// All members above are already Send-safe; the cpal::Stream is wrapped in
// `CpalStream` which has an `unsafe impl Send`.
```

> The "broadcast-ish via send to one consumer" comment in the code is honest about a known trade-off. The spec's threading model assumed broadcast. If you find the meter visibly stutters during smoke tests, change the stream.rs callback to do `tx_writer.try_send(buf.clone())` + `tx_meter.try_send(buf)` — two channels, each with its own receiver. Pick that up in the smoke-test feedback loop.

- [ ] **Step 2: Add `pub mod recorder;` to `capture/mod.rs`**

```rust
//! Capture session state machine + worker thread pipelines.

pub mod meter;
pub mod mixer;
pub mod recorder;
pub mod session;
pub mod stream;
pub mod writer;

pub use recorder::{ElapsedEvent, LevelEvent, Recorder, WaveformEvent};
pub use session::{AudioFormat, CaptureMode, CaptureSession, CaptureState};
```

- [ ] **Step 3: cargo check (no tests; this module is exercised by manual smoke)**

```bash
cd src-tauri && cargo check -p smart-noter-audio
```

Expected: build green.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/crates/audio/src/capture/
git commit -m "feat(audio): Recorder — wires StreamHandle + writer/meter threads + Tauri events"
```

---

### Task 3.3: Integration test — WAV round-trip

**Files:**
- Create: `src-tauri/crates/audio/tests/wav_roundtrip.rs`

- [ ] **Step 1: Write the integration test**

```rust
//! Generates 1 second of 440 Hz sine, writes WAV, reads it back, asserts samples
//! match within epsilon. Validates writer.rs end-to-end without touching WASAPI.

use smart_noter_audio::capture::writer::{AudioWriter, WavWriterImpl};
use std::path::PathBuf;

fn tmp(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("sn-roundtrip-{}-{}.wav", name, std::process::id()))
}

#[test]
fn sine_440_round_trip_matches_within_epsilon() {
    let path = tmp("sine440");
    let sample_rate = 48_000u32;
    let n = sample_rate as usize;
    let samples: Vec<f32> = (0..n)
        .map(|i| {
            let t = i as f32 / sample_rate as f32;
            (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.5
        })
        .collect();

    let mut w = WavWriterImpl::create(path.clone(), sample_rate, 1).unwrap();
    w.write(&samples).unwrap();
    let res = Box::new(w).finalize().unwrap();
    assert_eq!(res.sample_count, n as u64);

    let mut reader = hound::WavReader::open(&path).unwrap();
    assert_eq!(reader.spec().sample_rate, sample_rate);
    assert_eq!(reader.spec().channels, 1);

    let read_back: Vec<f32> = reader
        .samples::<i16>()
        .map(|s| s.unwrap() as f32 / 32_767.0)
        .collect();
    assert_eq!(read_back.len(), n);

    // Compare samples — quantisation gives ~3e-5 max error
    let max_err = read_back
        .iter()
        .zip(samples.iter())
        .map(|(a, b)| (a - b).abs())
        .fold(0.0f32, f32::max);
    assert!(
        max_err < 1e-3,
        "max diff after round-trip: {max_err}"
    );

    std::fs::remove_file(&path).ok();
}
```

- [ ] **Step 2: Run integration tests**

```bash
cd src-tauri && cargo test -p smart-noter-audio --test wav_roundtrip
```

Expected: 1 test pass.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/crates/audio/tests/wav_roundtrip.rs
git commit -m "test(audio): integration — 1s 440Hz sine WAV round-trip matches within ε"
```

---

### Phase 3 quality gate

```bash
cd src-tauri && cargo fmt --check && cargo clippy -p smart-noter-audio --all-targets -- -D warnings && cargo test -p smart-noter-audio
```

Expected: all green, ~28 tests pass (1 new integration).

---

## Phase 4 — Tauri commands + IPC

### Task 4.1: AppState additions for audio

**Files:**
- Modify: `src-tauri/src/state.rs`
- Modify: `src-tauri/src/main.rs`

- [ ] **Step 1: Find the existing `AppState` and add the audio fields**

Append to the struct in `state.rs` (alongside the existing `pool: SqlitePool`):

```rust
use parking_lot::Mutex;
use smart_noter_audio::capture::session::CaptureSession;
use smart_noter_audio::capture::recorder::Recorder;
use std::sync::Arc;

pub struct AppState {
    pub pool: sqlx::SqlitePool,
    pub capture_session: Arc<Mutex<CaptureSession>>,
    pub recorder: Arc<Mutex<Option<Recorder>>>,
}
```

Construct the new fields in the `AppState::new` (or wherever the state is built):

```rust
AppState {
    pool,
    capture_session: Arc::new(Mutex::new(CaptureSession::default())),
    recorder: Arc::new(Mutex::new(None)),
}
```

- [ ] **Step 2: Commit (no tests; types-only change)**

```bash
git add src-tauri/src/state.rs src-tauri/src/main.rs
git commit -m "feat(state): AppState gains capture_session + recorder slots for Sub-2"
```

---

### Task 4.2: `enumerate_audio_devices` command (replaces seed-backed one)

**Files:**
- Modify: `src-tauri/src/commands/devices.rs` (or wherever `list_audio_devices` lives)
- Modify: `src-tauri/src/main.rs` (specta builder registers the same handler)

- [ ] **Step 1: Replace the body**

Find the existing `list_audio_devices` (Foundation reads from `audio_devices` table). Replace its body with:

```rust
#[tauri::command]
#[specta::specta]
pub fn list_audio_devices() -> Result<Vec<smart_noter_audio::AudioDevice>, smart_noter_core::AppError> {
    smart_noter_audio::enumerate().map_err(Into::into)
}
```

Drop the now-unused `pool` parameter and the SELECT.

- [ ] **Step 2: Update `MeetingsRepo` / `seed.rs` so the table no longer gets seeded** (or delete the seed for `audio_devices` since the table is now unused). Drop the old `audio_devices` table in a new migration if you want, but it's lower-risk to leave it orphaned for Sub-2.

- [ ] **Step 3: Regenerate bindings**

```bash
cd src-tauri && cargo run --bin specta-export
```

Expected: `src/ipc/bindings.ts` shows the new `AudioDevice` (kind + sampleRate + channels + isDefault + recommended) and no more `Bilingual` on `name`/`desc`.

- [ ] **Step 4: Run TS build to confirm the frontend still compiles (will likely fail in PreRecord & Settings; fix in Phase 5)**

```bash
pnpm build 2>&1 | head -40
```

Expected: errors in `PreRecordPage.tsx` referencing `pickL(device.name, ...)` and `device.icon`. That's OK — Phase 5 fixes them.

- [ ] **Step 5: Commit (frontend will be red until Phase 5; that's the cost of moving the contract)**

```bash
git add src-tauri/src/commands/ src-tauri/src/main.rs
git commit -m "feat(ipc): list_audio_devices now backed by real WASAPI/cpal enumeration"
```

---

### Task 4.3: `start_preview` + `stop_preview`

**Files:**
- Modify: `src-tauri/src/commands/audio.rs` (create file if doesn't exist)
- Modify: `src-tauri/src/main.rs` (register commands)

- [ ] **Step 1: Add the two commands**

```rust
use smart_noter_audio::capture::recorder::Recorder;
use smart_noter_audio::capture::session::{AudioFormat, CaptureMode};
use smart_noter_core::AppError;

#[tauri::command]
#[specta::specta]
pub fn start_preview(
    state: tauri::State<'_, crate::state::AppState>,
    app: tauri::AppHandle,
    device_id: String,
    capture_mode: CaptureMode,
) -> Result<(), AppError> {
    // Preview is just a recorder pointed at a discard sink (writer writes to a
    // throwaway tmp file we delete on stop_preview). Cheap.
    let mut session = state.capture_session.lock();
    session.begin_preview(device_id.clone()).map_err(AppError::from)?;

    let tmp = std::env::temp_dir().join(format!("sn-preview-{}.wav", std::process::id()));
    let recorder = Recorder::start(app, capture_mode, device_id, AudioFormat::Wav, tmp)
        .map_err(AppError::from)?;
    *state.recorder.lock() = Some(recorder);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn stop_preview(state: tauri::State<'_, crate::state::AppState>) -> Result<(), AppError> {
    state.capture_session.lock().end_preview();
    if let Some(rec) = state.recorder.lock().take() {
        if let Ok((path, _, _)) = rec.stop() {
            let _ = std::fs::remove_file(path);
        }
    }
    Ok(())
}
```

- [ ] **Step 2: Register in `main.rs`'s specta builder**

```rust
.invoke_handler(tauri::generate_handler![
    // ...existing commands...
    commands::audio::start_preview,
    commands::audio::stop_preview,
])
```

And in the `specta_builder`:

```rust
.commands(specta::collect_commands![
    // ...existing...
    commands::audio::start_preview,
    commands::audio::stop_preview,
])
```

- [ ] **Step 3: Regenerate bindings + cargo check**

```bash
cd src-tauri && cargo check && cargo run --bin specta-export
```

Expected: bindings.ts now exposes `commands.startPreview` and `commands.stopPreview` with correct signatures.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands/audio.rs src-tauri/src/main.rs
git commit -m "feat(ipc): start_preview / stop_preview commands for PreRecord audio preview"
```

---

### Task 4.4: `start_recording` / `pause_recording` / `resume_recording`

**Files:**
- Modify: `src-tauri/src/commands/audio.rs`

- [ ] **Step 1: Append commands to `audio.rs`**

```rust
#[derive(serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RecordingStartedDto {
    pub session_id: String,
    pub sample_rate: u32,
    pub channels: u16,
}

#[tauri::command]
#[specta::specta]
pub fn start_recording(
    state: tauri::State<'_, crate::state::AppState>,
    app: tauri::AppHandle,
    device_id: String,
    capture_mode: CaptureMode,
    format: AudioFormat,
) -> Result<RecordingStartedDto, AppError> {
    let session_id = format!("sess-{}", uuid::Uuid::new_v4());

    // If a preview is running, stop it first (clean transition).
    let _ = stop_preview(state.clone());

    let mut sess = state.capture_session.lock();
    sess.begin_recording(session_id.clone()).map_err(AppError::from)?;

    let tmp_path = audio_dir()?.join(format!("tmp-{session_id}.{ext}", ext = ext_for(format)));
    let recorder = Recorder::start(app, capture_mode, device_id, format, tmp_path)
        .map_err(AppError::from)?;
    let sample_rate = recorder.stream.sample_rate;
    let channels = recorder.stream.channels;
    *state.recorder.lock() = Some(recorder);

    Ok(RecordingStartedDto {
        session_id,
        sample_rate,
        channels,
    })
}

#[tauri::command]
#[specta::specta]
pub fn pause_recording(state: tauri::State<'_, crate::state::AppState>) -> Result<(), AppError> {
    state.capture_session.lock().pause().map_err(AppError::from)?;
    if let Some(rec) = state.recorder.lock().as_ref() {
        rec.pause();
    }
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn resume_recording(state: tauri::State<'_, crate::state::AppState>) -> Result<(), AppError> {
    state.capture_session.lock().resume().map_err(AppError::from)?;
    if let Some(rec) = state.recorder.lock().as_ref() {
        rec.resume();
    }
    Ok(())
}

fn audio_dir() -> Result<std::path::PathBuf, AppError> {
    let dir = dirs::data_dir()
        .ok_or_else(|| AppError::Internal("no APPDATA dir".into()))?
        .join("com.smartnoter.app")
        .join("audio");
    std::fs::create_dir_all(&dir).map_err(|e| AppError::Internal(format!("create audio dir: {e}")))?;
    Ok(dir)
}

fn ext_for(fmt: AudioFormat) -> &'static str {
    match fmt {
        AudioFormat::Wav => "wav",
        AudioFormat::Flac => "flac",
    }
}
```

Add `uuid` + `dirs` to `Cargo.toml` of the `smart-noter` binary crate (top-level `src-tauri/Cargo.toml`):

```toml
uuid = { version = "1", features = ["v4"] }
dirs = "5"
```

- [ ] **Step 2: Register in main.rs** (add to both `invoke_handler` and `collect_commands`)

```rust
commands::audio::start_recording,
commands::audio::pause_recording,
commands::audio::resume_recording,
```

- [ ] **Step 3: Regenerate bindings**

```bash
cd src-tauri && cargo run --bin specta-export
```

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands/audio.rs src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "feat(ipc): start/pause/resume_recording commands wired to Recorder + state machine"
```

---

### Task 4.5: `stop_recording`, `finalize_recording`, `discard_recording`

**Files:**
- Modify: `src-tauri/src/commands/audio.rs`

- [ ] **Step 1: Append**

```rust
#[derive(serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CaptureResult {
    pub session_id: String,
    pub path: String,
    pub bytes: u64,
    pub duration_sec: u32,
}

#[tauri::command]
#[specta::specta]
pub fn stop_recording(state: tauri::State<'_, crate::state::AppState>) -> Result<CaptureResult, AppError> {
    let session_id = state
        .capture_session
        .lock()
        .current_session_id()
        .ok_or_else(|| smart_noter_audio::AudioError::NotRecording)
        .map(|s| s.to_string())
        .map_err(AppError::from)?;
    let rec = state
        .recorder
        .lock()
        .take()
        .ok_or_else(|| smart_noter_audio::AudioError::NotRecording)
        .map_err(AppError::from)?;
    let (path, bytes, duration_sec) = rec.stop().map_err(AppError::from)?;
    state
        .capture_session
        .lock()
        .stop(path.clone(), bytes, duration_sec)
        .map_err(AppError::from)?;
    Ok(CaptureResult {
        session_id,
        path: path.display().to_string(),
        bytes,
        duration_sec,
    })
}

#[tauri::command]
#[specta::specta]
pub async fn finalize_recording(
    state: tauri::State<'_, crate::state::AppState>,
    session_id: String,
    title: String,
    template_id: String,
) -> Result<smart_noter_core::Meeting, AppError> {
    let (sess_id, tmp_path, bytes, duration_sec) = state
        .capture_session
        .lock()
        .take_finished()
        .ok_or_else(|| AppError::Internal("no finished session to finalize".into()))?;
    if sess_id != session_id {
        return Err(AppError::Validation(format!(
            "session_id mismatch: have {sess_id}, got {session_id}"
        )));
    }
    let meeting_id = format!("m-{}", chrono::Utc::now().format("%Y%m%d"));
    let meeting_id = format!("{meeting_id}-{}", &sess_id[5..13]);
    let ext = tmp_path.extension().and_then(|s| s.to_str()).unwrap_or("wav");
    let final_path = audio_dir()?.join(format!("{meeting_id}.{ext}"));
    std::fs::rename(&tmp_path, &final_path)
        .map_err(|e| AppError::Internal(format!("rename {tmp_path:?}: {e}")))?;

    let mime = match ext {
        "wav" => Some("audio/wav".to_string()),
        "flac" => Some("audio/flac".to_string()),
        _ => None,
    };
    let now = chrono::Utc::now().to_rfc3339();
    let meeting = smart_noter_core::Meeting {
        id: meeting_id.clone(),
        title: smart_noter_core::Bilingual { es: title.clone(), en: None },
        template: template_id,
        date: now.clone(),
        duration_sec: duration_sec as i64,
        device_used: None,
        word_count: 0,
        summary: None,
        decisions: vec![],
        blockers: vec![],
    };
    let asset = smart_noter_core::MeetingAsset {
        id: format!("a-{}", uuid::Uuid::new_v4()),
        meeting_id: meeting_id.clone(),
        kind: "audio".into(),
        path: final_path.display().to_string(),
        bytes: bytes as i64,
        mime_type: mime,
        created_at: now,
    };
    smart_noter_db::repos::MeetingsRepo(&state.pool)
        .create_with_asset(&meeting, &asset)
        .await?;
    Ok(meeting)
}

#[tauri::command]
#[specta::specta]
pub fn discard_recording(state: tauri::State<'_, crate::state::AppState>) -> Result<(), AppError> {
    if let Some(rec) = state.recorder.lock().take() {
        if let Ok((path, _, _)) = rec.stop() {
            let _ = std::fs::remove_file(path);
        }
    }
    if let Some((_, tmp_path, _, _)) = state.capture_session.lock().take_finished() {
        let _ = std::fs::remove_file(tmp_path);
    }
    state.capture_session.lock().end_preview(); // safe no-op if not in preview
    Ok(())
}
```

Add `chrono` to `src-tauri/Cargo.toml`:

```toml
chrono = { version = "0.4", features = ["serde"] }
```

- [ ] **Step 2: Register in main.rs** (both lists)

```rust
commands::audio::stop_recording,
commands::audio::finalize_recording,
commands::audio::discard_recording,
```

- [ ] **Step 3: cargo check + regenerate bindings**

```bash
cd src-tauri && cargo check && cargo run --bin specta-export
```

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands/audio.rs src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "feat(ipc): stop_recording + finalize_recording (tx) + discard_recording"
```

---

### Task 4.6: Startup sweep for orphaned `tmp-*` files

**Files:**
- Modify: `src-tauri/src/main.rs`

- [ ] **Step 1: Add a sweep function and call it at startup**

```rust
fn sweep_orphan_tmp_files() {
    if let Some(dir) = dirs::data_dir() {
        let audio = dir.join("com.smartnoter.app").join("audio");
        if let Ok(entries) = std::fs::read_dir(&audio) {
            for e in entries.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                if name.starts_with("tmp-") {
                    let _ = std::fs::remove_file(e.path());
                    tracing::info!("swept orphan {name}");
                }
            }
        }
    }
}
```

Call it from the Tauri setup callback (before the window is shown), e.g.:

```rust
.setup(|app| {
    sweep_orphan_tmp_files();
    // ...existing setup...
    Ok(())
})
```

- [ ] **Step 2: Commit**

```bash
git add src-tauri/src/main.rs
git commit -m "feat(audio): startup sweep removes orphan tmp-* files in audio/"
```

---

### Phase 4 quality gate

```bash
cd src-tauri && cargo check && cargo clippy --workspace --all-targets -- -D warnings
```

Expected: clean. Tauri-side commands are not unit-testable without a `tauri::AppHandle` mock — they get covered by the manual smoke checklist.

---

## Phase 5 — Frontend wiring

### Task 5.1: `SegmentedControl` — per-option `disabled`

**Files:**
- Modify: `src/components/primitives/SegmentedControl/SegmentedControl.tsx`
- Modify: `src/components/primitives/SegmentedControl/SegmentedControl.module.css`
- Modify: `src/components/primitives/SegmentedControl/SegmentedControl.test.tsx`

- [ ] **Step 1: Update the option type + render path**

In `SegmentedControl.tsx`, change:

```tsx
export interface SegmentedOption<T extends string> {
  value: T;
  label: ReactNode;
  disabled?: boolean;
}
```

In the render, pass `disabled` to the button and add a `title` tooltip when disabled:

```tsx
<button
  key={o.value}
  type="button"
  role="tab"
  aria-selected={active}
  aria-disabled={o.disabled || undefined}
  disabled={o.disabled}
  className={`${styles.btn} ${active ? styles.active : ''}`}
  onClick={() => !o.disabled && onChange(o.value)}
  title={o.disabled ? 'Próximamente' : undefined}
>
  {o.label}
</button>
```

- [ ] **Step 2: Add `.btn:disabled` style to the module CSS**

```css
.btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}
```

- [ ] **Step 3: Add a test**

In `SegmentedControl.test.tsx`:

```tsx
it('ignores clicks on disabled options', async () => {
  const onChange = vi.fn();
  const opts: SegmentedOption<string>[] = [
    { value: 'a', label: 'A' },
    { value: 'b', label: 'B', disabled: true },
  ];
  render(<SegmentedControl<string> value="a" options={opts} onChange={onChange} />);
  await userEvent.click(screen.getByRole('tab', { name: 'B' }));
  expect(onChange).not.toHaveBeenCalled();
});
```

- [ ] **Step 4: Run tests**

```bash
pnpm test:run -- src/components/primitives/SegmentedControl
```

Expected: 3 tests pass (was 2, +1).

- [ ] **Step 5: Commit**

```bash
git add src/components/primitives/SegmentedControl/
git commit -m "feat(primitives): SegmentedControl — per-option disabled with Próximamente tooltip"
```

---

### Task 5.2: PreRecord — real device list + preview lifecycle

**Files:**
- Modify: `src/features/pre-record/PreRecordPage.tsx`
- Modify: `src/features/pre-record/PreRecordPage.test.tsx`

- [ ] **Step 1: Update `DeviceCard` and the device list to use the new `AudioDevice` shape**

Remove the `pickL` / `device.icon` usage. Add the icon mapping:

```tsx
import type { AudioDevice, AudioDeviceKind } from '@/ipc/bindings';

const iconFor = (kind: AudioDeviceKind): IconName =>
  kind === 'Loopback' ? 'monitor' : 'headphones';

function DeviceCard({ device, selected, onSelect }: {
  device: AudioDevice;
  selected: boolean;
  onSelect: () => void;
}) {
  const { lang } = useT();
  return (
    <button
      type="button"
      className={`${styles.optCard} ${selected ? styles.optCardSelected : ''}`}
      onClick={onSelect}
    >
      <div className={styles.iconBox}>
        <Icon name={iconFor(device.kind)} size={18} />
      </div>
      <div className={styles.optMeta}>
        <div className={styles.optName}>
          <span>{device.name}</span>
          {device.recommended && (
            <Chip variant="accent" disabled>
              {lang === 'es' ? 'Recomendado' : 'Recommended'}
            </Chip>
          )}
        </div>
        <div className={styles.optDesc}>
          {device.sampleRate / 1000}kHz · {device.channels === 1 ? 'mono' : 'stereo'}
        </div>
      </div>
      <div className={styles.radio} />
    </button>
  );
}
```

- [ ] **Step 2: Add preview lifecycle useEffect**

```tsx
import { invoke } from '@tauri-apps/api/core';

// inside PreRecordPage:
useEffect(() => {
  if (!deviceId) return;
  void invoke('start_preview', { deviceId, captureMode: 'System' });
  return () => { void invoke('stop_preview'); };
}, [deviceId]);
```

- [ ] **Step 3: AudioPreviewCard subscribes to `audio:level`**

```tsx
import { listen } from '@tauri-apps/api/event';

function AudioPreviewCard() {
  const { lang } = useT();
  const [level, setLevel] = useState(0);
  useEffect(() => {
    const un = listen<{ rms: number; peak: number }>('audio:level', e => setLevel(e.payload.rms));
    return () => { un.then(fn => fn()); };
  }, []);

  return (
    <div className={styles.previewCard}>
      <div className={styles.previewHead}>
        <div>
          <div className={styles.previewLabel}>
            {lang === 'es' ? 'Vista previa del audio' : 'Audio preview'}
          </div>
          <div className={styles.previewSub}>
            {lang === 'es'
              ? 'Reproduce algo en tu PC para verificar la señal'
              : 'Play something on your PC to check the signal'}
          </div>
        </div>
        <EqBar bars={8} />
      </div>
      <LevelBar level={level} />
    </div>
  );
}
```

- [ ] **Step 4: Pass captureMode + format to LiveRecording via `location.state`**

In the start button handler:

```tsx
function start() {
  navigate(Paths.LiveRecording(genSessionId()), {
    state: {
      name: name.trim() || (lang === 'es' ? 'Reunión sin título' : 'Untitled meeting'),
      templateId,
      deviceId,
      captureMode: 'System',
      format: settings?.recordingQuality === 'FLAC' ? 'Flac' : 'Wav',
    },
  });
}
```

(Where `settings` comes from `useGetSettingsQuery()`.)

- [ ] **Step 5: Update PreRecordPage.test.tsx**

The existing tests use mocked `list_audio_devices` returning `[]`. Adjust mock so it returns at least one new-shape `AudioDevice`:

```ts
if (cmd === 'list_audio_devices') {
  return [{
    id: 'd-L-test',
    name: 'Test Speakers',
    kind: 'Loopback',
    sampleRate: 48000,
    channels: 2,
    isDefault: true,
    recommended: true,
  }];
}
if (cmd === 'start_preview') return null;
if (cmd === 'stop_preview') return null;
```

- [ ] **Step 6: Run tests**

```bash
pnpm test:run -- src/features/pre-record
```

Expected: 2 tests pass.

- [ ] **Step 7: Commit**

```bash
git add src/features/pre-record/
git commit -m "feat(pre-record): real device list + preview lifecycle + level-driven LevelBar"
```

---

### Task 5.3: LiveRecording — events replace `useLiveTimer`

**Files:**
- Modify: `src/features/live-recording/LiveRecordingPage.tsx`
- Delete (or update): `src/features/live-recording/useLiveTimer.ts` — keep the hook for tests but stop using it in the page
- Modify: `src/features/live-recording/LiveRecordingPage.test.tsx`

- [ ] **Step 1: Replace `useLiveTimer(143)` with event-driven state**

```tsx
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

interface NavState {
  name?: string;
  templateId?: string;
  deviceId?: string;
  captureMode?: 'System' | 'Mic' | 'Mix';
  format?: 'Wav' | 'Flac';
}

export default function LiveRecordingPage() {
  const navigate = useNavigate();
  const location = useLocation();
  const { t, lang } = useT();
  const navState = (location.state ?? {}) as NavState;

  const [elapsed, setElapsed] = useState(0);
  const [paused, setPaused] = useState(false);
  const [level, setLevel] = useState(0);
  const [bars, setBars] = useState<number[]>(Array(36).fill(0));
  const [session, setSession] = useState<{ sessionId: string } | null>(null);
  const [stopResult, setStopResult] = useState<CaptureResult | null>(null);
  const [stopModalOpen, setStopModalOpen] = useState(false);

  // 1. Start on mount, defensive discard on unmount
  useEffect(() => {
    let cancelled = false;
    invoke<RecordingStartedDto>('start_recording', {
      deviceId: navState.deviceId,
      captureMode: navState.captureMode ?? 'System',
      format: navState.format ?? 'Wav',
    })
      .then(dto => { if (cancelled) void invoke('discard_recording'); else setSession(dto); })
      .catch(() => { /* error already surfaces via audio:error toast */ });
    return () => {
      cancelled = true;
      void invoke('discard_recording').catch(() => {});
    };
  }, []);

  // 2. Subscribe to events
  useEffect(() => {
    const unl = listen<{ rms: number }>('audio:level', e => setLevel(e.payload.rms));
    const unw = listen<{ bins: number[] }>('audio:waveform-bin', e => setBars(e.payload.bins));
    const une = listen<{ elapsedSec: number }>('audio:elapsed', e => setElapsed(e.payload.elapsedSec));
    return () => {
      unl.then(f => f()); unw.then(f => f()); une.then(f => f());
    };
  }, []);

  const onPauseToggle = async () => {
    if (paused) await invoke('resume_recording');
    else await invoke('pause_recording');
    setPaused(!paused);
  };

  const onStop = async () => {
    const result = await invoke<CaptureResult>('stop_recording');
    setStopResult(result);
    setStopModalOpen(true);
  };
  // ... rest of render (Waveform, LivePill, controls, meta bar) stays mostly the same
}
```

(Replace the existing render's Pause/Stop handlers with `onPauseToggle` / `onStop`. The Waveform component now accepts `bars` prop — see step 2.)

- [ ] **Step 2: Update `Waveform` component to accept externally supplied bars**

In `src/components/domain/Waveform/Waveform.tsx` add a new prop:

```tsx
export interface WaveformProps {
  bars?: number;
  paused?: boolean;
  className?: string;
  /** When provided, overrides the internal random heights with real audio data (0..1 per bin). */
  externalBins?: number[];
}

export function Waveform({ bars = 36, paused = false, className, externalBins }: WaveformProps) {
  const heightsRef = useRef<number[] | null>(null);
  if (!heightsRef.current || heightsRef.current.length !== bars) {
    heightsRef.current = Array.from({ length: bars }, () => 0.25 + Math.random() * 0.75);
  }
  const heights = externalBins ?? heightsRef.current;
  // ...rest unchanged
}
```

And pass `externalBins={bars}` from LiveRecording.

- [ ] **Step 3: Render `StopConfirmModal`** (created next task)

Append to the JSX returned by LiveRecordingPage (before the closing `</div>` of `.page`):

```tsx
{stopResult && (
  <StopConfirmModal
    open={stopModalOpen}
    onClose={() => setStopModalOpen(false)}
    capture={stopResult}
    suggestedTitle={navState.name ?? ''}
    templateId={navState.templateId ?? 'tecnica'}
  />
)}
```

- [ ] **Step 4: Update tests**

Mock `invoke` and `listen` minimally so the page mounts. The existing tests already pass with mocked `@tauri-apps/api/core`. Add a mock for `@tauri-apps/api/event`:

```tsx
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async () => () => {}),
}));
```

- [ ] **Step 5: Run tests**

```bash
pnpm test:run -- src/features/live-recording src/components/domain/Waveform
```

Expected: green.

- [ ] **Step 6: Commit**

```bash
git add src/features/live-recording/ src/components/domain/Waveform/
git commit -m "feat(live): replace useLiveTimer + fake bars with audio:level/waveform/elapsed events"
```

---

### Task 5.4: `StopConfirmModal` (new component)

**Files:**
- Create: `src/features/live-recording/StopConfirmModal/StopConfirmModal.tsx`
- Create: `src/features/live-recording/StopConfirmModal/StopConfirmModal.module.css`
- Create: `src/features/live-recording/StopConfirmModal/StopConfirmModal.test.tsx`

- [ ] **Step 1: Write the failing test first**

```tsx
import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import '@/i18n';
import { StopConfirmModal } from './StopConfirmModal';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(async (cmd: string) => {
    if (cmd === 'finalize_recording') return { id: 'm-new', title: { es: 'X', en: null } };
    if (cmd === 'discard_recording') return null;
    return null;
  }),
}));

const capture = {
  sessionId: 'sess-1',
  path: 'C:/tmp.wav',
  bytes: 1024,
  durationSec: 5,
};

function setup() {
  return render(
    <MemoryRouter>
      <StopConfirmModal
        open
        onClose={() => {}}
        capture={capture}
        suggestedTitle="Q4 review"
        templateId="tecnica"
      />
    </MemoryRouter>,
  );
}

describe('StopConfirmModal', () => {
  it('renders title input pre-filled and Save enabled', () => {
    setup();
    expect(screen.getByDisplayValue('Q4 review')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Guardar/i })).not.toBeDisabled();
  });

  it('disables Save when title is blank', async () => {
    setup();
    const input = screen.getByDisplayValue('Q4 review');
    await userEvent.clear(input);
    expect(screen.getByRole('button', { name: /Guardar/i })).toBeDisabled();
  });
});
```

- [ ] **Step 2: Run test — expect fail (component doesn't exist)**

```bash
pnpm test:run -- src/features/live-recording/StopConfirmModal
```

- [ ] **Step 3: Implement the component**

```tsx
import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useNavigate } from 'react-router-dom';
import { Button } from '@/components/primitives/Button/Button';
import { Icon } from '@/components/primitives/Icon/Icon';
import { Input } from '@/components/primitives/Input/Input';
import { Modal } from '@/components/primitives/Modal/Modal';
import { useT } from '@/i18n/useT';
import type { CaptureResult, Meeting } from '@/ipc/bindings';
import { Paths } from '@/router/paths';
import { fmtDuration } from '@/utils/format';
import styles from './StopConfirmModal.module.css';

export interface StopConfirmModalProps {
  open: boolean;
  onClose: () => void;
  capture: CaptureResult;
  suggestedTitle: string;
  templateId: string;
}

function fmtBytes(b: number): string {
  if (b < 1024) return `${b} B`;
  if (b < 1024 * 1024) return `${(b / 1024).toFixed(1)} KB`;
  return `${(b / 1024 / 1024).toFixed(1)} MB`;
}

export function StopConfirmModal({
  open,
  onClose,
  capture,
  suggestedTitle,
  templateId,
}: StopConfirmModalProps) {
  const { t, lang } = useT();
  const navigate = useNavigate();
  const [title, setTitle] = useState(suggestedTitle);

  const onSave = async () => {
    const meeting = await invoke<Meeting>('finalize_recording', {
      sessionId: capture.sessionId,
      title: title.trim(),
      templateId,
    });
    navigate(Paths.MeetingDetail(meeting.id));
  };

  const onDiscard = async () => {
    await invoke('discard_recording');
    navigate(Paths.Dashboard);
  };

  return (
    <Modal
      open={open}
      onClose={onDiscard}
      title={t('saveRecording')}
      subtitle={t('saveRecordingSub')}
      footer={
        <>
          <Button variant="danger" onClick={onDiscard}>
            {t('discard')}
          </Button>
          <Button variant="primary" onClick={onSave} disabled={title.trim() === ''}>
            {t('save')}
          </Button>
        </>
      }
    >
      <Input
        label={t('meetingNameLabel')}
        value={title}
        onChange={(e) => setTitle(e.target.value)}
        placeholder={t('meetingNamePh')}
        autoFocus
      />
      <div className={styles.summary}>
        <Icon name="clock" size={14} />
        <span>{fmtDuration(capture.durationSec)}</span>
        <span className={styles.sep} />
        <Icon name="download" size={14} />
        <span>{fmtBytes(capture.bytes)}</span>
      </div>
    </Modal>
  );
}
```

- [ ] **Step 4: CSS**

```css
.summary {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 14px;
  font-size: 13px;
  color: var(--text-muted);
}

.sep {
  width: 3px;
  height: 3px;
  border-radius: 50%;
  background: var(--text-subtle);
}
```

- [ ] **Step 5: Run tests**

```bash
pnpm test:run -- src/features/live-recording/StopConfirmModal
```

Expected: 2 tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/features/live-recording/StopConfirmModal/
git commit -m "feat(live): StopConfirmModal — name input, Save commits meeting + asset, Discard deletes"
```

---

### Task 5.5: SettingsPage — disable MP3 options

**Files:**
- Modify: `src/features/settings/SettingsPage.tsx`

- [ ] **Step 1: Update `qualityOptions`**

Find the `qualityOptions` array and apply:

```ts
const qualityOptions: { value: string; label: string; disabled?: boolean }[] = [
  { value: 'WAV 48k', label: 'WAV 48k' },
  { value: 'FLAC', label: 'FLAC' },
  { value: 'MP3 192k', label: 'MP3 192k', disabled: true },
  { value: 'MP3 320k', label: 'MP3 320k', disabled: true },
];
```

(The primitive change from Task 5.1 already supports the `disabled` flag.)

- [ ] **Step 2: Run tests**

```bash
pnpm test:run -- src/features/settings
```

Expected: green.

- [ ] **Step 3: Commit**

```bash
git add src/features/settings/SettingsPage.tsx
git commit -m "feat(settings): disable MP3 quality options (deferred to Sub-7 Export)"
```

---

### Task 5.6: Global `audio:error` Toast listener in `App.tsx`

**Files:**
- Modify: `src/App.tsx`

- [ ] **Step 1: Add the listener**

Add a new `useEffect` near the existing ones inside `App()`:

```tsx
import { listen } from '@tauri-apps/api/event';
import { toast } from './components/primitives/Toast/Toast';
import type { AudioErrorCode } from './ipc/bindings';

useEffect(() => {
  const un = listen<{ code: AudioErrorCode; message: string }>('audio:error', (e) => {
    const code = e.payload.code;
    const knownCodes: AudioErrorCode[] = [
      'DeviceNotFound', 'WasapiInit', 'FormatUnsupported', 'DiskFull', 'MixerOverflow',
    ];
    if (knownCodes.includes(code)) {
      toast.error(t('audioErrorTitle'), {
        description: t(`audioError.${code}` as never),
      });
    } else {
      toast.error(t('audioErrorTitle'), { description: e.payload.message });
    }
  });
  return () => { un.then((fn) => fn()); };
}, [t]);
```

- [ ] **Step 2: Run tests + build**

```bash
pnpm test:run && pnpm build 2>&1 | tail -5
```

Expected: tests pass, build clean.

- [ ] **Step 3: Commit**

```bash
git add src/App.tsx
git commit -m "feat: global audio:error listener routes to Toast with translated descriptions"
```

---

### Phase 5 quality gate

```bash
pnpm lint && pnpm check:hardcoded-strings && pnpm check:stories && pnpm test:run && pnpm build
```

Expected: all green. Test count: 99 (Foundation) + 4 (Sub-2 new) ≈ 103 Vitest.

---

## Phase 6 — Manual smoke test + CI updates

### Task 6.1: Update `.github/workflows/ci.yml`

**Files:**
- Modify: `.github/workflows/ci.yml`

- [ ] **Step 1: In the `backend` job, add a step for audio integration tests**

After the existing `cargo test --workspace` step, append:

```yaml
- name: cargo test audio integration
  working-directory: src-tauri
  run: cargo test -p smart-noter-audio --features audio-integration -- --include-ignored
  continue-on-error: true   # CI runners may not have audio hardware; gate softly
```

(The `continue-on-error: true` is intentional — the integration tests gated behind `audio-integration` only pass on a runner with real audio devices. Locally on Windows they pass; in standard GitHub-hosted runners they're allowed to skip.)

- [ ] **Step 2: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: optional audio-integration test step in backend job (soft-fail on hardware-less runners)"
```

---

### Task 6.2: CHANGELOG entry

**Files:**
- Modify: `CHANGELOG.md`

- [ ] **Step 1: Prepend the `0.2.0` entry above the existing `0.1.0` section**

```markdown
### [0.2.0] — Sub-2 Audio Capture — 2026-MM-DD

#### Added
- Real audio capture via WASAPI loopback (system) + cpal input (mic) + rubato-based mix
- 3 capture modes: System / Mic / Mix
- WAV (default) and FLAC formats; MP3 options remain in Settings as `disabled` placeholders (Sub-7)
- Real-time level meter and waveform driven by Tauri events (`audio:level @20Hz`, `audio:waveform-bin @10Hz`, `audio:elapsed @1Hz`)
- Pause/Resume that omits the paused span from the resulting file
- `StopConfirmModal` with Save (commits meeting + asset) and Discard (deletes tmp)
- New `meeting_assets` table (migration 0002) — 1:N relation, prepared for Sub-3/Sub-7 future assets
- 9 new Tauri commands (`enumerate_audio_devices` reworked, `start/stop_preview`, `start/pause/resume/stop_recording`, `finalize/discard_recording`)
- Global `audio:error` event listener routing to translated Toast in App.tsx
- Startup sweep of orphan `tmp-*` files in `%APPDATA%\com.smartnoter.app\audio\`

#### Changed
- `AudioDevice` shape: `name` is now plain string, `kind` enum replaces `icon: string` (UI derives the icon)
- The Foundation seed for `audio_devices` is no longer used (table orphaned, kept for backward compatibility)
- `SegmentedControl` primitive supports per-option `disabled` with "Próximamente" tooltip

#### Out of scope (still)
- Whisper transcription (Sub-3)
- AI summaries / RAG (Sub-5)
- MP3 export (Sub-7)
- Multiple parallel sessions
- Hot-plug device detection
```

- [ ] **Step 2: Commit**

```bash
git add CHANGELOG.md
git commit -m "docs(changelog): 0.2.0 — Sub-2 Audio Capture"
```

---

### Task 6.3: Manual smoke test execution

This task is **manual**, executed on a Windows 11 host with at least default render + capture devices. Tick off as you go.

- [ ] PreRecord shows at least the default Loopback + default Input device, names from OS
- [ ] Pick a Loopback device → AudioPreviewCard LevelBar reacts when you play music
- [ ] Click "Iniciar grabación" with a custom name → land in LiveRecording, timer mono advances from 0, waveform reacts in real time to audio
- [ ] Pause → timer freezes, waveform freezes at 30% opacity, level meter ~0
- [ ] Resume → timer continues from the paused value, no jump
- [ ] Stop → modal opens with prefilled title, duration + bytes shown
- [ ] Save → modal closes, navigates to `/meetings/<new-id>` with the recording shown in Meeting Detail
- [ ] Verify file exists at `%APPDATA%\com.smartnoter.app\audio\m-...wav` (or .flac if FLAC was set)
- [ ] Play the file in VLC / Audacity → audio matches what was playing on the system
- [ ] Discard path: record 3 seconds → Stop → Discard → no orphan file remains in `audio/`
- [ ] Kill `smart-noter.exe` mid-recording (Task Manager) → reopen → `tmp-*` files are gone
- [ ] Switch capture mode to Mix in Settings → record → file contains both system audio AND mic
- [ ] Switch quality to FLAC in Settings → record → `.flac` file is produced and plays
- [ ] DiskFull (optional, USB drive nearly full): fill drive → start recording → Toast appears, session ends with partial audio

If any of the above fail, fix BEFORE Phase 7.

---

## Phase 7 — Final validation + tag

### Task 7.1: Run full quality gate locally

- [ ] `pnpm lint` — 0 errors
- [ ] `pnpm check:hardcoded-strings` — clean
- [ ] `pnpm check:stories` — 11 components (Toast was added in Foundation closing)
- [ ] `pnpm test:run` — ~103 tests pass
- [ ] `cd src-tauri && cargo fmt --check`
- [ ] `cd src-tauri && cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cd src-tauri && cargo test --workspace` — ~28 tests pass (12 Foundation + ~16 Sub-2)
- [ ] `pnpm build` succeeds
- [ ] `pnpm tauri:build --debug` produces a new MSI

### Task 7.2: Tag the release

```bash
git tag -a v0.2.0-sub2-audio -m "Sub-2 — Real WASAPI/cpal audio capture, WAV/FLAC, meeting_assets"
```

(Push when ready: `git push origin main --tags`.)

### Task 7.3: Update foundation spec DoD reference

In `docs/superpowers/specs/2026-05-19-sub2-audio-capture-design.md`, change the header status line from:

```
> Status: **Spec approved 2026-05-19**, awaiting implementation plan.
```

to:

```
> Status: **Implemented 2026-MM-DD** — tag `v0.2.0-sub2-audio`.
```

Commit:

```bash
git add docs/superpowers/specs/2026-05-19-sub2-audio-capture-design.md
git commit -m "docs(spec): mark Sub-2 spec as implemented"
```

---

## Plan complete

Total tasks: **24** across 7 phases. Estimated effort: medium — most risk concentrated in Task 3.1 (WASAPI loopback wiring) and Task 3.2 (worker orchestration). The state machine, writers, mixer, and meter are pure-logic with strong unit-test coverage; the WASAPI plumbing is the part where careful adherence to `wasapi-rs`'s examples matters most.

After all tasks pass + Phase 6 manual smoke is green: tag `v0.2.0-sub2-audio` and Sub-2 is officially done.
