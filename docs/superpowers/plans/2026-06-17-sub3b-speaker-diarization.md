# Sub-3b: Speaker Diarization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace Sub-3a's single-speaker `S1` transcript with real "who spoke when" — diarization runs inside the existing transcription job, splits the transcript across N detected speakers, and exposes full manual correction (rename / merge / reassign / split).

**Architecture:** A new isolated `smart-noter-diarize` crate wraps `sherpa-rs` (sherpa-onnx, statically linked — no DLL in the MSI). The transcription job, when the persisted "Identify speakers" setting is ON, runs `decode → diarize + transcribe → align` over the same PCM; a pure, fully-tested aligner assigns each Whisper text segment to the diarization speaker with the greatest temporal overlap; persistence generalizes from a fixed `S1` to N participants with real per-speaker talk-time. Diarization failure degrades gracefully to Sub-3a (single `S1`) + a toast — the transcript is never lost. The plan **starts with a feasibility spike** (Phase 0) to pin the sherpa-rs API and validate static linking on Windows before building the rest.

**Tech Stack:** Rust (workspace crates), `sherpa-rs` (static feature, ONNX Runtime compiled in), `whisper-rs` 0.16 (reused), `sqlx` 0.8 (SQLite), Tauri 2 + `tauri-specta`, React + Redux Toolkit Query + i18next.

---

## Conventions used throughout this plan

**Build env preamble (Windows).** This machine's Claude shells do NOT inherit the persisted user env. **Every `cargo` and `git` command** (the lefthook pre-commit hook rebuilds native crates) must be prefixed with this preamble in a Bash tool call:

```bash
export PATH="$HOME/.cargo/bin:/c/Program Files (x86)/Microsoft Visual Studio/2022/BuildTools/Common7/IDE/CommonExtensions/Microsoft/CMake/CMake/bin:$PATH"
export LIBCLANG_PATH="C:/Program Files/LLVM/bin"
```

Symptom of forgetting it: `failed to run custom build command for whisper-rs-sys` → "is cmake not installed?" / "Unable to find libclang". Do NOT hardcode these paths into `lefthook.yml` (tried and reverted before).

**`whisper-rs` 0.16 gotcha (still applies to any new whisper work):** never use `FullParams::set_abort_callback_safe` (its trampoline is bugged). The existing `transcribe.rs` already wires the raw callback correctly; we reuse it unchanged.

**sqlx style:** the existing `transcript_repo` uses **unchecked** `sqlx::query(...)` / `sqlx::query_scalar(...)` (runtime-validated, no compile-time DB needed). **All new repo code in this plan uses the unchecked API too**, so we do **NOT** need to run `cargo sqlx prepare` or touch `src-tauri/.sqlx`. Adding a column (migration 0003) does not invalidate existing `query!` cache entries because none of them `SELECT *`. (If a future change uses the checked `query!` macro, regenerate with: `cd src-tauri && DATABASE_URL="sqlite://./crates/db/sn_prepare.db" cargo sqlx prepare --workspace -- --workspace --tests`.)

**After backend signature/command changes, regenerate the IPC + i18n artifacts** (both are git-tracked generated files):

```bash
# bindings (builds the specta-export bin — needs the env preamble)
pnpm generate:bindings
# i18n key union type (reads src/i18n/locales/es.json)
pnpm generate:i18n-keys
```

**Commit cadence:** every task ends with a commit. Commit messages are English; end each with the Co-Authored-By trailer the repo uses.

**Decision default locked during planning:** the persisted "Identify speakers" setting defaults **ON** (`true`). When ON but the diarization models are missing, the job degrades to Sub-3a (single `S1`) and emits a toast — it does not hard-fail.

---

## File Structure

**New crate `smart-noter-diarize`** (`src-tauri/crates/diarize/`) — isolates the heavy `sherpa-rs`, mirroring how `whisper` is isolated:
- `Cargo.toml` — `sherpa-rs` (static feature) + `serde`, `thiserror`, `ureq`, `sha2`, `smart-noter-core`; `diarize-integration` feature flag.
- `src/lib.rs` — module wiring + re-exports.
- `src/error.rs` — `DiarizationError` + `DiarizationErrorCode` + `From<…> for AppError`.
- `src/align.rs` — **pure** overlap-based aligner (the testable heart). No model dependency.
- `src/models.rs` — catalog (segmentation + embedding ONNX), on-demand download/verify/delete (mirrors `whisper::models`).
- `src/diarize.rs` — `sherpa-rs` pipeline wrapper (the only file that touches `sherpa-rs`).
- `tests/spike.rs` — feature-gated (`diarize-integration`) static-linking + API spike.

**Modified backend:**
- `src-tauri/Cargo.toml` — add `crates/diarize` to workspace members + as a dependency of the bin.
- `src-tauri/crates/db/migrations/0003_transcript_end_seconds.sql` — **new**: add `end_seconds` to `transcript_lines`.
- `src-tauri/crates/db/src/repos/transcript_repo.rs` — generalize `replace_lines` (N speakers, `end_seconds`, talk-time by duration).
- `src-tauri/crates/db/src/repos/participants_repo.rs` — add `merge_speakers`, `reassign_lines`, `create_speaker`, and a shared `recompute_speaker_stats` helper.
- `src-tauri/crates/core/src/models/settings.rs` — add `identify_speakers: bool` + `diarization_model: String`.
- `src-tauri/src/commands/transcription.rs` — branch the job on the toggle; add `speaker_count_hint` param; add diarization-model list/download/delete commands + events.
- `src-tauri/src/commands/meetings.rs` — add `merge_speakers`, `reassign_lines`, `create_speaker` commands.
- `src-tauri/src/lib.rs` — register the new commands with specta.

**Modified frontend:**
- `src/ipc/bindings.ts`, `src/i18n/keys.ts` — regenerated (do not hand-edit).
- `src/i18n/locales/es.json` + `en.json` — new diarization keys.
- `src/features/pre-record/PreRecordPage.tsx` — wire the "Identify speakers" toggle to the persisted setting + add the optional speaker-count field; carry the hint in nav state.
- `src/features/meeting-detail/useTranscription.ts` — `start(hint?)`; degrade toast on diarization-failed event.
- `src/features/meeting-detail/tabs/TranscriptTab.tsx` — reassign/split UI (clickable speaker chip + select-lines mode).
- `src/features/meeting-detail/side/SidePanel.tsx` — per-speaker "···" menu (Merge into… / add speaker).
- `src/features/settings/TranscriptionPanel.tsx` (or a sibling `DiarizationPanel.tsx`) — diarization-model manage UI.
- `src/store/api/meetings.api.ts` — RTK mutations for merge/reassign/create.

---

# Phase 0 — Feasibility Spike (de-risk sherpa-rs static linking on Windows)

**Purpose:** prove `sherpa-rs` **static linking works on this Windows toolchain** and validate that the pipeline detects the right number of speakers on a real recording, **before** building Phases 2–6. The sherpa-rs API is **already confirmed from the 0.6.8 source** (see below) — so the spike's job narrows to *linking + accuracy*, not API discovery. This mirrors Sub-3a's whisper-rs Phase-0 spike. **If static linking proves unworkable, STOP and switch to the pyannote-rs fallback** (Section "Fallback" below) — the rest of the plan (align, data model, UI, correction) is engine-agnostic and unchanged.

**Confirmed `sherpa-rs` 0.6.8 API** (read from `crates/sherpa-rs/src/diarize.rs`):

```rust
pub struct Segment { pub start: f32, pub end: f32, pub speaker: i32 } // start/end in SECONDS, speaker 0-based
pub struct DiarizeConfig {
    pub num_clusters: Option<i32>,   // Some(n) to force n speakers; None → use threshold
    pub threshold: Option<f32>,      // clustering distance threshold when num_clusters is None
    pub min_duration_on: Option<f32>,
    pub min_duration_off: Option<f32>,
    pub provider: Option<String>,
    pub debug: bool,
}
impl Diarize {
    pub fn new<P: AsRef<Path>>(segmentation_model: P, embedding_model: P, config: DiarizeConfig) -> Result<Self>;
    pub fn compute(&mut self, samples: Vec<f32>, progress_callback: Option<ProgressCallback>) -> Result<Vec<Segment>>;
}
```

`num_clusters` and `threshold` are mutually exclusive. Input audio must be **mono 16 kHz f32 PCM**.

**Models already downloaded** (during planning) to `C:/Users/erick/diarize-models/`:
- `segmentation.onnx` — pyannote-segmentation-3-0, 5.71 MB, sha256 `220ad67ca923bef2fa91f2390c786097bf305bceb5e261d4af67b38e938e1079`
- `embedding.onnx` — wespeaker_en_voxceleb_CAM++, 27.93 MB, sha256 `c46fad10b5f81e1aa4a60c162714208577093655076c5450f8c469e522ec54ef`

### Task 0.1: Scaffold the `smart-noter-diarize` crate

**Files:**
- Create: `src-tauri/crates/diarize/Cargo.toml`
- Create: `src-tauri/crates/diarize/src/lib.rs`
- Create: `src-tauri/crates/diarize/src/error.rs`
- Modify: `src-tauri/Cargo.toml` (workspace members + bin dependency)

- [ ] **Step 1: Create the crate manifest**

`src-tauri/crates/diarize/Cargo.toml`:

```toml
[package]
name = "smart-noter-diarize"
version.workspace = true
edition.workspace = true

[dependencies]
# `static` static-links sherpa-onnx (no onnxruntime.dll to ship).
# `download-binaries` fetches pre-built STATIC libs from GitHub releases
# (sherpa-onnx-<tag>-win-x64-static.tar.bz2) — REQUIRED on Windows 11: the
# `static`-only path builds sherpa-onnx from source via CMake, whose
# show-info.cmake calls `wmic`, removed in Win11 → configure fails. With
# download-binaries, is_dynamic stays false (the win-x64 dist has both static
# and dynamic variants and no `is_dynamic` override), so linking is static and
# NO DLL is copied to target. `tts` is excluded (sherpa-rs forbids
# static+download-binaries+tts together; prebuilt static has no TTS anyway).
sherpa-rs = { version = "0.6.8", default-features = false, features = ["static", "download-binaries"] }
ureq = "2.10"
sha2 = "0.10"
serde = { workspace = true }
thiserror = { workspace = true }
smart-noter-core = { path = "../core" }

[features]
# Soft-gated integration tests (need the ONNX models + static libs), like whisper-integration.
diarize-integration = []

[dev-dependencies]
tempfile = "3"
hound = "3.5"
```

> NOTE (verified during Task 0.1 execution): the spec's bare `features = ["static"]` does NOT build on Windows 11 — sherpa-onnx's bundled CMake calls `wmic` (removed in Win11). The working config is `["static", "download-binaries"]`, which downloads pre-built **static** libs (~1 min) instead of compiling from source. Outcome is identical to the spec's intent: static linkage, no `onnxruntime.dll` in the MSI. Confirmed by reading `sherpa-rs-sys-0.6.8/build.rs` (final `if is_dynamic { copy DLLs }` is skipped) + `dist.json` (win-x64 has a `static` archive). If even this fails to LINK a binary on this toolchain, that is the Phase-0 finding → pyannote-rs fallback (see end of plan).

