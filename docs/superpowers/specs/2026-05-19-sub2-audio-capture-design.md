# Sub-project 2 — Audio Capture (Design)

> Status: **Implemented 2026-06-15** — tag `v0.2.0-sub2-audio`.
> Depends on: Foundation v0.1.0 (this branch's `main` after merge).
> Unblocks: Sub-3 (Whisper transcription), Sub-7 (Export to MD/PDF/MP3).

## 1. Objective

Deliver a working **end-to-end audio capture vertical**: from "Iniciar grabación" in PreRecord all the way to a fresh meeting row in SQLite with a persisted `.wav` (or `.flac`) on disk, with real-time level + waveform feedback in the LiveRecording UI driven by actual audio data.

After Sub-2, a user can:

1. Open PreRecord, see real audio devices enumerated from the OS via WASAPI/cpal
2. Pick a device, mode (system / mic / mix), template, and meeting name
3. Click "Iniciar grabación" → land in LiveRecording with a live level meter and waveform sourced from the audio stream
4. Pause and resume — the resulting audio file has the paused span omitted (no silence inserted)
5. Click Stop → confirm in a modal with a final title and the audio metadata (duration + bytes)
6. Save → a meeting row is committed with an associated `meeting_assets` row; the app navigates to the new Meeting Detail
7. Discard → the audio file is deleted and the user returns to Dashboard

Transcription, AI summaries, and export are explicitly out of scope (Sub-3 / Sub-5 / Sub-7).

## 2. Scope decisions (from brainstorming)

| Topic | Decision |
|---|---|
| Vertical depth | Full vertical — device enum + capture + WAV/FLAC on disk + DB persistence + real-time UI |
| Capture modes | All 3: System Loopback, Mic only, Mix (system + mic) |
| Audio formats | WAV (default) + FLAC. MP3 options in Settings stay disabled, deferred to Sub-7 |
| Real-time feedback | Yes for both level meter and waveform — Tauri events from Rust at 20 Hz / 10 Hz |
| Pause behaviour | Real pause — writing stops, on resume the file continues appending (no silence padding) |
| Stop UX | Confirmation modal with editable title + Save / Discard buttons |
| Meeting↔Asset linkage | New `meeting_assets` table (1:N relation), seeded for future transcript/export assets |
| Architecture style | Event-driven (Approach 2) — `tauri::State<Arc<Mutex<CaptureSession>>>` + Tauri events for live feedback |

## 3. Architecture overview

```
┌────────────────────────────── PreRecord ──────────────────────────────┐
│  list_audio_devices → real WASAPI enumeration                          │
│  user picks: device, template, captureMode, meetingName               │
│  AudioPreviewCard subscribes to audio:level for live preview          │
│  click "Iniciar grabación" → navigate to /record/live/<sessionId>      │
└────────────────────────┬───────────────────────────────────────────────┘
                         │ location.state: { deviceId, captureMode, name, templateId }
                         ▼
┌──────────────────────── LiveRecording ────────────────────────────────┐
│  on mount: invoke('start_recording', { ... })                          │
│  listen('audio:level', e => setLevel(e.payload))                       │
│  listen('audio:waveform-bin', e => setWaveBars(e.payload))             │
│  listen('audio:elapsed', e => setElapsed(e.payload.elapsedSec))        │
│  Pause/Resume → invoke('pause_recording') / invoke('resume_recording') │
│  Stop → invoke('stop_recording') → opens StopConfirmModal              │
└────────────────────────┬───────────────────────────────────────────────┘
                         │ on Save: invoke('finalize_recording', { title })
                         ▼
┌─────────────────── crate audio (Rust, ~1000 LOC) ─────────────────────┐
│                                                                         │
│   ┌─ devices.rs ──┐     ┌─ capture/stream.rs ───┐                       │
│   │ wasapi-rs +   │     │ open WASAPI loopback   │                       │
│   │ cpal          │     │ +/or cpal mic         │                       │
│   │ enumeration   │     │ → samples MPSC tx     │                       │
│   └──────────────┘     └─────────┬──────────────┘                       │
│                                   │ f32 samples                          │
│                ┌──────────────────┼──────────────────┐                  │
│                ▼                  ▼                  ▼                  │
│   ┌─ mixer.rs ─────┐  ┌─ writer.rs ────────┐  ┌─ meter.rs ──────────┐  │
│   │ combine 2      │  │ accumulate to file │  │ rolling RMS + peak  │  │
│   │ streams +      │  │ (WAV/FLAC) on disk │  │ emit Tauri events:  │  │
│   │ rubato resample│  │                    │  │ audio:level @20Hz   │  │
│   └────────────────┘  └────────────────────┘  │ audio:waveform @10Hz│  │
│                                                └─────────────────────┘  │
└────────────────────────────────────────────────────────────────────────┘
                         │ on stop: returns { path, bytes, durationSec }
                         ▼
┌──────────────────────── crate db ──────────────────────────────────────┐
│  migration 0002: CREATE TABLE meeting_assets (...)                      │
│  MeetingsRepo.create_with_asset(meeting, asset)                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### 3.1 Affected crates

- **`smart-noter-audio`** — new implementation, ~1240 LOC across 9 files
- **`smart-noter-db`** — migration `0002_meeting_assets.sql`, new `MeetingAssetsRepo`, `MeetingsRepo::create_with_asset()`
- **`smart-noter-core`** — new `MeetingAsset` model, new `AppError::Audio` variant, `AudioErrorCode` enum
- **`src-tauri/src/commands/`** — 7 new audio commands + reworked `list_audio_devices`

### 3.2 Affected frontend modules

- `src/features/pre-record/PreRecordPage.tsx` — preview lifecycle, real device list, `AudioPreviewCard` driven by events
- `src/features/live-recording/LiveRecordingPage.tsx` — replace `useLiveTimer` and fake waveform with event-driven values; pause/resume/stop wired to commands
- `src/features/live-recording/StopConfirmModal/` — new component
- `src/features/settings/SettingsPage.tsx` — disable MP3 options in `recordingQuality` SegmentedControl
- `src/components/primitives/SegmentedControl/SegmentedControl.tsx` — new `disabled?: boolean` per option
- `src/App.tsx` — global `audio:error` listener wired to Toast

## 4. Rust crate `smart-noter-audio` (~1240 LOC across 9 files)

### 4.1 Dependencies (Cargo.toml)

```toml
[dependencies]
wasapi = "0.16"           # raw WASAPI loopback (loopback support more mature than cpal here)
cpal = "0.15"             # cross-cutting device enum + mic input
hound = "3.5"             # WAV writer
claxon = "0.4"            # FLAC encoder
rubato = "0.15"           # high-quality SRC for Mix mode
crossbeam-channel = "0.5" # MPSC bounded for audio→writer
parking_lot = "0.12"      # faster Mutex
thiserror = { workspace = true }
tracing = { workspace = true }
serde = { workspace = true }
specta = { workspace = true }

# Re-exports of core types
smart-noter-core = { path = "../core" }

[dev-dependencies]
rstest = "0.21"
```

### 4.2 File layout + responsibility

| Path | LOC | Responsibility |
|---|---|---|
| `lib.rs` | ~30 | Re-exports, crate-level doc, `pub fn enumerate_devices` shortcut |
| `error.rs` | ~60 | `AudioError` enum (8 variants) — see § 6 |
| `devices.rs` | ~180 | `pub fn enumerate() -> Result<Vec<AudioDevice>>` — combines WASAPI render endpoints (loopback) + cpal input devices (mics). Deterministic id = hash of WASAPI endpoint ID. |
| `capture/mod.rs` | ~250 | `CaptureSession` state machine: `Idle → Recording → Paused → Recording → Stopped`. Owns `RecordingHandle { workers: Vec<JoinHandle>, control_tx: Sender<Cmd> }`. |
| `capture/stream.rs` | ~200 | Opens stream(s) per `CaptureMode`. System=1 WASAPI loopback. Mic=1 cpal input. Mix=both. Spawns audio callback that does `tx.try_send(buf.to_vec())` with overflow counter. |
| `capture/mixer.rs` | ~130 | Only active in Mix mode. Drains 2 channels, resamples to 48 kHz via rubato if mismatched, sums sample-by-sample with gain 0.7 (anti-clip), emits to writer channel. |
| `capture/writer.rs` | ~180 | Worker thread: drain `Receiver<Vec<f32>>` → encode → `hound::WavWriter` or `claxon::FlacWriter` → flush every 100 samples. Pause/resume gate. |
| `capture/meter.rs` | ~170 | Parallel worker (broadcast channel from writer input): per 2400-sample block computes RMS + peak, maintains rolling 36-bin buffer, emits Tauri events at 20 Hz (level), 10 Hz (waveform), and 1 Hz (elapsed seconds derived from samples written). |
| `events.rs` | ~40 | Event-name constants + serializable payload structs with specta derives |

**Total**: ~1240 LOC including blank lines and doc comments.

### 4.3 Threading model

- **Audio callback thread** (cpal/wasapi-owned): zero allocations in hot path; only `tx.try_send(buf.to_vec())`. Overflow counter incremented on full channel. Buffer copy is unavoidable because the callback's borrowed slice cannot outlive the callback frame.
- **Mixer thread** (Mix mode only): drains both source channels via `select!`, resamples, sums, re-emits.
- **Writer thread**: single consumer of the post-mixer channel. Writes to disk. Sleeps when paused (channel stays open, samples accumulate up to bounded capacity, then drop with overflow log).
- **Meter thread**: broadcast subscriber of the same channel. Computes RMS/peak, throttles events.
- **Tauri command threads**: short-lived. Lock the `Mutex<CaptureSession>` only for state transitions.

State machine:

```
Idle ──start──▶ Recording ──pause──▶ Paused ──resume──▶ Recording
                    │                     │
                    └──── stop ───┬───────┘
                                  ▼
                              Stopped ──finalize──▶ Idle (cleanup threads, return CaptureResult)
                                  │
                              discard ──▶ Idle (delete file)
```

Invalid transitions return `AudioError::AlreadyRecording` or `AudioError::NotRecording`. State checks live in `capture/mod.rs::CaptureSession::*` methods.

### 4.4 Channel sizing

- audio→writer: bounded `crossbeam_channel::bounded(64)` (≈64 × 480 samples × 4 bytes = 122 KB headroom at 48k stereo). Overflow → drop + counter.
- audio→mixer (each source): same bound.
- meter broadcast: zero-cost (it's a peek-into-writer's input, not a copy).

## 5. IPC contract

### 5.1 Commands (typed via tauri-specta)

| Command | Args | Returns | Notes |
|---|---|---|---|
| `enumerate_audio_devices` | — | `Vec<AudioDevice>` | Replaces the existing seed-backed implementation |
| `start_preview` | `{ deviceId, captureMode }` | `()` | Like start_recording but no file writer — meter only |
| `stop_preview` | — | `()` | Idempotent |
| `start_recording` | `{ deviceId, captureMode, format }` | `RecordingStartedDto { sessionId, sampleRate, channels }` | Allocates `tmp-<sessionId>.<ext>` on disk |
| `pause_recording` | — | `()` | Errors if not Recording |
| `resume_recording` | — | `()` | Errors if not Paused |
| `stop_recording` | — | `CaptureResult { sessionId, path, bytes, durationSec }` | Closes file, returns metadata. Does NOT touch DB. |
| `finalize_recording` | `{ sessionId, title, templateId }` | `Meeting` | Renames tmp → final, inserts meeting + asset transactionally |
| `discard_recording` | — | `()` | Deletes tmp file. Safe to call from any state. |

### 5.2 Events (Rust → React)

| Event name | Payload | Rate | Subscribers |
|---|---|---|---|
| `audio:level` | `{ rms: f32, peak: f32 }` (0..1) | ~20 Hz | LiveRecording LevelBar, PreRecord AudioPreviewCard LevelBar |
| `audio:waveform-bin` | `{ bins: f32[36] }` | ~10 Hz | LiveRecording Waveform |
| `audio:elapsed` | `{ elapsedSec: u32 }` | 1 Hz | LiveRecording timer (single source of truth, replaces `useLiveTimer`) |
| `audio:error` | `AudioError` (tagged enum) | on error | `App.tsx` global Toast handler |

### 5.3 New types in bindings.ts

```rust
#[derive(Serialize, Deserialize, specta::Type)]
pub enum AudioFormat { Wav, Flac }

#[derive(Serialize, Deserialize, specta::Type)]
pub enum AudioDeviceKind { Loopback, Input }

#[derive(Serialize, Deserialize, specta::Type)]
pub struct RecordingStartedDto {
    pub session_id: String,
    pub sample_rate: u32,
    pub channels: u16,
}

#[derive(Serialize, Deserialize, specta::Type)]
pub struct CaptureResult {
    pub session_id: String,
    pub path: String,
    pub bytes: u64,
    pub duration_sec: u32,
}

#[derive(Serialize, Deserialize, specta::Type)]
pub struct MeetingAsset {
    pub id: String,
    pub meeting_id: String,
    pub kind: String,           // "audio" | "transcript" | "export"
    pub path: String,
    pub bytes: u64,
    pub mime_type: Option<String>,
    pub created_at: String,
}
```

### 5.4 Breaking change: `AudioDevice` shape

```rust
// BEFORE (Foundation, mocked by seed)
pub struct AudioDevice {
    pub id: String,
    pub name: Bilingual,
    pub desc: Bilingual,
    pub icon: String,
    pub recommended: bool,
    pub active: bool,
}

// AFTER (Sub-2, real)
pub struct AudioDevice {
    pub id: String,             // hash of WASAPI endpoint ID
    pub name: String,           // from OS — no longer Bilingual
    pub kind: AudioDeviceKind,  // Loopback | Input
    pub sample_rate: u32,
    pub channels: u16,
    pub is_default: bool,
    pub recommended: bool,      // computed: Loopback → true
}
```

Frontend `DeviceCard` derives the icon from `kind` (`Loopback → 'monitor'`, `Input → 'headphones'`). The seed insert for `audio_devices` is removed from `seed.rs`.

## 6. Failure modes + error handling

### 6.1 AudioError enum (Rust)

```rust
#[derive(thiserror::Error, Debug, Serialize, specta::Type)]
#[serde(tag = "code", content = "message")]
pub enum AudioError {
    #[error("Device not found: {0}")]
    DeviceNotFound(String),

    #[error("Failed to initialize WASAPI: {hresult}")]
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
```

Mapped to the global `AppError` via `From<AudioError> for AppError`.

### 6.2 Failure matrix

| Failure | Detection | UI behaviour | Recovery |
|---|---|---|---|
| `DeviceNotFound` | `start_recording` Err | Toast "Dispositivo no disponible" + nav to `/record/new` | Re-enumerate, pick another |
| `WasapiInit` | `start_recording` Err | Toast "No se pudo iniciar la captura" + nav back | Log HRESULT |
| `FormatUnsupported` | `start_recording` validation | Toast "Formato no soportado, usando WAV" — silent fallback | Auto-fallback |
| `AlreadyRecording` | State check | Silent in prod (idempotent), warn log | Defensive `discard_recording` in `useEffect` cleanup |
| `NotRecording` | State check | Silent (idempotent) | — |
| `DiskFull` | Writer thread WriteAll Err | Emit `audio:error`. Session not interrupted — writer closes file with what it has, exits. `stop_recording` reports partial result | User finalizes partial or discards |
| `MixerOverflow` | Audio callback `try_send` fail counter ≥ 100 | Emit `audio:error`, continue (degraded) | Logs |
| Crash mid-recording | OS process death | `tmp-*` files orphaned | Startup sweep deletes `tmp-*` |
| Worker panic | `catch_unwind` in each thread | Reports via `audio:error`, auto-stops session | Crash log |

### 6.3 Retry policy

| Operation | Retry? |
|---|---|
| Enumerate devices | No retry; UI shows Toast on failure |
| Start recording | No automatic retry; user retries from PreRecord |
| Pause/Resume | Idempotent (no error if state already matches) |
| Stop | Force-success — even on writer thread failure, return what we have |
| Finalize → DB write | 1 retry with 100ms backoff if `database is locked`; else error |
| Discard → file delete | Best-effort, only log on failure (startup sweep covers it) |

### 6.4 Logging

- `tracing::info!` on every state transition
- `tracing::warn!` on isolated sample drops, format fallbacks
- `tracing::error!` on `WasapiInit` / `DiskFull` / panic — including raw HRESULT when applicable
- Routed through `tauri-plugin-log` to `%APPDATA%\com.smartnoter.app\logs\`

## 7. DB schema changes

### 7.1 Migration `0002_meeting_assets.sql`

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

- `ON DELETE CASCADE` — deleting a meeting drops its asset rows. Filesystem cleanup is repo callers' responsibility (see § 7.3).
- No UNIQUE on `(meeting_id, kind)` — a meeting could conceivably have multiple audios in the future (e.g., separate loopback + mic). Sub-2 inserts one `kind='audio'` per meeting.

### 7.2 Repo additions

```rust
// crates/db/src/repos/meeting_assets.rs
pub struct MeetingAssetsRepo<'a>(&'a SqlitePool);

impl MeetingAssetsRepo<'_> {
    pub async fn create(&self, asset: &MeetingAsset) -> Result<()>;
    pub async fn list_by_meeting(&self, meeting_id: &str) -> Result<Vec<MeetingAsset>>;
    pub async fn get_audio(&self, meeting_id: &str) -> Result<Option<MeetingAsset>>;
    pub async fn delete(&self, asset_id: &str) -> Result<Option<String>>;   // returns path
}
```

```rust
// MeetingsRepo addition
impl MeetingsRepo<'_> {
    pub async fn create_with_asset(
        &self,
        meeting: &Meeting,
        asset: &MeetingAsset,
    ) -> Result<()>;   // single transaction
}
```

### 7.3 Filesystem cleanup convention

- Repos never touch the disk.
- `finalize_recording`: transactional INSERT, then no further file action (the tmp file was already renamed to the final path before the DB call)
- `discard_recording`: `fs::remove_file(tmp_path)`. No DB row to clean.
- `delete_meeting` (future): list assets → `fs::remove_file()` each → `MeetingsRepo::delete()` (cascade drops rows)
- Startup sweep: `fs::read_dir("audio/")`, remove files matching `tmp-*.<ext>` regardless of mtime

### 7.4 Storage layout

```
%APPDATA%\com.smartnoter.app\
├── db.sqlite
└── audio\
    ├── m-2026-05-19-abc123def.wav      # final
    ├── m-2026-05-19-def456ghi.flac     # final
    ├── tmp-d24c9e6f-...wav              # active recording, will be renamed on finalize
    └── ...
