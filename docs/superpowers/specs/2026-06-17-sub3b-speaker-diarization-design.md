# Smart Noter — Sub-3b: Speaker Diarization — Design Spec

**Date:** 2026-06-17
**Status:** Approved
**Scope:** Real speaker diarization — replace Sub-3a's single-speaker `S1` with "who spoke when", integrated into the existing transcription flow, with full manual correction.
**Depends on:** Sub-3a (Whisper transcription) — shipped (v0.3.0). **Unblocks:** richer per-speaker output for Sub-5 (AI Summary).

---

## 1. Goal & Decisions

Sub-3a transcribes audio and assigns every line to a single participant `S1`. Sub-3b detects **who is speaking when** and splits the transcript across the real speakers, integrated transparently into transcription.

Decisions taken during brainstorming (2026-06-17):

| Dimension | Decision |
|---|---|
| Accuracy level | **High accuracy** — pyannote-class pipeline; accepts a native ONNX dependency |
| Speaker count | **Auto-detect + optional hint** — clustering decides by default; user may pass an expected count |
| Flow | **Integrated** with transcription (single step), gated by the existing "Identify speakers" toggle |
| Manual correction | **Full** — rename + merge speakers + reassign lines + split |
| Engine | **sherpa-rs (static linking)** — sherpa-onnx via Rust bindings, ONNX Runtime compiled into the binary |

## 2. Non-goals