- [ ] **Step 2: Create the error type** (mirror `whisper/src/error.rs`)

`src-tauri/crates/diarize/src/error.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DiarizationErrorCode {
    ModelNotDownloaded,
    ModelLoadFailed,
    DiarizationFailed,
    DownloadBusy,
    DownloadFailed,
    Cancelled,
}

#[derive(Debug, thiserror::Error)]
#[error("{code:?}: {message}")]
pub struct DiarizationError {
    pub code: DiarizationErrorCode,
    pub message: String,
}

impl From<DiarizationError> for smart_noter_core::AppError {
    fn from(e: DiarizationError) -> Self {
        smart_noter_core::AppError::Transcription {
            code: format!("{:?}", e.code),
            message: e.message,
        }
    }
}
```

> NOTE: we reuse `AppError::Transcription` (the diarization UI lives inside the transcription flow) rather than adding a new `AppError` variant — keeps the IPC error surface unchanged.

- [ ] **Step 3: Create the lib root**

`src-tauri/crates/diarize/src/lib.rs` (minimal — each later phase adds its own `pub mod` line so every task compiles on its own):

```rust
//! Local speaker diarization: model management, sherpa-rs pipeline, and a pure aligner.

pub mod error;

pub use error::{DiarizationError, DiarizationErrorCode};

// pub mod align;    — added in Phase 1
// pub mod models;   — added in Phase 2
// pub mod diarize;  — added in Phase 3
```

> NOTE: do NOT declare `align`/`models`/`diarize` here yet — each is added in its own phase together with the file, so `cargo check` stays green between tasks (no placeholder files needed).

- [ ] **Step 4: Register the crate in the workspace**

In `src-tauri/Cargo.toml`, add to `[workspace] members` (after `"crates/whisper"`):

```toml
  "crates/diarize",
```

and add to the bin's `[dependencies]` (after the `smart-noter-whisper` line):

```toml
smart-noter-diarize = { path = "crates/diarize" }
```

- [ ] **Step 5: Verify it compiles**

Run (with env preamble):
```bash
cd src-tauri && cargo check -p smart-noter-diarize
```
Expected: PASS (the first sherpa-rs build is slow — several minutes — as it compiles ONNX Runtime statically). If it fails to **find** the crate version, adjust the `sherpa-rs` version in Cargo.toml and retry. If it fails to **link/build** the native libs, that is the spike's real finding — proceed to Task 0.2 to diagnose, and invoke the fallback decision if unresolved.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/crates/diarize src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "feat(diarize): scaffold smart-noter-diarize crate (sherpa-rs static)"
```

### Task 0.2: Spike sherpa-rs — pin the API and prove static linking

**Files:**
- Create: `src-tauri/crates/diarize/tests/spike.rs`

- [ ] **Step 1: Write the feature-gated spike test**

`src-tauri/crates/diarize/tests/spike.rs`:

```rust
#![cfg(feature = "diarize-integration")]

// Run manually (models already downloaded to C:\Users\erick\diarize-models):
//   SHERPA_SEG_MODEL=C:\Users\erick\diarize-models\segmentation.onnx \
//   SHERPA_EMB_MODEL=C:\Users\erick\diarize-models\embedding.onnx \
//   SHERPA_TEST_WAV=C:\path\two-speakers.wav \
//   cargo test -p smart-noter-diarize --features diarize-integration -- --ignored spike --nocapture
//
// GOAL: confirm sherpa-rs links STATICALLY on Windows and finds the right
// number of speakers. The API below is already confirmed from sherpa-rs 0.6.8.
#[test]
#[ignore = "needs real ONNX models + a wav via env vars"]
fn spike_diarizes_two_speakers() {
    use sherpa_rs::diarize::{Diarize, DiarizeConfig};

    let seg = std::env::var("SHERPA_SEG_MODEL").expect("set SHERPA_SEG_MODEL");
    let emb = std::env::var("SHERPA_EMB_MODEL").expect("set SHERPA_EMB_MODEL");
    let wav = std::env::var("SHERPA_TEST_WAV").expect("set SHERPA_TEST_WAV");

    let (samples, sample_rate) = read_wav_f32_mono(&wav);
    assert_eq!(sample_rate, 16_000, "models expect 16 kHz");

    let config = DiarizeConfig {
        num_clusters: None,        // auto-detect via threshold; the real wrapper passes Some(n) for a hint
        threshold: Some(0.5),
        min_duration_on: Some(0.3),
        min_duration_off: Some(0.5),
        provider: None,
        debug: false,
    };
    let mut sd = Diarize::new(&seg, &emb, config).expect("init diarizer");
    let segments = sd.compute(samples, None).expect("diarize");

    let speakers: std::collections::BTreeSet<i32> = segments.iter().map(|s| s.speaker).collect();
    for s in &segments {
        println!("start={:.2}s end={:.2}s speaker={}", s.start, s.end, s.speaker);
    }
    println!("distinct speakers = {}", speakers.len());
    // On a clean 2-voice recording this should be 2. If linking worked but the
    // count is off, tune threshold/min_duration in the real wrapper (Phase 3).
    assert!(!segments.is_empty(), "expected at least one diarized segment");
}

fn read_wav_f32_mono(path: &str) -> (Vec<f32>, u32) {
    let mut r = hound::WavReader::open(path).expect("open wav");
    let spec = r.spec();
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => r.samples::<f32>().map(|s| s.unwrap()).collect(),
        hound::SampleFormat::Int => r
            .samples::<i32>()
            .map(|s| s.unwrap() as f32 / i32::from(i16::MAX) as f32)
            .collect(),
    };
    (samples, spec.sample_rate)
}
```

- [ ] **Step 2: Models + test wav**

The two ONNX models are **already downloaded** to `C:/Users/erick/diarize-models/` (`segmentation.onnx`, `embedding.onnx`) with sha256 recorded above. You only need a 2-speaker test `.wav` (16 kHz mono): use a real recording or generate one with two es-MX TTS voices (see Phase 7). Set `SHERPA_SEG_MODEL`, `SHERPA_EMB_MODEL` to the two model paths and `SHERPA_TEST_WAV` to the wav.

- [ ] **Step 3: Run the spike**

Run (with env preamble + the env vars from Step 2):
```bash
cd src-tauri && cargo test -p smart-noter-diarize --features diarize-integration -- --ignored spike_diarizes_two_speakers --nocapture
```
Expected: it links statically and runs. Replace the `panic!` with the real API calls, observe the printed `start/end/speaker` values, and confirm the model finds **2** speakers on the 2-speaker wav.

- [ ] **Step 4: Confirm the API matches reality**

The API is already pinned from 0.6.8 source (above). If the spike reveals ANY divergence (field renamed, units differ, `compute` signature changed in the installed version), the **running code is authoritative** — update the spike comment AND the Phase-3 `diarize.rs` wrapper to match. Otherwise just confirm the printed segments look sane (correct speaker count, plausible time ranges).

- [ ] **Step 5: Decision gate (static linking)**

- ✅ Static link works → continue to Phase 1.
- ❌ Static link unworkable on Windows after reasonable effort → **STOP**, invoke the pyannote-rs fallback (replace the `sherpa-rs` dependency + `diarize.rs` only; align/models/data/UI unchanged), and note the deviation in `project_sub3b_diarization_state` memory.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/crates/diarize/tests/spike.rs src-tauri/crates/diarize/Cargo.toml
git commit -m "test(diarize): sherpa-rs static-linking + API spike (Phase 0)"
```

---

# Phase 1 — Pure Alignment (model-free, fully TDD)

The aligner is the key new, model-free, independently testable unit: each Whisper **text** segment is assigned the diarization speaker with the **greatest temporal overlap**. Built test-first.

### Task 1.1: Implement the overlap aligner

**Files:**
- Create: `src-tauri/crates/diarize/src/align.rs`
- Modify: `src-tauri/crates/diarize/src/lib.rs` (declare + re-export)
- Test: same file (`#[cfg(test)] mod tests`)

- [ ] **Step 0: Declare the module**

In `src-tauri/crates/diarize/src/lib.rs`, add (replacing the `// pub mod align;` comment):

```rust
pub mod align;
pub use align::{align, AlignedLine, DiarSegment};
```

- [ ] **Step 1: Write the failing tests**

Create `src-tauri/crates/diarize/src/align.rs` with:

```rust
//! Pure, model-free alignment: assign each transcription text segment the
//! diarization speaker whose time range overlaps it most. No external deps.

/// One diarization region (output of the sherpa-rs pipeline), in **milliseconds**.
/// (Phase 3 converts sherpa's seconds → ms before calling `align`.)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiarSegment {
    pub start_ms: u32,
    pub end_ms: u32,
    pub speaker: u32,
}

/// A transcription segment after speaker assignment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlignedLine {
    pub start_ms: u32,
    pub end_ms: u32,
    pub speaker: u32,
    pub text: String,
}

/// Minimal text-segment shape the aligner needs (a structural mirror of
/// `smart_noter_whisper::Segment`, kept local so this crate has no whisper dep).
#[derive(Debug, Clone)]
pub struct TextSegment {
    pub start_ms: u32,
    pub end_ms: u32,
    pub text: String,
}

/// Overlap (in ms) of [a0,a1) and [b0,b1); 0 if disjoint.
fn overlap_ms(a0: u32, a1: u32, b0: u32, b1: u32) -> u32 {
    let lo = a0.max(b0);
    let hi = a1.min(b1);
    hi.saturating_sub(lo)
}

/// Distance (in ms) from text segment [a0,a1) to diar segment [b0,b1); 0 if they touch/overlap.
fn gap_ms(a0: u32, a1: u32, b0: u32, b1: u32) -> u32 {
    if a1 <= b0 {
        b0 - a1
    } else if b1 <= a0 {
        a0 - b1
    } else {
        0
    }
}

/// Assign each text segment a speaker. Rule: the diar segment with the greatest
/// overlap wins; ties break to the lower speaker number for determinism. If a
/// text segment overlaps no diar segment, fall back to the **nearest** diar
/// segment (smallest gap). If there are no diar segments at all, everything is
/// speaker 0.
pub fn align(texts: &[TextSegment], diar: &[DiarSegment]) -> Vec<AlignedLine> {
    texts
        .iter()
        .map(|t| {
            let speaker = pick_speaker(t.start_ms, t.end_ms, diar);
            AlignedLine {
                start_ms: t.start_ms,
                end_ms: t.end_ms,
                speaker,
                text: t.text.clone(),
            }
        })
        .collect()
}

fn pick_speaker(t0: u32, t1: u32, diar: &[DiarSegment]) -> u32 {
    if diar.is_empty() {
        return 0;
    }
    // 1) best by overlap
    let mut best: Option<(u32 /*overlap*/, u32 /*speaker*/)> = None;
    for d in diar {
        let ov = overlap_ms(t0, t1, d.start_ms, d.end_ms);
        if ov > 0 {
            match best {
                Some((bov, bsp)) if (ov, std::cmp::Reverse(d.speaker)) <= (bov, std::cmp::Reverse(bsp)) => {}
                _ => best = Some((ov, d.speaker)),
            }
        }
    }
    if let Some((_, sp)) = best {
        return sp;
    }
    // 2) no overlap → nearest by gap (ties → lower speaker)
    let mut nearest: Option<(u32 /*gap*/, u32 /*speaker*/)> = None;
    for d in diar {
        let g = gap_ms(t0, t1, d.start_ms, d.end_ms);
        match nearest {
            Some((bg, bsp)) if (g, d.speaker) >= (bg, bsp) => {}
            _ => nearest = Some((g, d.speaker)),
        }
    }
    nearest.map(|(_, sp)| sp).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(start_ms: u32, end_ms: u32, text: &str) -> TextSegment {
        TextSegment { start_ms, end_ms, text: text.into() }
    }
    fn d(start_ms: u32, end_ms: u32, speaker: u32) -> DiarSegment {
        DiarSegment { start_ms, end_ms, speaker }
    }

    #[test]
    fn clean_turns_each_line_gets_its_speaker() {
        let texts = vec![t(0, 1000, "hola"), t(2000, 3000, "que tal")];
        let diar = vec![d(0, 1500, 0), d(1500, 3500, 1)];
        let out = align(&texts, &diar);
        assert_eq!(out[0].speaker, 0);
        assert_eq!(out[1].speaker, 1);
    }

    #[test]
    fn text_straddling_a_boundary_goes_to_the_greater_overlap() {
        // 0..1200 overlaps spk0 by 1000ms (0..1000) and spk1 by 200ms (1000..1200)
        let texts = vec![t(0, 1200, "straddle")];
        let diar = vec![d(0, 1000, 0), d(1000, 5000, 1)];
        assert_eq!(align(&texts, &diar)[0].speaker, 0);
    }

    #[test]
    fn no_overlap_falls_back_to_nearest() {
        let texts = vec![t(4000, 4500, "orphan")];
        let diar = vec![d(0, 1000, 0), d(3000, 3800, 1)]; // nearest is spk1 (gap 200)
        assert_eq!(align(&texts, &diar)[0].speaker, 1);
    }

    #[test]
    fn empty_diarization_assigns_speaker_zero() {
        let texts = vec![t(0, 1000, "alone")];
        assert_eq!(align(&texts, &[])[0].speaker, 0);
    }

    #[test]
    fn overlap_tie_breaks_to_lower_speaker_number() {
        // equal 500ms overlap with spk0 and spk1 → spk0 wins
        let texts = vec![t(500, 1500, "tie")];
        let diar = vec![d(0, 1000, 1), d(1000, 2000, 0)];
        assert_eq!(align(&texts, &diar)[0].speaker, 0);
    }

    #[test]
    fn preserves_text_and_timestamps() {
        let texts = vec![t(7, 9, "x")];
        let diar = vec![d(0, 100, 3)];
        let out = align(&texts, &diar);
        assert_eq!(out[0].start_ms, 7);
        assert_eq!(out[0].end_ms, 9);
        assert_eq!(out[0].text, "x");
    }
}
```