```

- Final naming: `<meeting_id>.<ext>` where `meeting_id` is generated at `finalize_recording` time (ULID-style `m-` + date + random)
- Path stored is **absolute** to survive future moves with explicit migration

## 8. Frontend changes

### 8.1 PreRecordPage (~30 LOC delta)

- `useListAudioDevicesQuery` hook unchanged — backend now returns real data
- `AudioDevice.icon: string` removed → `iconFor(kind: AudioDeviceKind): IconName`
- `device.name` is now plain `string`, not `Bilingual`
- New `useEffect` controls preview lifecycle:

```ts
useEffect(() => {
  if (!deviceId) return;
  void invoke('start_preview', { deviceId, captureMode: 'system' });
  return () => { void invoke('stop_preview'); };
}, [deviceId]);
```

- `AudioPreviewCard` subscribes to `audio:level`:

```tsx
const [level, setLevel] = useState(0);
useEffect(() => {
  const un = listen<{ rms: number }>('audio:level', e => setLevel(e.payload.rms));
  return () => { un.then(fn => fn()); };
}, []);
return <LevelBar level={level} />;
```

### 8.2 LiveRecordingPage (~60 LOC delta)

- `useLiveTimer(143)` removed; timer state driven by `audio:elapsed` event
- Three new event subscriptions (`audio:level`, `audio:waveform-bin`, `audio:elapsed`) — each with proper cleanup
- Start on mount with defensive cancel:

```ts
useEffect(() => {
  let cancelled = false;
  invoke<RecordingStartedDto>('start_recording', { deviceId, captureMode, format })
    .then(dto => { if (cancelled) void invoke('discard_recording'); else setSession(dto); });
  return () => { cancelled = true; void invoke('discard_recording').catch(() => {}); };
}, []);
```

- Pause/Resume handlers wired to commands; Stop opens `StopConfirmModal` instead of navigating

### 8.3 StopConfirmModal (~80 LOC, new)

`src/features/live-recording/StopConfirmModal/StopConfirmModal.{tsx,module.css,test.tsx}`. Composed of `Modal` + `Input` + `Button` primitives.

- Pre-fills title from PreRecord's `name` (or empty)
- Save button disabled when title is blank
- Shows `fmtDuration(durationSec)` + `fmtBytes(bytes)` summary
- Save → `invoke('finalize_recording')` → navigate to `Paths.MeetingDetail(meeting.id)`
- Discard → `invoke('discard_recording')` → navigate to `Paths.Dashboard`
- Close (backdrop / Esc) is treated as Discard for safety

New i18n keys: `saveRecording`, `saveRecordingSub`, `save`, `discard` (8 strings total).

### 8.4 SettingsPage (~10 LOC delta)

- `SegmentedControl<string>` for `recordingQuality` filters disabled options:

```ts
const qualityOptions = [
  { value: 'WAV 48k', label: 'WAV 48k' },
  { value: 'FLAC', label: 'FLAC' },
  { value: 'MP3 192k', label: 'MP3 192k', disabled: true },
  { value: 'MP3 320k', label: 'MP3 320k', disabled: true },
];
```

### 8.5 SegmentedControl primitive (~10 LOC delta)

`SegmentedOption<T>` gets an optional `disabled?: boolean`. The button renders `disabled` accordingly, with a `title` tooltip "Próximamente" when set. Backward-compatible — existing usages remain unchanged.

### 8.6 Global audio:error listener (in App.tsx)

```ts
useEffect(() => {
  const un = listen<AudioError>('audio:error', e => {
    toast.error(t('audioErrorTitle'), {
      description: t(`audioError.${e.payload.code}` as TKey),
    });
  });
  return () => { un.then(fn => fn()); };
}, [t]);
```

New i18n keys: `audioErrorTitle` + `audioError.DeviceNotFound`, `audioError.WasapiInit`, `audioError.FormatUnsupported`, `audioError.DiskFull`, `audioError.MixerOverflow` (12 strings total).

## 9. Testing strategy

### 9.1 Rust unit tests (~33 tests)

| File | Count | Coverage |
|---|---|---|
| `error.rs` | 3 | Round-trip serialize/deserialize, error messages have context |
| `devices.rs` | 4 | Non-panic, ≥1 device in CI, kind discrimination, deterministic ids |
| `capture/mixer.rs` | 6 | Resampling 44.1→48, no clipping at 0.7 gain, edge cases, perf <10ms for 1s |
| `capture/writer.rs` | 8 | WAV header validity, FLAC header validity, append-on-resume, discard cleans, tmp→final rename |
| `capture/meter.rs` | 5 | RMS of known sine, peak detection, decimation correctness, throttle obeys 20/10 Hz, bins in 0..1 |
| `capture/mod.rs` | 7 | All valid state transitions, invalid transitions return correct errors, idempotent double-stop, discard from any state |

### 9.2 Rust integration tests (~3 tests in `crates/audio/tests/`)

- `wav_roundtrip.rs` — generate 1s sine 440 Hz, write WAV, reopen with `hound::WavReader`, samples match ±epsilon
- `flac_roundtrip.rs` — same with `claxon`
- `pipeline.rs` — fake-cpal feeds prepared buffer, the callback→writer→meter chain runs for 100ms, event payloads are correct

### 9.3 Manual smoke tests (documented here; not in CI)

WASAPI requires real hardware. The following must be run manually on Windows 11 before tagging Sub-2:

1. **Device enumeration**: PreRecord shows at least the default render endpoint + default capture device
2. **Loopback recording**: play music in Spotify, record 10s, resulting `.wav` contains the audio (verify in Audacity)
3. **Mic recording**: speak for 5s, `.wav` contains recognisable speech
4. **Mix recording**: music + voice simultaneously, both signals distinguishable in the `.wav`
5. **Real pause**: record → pause 5s → resume → stop. Playback has no 5s of silence
6. **Stop confirmation**: name and save → meeting appears in MeetingsList with correct duration
7. **Discard**: record → discard → no orphan file in `audio/`
8. **Crash recovery**: kill `smart-noter.exe` mid-recording → reopen → `tmp-*` swept at startup
9. **DiskFull**: fill a small drive on purpose → Toast warning appears → session ends with partial audio

### 9.4 Frontend tests (~12 new / updated Vitest)

- `StopConfirmModal.test.tsx` — 3 tests (renders with initial title, Save disabled on empty title, Discard fires callback)
- `LiveRecordingPage.test.tsx` — updated to mock `listen` + `invoke`, verify level/waveform update, mock for start/pause/stop/discard
- `PreRecordPage.test.tsx` — preview mount/unmount lifecycle test
- `SettingsPage.test.tsx` — disabled segmented options are not clickable

### 9.5 Playwright E2E

- `navigation.spec.ts`, `visual.spec.ts` unchanged
- `persistence.spec.ts` extends: record 3s, save with title "E2E test", confirm row appears in MeetingsList
- Real-audio E2E remains manual (Playwright can't inject system audio)

### 9.6 Coverage thresholds (unchanged from Foundation)

- Rust audio crate: ≥80%
- Frontend primitives: ≥90%
- `cargo llvm-cov --workspace --fail-under-lines 80` should continue passing

### 9.7 CI workflow

- `backend` job picks up the new audio tests automatically via `cargo test --workspace`
- New step in `backend`: `cargo test --workspace --features audio-integration` to include the `tests/` dir integration suite
- `e2e` job unchanged

## 10. Definition of Done (Sub-2)

**Sub-2 is ready to merge when ALL of the following pass:**

### Functionality

- [ ] Real device enumeration replaces the Foundation seed list
- [ ] System loopback mode records audio playing through any app (Spotify, Teams, Zoom, browser)
- [ ] Mic mode records the selected input device
- [ ] Mix mode records both loopback + mic, summed in a single channel pair
- [ ] Pause omits the paused span from the resulting file (no silence padding)
- [ ] Stop opens the confirmation modal; Save commits a new meeting + asset row; Discard deletes the file
- [ ] LiveRecording level meter and waveform react to actual audio in real time
- [ ] PreRecord AudioPreviewCard level meter reacts to actual audio when a device is selected
- [ ] DiskFull mid-recording surfaces a Toast and the partial audio is recoverable
- [ ] Crash recovery: `tmp-*` files are swept on next startup
- [ ] `audio:error` events surface as Toasts via App.tsx global listener

### Quality

- [ ] `pnpm biome check` passes
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes
- [ ] `pnpm test:run` passes (existing 99 + ~12 new = ~111)
- [ ] `cargo test --workspace` passes (existing 12 + ~33 + ~3 = ~48)
- [ ] `pnpm check:hardcoded-strings` passes
- [ ] `pnpm check:stories` passes
- [ ] All 9 manual smoke tests (§ 9.3) pass on Windows 11
- [ ] No new `// TODO`, `// FIXME`, `// XXX` in merged code

