# Sub-project 3a — Whisper Local Transcription (Design)

> Status: **Spec approved 2026-06-15**, awaiting implementation plan.
> Depends on: Sub-2 Audio Capture (`v0.2.0-sub2-audio`) — provides the saved `.wav`/`.flac` per meeting.
> Unblocks: Sub-3b (Speaker Diarization), Sub-5 (AI Summary + Chat — also owns transcript translation).

## 1. Goal & scope

Take the audio file a meeting already has (stored by Sub-2's `finalize_recording`) and produce a
time-stamped transcript in the spoken language, persisted to the existing `transcript_lines` table and
rendered by the existing `TranscriptTab`. Includes a model manager (select / download / delete Whisper
models) and an auto-or-manual trigger with live progress and cancellation.

**3a is the first half of the original Sub-3.** Real speaker diarization (embeddings + clustering, plus
Silero VAD) is split out to **Sub-3b**. Transcript translation to the user's native language is owned by
**Sub-5** (LLM). This spec deliberately keeps 3a to "audio → readable single-speaker transcript".

### Locked decisions (from brainstorming 2026-06-15)

| Topic | Decision |
|---|---|
| Engine | `whisper-rs` (whisper.cpp binding), native Rust, in the empty `smart-noter-whisper` crate |
| Models | Catalog of `base` / `small` / `medium` / `large-v3`; on-demand download + progress; **default `large-v3`** |
| Compute | **CPU-first**. `cuda` feature defined but disabled → deferred (needs CUDA toolkit) |
| Diarization | **None in 3a** — single speaker `S1`. Whisper's own segments are the lines. VAD + real diarization → Sub-3b |
| Trigger | **Auto on save**, gated by a new Settings toggle; plus a manual "Transcribe" button. Progress + cancel |
| Language | Transcribe the **spoken** language (auto-detect). No translation in 3a (→ Sub-5). A `native_language` Settings field is added now for Sub-5 |
| Progress IPC | **Global Tauri events keyed by `meetingId`** (re-subscribable, mirrors Sub-2), + `get_transcription_state` for re-mount sync — a Channel would orphan progress if the view is left and revisited mid-job |
| Concurrency | **One transcription at a time**; second request → `TranscriptionBusy` |

## 2. Architecture

Fill the currently-empty `smart-noter-whisper` crate (`src-tauri/crates/whisper/`, today just a
`version()` stub). New deps: `whisper-rs` (CPU build; `cuda` feature defined, off). Audio decoding reuses
the crates already pulled in by Sub-2: `hound` (WAV), `claxon` (FLAC), `rubato` (resample).

Modules (each one unit with a single responsibility and a clear interface):

- **`models`** — model catalog + on-disk management.
- **`decode`** — audio file → 16 kHz mono `f32` PCM (what Whisper requires).
- **`transcribe`** — PCM → time-stamped segments, with progress + abort callbacks.

Orchestration (locate audio, resolve model, run, persist, stream progress) lives in a Tauri command layer
(`src-tauri/src/commands/transcription.rs`), not in the crate, so the crate stays free of Tauri/DB
concerns and is testable in isolation.

### 2.1 `models`

- **Catalog** (static): for each model — `id`, display `name`, `size_mb`, `sha256`, `url` (ggml `.bin` on
  Hugging Face). Supported: `base` (~150 MB), `small` (~500 MB), `medium` (~1.5 GB), `large-v3` (~3 GB).
- **Storage:** `%APPDATA%\com.smartnoter.app\models\` (sibling to Sub-2's `audio\`).
- **API:** `list()` (each model + `downloaded: bool`), `download(id, progress_cb)` (stream to a
  `tmp-*` file → verify sha256 → atomic rename; bad hash → delete + error), `delete(id)`, `path(id)`.

### 2.2 `decode`

`decode_to_pcm_16k_mono(path) -> Result<Vec<f32>>`: WAV via `hound`, FLAC via `claxon` → normalize to
`f32` → downmix to mono → resample to 16 kHz with `rubato`. Self-contained (no dependency on the `audio`
capture crate).

### 2.3 `transcribe`

`transcribe(pcm: &[f32], model_path, opts, progress_cb, abort_flag) -> Result<Vec<Segment>>` where
`Segment { start_ms: u32, end_ms: u32, text: String }`.

- whisper-rs `FullParams` with per-segment timestamps; language auto-detect (overridable via `opts`
  later); single-pass `transcribe` task (no translate).
- whisper-rs **progress callback** → `progress_cb(pct)`; **new-segment callback** → stream partial lines.
- whisper-rs **abort callback** reads `abort_flag: Arc<AtomicBool>` → cooperative cancellation.

## 3. Data flow

```
Record → Save (finalize_recording already stores the audio MeetingAsset)
  → Meeting Detail (Save passes a `justRecorded` nav flag)
  → transcribe_meeting(meetingId)                     [auto if justRecorded + setting on, else manual button]
      → MeetingAssetsRepo::get_audio(meetingId) → path
      → resolve model from settings.transcription_model; if not downloaded → ModelNotDownloaded
      → decode → 16 kHz mono PCM
      → spawn blocking thread: whisper (emits transcription:progress + transcription:segment events)
      → on completion: persist transcript_lines + participant S1 + meetings.word_count (one transaction)
                       → emit transcription:completed
  → TranscriptTab refetch → renders the lines
```

The `TranscriptTab` subscribes to the `transcription:*` events (filtered by `meetingId`) and, on mount,
calls `get_transcription_state()` to re-attach to an already-running job (e.g. the user left and came back).

## 4. IPC — commands & events

New commands (`src-tauri/src/commands/transcription.rs`). Each long job is fire-and-forget: the command
kicks it off (or rejects) and returns; progress arrives via global events keyed by id, so a remounted view
re-attaches by re-subscribing + calling the matching `*_state` query.

```
transcribe_meeting(meetingId: String) -> Result<(), AppError>   // rejects: ModelNotDownloaded, TranscriptionBusy
cancel_transcription(meetingId: String) -> Result<(), AppError>
get_transcription_state() -> Option<TranscriptionState>         // { meetingId, pct } if a job is active
list_whisper_models() -> Vec<WhisperModelInfo>                  // { id, name, sizeMb, downloaded, downloadingPct? }
download_whisper_model(id: String) -> Result<(), AppError>      // rejects: DownloadBusy
delete_whisper_model(id: String) -> Result<(), AppError>
```

Global events (payload always carries the id; with one active job the frontend filters by it):

- Transcription: `transcription:progress { meetingId, pct }` · `transcription:segment { meetingId, t, text }`
  (live partial line) · `transcription:completed { meetingId, lineCount, wordCount }` ·
  `transcription:failed { meetingId, code, message }` · `transcription:cancelled { meetingId }`.
- Download: `whisper-download:progress { id, pct, bytesDownloaded, bytesTotal }` ·
  `whisper-download:completed { id }` · `whisper-download:failed { id, code, message }`.

This mirrors Sub-2's `audio:*` event + global-listener pattern (one code path, re-subscribable across
navigation) rather than a per-invocation Channel, which would orphan progress when the Meeting Detail view
is left and revisited mid-transcription.

## 5. Persistence

Reuse the existing **`transcript_lines`** table (migration `0001`) — no new migration:
`{ meeting_id, t_seconds, t_display, speaker_id, text_es, text_en }`.

- One row per Whisper segment: `t_seconds` = `start_ms / 1000`, `t_display` = `HH:MM:SS`,
  `speaker_id` = the meeting's single participant `S1`, `text_es` = transcribed text, `text_en` = `null`.
- **Naming caveat:** the spoken text always goes in `text_es` (the `NOT NULL` "primary" column) regardless
  of language; the UI shows it via `pickL`'s `es` fallback. When Sub-5 adds native-language translation it
  populates the other column. The schema's `es`/`en` columns are treated as "primary / translated", not
  literally Spanish/English. No schema change in 3a.
- Create **one participant `S1`** per meeting (label `S1`, `color_class` `s-color-1`) and update
  `meetings.word_count`. Line inserts + participant + word_count are written in **one transaction**
  (all-or-nothing).

## 6. Settings & UI

New `AppSettings` fields (Rust `core/models/settings.rs` + TS binding + default + persistence):

- `auto_transcribe: bool` — default `true`.
- `native_language: String` — e.g. `"es"`; stored now, consumed by Sub-5.
- `transcription_model` (already exists) now stores the **model id** (migrate the current
  `"Whisper Large v3"` default to `large-v3`).

UI — a **"Transcription" panel in Settings** (this is the deliverable's "model selection/download"):
model list with state (downloaded / available), **Download** button with progress bar, active-model
selector, **auto-transcribe** toggle, **native language** select.

**Auto-trigger (frontend-driven, one code path):** Save passes a `justRecorded` nav flag into Meeting
Detail. `TranscriptTab` auto-invokes `transcribe_meeting` only when **`justRecorded` is set AND
`auto_transcribe` is on** — so opening an old, never-transcribed meeting does *not* silently kick off a job
(and a no-speech meeting won't retry on every visit). The manual **"Transcribe"** button calls the exact
same command for the off-auto case, an old meeting, or a re-transcribe. On mount it also calls
`get_transcription_state()` to re-attach to a job already running for this meeting.

## 7. Concurrency

One transcription at a time. `AppState` holds a single slot `{ meeting_id, abort_flag: Arc<AtomicBool> }`.
A second `transcribe_meeting` while one is active → `TranscriptionBusy`. Model downloads are likewise
serialized (one at a time).

## 8. Error handling

Error codes (transcription error enum → `AppError`): `ModelNotDownloaded`, `TranscriptionBusy`,
`DecodeFailed`, `ModelLoadFailed`, `InferenceFailed`, `DownloadBusy`, `DownloadFailed` (network or sha256
mismatch). And `Cancelled` — **not** an error.

- Surfaced as a terminal `transcription:failed { code, message }` event plus the kickoff invoke rejection
  (for synchronous rejects like `TranscriptionBusy` / `ModelNotDownloaded`). The frontend maps the code to a
  translated toast, reusing Sub-2's `audio:error` i18n keys + per-code dedup pattern. `transcription:cancelled`
  → no toast, just resets the UI.
- **Atomicity:** lines persist only on `Completed`. A mid-inference failure or a cancel leaves no
  half-transcript; re-transcription starts clean.
- Download with bad sha256 → delete the partial → `DownloadFailed`. **No resume** in v1 (full re-download)
  — noted as minor debt.

## 9. Testing

- **Rust unit:** model-catalog integrity (ids/urls/sha non-empty, unique), `decode` golden (tiny WAV
  fixture → expected sample count at 16 kHz mono), segment→`TranscriptLine` mapping (timestamps→`t_display`,
  text), `word_count` computation, sha256 verify helper, serde for the new settings fields and the event
  enums.
- **Real inference:** behind a `whisper-integration` feature flag (mirrors Sub-2's `audio-integration`),
  needing a small model present locally; **not run in normal CI** (no multi-GB downloads). The compile-check
  of the crate + feature does enter the gate.
- **Frontend (vitest):** `TranscriptTab` states (empty / in-progress from `transcription:progress` events /
  completed renders lines / failed shows toast / cancelled resets), auto-trigger logic (auto only when the
  `justRecorded` flag is set + enabled; skipped on a plain visit, when disabled, or when already
  transcribed), manual button, cancel button, re-attach via `get_transcription_state` — all with mocked
  events. Settings panel (model list, download progress, select, toggles).
- **CI:** add `cargo check -p smart-noter-whisper` (+ the `whisper-integration` feature compile) to the
  gate, as Sub-2 did for `audio-integration`. No heavy inference in CI.

## 10. Distribution notes

- `whisper-rs` compiles whisper.cpp → needs a C/C++ toolchain (MSVC already present via Tauri). CPU build;
  adds compile time.
- Models are downloaded at runtime → the MSI stays small (no multi-GB bundle).
- `cuda` feature deferred (needs CUDA toolkit) — documented as future GPU work.
- Regenerate `bindings.ts` (specta) for the new types/commands and `keys.ts` (i18n) for the new strings.

## 11. Non-goals (YAGNI)

- VAD and real diarization → **Sub-3b**.
- Transcript translation to native language → **Sub-5**.
- Resumable model downloads; multiple concurrent transcriptions; per-meeting manual language override;
  GPU/CUDA build — all deferred.

## 12. Risks & open questions

- **whisper-rs build on Windows/MSVC** is the main integration risk (whisper.cpp C++ build). Validate early
  with a thin "load model → transcribe 5 s clip" spike before building the full pipeline.
- **large-v3 on CPU is slow** (minutes for long meetings). Auto-on-save + visible progress + cancel mitigate
  the UX; the Settings toggle lets users opt out or pick a smaller model.
- **Model URLs / sha256** must be pinned to a stable Hugging Face source; verify the exact ggml asset names
  and hashes during planning.