- [ ] **Step 2: Run the tests to verify they pass**

Run (with env preamble):
```bash
cd src-tauri && cargo test -p smart-noter-diarize align
```
Expected: 6 tests PASS. (The code and tests are written together here because the aligner is pure and small; if any tie-break test fails, the bug is in `pick_speaker`'s comparison — fix until green.)

- [ ] **Step 3: Commit**

```bash
git add src-tauri/crates/diarize/src/align.rs
git commit -m "feat(diarize): pure overlap-based speaker aligner + tests"
```

---

# Phase 2 — Diarization Models (catalog / download / verify / delete)

Mirrors `whisper::models` exactly (proven download+sha256+atomic-rename code). Two ONNX components form one diarization set.

### Task 2.1: Implement the models module

**Files:**
- Create: `src-tauri/crates/diarize/src/models.rs`
- Modify: `src-tauri/crates/diarize/src/lib.rs` (declare)
- Test: same file

- [ ] **Step 0: Declare the module**

In `src-tauri/crates/diarize/src/lib.rs`, add (replacing the `// pub mod models;` comment):

```rust
pub mod models;
```

- [ ] **Step 1: Write the module with tests**

Create `src-tauri/crates/diarize/src/models.rs` with the following (URLs + sha256 + sizes are already filled with the real values verified during planning):

```rust
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use crate::error::{DiarizationError, DiarizationErrorCode};

/// One downloadable ONNX component of the diarization set.
#[derive(Debug, Clone)]
pub struct ModelSpec {
    pub id: &'static str,           // stable id, also the on-disk file name
    pub display_name: &'static str,
    pub size_mb: u32,
    pub sha256: &'static str,
    pub url: &'static str,
}

impl ModelSpec {
    pub fn file_name(&self) -> String {
        format!("{}.onnx", self.id)
    }
}

/// The canonical diarization set: a pyannote-style segmentation model + a
/// speaker-embedding model. Both must be present to diarize.
/// Values verified by downloading the files during planning (Task 0.2).
pub const CATALOG: &[ModelSpec] = &[
    ModelSpec {
        id: "segmentation",
        display_name: "Speaker Segmentation (pyannote 3.0)",
        size_mb: 6, // 5_992_913 bytes
        sha256: "220ad67ca923bef2fa91f2390c786097bf305bceb5e261d4af67b38e938e1079",
        // Direct .onnx on HuggingFace (avoids the .tar.bz2 archive on GitHub releases).
        url: "https://huggingface.co/csukuangfj/sherpa-onnx-pyannote-segmentation-3-0/resolve/main/model.onnx",
    },
    ModelSpec {
        id: "embedding",
        display_name: "Speaker Embedding (WeSpeaker CAM++)",
        size_mb: 28, // 29_292_684 bytes
        sha256: "c46fad10b5f81e1aa4a60c162714208577093655076c5450f8c469e522ec54ef",
        url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/speaker-recongition-models/wespeaker_en_voxceleb_CAM++.onnx",
    },
];

pub fn find(id: &str) -> Option<&'static ModelSpec> {
    CATALOG.iter().find(|m| m.id == id)
}

/// Diarization models live in `<app_data>/diarize-models` (separate from whisper's `models`).
pub fn models_dir(app_data: &Path) -> PathBuf {
    app_data.join("diarize-models")
}

pub fn model_path(app_data: &Path, id: &str) -> Option<PathBuf> {
    find(id).map(|m| models_dir(app_data).join(m.file_name()))
}

/// True only when EVERY component in the catalog is present on disk.
pub fn all_present(app_data: &Path) -> bool {
    CATALOG
        .iter()
        .all(|m| models_dir(app_data).join(m.file_name()).is_file())
}

#[derive(Debug, Clone)]
pub struct ModelStatus {
    pub id: &'static str,
    pub display_name: &'static str,
    pub size_mb: u32,
    pub downloaded: bool,
}

pub fn list(app_data: &Path) -> Vec<ModelStatus> {
    CATALOG
        .iter()
        .map(|m| ModelStatus {
            id: m.id,
            display_name: m.display_name,
            size_mb: m.size_mb,
            downloaded: models_dir(app_data).join(m.file_name()).is_file(),
        })
        .collect()
}

fn err(code: DiarizationErrorCode, message: impl Into<String>) -> DiarizationError {
    DiarizationError { code, message: message.into() }
}

pub fn verify_sha256(path: &Path, expected: &str) -> Result<(), DiarizationError> {
    use sha2::{Digest, Sha256};
    let mut file = std::fs::File::open(path)
        .map_err(|e| err(DiarizationErrorCode::DownloadFailed, e.to_string()))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 1 << 16];
    loop {
        let n = file
            .read(&mut buf)
            .map_err(|e| err(DiarizationErrorCode::DownloadFailed, e.to_string()))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let got: String = hasher.finalize().iter().map(|b| format!("{b:02x}")).collect();
    if got.eq_ignore_ascii_case(expected) {
        Ok(())
    } else {
        Err(err(DiarizationErrorCode::DownloadFailed, format!("sha256 mismatch: got {got}")))
    }
}

pub fn delete(app_data: &Path, id: &str) -> Result<(), DiarizationError> {
    let spec = find(id)
        .ok_or_else(|| err(DiarizationErrorCode::DownloadFailed, format!("unknown model {id}")))?;
    let path = models_dir(app_data).join(spec.file_name());
    if path.exists() {
        std::fs::remove_file(&path)
            .map_err(|e| err(DiarizationErrorCode::DownloadFailed, e.to_string()))?;
    }
    Ok(())
}

/// Stream-download one component to `tmp-<file>`, verify sha256, atomic rename.
pub fn download(
    app_data: &Path,
    id: &str,
    mut progress: impl FnMut(u32, u64, u64),
    is_cancelled: impl Fn() -> bool,
) -> Result<(), DiarizationError> {
    let spec = find(id)
        .ok_or_else(|| err(DiarizationErrorCode::DownloadFailed, format!("unknown model {id}")))?;
    let dir = models_dir(app_data);
    std::fs::create_dir_all(&dir)
        .map_err(|e| err(DiarizationErrorCode::DownloadFailed, e.to_string()))?;
    let final_path = dir.join(spec.file_name());
    let tmp_path = dir.join(format!("tmp-{}", spec.file_name()));

    let resp = ureq::get(spec.url)
        .call()
        .map_err(|e| err(DiarizationErrorCode::DownloadFailed, e.to_string()))?;
    let total: u64 = resp
        .header("Content-Length")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    let mut reader = resp.into_reader();
    let mut file = std::fs::File::create(&tmp_path)
        .map_err(|e| err(DiarizationErrorCode::DownloadFailed, e.to_string()))?;
    let mut buf = [0u8; 1 << 16];
    let mut downloaded: u64 = 0;
    loop {
        if is_cancelled() {
            let _ = std::fs::remove_file(&tmp_path);
            return Err(err(DiarizationErrorCode::Cancelled, "download cancelled"));
        }
        let n = reader
            .read(&mut buf)
            .map_err(|e| err(DiarizationErrorCode::DownloadFailed, e.to_string()))?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])
            .map_err(|e| err(DiarizationErrorCode::DownloadFailed, e.to_string()))?;
        downloaded += n as u64;
        let pct = (downloaded * 100).checked_div(total).unwrap_or(0) as u32;
        progress(pct, downloaded, total);
    }
    drop(file);

    verify_sha256(&tmp_path, spec.sha256).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp_path);
    })?;
    std::fs::rename(&tmp_path, &final_path)
        .map_err(|e| err(DiarizationErrorCode::DownloadFailed, e.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    fn hex(bytes: &[u8]) -> String {
        let mut h = Sha256::new();
        h.update(bytes);
        h.finalize().iter().map(|b| format!("{b:02x}")).collect()
    }

    #[test]
    fn catalog_has_two_components_with_complete_metadata() {
        let ids: Vec<&str> = CATALOG.iter().map(|m| m.id).collect();
        assert_eq!(ids, vec!["segmentation", "embedding"]);
        for m in CATALOG {
            assert!(!m.id.is_empty());
            assert!(!m.display_name.is_empty());
            assert_eq!(m.sha256.len(), 64, "sha256 must be 64 hex chars for {}", m.id);
        }
    }

    #[test]
    fn all_present_true_only_when_both_files_exist() {
        let tmp = tempfile::tempdir().unwrap();
        let app = tmp.path();
        std::fs::create_dir_all(models_dir(app)).unwrap();
        assert!(!all_present(app));
        std::fs::write(models_dir(app).join("segmentation.onnx"), b"x").unwrap();
        assert!(!all_present(app)); // only one of two
        std::fs::write(models_dir(app).join("embedding.onnx"), b"y").unwrap();
        assert!(all_present(app));
    }

    #[test]
    fn list_marks_present_files_as_downloaded() {
        let tmp = tempfile::tempdir().unwrap();
        let app = tmp.path();
        std::fs::create_dir_all(models_dir(app)).unwrap();
        std::fs::write(models_dir(app).join("segmentation.onnx"), b"x").unwrap();
        let listed = list(app);
        assert!(listed.iter().find(|m| m.id == "segmentation").unwrap().downloaded);
        assert!(!listed.iter().find(|m| m.id == "embedding").unwrap().downloaded);
    }

    #[test]
    fn verify_sha256_accepts_match_rejects_mismatch() {
        let tmp = tempfile::tempdir().unwrap();
        let f = tmp.path().join("blob.onnx");
        std::fs::write(&f, b"hello").unwrap();
        assert!(verify_sha256(&f, &hex(b"hello")).is_ok());
        assert!(verify_sha256(&f, &"0".repeat(64)).is_err());
    }

    #[test]
    fn delete_removes_a_downloaded_component() {
        let tmp = tempfile::tempdir().unwrap();
        let app = tmp.path();
        std::fs::create_dir_all(models_dir(app)).unwrap();
        let p = models_dir(app).join("segmentation.onnx");
        std::fs::write(&p, b"x").unwrap();
        delete(app, "segmentation").unwrap();
        assert!(!p.exists());
    }
}
```

- [ ] **Step 2: Run the tests**

Run (with env preamble):
```bash
cd src-tauri && cargo test -p smart-noter-diarize models
```
Expected: 5 tests PASS. (`catalog_has_two_components…` asserts `sha256.len()==64`; the catalog already has the real 64-hex values.)

- [ ] **Step 3: Commit**

```bash
git add src-tauri/crates/diarize/src/models.rs
git commit -m "feat(diarize): ONNX model catalog + download/verify/delete + tests"
```

---

# Phase 3 — Diarization Pipeline (sherpa-rs wrapper)

The only module that touches `sherpa-rs`. Uses the API **pinned in Task 0.2** — if the code below diverges from the spike's findings, the spike wins.

### Task 3.1: Implement the diarize wrapper

**Files:**
- Create: `src-tauri/crates/diarize/src/diarize.rs`
- Modify: `src-tauri/crates/diarize/src/lib.rs` (declare + re-export)

- [ ] **Step 1: Add the module declaration**

In `src-tauri/crates/diarize/src/lib.rs`, replace the trailing comment with:

```rust
pub mod diarize;

pub use diarize::{diarize, DiarizeOpts};
```

- [ ] **Step 2: Write the wrapper**

`src-tauri/crates/diarize/src/diarize.rs` (adjust the `sherpa_rs` calls to the spike-pinned API):

```rust
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::align::DiarSegment;
use crate::error::{DiarizationError, DiarizationErrorCode};

/// Knobs for one diarization run.
#[derive(Debug, Clone, Default)]
pub struct DiarizeOpts {
    /// `Some(n)` forces exactly n speakers (the user's hint); `None` auto-detects via clustering.
    pub num_speakers: Option<u32>,
}

fn derr(code: DiarizationErrorCode, m: impl Into<String>) -> DiarizationError {
    DiarizationError { code, message: m.into() }
}

/// Run sherpa-rs diarization over 16 kHz mono f32 PCM. Returns speaker regions
/// in **milliseconds** (sherpa reports seconds; we convert here so `align` is ms-only).
/// `abort` is polled cooperatively; on abort we return `Cancelled`.
pub fn diarize(
    pcm: &[f32],
    seg_model: &Path,
    emb_model: &Path,
    opts: &DiarizeOpts,
    abort: Arc<AtomicBool>,
) -> Result<Vec<DiarSegment>, DiarizationError> {
    if abort.load(Ordering::Relaxed) {
        return Err(derr(DiarizationErrorCode::Cancelled, "cancelled before start"));
    }

    use sherpa_rs::diarize::{Diarize, DiarizeConfig};

    // num_clusters and threshold are mutually exclusive: a hint forces the count;
    // otherwise auto-detect via the clustering threshold.
    let (num_clusters, threshold) = match opts.num_speakers {
        Some(n) => (Some(n as i32), None),
        None => (None, Some(0.5_f32)),
    };
    let config = DiarizeConfig {
        num_clusters,
        threshold,
        min_duration_on: Some(0.3),
        min_duration_off: Some(0.5),
        provider: None,
        debug: false,
    };

    let mut sd = Diarize::new(
        seg_model.to_string_lossy().as_ref(),
        emb_model.to_string_lossy().as_ref(),
        config,
    )
    .map_err(|e| derr(DiarizationErrorCode::ModelLoadFailed, e.to_string()))?;

    // sherpa-rs `compute` takes ownership of the samples; it has no abort hook, so
    // diarization runs to completion once started (we checked `abort` above; the
    // job-level cancel still interrupts the whisper phase). Segments come back
    // sorted by start, in SECONDS — convert to ms for the aligner.
    let raw = sd
        .compute(pcm.to_vec(), None)
        .map_err(|e| derr(DiarizationErrorCode::DiarizationFailed, e.to_string()))?;

    let segments = raw
        .into_iter()
        .map(|s| DiarSegment {
            start_ms: (s.start.max(0.0) * 1000.0) as u32,
            end_ms: (s.end.max(0.0) * 1000.0) as u32,
            speaker: s.speaker.max(0) as u32,
        })
        .collect();
    Ok(segments)
}
```

> NOTE: there is no compile-time unit test for `diarize` (it needs models + native libs); it is exercised by the Phase-0 spike and the Phase-7 smoke. After wiring the real API, confirm it compiles.

- [ ] **Step 3: Verify it compiles**

Run (with env preamble):
```bash
cd src-tauri && cargo check -p smart-noter-diarize
```
Expected: PASS. If the installed `sherpa-rs` 0.6.8 differs from the confirmed API (e.g. `compute` signature), fix to match the spike's working code. Keep `DiarizeOpts`/return type stable — Phase 5 depends on them.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/crates/diarize/src/diarize.rs src-tauri/crates/diarize/src/lib.rs
git commit -m "feat(diarize): sherpa-rs pipeline wrapper (ms output, abort-aware)"
```

---

# Phase 4 — Data Model & Persistence

### Task 4.1: Migration 0003 — add `end_seconds`

**Files:**
- Create: `src-tauri/crates/db/migrations/0003_transcript_end_seconds.sql`
- Test: `src-tauri/crates/db/tests/migration.rs` (append a case)

- [ ] **Step 1: Write the migration**

`src-tauri/crates/db/migrations/0003_transcript_end_seconds.sql`:

```sql
-- Sub-3a stored only the start of each line. Diarization talk_pct is computed by
-- real speech duration, so store the segment end too. Nullable: legacy Sub-3a rows
-- have no end and are treated as zero-duration when recomputing.
ALTER TABLE transcript_lines ADD COLUMN end_seconds INTEGER;
```

- [ ] **Step 2: Write the failing test**

Append to `src-tauri/crates/db/tests/migration.rs`:

```rust
#[tokio::test]
async fn migration_0003_adds_end_seconds_column() {
    let pool = smart_noter_db::init_pool_in_memory().await.unwrap();
    // PRAGMA table_info returns one row per column; assert end_seconds exists.
    let cols: Vec<String> = sqlx::query_scalar("SELECT name FROM pragma_table_info('transcript_lines')")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert!(cols.iter().any(|c| c == "end_seconds"), "got columns: {cols:?}");
}
```

- [ ] **Step 3: Run the test**

Run (with env preamble):
```bash
cd src-tauri && cargo test -p smart-noter-db migration_0003
```
Expected: PASS (migrations run automatically in `init_pool_in_memory`). If FAIL "no such column", the migration file name must sort after `0002` and contain the `ALTER`.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/crates/db/migrations/0003_transcript_end_seconds.sql src-tauri/crates/db/tests/migration.rs
git commit -m "feat(db): migration 0003 add transcript_lines.end_seconds"
```

### Task 4.2: Generalize `replace_lines` to N speakers + duration talk_pct

**Files:**
- Modify: `src-tauri/crates/db/src/repos/transcript_repo.rs`

- [ ] **Step 1: Replace `LineInput` and `replace_lines`**

Replace lines 1–65 of `src-tauri/crates/db/src/repos/transcript_repo.rs` (the `LineInput` struct + `replace_lines` fn, keeping the `use` lines) with:

```rust
use crate::DbError;
use sqlx::SqlitePool;

/// One transcript line to persist. `speaker_idx` is the 0-based detected speaker
/// (Sub-3a passes 0 for all lines → a single S1). `end_seconds` enables talk_pct
/// by real duration.
#[derive(Debug, Clone)]
pub struct LineInput {
    pub t_seconds: i64,
    pub end_seconds: i64,
    pub t_display: String,
    pub text_es: String,
    pub speaker_idx: usize,
}

/// Color class for the nth (0-based) speaker. The Avatar component defines
/// s-color-1..8 and falls back to s1 beyond that; we cycle 1..=8.
fn color_for(idx: usize) -> String {
    format!("s-color-{}", (idx % 8) + 1)
}

/// Replace a meeting's transcript atomically. Creates exactly `speaker_count`
/// participants (S1..Sn), wipes old lines, inserts the new ones with their
/// speaker, and sets per-speaker word_count + talk_pct (by speech duration).
/// Idempotent. `speaker_count` must be >= 1 and cover every `speaker_idx` used.
pub async fn replace_lines(
    pool: &SqlitePool,
    meeting_id: &str,
    lines: &[LineInput],
    speaker_count: usize,
    word_count: i64,
) -> Result<(), DbError> {
    let speaker_count = speaker_count.max(1);
    let mut tx = pool.begin().await?;

    // Wipe lines first (FK: lines reference participants), then participants.
    sqlx::query("DELETE FROM transcript_lines WHERE meeting_id = ?")
        .bind(meeting_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM participants WHERE meeting_id = ?")
        .bind(meeting_id)
        .execute(&mut *tx)
        .await?;

    // Create S1..Sn.
    let speaker_id = |idx: usize| format!("p-{meeting_id}-S{}", idx + 1);
    for idx in 0..speaker_count {
        sqlx::query(
            "INSERT INTO participants (id, meeting_id, label, name, color_class, word_count, talk_pct)
             VALUES (?, ?, ?, NULL, ?, 0, 0)",
        )
        .bind(speaker_id(idx))
        .bind(meeting_id)
        .bind(format!("S{}", idx + 1))
        .bind(color_for(idx))
        .execute(&mut *tx)
        .await?;
    }

    // Insert lines + accumulate per-speaker words & duration.
    let mut words_per = vec![0i64; speaker_count];
    let mut dur_per = vec![0i64; speaker_count];
    for l in lines {
        let idx = l.speaker_idx.min(speaker_count - 1);
        sqlx::query(
            "INSERT INTO transcript_lines (meeting_id, t_seconds, end_seconds, t_display, speaker_id, text_es, text_en)
             VALUES (?, ?, ?, ?, ?, ?, NULL)",
        )
        .bind(meeting_id)
        .bind(l.t_seconds)
        .bind(l.end_seconds)
        .bind(&l.t_display)
        .bind(speaker_id(idx))
        .bind(&l.text_es)
        .execute(&mut *tx)
        .await?;
        words_per[idx] += l.text_es.split_whitespace().count() as i64;
        dur_per[idx] += (l.end_seconds - l.t_seconds).max(0);
    }

    // talk_pct by duration (fallback to word share if total duration is 0).
    let total_dur: i64 = dur_per.iter().sum();
    let total_words: i64 = words_per.iter().sum::<i64>().max(1);
    for idx in 0..speaker_count {
        let pct = if total_dur > 0 {
            ((dur_per[idx] * 100) as f64 / total_dur as f64).round() as i64
        } else {
            (words_per[idx] * 100) / total_words
        };
        sqlx::query("UPDATE participants SET word_count = ?, talk_pct = ? WHERE id = ?")
            .bind(words_per[idx])
            .bind(pct)
            .bind(speaker_id(idx))
            .execute(&mut *tx)
            .await?;
    }

    sqlx::query("UPDATE meetings SET word_count = ? WHERE id = ?")
        .bind(word_count)
        .bind(meeting_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(())
}
```

- [ ] **Step 2: Update the existing test + add a multi-speaker test**

In the same file's `#[cfg(test)] mod tests`, replace the body of `replace_lines_creates_s1_inserts_lines_and_sets_word_count` so the `LineInput`s include the new fields and the call passes `speaker_count`, then add a multi-speaker case. Replace the existing test function with:

```rust
    #[tokio::test]
    async fn replace_lines_single_speaker_creates_s1_and_word_counts() {
        let pool = init_pool_in_memory().await.unwrap();
        sqlx::query("INSERT INTO meetings (id, title_es, template_id, date, duration_sec, word_count) VALUES ('m-1','t','tecnica','2026-06-15',10,0)")
            .execute(&pool).await.unwrap();

        let lines = vec![
            LineInput { t_seconds: 0, end_seconds: 4, t_display: "00:00:00".into(), text_es: "hola equipo".into(), speaker_idx: 0 },
            LineInput { t_seconds: 4, end_seconds: 8, t_display: "00:00:04".into(), text_es: "vamos a empezar".into(), speaker_idx: 0 },
        ];
        replace_lines(&pool, "m-1", &lines, 1, 5).await.unwrap();

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM transcript_lines WHERE meeting_id='m-1'")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(count, 2);
        let speakers: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM participants WHERE meeting_id='m-1'")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(speakers, 1);
        let pct: i64 = sqlx::query_scalar("SELECT talk_pct FROM participants WHERE meeting_id='m-1'")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(pct, 100);

        // Re-running replaces, not appends.
        replace_lines(&pool, "m-1", &lines[..1], 1, 2).await.unwrap();
        let count2: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM transcript_lines WHERE meeting_id='m-1'")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(count2, 1);
    }

    #[tokio::test]
    async fn replace_lines_two_speakers_splits_talk_pct_by_duration() {
        let pool = init_pool_in_memory().await.unwrap();
        sqlx::query("INSERT INTO meetings (id, title_es, template_id, date, duration_sec, word_count) VALUES ('m-2','t','tecnica','2026-06-15',10,0)")
            .execute(&pool).await.unwrap();

        // S1 speaks 9s, S2 speaks 3s → 75% / 25%.
        let lines = vec![
            LineInput { t_seconds: 0, end_seconds: 9, t_display: "00:00:00".into(), text_es: "uno dos tres".into(), speaker_idx: 0 },
            LineInput { t_seconds: 9, end_seconds: 12, t_display: "00:00:09".into(), text_es: "cuatro".into(), speaker_idx: 1 },
        ];
        replace_lines(&pool, "m-2", &lines, 2, 4).await.unwrap();

        let speakers: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM participants WHERE meeting_id='m-2'")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(speakers, 2);
        let s1: i64 = sqlx::query_scalar("SELECT talk_pct FROM participants WHERE id='p-m-2-S1'")
            .fetch_one(&pool).await.unwrap();
        let s2: i64 = sqlx::query_scalar("SELECT talk_pct FROM participants WHERE id='p-m-2-S2'")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(s1, 75);
        assert_eq!(s2, 25);
        let colors: Vec<String> = sqlx::query_scalar("SELECT color_class FROM participants WHERE meeting_id='m-2' ORDER BY label")
            .fetch_all(&pool).await.unwrap();
        assert_eq!(colors, vec!["s-color-1", "s-color-2"]);
    }
```

- [ ] **Step 3: Run the tests**

Run (with env preamble):
```bash
cd src-tauri && cargo test -p smart-noter-db replace_lines
```
Expected: 2 tests PASS.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/crates/db/src/repos/transcript_repo.rs
git commit -m "feat(db): replace_lines generalized to N speakers + duration talk_pct"
```

### Task 4.3: Correction ops — recompute helper, merge, reassign, create

**Files:**
- Modify: `src-tauri/crates/db/src/repos/participants_repo.rs`

- [ ] **Step 1: Add the ops + a shared recompute helper**

Append to `src-tauri/crates/db/src/repos/participants_repo.rs` (before the `#[cfg(test)]` module):

```rust
/// Recompute word_count + talk_pct for every participant of a meeting from the
/// current transcript_lines, inside an existing transaction. talk_pct is by
/// speech duration (end_seconds - t_seconds), falling back to word share when
/// total duration is 0. Participants with no lines get 0/0.
async fn recompute_stats_tx(
    tx: &mut sqlx::SqliteConnection,
    meeting_id: &str,
) -> Result<(), DbError> {
    // (participant_id, words, duration) aggregated from lines.
    let rows = sqlx::query_as::<_, (String, i64, i64)>(
        r#"SELECT p.id,
                  COALESCE(SUM((LENGTH(TRIM(tl.text_es)) - LENGTH(REPLACE(TRIM(tl.text_es), ' ', '')) + 1)
                               * (CASE WHEN TRIM(tl.text_es) = '' THEN 0 ELSE 1 END)), 0) AS words,
                  COALESCE(SUM(MAX(COALESCE(tl.end_seconds, tl.t_seconds) - tl.t_seconds, 0)), 0) AS dur
           FROM participants p
           LEFT JOIN transcript_lines tl ON tl.speaker_id = p.id
           WHERE p.meeting_id = ?
           GROUP BY p.id"#,
    )
    .bind(meeting_id)
    .fetch_all(&mut *tx)
    .await?;

    let total_dur: i64 = rows.iter().map(|(_, _, d)| *d).sum();
    let total_words: i64 = rows.iter().map(|(_, w, _)| *w).sum::<i64>().max(1);
    for (id, words, dur) in &rows {
        let pct = if total_dur > 0 {
            ((dur * 100) as f64 / total_dur as f64).round() as i64
        } else {
            (words * 100) / total_words
        };
        sqlx::query("UPDATE participants SET word_count = ?, talk_pct = ? WHERE id = ?")
            .bind(words)
            .bind(pct)
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }
    Ok(())
}

/// Merge `from` into `into`: reassign all of `from`'s lines to `into`, delete
/// `from`, and recompute stats. No-op-safe if `from == into`.
pub async fn merge_speakers(pool: &SqlitePool, into: &str, from: &str) -> Result<(), DbError> {
    if into == from {
        return Ok(());
    }
    let mut tx = pool.begin().await?;
    sqlx::query("UPDATE transcript_lines SET speaker_id = ? WHERE speaker_id = ?")
        .bind(into)
        .bind(from)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM participants WHERE id = ?")
        .bind(from)
        .execute(&mut *tx)
        .await?;
    // Recompute for the meeting that owns `into`.
    let meeting_id: String =
        sqlx::query_scalar("SELECT meeting_id FROM participants WHERE id = ?")
            .bind(into)
            .fetch_one(&mut *tx)
            .await?;
    recompute_stats_tx(&mut tx, &meeting_id).await?;
    tx.commit().await?;
    Ok(())
}

/// Reassign specific lines to `speaker_id` (existing OR newly created → this is
/// "split"). Recomputes stats for the speaker's meeting.
pub async fn reassign_lines(
    pool: &SqlitePool,
    line_ids: &[i64],
    speaker_id: &str,
) -> Result<(), DbError> {
    if line_ids.is_empty() {
        return Ok(());
    }
    let mut tx = pool.begin().await?;
    for id in line_ids {
        sqlx::query("UPDATE transcript_lines SET speaker_id = ? WHERE id = ?")
            .bind(speaker_id)
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }
    let meeting_id: String =
        sqlx::query_scalar("SELECT meeting_id FROM participants WHERE id = ?")
            .bind(speaker_id)
            .fetch_one(&mut *tx)
            .await?;
    recompute_stats_tx(&mut tx, &meeting_id).await?;
    tx.commit().await?;
    Ok(())
}

/// Create a new speaker for a meeting (label S{next}, next free color). Returns its id.
pub async fn create_speaker(pool: &SqlitePool, meeting_id: &str) -> Result<String, DbError> {
    let mut tx = pool.begin().await?;
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM participants WHERE meeting_id = ?")
            .bind(meeting_id)
            .fetch_one(&mut *tx)
            .await?;
    let idx = count as usize; // 0-based
    let id = format!("p-{meeting_id}-S{}", idx + 1);
    let label = format!("S{}", idx + 1);
    let color = format!("s-color-{}", (idx % 8) + 1);
    sqlx::query(
        "INSERT INTO participants (id, meeting_id, label, name, color_class, word_count, talk_pct)
         VALUES (?, ?, ?, NULL, ?, 0, 0)",
    )
    .bind(&id)
    .bind(meeting_id)
    .bind(&label)
    .bind(&color)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(id)
}
```

> NOTE: `recompute_stats_tx` counts words in SQL with a whitespace-token approximation (single-space separated). For mixed whitespace this can differ slightly from Rust's `split_whitespace`; that's acceptable for a recomputed display stat. If exactness matters, fetch the texts and count in Rust — but keep it in the same transaction.

- [ ] **Step 2: Add tests**

Append inside the existing `#[cfg(test)] mod tests` in the same file:

```rust
    async fn setup_two_speakers() -> SqlitePool {
        let pool = init_pool_in_memory().await.unwrap();
        sqlx::query("INSERT INTO meetings (id, title_es, template_id, date, duration_sec) VALUES ('m','M','tecnica','2025-01-01',100)")
            .execute(&pool).await.unwrap();
        for (id, label, color) in [("p-m-S1","S1","s-color-1"), ("p-m-S2","S2","s-color-2")] {
            sqlx::query("INSERT INTO participants (id, meeting_id, label, color_class) VALUES (?, 'm', ?, ?)")
                .bind(id).bind(label).bind(color).execute(&pool).await.unwrap();
        }
        // S1: 0..6 (6s, 2 words); S2: 6..10 (4s, 1 word)
        sqlx::query("INSERT INTO transcript_lines (id, meeting_id, t_seconds, end_seconds, t_display, speaker_id, text_es) VALUES (1,'m',0,6,'00:00:00','p-m-S1','hola mundo')")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO transcript_lines (id, meeting_id, t_seconds, end_seconds, t_display, speaker_id, text_es) VALUES (2,'m',6,10,'00:00:06','p-m-S2','adios')")
            .execute(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn merge_reassigns_lines_deletes_from_and_recomputes() {
        let pool = setup_two_speakers().await;
        merge_speakers(&pool, "p-m-S1", "p-m-S2").await.unwrap();
        let speakers: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM participants WHERE meeting_id='m'")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(speakers, 1);
        let s1_lines: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM transcript_lines WHERE speaker_id='p-m-S1'")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(s1_lines, 2);
        let pct: i64 = sqlx::query_scalar("SELECT talk_pct FROM participants WHERE id='p-m-S1'")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(pct, 100);
    }

    #[tokio::test]
    async fn reassign_moves_line_and_recomputes_duration_pct() {
        let pool = setup_two_speakers().await;
        // Move line 1 (the 6s line) to S2 → S2 has 10s of 10s = 100%, S1 = 0%.
        reassign_lines(&pool, &[1], "p-m-S2").await.unwrap();
        let s2: i64 = sqlx::query_scalar("SELECT talk_pct FROM participants WHERE id='p-m-S2'")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(s2, 100);
        let s1: i64 = sqlx::query_scalar("SELECT talk_pct FROM participants WHERE id='p-m-S1'")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(s1, 0);
    }

    #[tokio::test]
    async fn create_speaker_adds_next_label_and_color() {
        let pool = setup_two_speakers().await;
        let id = create_speaker(&pool, "m").await.unwrap();
        assert_eq!(id, "p-m-S3");
        let (label, color): (String, String) =
            sqlx::query_as("SELECT label, color_class FROM participants WHERE id='p-m-S3'")
                .fetch_one(&pool).await.unwrap();
        assert_eq!(label, "S3");
        assert_eq!(color, "s-color-3");
    }
```

- [ ] **Step 3: Run the tests**

Run (with env preamble):
```bash
cd src-tauri && cargo test -p smart-noter-db -- merge_ reassign create_speaker
```
Expected: 3 tests PASS.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/crates/db/src/repos/participants_repo.rs
git commit -m "feat(db): merge/reassign/create speaker ops + duration recompute"
```

---

# Phase 5 — Backend Commands & Job Integration

### Task 5.1: Settings — `identify_speakers` + `diarization_model`

**Files:**
- Modify: `src-tauri/crates/core/src/models/settings.rs`

- [ ] **Step 1: Add the fields + defaults**

In `AppSettings` (after `pub auto_transcribe: bool,`), add:

```rust
    pub identify_speakers: bool,
    pub diarization_model: String,
```

In `impl Default for AppSettings` (after `auto_transcribe: true,`), add:

```rust
            identify_speakers: true,
            diarization_model: "default".into(),
```

- [ ] **Step 2: Add a defaults test**

Append to the `#[cfg(test)] mod tests` in the same file:

```rust
    #[test]
    fn defaults_enable_speaker_identification() {
        let d = AppSettings::default();
        assert!(d.identify_speakers);
        assert_eq!(d.diarization_model, "default");
    }
```

> NOTE: `settings_repo::get` deserializes with `serde_json::from_str(...).unwrap_or_default()`, and persisted blobs predating these fields will be missing them. To avoid silently resetting the whole settings object to defaults when one field is absent, add `#[serde(default)]` to the two new fields so old blobs deserialize with these defaults instead of failing. Add above each new field:
> ```rust
>     #[serde(default = "default_true")]
>     pub identify_speakers: bool,
>     #[serde(default = "default_diar_model")]
>     pub diarization_model: String,
> ```
> and define near the bottom of the file:
> ```rust
> fn default_true() -> bool { true }
> fn default_diar_model() -> String { "default".into() }
> ```

- [ ] **Step 3: Run the test**

Run (with env preamble):
```bash
cd src-tauri && cargo test -p smart-noter-core defaults_enable_speaker
```
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/crates/core/src/models/settings.rs
git commit -m "feat(core): add identify_speakers + diarization_model settings"
```

### Task 5.2: Diarization-model commands + events

**Files:**
- Modify: `src-tauri/src/commands/transcription.rs`
- Modify: `src-tauri/src/lib.rs` (register)

- [ ] **Step 1: Add list/download/delete commands**

Append to `src-tauri/src/commands/transcription.rs` (after `delete_whisper_model`):

```rust
use smart_noter_diarize::models as diar_models;

#[derive(Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DiarizationModelInfo {
    pub id: String,
    pub name: String,
    pub size_mb: u32,
    pub downloaded: bool,
}