### Documentation

- [ ] CHANGELOG.md entry for `0.2.0 — Sub-2 Audio Capture`
- [ ] README.md scripts table updated if any new pnpm scripts are added
- [ ] This spec is linked from CHANGELOG.md

## 11. Out of scope (deferred)

- **Whisper transcription** of the recorded audio — Sub-3
- **MP3 export** of the recorded WAV/FLAC — Sub-7
- **Multiple parallel sessions** — single-session only; UI guards against opening Live while a session is active
- **Hot-plug device detection** — devices re-enumerate when PreRecord opens; mid-session device changes are not handled
- **Network/cloud capture** — Sub-2 is local hardware only
- **Code-signing the MSI** — Sub-8 distribution
- **Audio level customisation** (gain, noise gate, normalisation) — possible future polish, not Sub-2

## 12. Open questions / risks

- **`wasapi` crate maintenance**: the loopback support is mature but the crate has fewer maintainers than cpal. Mitigation: encapsulate WASAPI calls behind `devices.rs` + `capture/stream.rs` so a swap to a different backend is local.
- **Real pause append**: hound supports appending to an existing WAV but FLAC encoders typically write a single stream. For FLAC, pause may require closing the encoder + reopening on resume with a fresh frame header, then post-processing to concatenate. **Mitigation**: in Sub-2's FLAC implementation, pause/resume may insert frame boundaries; we verify in manual smoke test #5 that playback is gapless. If not, fall back to writing the FLAC file as a single non-pausable stream (pause toggles UI state but the file accumulates silence — documented as a known divergence from WAV behaviour).
- **MixerOverflow under load**: if the user's CPU is pegged (e.g., another app using GPU heavily on the same Whisper-rs path in Sub-3), the mixer thread may not keep up. We mitigate with bounded channels + drop counter + Toast warning, but a pathological case could lose ≥1% of frames. Acceptable for Sub-2.

---

**Status**: design approved 2026-05-19. Implementation plan to be written next via `superpowers:writing-plans`.