- **Source separation.** "Handle overlaps" means the diarizer resolves overlapping speech *internally* to assign each region better — it does NOT split simultaneous speech into two texts. Final representation stays **one line = one speaker** (the dominant one), because Whisper does not separate overlapping voices. Visible simultaneity is out of scope.
- **Cross-meeting speaker identity.** No persistent voice profiles / recognizing the same person across meetings. Each meeting is diarized independently. (Was offered and declined.)
- **Real-time / live diarization.** Diarization is a post-recording step over the saved `.wav`, like transcription.
- **GPU acceleration.** CPU-first; GPU is future work (mirrors Sub-3a's deferred CUDA).

## 3. Engine — sherpa-rs (static)

[sherpa-rs](https://github.com/thewh1teagle/sherpa-rs) wraps [sherpa-onnx](https://k2-fsa.github.io/sherpa/onnx/speaker-diarization/index.html) and provides offline speaker diarization (VAD + pyannote-style segmentation + speaker embeddings + clustering) with a Rust `diarize` example. Chosen because:

- **Windows confirmed** + **static linking** (`static` feature) → ONNX Runtime is compiled into the binary, **no `onnxruntime.dll` to ship in the MSI** (Sub-8).
- Pre-converted ONNX models from k2-fsa are **not gated** and downloadable on demand, mirroring the Whisper model flow.
- Auto-detect via clustering threshold **or** fixed `num_clusters` → covers "auto + optional hint" directly.

**Models** (downloaded on demand, like Whisper): a segmentation model (~pyannote segmentation, small) + a speaker-embedding model (3D-Speaker / wespeaker). Catalogued with sha256 + progress.

**Build dependency:** sherpa-onnx static libs must be available to the linker (a new build-tool dependency, analogous to LLVM/cmake for `whisper-rs` — see the whisper build-toolchain note). Validated in the Phase-0 spike.

**Fallback (plan B):** if static sherpa on Windows proves unworkable, [pyannote-rs](https://github.com/RustedBytes/pyannote-rs) (pure-Rust Burn, no ONNX Runtime) — lower-accuracy clustering, but the rest of this design (data model, alignment, UI, correction) is engine-agnostic and unchanged.

## 4. Architecture & Pipeline

Diarization runs inside the **existing transcription job** (`commands/transcription.rs`). When the "Identify speakers" toggle is ON:

```
audio .wav
  └─ decode → PCM 16 kHz mono                 (Sub-3a, reused)
       ├─ [1] diarize (sherpa-rs)  → segments {start, end, speaker#}
       ├─ [2] transcribe (whisper) → segments {start, end, text}   (Sub-3a, reused)
       └─ [3] align: each TEXT segment is assigned the speaker with
              the greatest temporal overlap → lines {t, end, speaker#, text}
  └─ persist: create N participants (S1..Sn) + assign each line + real talk_pct
  └─ events → UI renders lines split by speaker
```

- Diarization and transcription run over the **same PCM**, sequentially, in the same job/thread; progress events reuse the Sub-3a channel.
- Toggle OFF → exact Sub-3a behavior (single `S1`). No regression.
- "Identify speakers" is a **persisted setting** (alongside `autoTranscribe`), honored by both paths: auto-transcription after recording, and the **manual "Transcribe" button on an existing meeting** — so a Sub-3a `S1`-only meeting can be re-diarized by re-running it. The pre-record toggle edits this setting.
- **Alignment** is the key new, model-free, independently testable unit.

## 5. Data Model & Persistence

The current schema already models multi-speaker (`participants` is not limited to one; `transcript_lines.speaker_id` already references a participant). Changes are small:

**Migration `0003`:** add `end_seconds` to `transcript_lines`. Whisper already produces each segment's end (`end_ms`); Sub-3a discarded it. Storing it enables **`talk_pct` by real speech duration** (not word count) and **recomputation after corrections**.

**Persistence:**
- `transcript_repo::replace_lines` generalizes from the fixed `S1` to: take the detected speakers + each line's assigned speaker, create **N participants** (`S1..Sn`, colors `s-color-1..n` cycling past 5), and assign each line.

**New repo ops (full correction):**
- `merge_speakers(into, from)` — reassign `from`'s lines to `into`, delete `from`, recompute `word_count`/`talk_pct`.
- `reassign_lines(line_ids, speaker_id)` — change lines' speaker (existing **or new** → this *is* "split").
- `create_speaker(meeting_id)` + rename (already exists).
- Each op recomputes `word_count`/`talk_pct` for affected speakers.

## 6. UI

- **Pre-record** (existing "Identify speakers" toggle): add an optional **"expected speaker count"** field (empty = auto-detect) — the optional hint.
- **Transcript tab**: lines render split by speaker, each with avatar + color (`s-color-1..n`); the right participants panel lists real speakers with `talk_pct`.
- **Full correction** (reusing existing components):
  1. **Reassign / split** — a line's speaker chip becomes clickable → menu of existing speakers + **"➕ New speaker"** (creating `Sn+1` and assigning = split). A **"Select lines" mode** (checkboxes) reassigns several at once.
  2. **Merge** — each speaker in the participants panel gets a "···" → **"Merge into…"**.
  3. **Rename** — already exists.
- Every action calls its backend command, which recomputes `talk_pct`/`word_count`; the UI **refetches** the meeting (same pattern as `transcription:completed`). No fragile local state.
- Heaviest FE piece: the multi-select line mode; the rest are menus over existing components.

## 7. Components (code structure)

- **New crate `smart-noter-diarize`** (isolates the heavy `sherpa-rs`, mirroring how `whisper` is isolated): `models` (catalog + on-demand download of the 2 ONNX models, sha256/progress), `diarize` (sherpa-rs pipeline), `align` (pure alignment), `error`.
- **Job** (`commands/transcription.rs`) extended: after `decode`, branch on the toggle (diarize+transcribe+align vs Sub-3a transcribe-only).
- **New Tauri commands**: `merge_speakers`, `reassign_lines`, `create_speaker` + diarization-model `list/download/delete`. The speaker-count hint is a param of `transcribe_meeting`.
- **Settings**: diarization model id (+ default). Toggle + hint fit the existing settings flow.

## 8. Error Handling

- Missing models → prompt download (like Whisper).
- **Diarization fails but transcription succeeds → degrade to Sub-3a** (single `S1`) + a toast — never lose the transcript.
- Reuse the `AppError` pattern with diarization-specific codes (`ModelNotDownloaded`, `DiarizationFailed`, …), nested `{code, message}` like Audio/Transcription.

## 9. Testing

- **Unit**: `align` (pure; synthetic segments — clean turns, overlaps, text straddling a boundary) and the repo ops (`merge`/`reassign`/recompute) against an in-memory DB.
- **Integration**: the sherpa-rs spike, feature-gated (`diarize-integration`), soft in CI like `whisper-integration` until the runner has the libs.
- **Smoke**: record with **two es-MX TTS voices (Sabina + Raúl)** → verify split into S1/S2 + exercise merge / reassign / split.

## 10. Spike (Phase 0) & Risks

The implementation plan **starts with a feasibility spike**: build `sherpa-rs` with static linking on Windows and diarize a real `.wav`, to pin the exact API (inputs/outputs, how `num_clusters`/threshold are passed) and validate linking **before** building the rest. This mirrors Sub-3a's whisper-rs spike.

| Risk | Mitigation |
|---|---|
| Static sherpa-onnx linking fails on Windows | Phase-0 spike; fallback to pyannote-rs/Burn (design unchanged) |
| sherpa-rs API differs from assumptions | Spike pins it before planning the rest |
| Diarization too slow on CPU for long meetings | Measure in spike; it's post-process (not live); GPU deferred |
| Alignment edge cases (overlap, straddling) | Covered by unit tests on the pure aligner |

## 11. Future (out of this spec)

- Cross-meeting voice identity, source separation, GPU — all deferred.
- Per-speaker summaries consume this output in Sub-5.