#[tauri::command]
#[specta::specta]
pub fn list_diarization_models(app: tauri::AppHandle) -> Result<Vec<DiarizationModelInfo>, AppError> {
    let dir = app_data(&app)?;
    Ok(diar_models::list(&dir)
        .into_iter()
        .map(|m| DiarizationModelInfo {
            id: m.id.to_string(),
            name: m.display_name.to_string(),
            size_mb: m.size_mb,
            downloaded: m.downloaded,
        })
        .collect())
}

#[tauri::command]
#[specta::specta]
pub fn download_diarization_model(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
    id: String,
) -> Result<(), AppError> {
    let handle = DownloadHandle {
        id: id.clone(),
        abort: Arc::new(AtomicBool::new(false)),
    };
    {
        let mut slot = state.download.lock();
        if slot.is_some() {
            return Err(terr(TranscriptionErrorCode::DownloadBusy, "a download is already running"));
        }
        *slot = Some(handle.clone());
    }
    let dir = app_data(&app)?;
    let slot = state.download.clone();
    let abort = handle.abort.clone();
    let app2 = app.clone();
    let id2 = id.clone();
    std::thread::spawn(move || {
        let app3 = app2.clone();
        let id3 = id2.clone();
        let progress = move |pct: u32, dl: u64, total: u64| {
            let _ = app3.emit(
                "diarization-download:progress",
                DownloadProgressEvent { id: id3.clone(), pct, bytes_downloaded: dl, bytes_total: total },
            );
        };
        let is_cancelled = {
            let a = abort.clone();
            move || a.load(Ordering::Relaxed)
        };
        match diar_models::download(&dir, &id2, progress, is_cancelled) {
            Ok(()) => {
                let _ = app2.emit("diarization-download:completed", DownloadDoneEvent { id: id2.clone() });
            }
            Err(e) => {
                let _ = app2.emit(
                    "diarization-download:failed",
                    DownloadFailEvent { id: id2.clone(), code: format!("{:?}", e.code), message: e.message },
                );
            }
        }
        *slot.lock() = None;
    });
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn delete_diarization_model(app: tauri::AppHandle, id: String) -> Result<(), AppError> {
    let dir = app_data(&app)?;
    diar_models::delete(&dir, &id).map_err(AppError::from)
}
```

> NOTE: this reuses the existing `DownloadProgressEvent`/`DownloadDoneEvent`/`DownloadFailEvent` structs (already defined in this file) and the single `state.download` slot — so a whisper download and a diarization download can't run simultaneously, which is fine (both are user-initiated, one at a time).

- [ ] **Step 2: Register the commands**

In `src-tauri/src/lib.rs` `collect_commands!`, after `commands::transcription::delete_whisper_model,` add:

```rust
        commands::transcription::list_diarization_models,
        commands::transcription::download_diarization_model,
        commands::transcription::delete_diarization_model,
```

- [ ] **Step 3: Verify it compiles**

Run (with env preamble):
```bash
cd src-tauri && cargo check -p smart-noter
```
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands/transcription.rs src-tauri/src/lib.rs
git commit -m "feat(commands): diarization model list/download/delete + events"
```

### Task 5.3: Integrate diarization into the transcription job

**Files:**
- Modify: `src-tauri/src/commands/transcription.rs`

- [ ] **Step 1: Add the `speaker_count_hint` param + read the setting + load models**

In `transcribe_meeting`, change the signature to accept the hint:

```rust
pub async fn transcribe_meeting(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
    meeting_id: String,
    speaker_count_hint: Option<u32>,
) -> Result<(), AppError> {
```

After the existing `model_path` resolution block (the whisper model), add resolution of the diarization toggle + model paths (these are plain values moved into the thread):

```rust
    // Diarization: ON unless the user turned it off; needs BOTH ONNX models present.
    let diarize_on = settings.identify_speakers;
    let diar_seg = diar_models::model_path(&app_dir, "segmentation");
    let diar_emb = diar_models::model_path(&app_dir, "embedding");
    let diar_models_ready = diarize_on
        && diar_seg.as_ref().map(|p| p.is_file()).unwrap_or(false)
        && diar_emb.as_ref().map(|p| p.is_file()).unwrap_or(false);
```

- [ ] **Step 2: Branch the job body**

Inside the spawned thread, after `pcm` is decoded and after `transcribe(...)` produces `segments`, replace the "Map segments -> lines" block (lines ~213–225 in the current file) with diarization-aware mapping. Replace from `// Map segments -> lines + word_count.` through the construction of `lines`/`words` with:

```rust
        // Decide speakers. When diarization is requested AND its models are
        // present, diarize + align; otherwise fall back to single-speaker (S1).
        let mut speaker_count = 1usize;
        let mut speaker_idx: Vec<usize> = vec![0; segments.len()];

        if diar_models_ready {
            let seg_model = diar_seg.clone().unwrap();
            let emb_model = diar_emb.clone().unwrap();
            let opts = smart_noter_diarize::DiarizeOpts { num_speakers: speaker_count_hint };
            match smart_noter_diarize::diarize(&pcm, &seg_model, &emb_model, &opts, abort.clone()) {
                Ok(diar_segs) => {
                    // Map whisper segments -> diarize::TextSegment (ms), align, read back speakers.
                    let texts: Vec<smart_noter_diarize::align::TextSegment> = segments
                        .iter()
                        .map(|s| smart_noter_diarize::align::TextSegment {
                            start_ms: s.start_ms,
                            end_ms: s.end_ms,
                            text: s.text.clone(),
                        })
                        .collect();
                    let aligned = smart_noter_diarize::align(&texts, &diar_segs);
                    let max_spk = aligned.iter().map(|a| a.speaker).max().unwrap_or(0);
                    speaker_count = (max_spk as usize) + 1;
                    speaker_idx = aligned.iter().map(|a| a.speaker as usize).collect();
                }
                Err(e) if e.code == smart_noter_diarize::DiarizationErrorCode::Cancelled => {
                    let _ = app2.emit("transcription:cancelled", CancelledEvent { meeting_id: mid.clone() });
                    finish(&slot);
                    return;
                }
                Err(e) => {
                    // Degrade to Sub-3a (single S1) but tell the UI so it can toast.
                    let _ = app2.emit(
                        "diarization:degraded",
                        FailedEvent {
                            meeting_id: mid.clone(),
                            code: format!("{:?}", e.code),
                            message: e.message,
                        },
                    );
                    // speaker_count stays 1, speaker_idx stays all-zero.
                }
            }
        }

        // Map segments -> lines + word_count (now speaker-aware).
        let mut lines = Vec::with_capacity(segments.len());
        let mut words = 0u32;
        for (i, s) in segments.iter().enumerate() {
            let t_seconds = (s.start_ms / 1000) as i64;
            let end_seconds = (s.end_ms / 1000) as i64;
            let t_display = smart_noter_whisper::transcribe::fmt_timestamp(t_seconds as u32);
            words += smart_noter_whisper::transcribe::word_count(&s.text);
            lines.push(LineInput {
                t_seconds,
                end_seconds,
                t_display,
                text_es: s.text.clone(),
                speaker_idx: speaker_idx[i],
            });
        }
```

Then update the persistence call to pass `speaker_count`:

```rust
        let persisted = tauri::async_runtime::block_on(replace_lines(
            &pool, &mid, &lines, speaker_count, words as i64,
        ));
```

- [ ] **Step 3: Handle the toggle-on-but-models-missing case (toast, no fail)**

Immediately after `pcm` is decoded successfully (before transcription), if the user wants diarization but the models are missing, emit the degraded event once so the UI can prompt/toast — but still proceed with transcription:

```rust
        if diarize_on && !diar_models_ready {
            let _ = app2.emit(
                "diarization:degraded",
                FailedEvent {
                    meeting_id: mid.clone(),
                    code: "ModelNotDownloaded".into(),
                    message: "diarization models not downloaded".into(),
                },
            );
        }
```

- [ ] **Step 4: Verify it compiles**

Run (with env preamble):
```bash
cd src-tauri && cargo check -p smart-noter
```
Expected: PASS. (`LineInput` now requires `end_seconds` + `speaker_idx`; the loop above supplies both.)

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/transcription.rs
git commit -m "feat(transcription): diarize+align in job, degrade to S1 on failure"
```

### Task 5.4: Correction commands (merge / reassign / create)

**Files:**
- Modify: `src-tauri/src/commands/meetings.rs`
- Modify: `src-tauri/src/lib.rs` (register)

- [ ] **Step 1: Add the commands**

Append to `src-tauri/src/commands/meetings.rs`:

```rust
#[tauri::command]
#[specta::specta]
pub async fn merge_speakers(
    state: State<'_, AppState>,
    into: String,
    from: String,
) -> Result<(), AppError> {
    participants_repo::merge_speakers(&state.pool, &into, &from)
        .await
        .map_err(from_db)
}

#[tauri::command]
#[specta::specta]
pub async fn reassign_lines(
    state: State<'_, AppState>,
    line_ids: Vec<i64>,
    speaker_id: String,
) -> Result<(), AppError> {
    participants_repo::reassign_lines(&state.pool, &line_ids, &speaker_id)
        .await
        .map_err(from_db)
}

#[tauri::command]
#[specta::specta]
pub async fn create_speaker(
    state: State<'_, AppState>,
    meeting_id: String,
) -> Result<String, AppError> {
    participants_repo::create_speaker(&state.pool, &meeting_id)
        .await
        .map_err(from_db)
}
```

- [ ] **Step 2: Register them**

In `src-tauri/src/lib.rs` `collect_commands!`, after `commands::meetings::rename_participant,` add:

```rust
        commands::meetings::merge_speakers,
        commands::meetings::reassign_lines,
        commands::meetings::create_speaker,
```

- [ ] **Step 3: Expose `transcript_lines.id` to the frontend (needed for reassign)**

`reassign_lines` takes line ids, but `TranscriptLine` (in `core/src/models/meeting.rs`) has no `id`. Add it so the FE can pass ids. In `core/src/models/meeting.rs`, add to `TranscriptLine`:

```rust
    pub id: i64,
```

(place it first, before `t`). Then in `db/src/repos/meetings_repo.rs` `get_detail`, update the transcript query + mapper:

```rust
    let transcript = sqlx::query!(
        "SELECT id, t_display, speaker_id, text_es, text_en FROM transcript_lines WHERE meeting_id = ? ORDER BY t_seconds",
        id
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|r| TranscriptLine {
        id: r.id,
        t: r.t_display,
        speaker_id: r.speaker_id.unwrap_or_default(),
        text: Bilingual { es: r.text_es, en: r.text_en },
    })
    .collect();
```

> NOTE: this `query!` is the checked macro. Adding `id` to the SELECT changes its shape, so this is the ONE place that needs the `.sqlx` cache regenerated. After this edit run: `cd src-tauri && DATABASE_URL="sqlite://./crates/db/sn_prepare.db" cargo sqlx prepare --workspace -- --workspace --tests` (install sqlx-cli first if missing: `cargo install sqlx-cli --no-default-features --features sqlite,rustls`). Commit the regenerated `src-tauri/.sqlx/*.json`. Alternatively, convert this single query to the unchecked `sqlx::query(...)` + manual row mapping to avoid the prepare step — match the team's preference.

- [ ] **Step 4: Verify it compiles + tests pass**

Run (with env preamble):
```bash
cd src-tauri && cargo check -p smart-noter && cargo test -p smart-noter-db
```
Expected: PASS. (`SELECT id` requires `transcript_lines.id` — it exists, `INTEGER PRIMARY KEY AUTOINCREMENT`.)

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/meetings.rs src-tauri/src/lib.rs src-tauri/crates/core/src/models/meeting.rs src-tauri/crates/db/src/repos/meetings_repo.rs src-tauri/.sqlx
git commit -m "feat(commands): merge/reassign/create speaker + expose line ids"
```

### Task 5.5: Regenerate IPC bindings

**Files:**
- Modify (generated): `src/ipc/bindings.ts`

- [ ] **Step 1: Regenerate**

Run (with env preamble):
```bash
pnpm generate:bindings
```
Expected: `src/ipc/bindings.ts` updates with `mergeSpeakers`, `reassignLines`, `createSpeaker`, the diarization-model commands, `transcribeMeeting`'s new `speakerCountHint` arg, `TranscriptLine.id`, `Participant` unchanged, and the new `AppSettings` fields (`identifySpeakers`, `diarizationModel`).

- [ ] **Step 2: Verify TS still type-checks**

Run:
```bash
pnpm check:node-types && pnpm build
```
Expected: `tsc` PASS (the FE tasks in Phase 6 consume the new types; if `build` fails only on not-yet-written FE usage, that's expected until Phase 6 — but the bindings file itself must be valid).

- [ ] **Step 3: Commit**

```bash
git add src/ipc/bindings.ts
git commit -m "chore(ipc): regenerate bindings for diarization + correction commands"
```

---

# Phase 6 — Frontend

### Task 6.1: i18n keys for diarization

**Files:**
- Modify: `src/i18n/locales/es.json`, `src/i18n/locales/en.json`
- Modify (generated): `src/i18n/keys.ts`

- [ ] **Step 1: Add the keys**

Add to `src/i18n/locales/es.json`:

```json
  "diarize.modelSection": "Modelos de diarización",
  "diarize.download": "Descargar",
  "diarize.delete": "Eliminar",
  "diarize.degraded": "No se pudieron identificar hablantes; se guardó como un solo hablante.",
  "diarize.modelsMissing": "Descarga los modelos de diarización para identificar hablantes.",
  "diarize.expectedСount": "Número de hablantes (opcional)",
  "speaker.reassign": "Reasignar a…",
  "speaker.newSpeaker": "➕ Nuevo hablante",
  "speaker.merge": "Fusionar en…",
  "speaker.selectLines": "Seleccionar líneas",
  "speaker.applyReassign": "Reasignar selección"
```

and the English equivalents to `src/i18n/locales/en.json`:

```json
  "diarize.modelSection": "Diarization models",
  "diarize.download": "Download",
  "diarize.delete": "Delete",
  "diarize.degraded": "Couldn't identify speakers; saved as a single speaker.",
  "diarize.modelsMissing": "Download the diarization models to identify speakers.",
  "diarize.expectedCount": "Number of speakers (optional)",
  "speaker.reassign": "Reassign to…",
  "speaker.newSpeaker": "➕ New speaker",
  "speaker.merge": "Merge into…",
  "speaker.selectLines": "Select lines",
  "speaker.applyReassign": "Reassign selection"
```

> NOTE: fix the typo'd key `diarize.expectedСount` (it contains a Cyrillic С in this snippet) → use `diarize.expectedCount` consistently in BOTH files and in code. The generator unions keys from `es.json`, so the es file must contain the exact ASCII key `diarize.expectedCount`.

- [ ] **Step 2: Regenerate the key type**

Run:
```bash
pnpm generate:i18n-keys
```
Expected: `src/i18n/keys.ts` gains the new keys in the `TKey` union.

- [ ] **Step 3: Commit**

```bash
git add src/i18n/locales/es.json src/i18n/locales/en.json src/i18n/keys.ts
git commit -m "feat(i18n): diarization + speaker-correction keys"
```

### Task 6.2: Persist the "Identify speakers" toggle + speaker-count hint in pre-record

**Files:**
- Modify: `src/features/pre-record/PreRecordPage.tsx`

- [ ] **Step 1: Bind the toggle to settings + add the count field**

In `PreRecordPage.tsx`, replace the local `autoId` state with the persisted setting. Load settings (the app already has `getSettings`/`updateSettings`; use the same hook the SettingsPage uses — confirm the exact hook name in `src/store/api`). Wire the existing `SettingRow` for `autoIdSpeakers` to `settings.identifySpeakers` and call `updateSettings({ ...settings, identifySpeakers: v })` on change. Add an optional numeric input below it, shown only when the toggle is on:

```tsx
{autoId && (
  <label className={styles.hintRow}>
    {t('diarize.expectedCount')}
    <input
      type="number"
      min={1}
      max={8}
      value={speakerHint ?? ''}
      onChange={(e) => setSpeakerHint(e.target.value === '' ? null : Number(e.target.value))}
      placeholder="auto"
    />
  </label>
)}
```

with local state `const [speakerHint, setSpeakerHint] = useState<number | null>(null);`.

- [ ] **Step 2: Carry the hint into nav state**

Where the page navigates to `LiveRecordingPage` (the `navState` object, currently `{ name, templateId, deviceId, captureMode, format }`), add `speakerHint`:

```tsx
    speakerHint,
```

so it flows: PreRecord → LiveRecording → MeetingDetail (via the existing `justRecorded` nav chain). At the MeetingDetail end, read `location.state?.speakerHint` and pass it into `start(hint)` (Task 6.3).

> NOTE: trace the existing nav chain that carries `justRecorded` (the flag the TranscriptTab auto-start already reads). Add `speakerHint` alongside it at each hop. If the chain drops state at any hop, persist the hint in settings instead as a fallback.

- [ ] **Step 3: Verify**

Run:
```bash
pnpm test:run -- PreRecord
```
Expected: existing PreRecord tests PASS (update any snapshot/assertion that referenced the old local `autoId` default).

- [ ] **Step 4: Commit**

```bash
git add src/features/pre-record/PreRecordPage.tsx
git commit -m "feat(pre-record): persist identify-speakers toggle + speaker-count hint"
```

### Task 6.3: useTranscription — `start(hint)` + degraded toast

**Files:**
- Modify: `src/features/meeting-detail/useTranscription.ts`

- [ ] **Step 1: Thread the hint into the command**

Change `start` to accept an optional hint and pass it to the command:

```ts
const start = useCallback(async (speakerCountHint?: number | null) => {
  await invoke('transcribe_meeting', {
    meetingId,
    speakerCountHint: speakerCountHint ?? null,
  });
}, [meetingId]);
```

- [ ] **Step 2: Listen for the degraded event + toast**

Add a listener (next to the existing `transcription:*` listeners) for `diarization:degraded`:

```ts
const unlistenDegraded = await listen<{ meetingId: string; code: string }>(
  'diarization:degraded',
  (e) => {
    if (e.payload.meetingId !== meetingId) return;
    toast(e.payload.code === 'ModelNotDownloaded' ? t('diarize.modelsMissing') : t('diarize.degraded'));
  },
);
```

and add `unlistenDegraded()` to the cleanup. (Use the same `toast` import — `sonner` — and `useT` the rest of the hook already has, or pass `t` in.)

- [ ] **Step 3: Pass the hint from the auto-start + manual button**

In `TranscriptTab.tsx`, the auto-start effect calls `start()`. Change it to `start(location.state?.speakerHint ?? null)` (read the hint carried in Task 6.2). The manual "Transcribe" button stays `start()` (auto-detect).

- [ ] **Step 4: Verify**

Run:
```bash
pnpm test:run -- useTranscription
```
Expected: PASS (add a test asserting `transcribe_meeting` is invoked with `speakerCountHint`).

- [ ] **Step 5: Commit**

```bash
git add src/features/meeting-detail/useTranscription.ts src/features/meeting-detail/tabs/TranscriptTab.tsx
git commit -m "feat(transcription-ui): pass speaker-count hint + degraded toast"
```

### Task 6.4: RTK mutations for correction

**Files:**
- Modify: `src/store/api/meetings.api.ts`

- [ ] **Step 1: Add mutations**

Next to `renameParticipant` add:

```ts
mergeSpeakers: b.mutation<void, { into: string; from: string }>({
  query: (args) => ({ cmd: 'merge_speakers', args }),
  invalidatesTags: ['Meeting'],
}),
reassignLines: b.mutation<void, { lineIds: number[]; speakerId: string }>({
  query: (args) => ({ cmd: 'reassign_lines', args }),
  invalidatesTags: ['Meeting'],
}),
createSpeaker: b.mutation<string, { meetingId: string }>({
  query: (args) => ({ cmd: 'create_speaker', args }),
  invalidatesTags: ['Meeting'],
}),
```

and export the generated hooks (`useMergeSpeakersMutation`, `useReassignLinesMutation`, `useCreateSpeakerMutation`).

> NOTE: confirm arg casing — the tauri base query forwards `args` to `invoke(cmd, args)`, and Tauri expects camelCase keys matching the Rust params (`into`, `from`, `lineIds`→`line_ids`? — tauri-specta maps camelCase `lineIds` to the Rust `line_ids` automatically; verify against the regenerated bindings' command signatures and use whatever casing they expose).

- [ ] **Step 2: Verify**

Run:
```bash
pnpm check:node-types
```
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add src/store/api/meetings.api.ts
git commit -m "feat(store): RTK mutations for merge/reassign/create speaker"
```

### Task 6.5: TranscriptTab — reassign / split UI

**Files:**
- Modify: `src/features/meeting-detail/tabs/TranscriptTab.tsx`
- Modify: `src/features/meeting-detail/tabs/TranscriptTab.module.css` (add styles)

- [ ] **Step 1: Make the speaker chip a reassign menu + add select-lines mode**

Add a `selectMode` toggle and a `selected: Set<number>` of line ids. When `selectMode` is off, clicking a line's speaker label opens a small menu listing all `meeting.participants` (each calls `reassignLines({ lineIds: [line.id], speakerId: p.id })`) plus a **"➕ New speaker"** item that calls `createSpeaker({ meetingId })` then `reassignLines` with the returned id. When `selectMode` is on, lines show checkboxes; a toolbar shows `t('speaker.applyReassign')` which opens the same speaker menu and reassigns ALL `selected` at once.

Concrete menu handler:

```tsx
const [merge/* unused here */] = [] as const;
const [reassignLines] = useReassignLinesMutation();
const [createSpeaker] = useCreateSpeakerMutation();

async function reassignTo(speakerId: string, lineIds: number[]) {
  await reassignLines({ lineIds, speakerId });
}
async function reassignToNew(lineIds: number[]) {
  const newId = await createSpeaker({ meetingId: meeting.id }).unwrap();
  await reassignLines({ lineIds, speakerId: newId });
}
```

Each transcript line (currently keyed `${l.t}-${l.speakerId}`) must now key on `l.id` and, in select mode, render a checkbox bound to `selected`.

- [ ] **Step 2: Verify**

Run:
```bash
pnpm test:run -- TranscriptTab
```
Expected: PASS (update the test that renders transcript lines to include `id` on the fake `TranscriptLine`s; assert the reassign menu appears on click and the select-mode checkboxes toggle).

- [ ] **Step 3: Commit**

```bash
git add src/features/meeting-detail/tabs/TranscriptTab.tsx src/features/meeting-detail/tabs/TranscriptTab.module.css
git commit -m "feat(transcript-ui): reassign + split (select-lines) correction"
```

### Task 6.6: SidePanel — merge menu

**Files:**
- Modify: `src/features/meeting-detail/side/SidePanel.tsx`

- [ ] **Step 1: Add a per-speaker "···" → "Merge into…" menu**

For each participant row, add a "···" button that opens a menu listing the OTHER participants; selecting one calls `mergeSpeakers({ into: otherId, from: thisId })`. Keep the existing rename-on-click behavior; the "···" must `stopPropagation` so it doesn't trigger rename.

```tsx
const [mergeSpeakers] = useMergeSpeakersMutation();
// in the row, alongside talk_pct:
<button className={styles.menuBtn} onClick={(e) => { e.stopPropagation(); setMenuFor(p.id); }}>···</button>
{menuFor === p.id && (
  <div className={styles.menu}>
    {participants.filter((o) => o.id !== p.id).map((o) => (
      <button key={o.id} onClick={() => { void mergeSpeakers({ into: o.id, from: p.id }); setMenuFor(null); }}>
        {t('speaker.merge')} {fallbackName(o, lang)}
      </button>
    ))}
  </div>
)}
```

with `const [menuFor, setMenuFor] = useState<string | null>(null);`.

- [ ] **Step 2: Verify**

Run:
```bash
pnpm test:run -- SidePanel
```
Expected: PASS (add a test: clicking "···" shows other speakers; selecting one invokes the merge mutation).

- [ ] **Step 3: Commit**

```bash
git add src/features/meeting-detail/side/SidePanel.tsx
git commit -m "feat(side-panel): merge-speakers menu"
```

### Task 6.7: Diarization model panel in Settings

**Files:**
- Modify: `src/features/settings/TranscriptionPanel.tsx` (or create `DiarizationPanel.tsx` rendered nearby)

- [ ] **Step 1: Mirror the Whisper model UI for diarization**

Add a section that calls `invoke<DiarizationModelInfo[]>('list_diarization_models')`, listens to `diarization-download:progress|completed|failed`, and offers Download/Delete per component (`download_diarization_model` / `delete_diarization_model`). This is structurally identical to the existing Whisper model section — copy its shape, swapping command + event names and the i18n keys (`diarize.modelSection`, `diarize.download`, `diarize.delete`).

- [ ] **Step 2: Verify**

Run:
```bash
pnpm test:run -- TranscriptionPanel
```
Expected: PASS (extend the existing panel test or add one mocking `list_diarization_models`).

- [ ] **Step 3: Commit**

```bash
git add src/features/settings/
git commit -m "feat(settings): diarization model manage panel"
```

### Task 6.8: Full frontend gate

- [ ] **Step 1: Run the full FE suite + lint + build**

```bash
pnpm test:run && pnpm lint && pnpm build
```
Expected: all green. Fix any fallout (snapshots, missing i18n keys flagged by `check:hardcoded-strings`).

- [ ] **Step 2: Commit any fixes**

```bash
git add -A
git commit -m "test(fe): green suite for diarization UI"
```

---

# Phase 7 — Smoke & Release

### Task 7.1: Manual smoke (real diarization, two voices)

- [ ] **Step 1: Build + run the app**

```bash
pnpm tauri:dev
```
(with env preamble for the cargo build underneath).

- [ ] **Step 2: Download the diarization models** via Settings → Diarization models (both components → progress → sha256 → on disk).

- [ ] **Step 3: Produce a 2-speaker recording.** Play a clip with two distinct es-MX TTS voices (e.g. Sabina + Raúl) through the captured device, OR feed a prepared 2-voice `.wav`. Keep it ≥30s so it crosses whisper's encode windows.

- [ ] **Step 4: Verify the happy path:** transcription completes → transcript renders split into **S1/S2** with two colors → the side panel shows two speakers with plausible talk_pct summing ~100%.

- [ ] **Step 5: Exercise correction:** rename S1; reassign a single line to S2; enter select-lines mode and reassign several; create a new speaker via "➕ New speaker" (split); merge S2 into S1. After each, the meeting refetches and talk_pct/word_count update.

- [ ] **Step 6: Verify graceful degrade:** delete one diarization model, record again → transcript still appears as a single S1 + the degraded toast fires (no lost transcript, no red error).

- [ ] **Step 7: Verify cancel** mid-job still emits `transcription:cancelled` and frees the slot.

- [ ] **Step 8:** Fix any bugs TDD-style (write the failing test first), re-verify in the running app, commit each fix.

### Task 7.2: Release

- [ ] **Step 1: Update CHANGELOG + bump version** (0.3.0 → 0.4.0) following the repo's existing CHANGELOG style. Commit.

- [ ] **Step 2: Mark the spec implemented** — set `Status: Implemented` in `docs/superpowers/specs/2026-06-17-sub3b-speaker-diarization-design.md`. Commit.

- [ ] **Step 3: Backend + frontend gates green:**

```bash
cd src-tauri && cargo test --workspace
# back to repo root:
pnpm test:run && pnpm build
```

- [ ] **Step 4: Tag + (with user confirmation) push.** Use the user's existing remote (`github.com/sorcia25/Smart-Noter`). Tag `v0.4.0-sub3b-diarization`. NOTE: pushing triggers CI; the `build`/`e2e` frontend jobs are pre-existing red (GitHub issue #1) and the whisper/diarize integration gates are soft — confirm with the user before pushing, per their CI-debt stance.

- [ ] **Step 5: Update memory** — set `project_sub3b_diarization_state` to SHIPPED with the tag, smoke findings, any spike deviations (especially if the pyannote-rs fallback was used), and the new `diarization_model`/`identify_speakers` settings.

---

## Fallback (Plan B): pyannote-rs (pure-Rust Burn)

If Task 0.2 shows `sherpa-rs` static linking is unworkable on Windows:
- Replace the `sherpa-rs` dependency in `crates/diarize/Cargo.toml` with `pyannote-rs` (pure-Rust Burn backend — no ONNX Runtime, no native link step).
- Rewrite ONLY `src/diarize.rs` to its API; `models.rs` may need different model files (update the catalog URLs/sha256).
- **Everything else is unchanged:** `align.rs`, the data model (migration 0003, `replace_lines`, correction ops), all commands, and the entire frontend are engine-agnostic.
- Note the lower clustering accuracy in the smoke + memory.
