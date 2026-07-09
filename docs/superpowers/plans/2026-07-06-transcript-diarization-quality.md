# Transcript & Diarization Quality Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stop whisper dropping the last 3–12s of transcripts, and let users force the diarization speaker count (proactively at save + correctively via a fast re-diarize that keeps the transcript text).

**Architecture:** A = two small whisper-param changes (relax the no-speech gate + force native language on the local path). B = a save-time participant-count input feeding the already-wired `speakerHint`, plus a new `rediarize_meeting` command that re-decodes the WAV, re-runs ONLY diarization+align with a forced count, and reuses `transcript_repo::replace_lines` to atomically re-persist speakers — surfaced by a "Re-asignar hablantes" control in the transcript tab.

**Tech Stack:** Rust (whisper-rs/whisper.cpp, sherpa-rs diarize, sqlx/SQLite) · React/TS · Tauri 2.

---

## Conventions

**Env preamble — prefix EVERY `cargo`/`git` command** (pre-commit hook builds whisper; run cargo from `src-tauri/`):
```bash
export PATH="$HOME/.cargo/bin:/c/Program Files (x86)/Microsoft Visual Studio/2022/BuildTools/Common7/IDE/CommonExtensions/Microsoft/CMake/CMake/bin:$PATH"; export LIBCLANG_PATH="C:/Program Files/LLVM/bin"
```
- Branch: `feat/transcript-diarization-quality` (already created; spec committed at `241a26c`).
- Pre-commit hook: `cargo clippy --workspace --all-targets -- -D warnings` + fmt + biome. Zero warnings. Frontend: pnpm.
- **Diarization + whisper runtime behavior is smoke-validated (needs models + audio), NOT unit-tested.** Unit tests cover the pure logic (opts, line-rebuild, DB reset) and frontend wiring.

## File Structure

| File | Role | Change |
|---|---|---|
| `src-tauri/crates/whisper/src/transcribe.rs` | whisper runner | Modify — add `no_speech_thold` to `TranscribeOpts`, set it, add verify log |
| `src-tauri/src/commands/transcription.rs` | transcribe/rediarize commands | Modify — force native language on local; **add `rediarize_meeting`** |
| `src-tauri/crates/db/src/repos/transcript_repo.rs` | transcript persistence | Modify — add `segments_for` loader (reuse `replace_lines`) |
| `src-tauri/src/lib.rs` (or the invoke_handler) | command registration | Modify — register `rediarize_meeting` |
| `src/features/live-recording/StopConfirmModal/StopConfirmModal.tsx` | save modal | Modify — participant-count input |
| `src/features/meeting-detail/tabs/TranscriptTab.tsx` | transcript tab | Modify — "Re-asignar hablantes" control + confirm |
| `src/features/meeting-detail/useRediarize.ts` | re-diarize hook | **Create** — invoke + loading state |
| `src/i18n/locales/{es,en}.json` | i18n | Modify — new strings |
| `src/ipc/bindings.ts` | generated bindings (gitignored) | Regenerate |

---

## Task A1: Relax whisper's no-speech gate + verify log

**Root cause (confirmed):** whisper.cpp's `is_no_speech` gate (`no_speech_prob > 0.6 && avg_logprobs < -1.0`) drops a whole 30s window's text; quiet trailing speech (amplified by v1.2 AEC noise-suppression) trips it on the final window. Raising `no_speech_thold` keeps that text.

**Files:** Modify `src-tauri/crates/whisper/src/transcribe.rs`.

- [ ] **Step 1: Failing test for the new default.** In `transcribe.rs`'s `#[cfg(test)] mod tests`, add:
```rust
    #[test]
    fn transcribe_opts_default_relaxes_no_speech_and_no_forced_language() {
        let o = TranscribeOpts::default();
        assert_eq!(o.no_speech_thold, 0.9, "relaxed gate keeps quiet tail windows");
        assert!(o.language.is_none(), "language still defaults to auto unless forced");
    }
```

- [ ] **Step 2: Run it — must fail** (`no_speech_thold` field doesn't exist yet).
Run: `cargo test -p smart-noter-whisper transcribe_opts_default -- --nocapture`
Expected: FAIL (compile error: no field `no_speech_thold`).

- [ ] **Step 3: Add the field + set it + the verify log.** In `TranscribeOpts` (lines 26-30) add the field, and in `Default` (32-39) default it to `0.9`:
```rust
pub struct TranscribeOpts {
    pub n_threads: i32,
    /// `None` → auto-detect language; `Some("es")` to force.
    pub language: Option<String>,
    /// Whisper's per-window no-speech gate threshold. whisper.cpp's default 0.6
    /// drops a whole 30 s window when `no_speech_prob` exceeds it (and avg_logprobs
    /// < -1.0), which silently loses quiet trailing speech (the last 3–12 s bug,
    /// worsened by the v1.2 AEC noise-suppression). Relaxed to 0.9 so tail windows
    /// still emit; the `text.is_empty()` skip below drops truly-empty output.
    pub no_speech_thold: f32,
}
```
```rust
impl Default for TranscribeOpts {
    fn default() -> Self {
        Self {
            n_threads: 4,
            language: None,
            no_speech_thold: 0.9,
        }
    }
}
```
In `transcribe()` after line 106 (`set_language`), add:
```rust
    params.set_no_speech_thold(opts.no_speech_thold);
```
After `classify_full_outcome(...)?;` (line 124), before `let n = state.full_n_segments();`, add the verify log:
```rust
    // Tail-drop verification (v1.2 whisper tail fix): the last segment's end should
    // reach ~the pcm duration. If it's short, whisper's no-speech gate dropped the tail.
    let n = state.full_n_segments();
    let last_end_ms = (0..n)
        .rev()
        .find_map(|i| state.get_segment(i))
        .map(|s| (s.end_timestamp().max(0) * 10) as u64)
        .unwrap_or(0);
    let pcm_ms = (pcm.len() as f64 / 16_000.0 * 1000.0) as u64;
    tracing::info!(segments = n, last_end_ms, pcm_ms, "whisper transcription tail check");
```
Then reuse `n` in the existing extraction loop (remove the now-duplicate `let n = state.full_n_segments();` at old line 129 — the loop `for i in 0..n` keeps working with the `n` bound above).

Verify `whisper-rs` 0.16 exposes `FullParams::set_no_speech_thold(f32)` — if the exact name differs, `cargo build` will tell you; adjust to the crate's real setter.

- [ ] **Step 4: Run tests — pass.**
Run: `cargo test -p smart-noter-whisper` → PASS (the new test + existing). `cargo build -p smart-noter-whisper` clean.

- [ ] **Step 5: Commit.**
```bash
git add src-tauri/crates/whisper/src/transcribe.rs
git commit -m "fix(whisper): relax no-speech gate to 0.9 so quiet tail speech is not dropped"
```

---

## Task A2: Force native language on the local transcribe path

**Amplifier:** the local path passes `TranscribeOpts::default()` → `language: None` → "auto", even though `settings.native_language = "es"`. Auto-detect + low confidence on quiet Spanish tail makes the `avg_logprobs < -1.0` half of the gate easier to hit. Only the cloud path uses the native language today.

**Files:** Modify `src-tauri/src/commands/transcription.rs` (the LOCAL branch, ~line 234).

- [ ] **Step 1: Locate + change.** In the local branch, the call is currently `let opts = TranscribeOpts::default(); match transcribe(&pcm, model_path, &opts, progress, abort.clone())`. Replace the `TranscribeOpts::default()` construction with one that forces the native language:
```rust
                let opts = TranscribeOpts {
                    language: Some(settings.native_language.clone()),
                    ..TranscribeOpts::default()
                };
```
(`settings` is in scope — it's the `AppSettings` from `resolve_transcription_provider`, captured by the job closure. `native_language` defaults to `"es"`.)

- [ ] **Step 2: Build + existing tests.**
Run: `cargo build -p smart-noter && cargo test -p smart-noter-whisper -p smart-noter-core`
Expected: clean + PASS. Clippy `--workspace --all-targets -- -D warnings` clean, `cargo fmt`.

- [ ] **Step 3: Commit.**
```bash
git add src-tauri/src/commands/transcription.rs
git commit -m "fix(transcription): force native language on the local whisper path (tail confidence)"
```

---

## Task B1: Participant-count input in the save modal

Populate the already-wired `speakerHint` from a number input so the initial (auto)transcription forces the count. Zero backend.

**Files:** Modify `src/features/live-recording/StopConfirmModal/StopConfirmModal.tsx` and `StopConfirmModal.test.tsx`; `src/i18n/locales/{es,en}.json`.

- [ ] **Step 1: Failing test.** In `StopConfirmModal.test.tsx`, add (follow the file's existing render/mocks; `finalize_recording` is mocked to return a meeting and `navigate` is spied):
```tsx
  it('typing a participant count navigates with that speakerHint', async () => {
    // ...render the modal open (reuse the file's existing setup helper)...
    const countInput = screen.getByLabelText('Participantes (opcional)');
    await userEvent.type(countInput, '3');
    await userEvent.click(screen.getByRole('button', { name: /Guardar|Save/i }));
    const [, navOptions] = navigateSpy.mock.calls[0] as [string, { state: Record<string, unknown> }];
    expect(navOptions.state.speakerHint).toBe(3);
  });
```

- [ ] **Step 2: Run — fail** (`pnpm test -- StopConfirmModal`): no such input.

- [ ] **Step 3: Add the input + local state.** In `StopConfirmModal.tsx`, after the `title`/`busy` state (line 45-46), add:
```tsx
  const [speakerCount, setSpeakerCount] = useState<number | null>(speakerHint);
```
Change the navigate in `onSave` (line 59) to use the local value:
```tsx
      navigate(Paths.MeetingDetail(meeting.id), { state: { justRecorded: true, speakerHint: speakerCount } });
```
Add the input inside the `<Modal>` body, after the title `<Input>` (line 116), before the summary `<div>`:
```tsx
      <Input
        label={t('speakerCountLabel')}
        type="number"
        min={1}
        max={10}
        value={speakerCount === null ? '' : String(speakerCount)}
        onChange={(e) => {
          const v = e.target.value.trim();
          setSpeakerCount(v === '' ? null : Math.max(1, Math.min(10, Number.parseInt(v, 10) || 1)));
        }}
        placeholder={t('speakerCountPh')}
      />
```
(If the `Input` primitive doesn't forward `type`/`min`/`max`, pass them through or use a plain `<input>` styled like the others — check `components/primitives/Input`.)

- [ ] **Step 4: i18n.** Add to BOTH `src/i18n/locales/es.json` and `en.json`:
  - `speakerCountLabel`: ES `"Participantes (opcional)"`, EN `"Participants (optional)"`.
  - `speakerCountPh`: ES `"Auto-detectar"`, EN `"Auto-detect"`.
Run `pnpm generate:i18n-keys` if the project regenerates a keys file (it's gitignored).

- [ ] **Step 5: Run — pass + suite + typecheck.**
Run: `pnpm test -- StopConfirmModal` → PASS. `pnpm test` full + `pnpm tsc --noEmit` clean.

- [ ] **Step 6: Commit.**
```bash
git add src/features/live-recording/StopConfirmModal/ src/i18n
git commit -m "feat(diarization): participant-count input in the save modal (forces speaker count)"
```

---

## Task B2: `segments_for` loader + `rediarize_meeting` command

Re-run ONLY diarization on an existing meeting with a forced count, reusing `replace_lines`.

**Files:** Modify `src-tauri/crates/db/src/repos/transcript_repo.rs` (add `segments_for`); `src-tauri/src/commands/transcription.rs` (add `rediarize_meeting`); register the command.

- [ ] **Step 1: Failing test for the loader.** In `transcript_repo.rs` `#[cfg(test)] mod tests`, add (mirror the existing `replace_lines` test setup — an in-memory pool with migrations + a meeting):
```rust
    #[tokio::test]
    async fn segments_for_returns_lines_in_time_order() {
        let pool = crate::test_pool().await; // reuse the crate's test-pool helper
        // seed a meeting + 2 lines via replace_lines
        let lines = vec![
            LineInput { t_seconds: 0, end_seconds: 5, t_display: "00:00:00".into(), text_es: "hola".into(), speaker_idx: 0 },
            LineInput { t_seconds: 5, end_seconds: 9, t_display: "00:00:05".into(), text_es: "adios".into(), speaker_idx: 0 },
        ];
        // (create the meeting row first per the crate's test helpers)
        replace_lines(&pool, "m1", &lines, 1, 2).await.unwrap();
        let segs = segments_for(&pool, "m1").await.unwrap();
        assert_eq!(segs.len(), 2);
        assert_eq!(segs[0], (0, 5, "hola".to_string()));
        assert_eq!(segs[1], (5, 9, "adios".to_string()));
    }
```
(Adapt to the crate's actual test-pool/meeting-seed helpers — see the existing `replace_lines_single_speaker_creates_s1_and_word_counts` test in this file for the exact setup.)

- [ ] **Step 2: Run — fail** (`cargo test -p smart-noter-db segments_for`): no such fn.

- [ ] **Step 3: Implement `segments_for`.** Add to `transcript_repo.rs`:
```rust
/// Load a meeting's transcript segments (start/end seconds + Spanish text) in time
/// order — the input to a re-diarization + re-align that keeps the transcript text.
pub async fn segments_for(
    pool: &SqlitePool,
    meeting_id: &str,
) -> Result<Vec<(i64, i64, String)>, DbError> {
    let rows: Vec<(i64, i64, String)> = sqlx::query_as(
        "SELECT t_seconds, end_seconds, text_es FROM transcript_lines
         WHERE meeting_id = ? ORDER BY t_seconds",
    )
    .bind(meeting_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
```

- [ ] **Step 4: Run loader test — pass.** `cargo test -p smart-noter-db segments_for` → PASS.

- [ ] **Step 5: Implement `rediarize_meeting`.** In `transcription.rs`, add the command (mirrors `transcribe_meeting`'s audio/model/diarize setup; imports `diarize`, `DiarizeOpts`, `align`, `TextSegment`, `decode`, `replace_lines`, `LineInput`, `word_count`, `fmt_timestamp` already used in this file):
```rust
#[tauri::command]
#[specta::specta]
pub async fn rediarize_meeting(
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::AppState>,
    meeting_id: String,
    speaker_count: u32,
) -> Result<(), AppError> {
    use smart_noter_db::repos::transcript_repo::{replace_lines, segments_for, LineInput};
    // 1. Audio (WAV) — same resolver as transcribe_meeting; None => audio deleted.
    let audio = smart_noter_db::repos::meetings_repo::get_audio(&state.pool, &meeting_id).await?;
    let audio_path = match audio {
        Some(a) => std::path::PathBuf::from(a.path),
        None => return Err(AppError::NotFound(format!("no audio for {meeting_id}"))),
    };
    // 2. Diarization models must be present.
    let app_dir = app_data(&app)?;
    let diar_seg = diar_models::model_path(&app_dir, "segmentation")
        .filter(|p| p.is_file())
        .ok_or_else(|| AppError::NotFound("diarization models not downloaded".into()))?;
    let diar_emb = diar_models::model_path(&app_dir, "embedding")
        .filter(|p| p.is_file())
        .ok_or_else(|| AppError::NotFound("diarization models not downloaded".into()))?;
    // 3. Existing transcript segments (keep the text).
    let segs = segments_for(&state.pool, &meeting_id).await?;
    if segs.is_empty() {
        return Err(AppError::NotFound(format!("no transcript for {meeting_id}")));
    }
    // 4. CPU work off the async runtime: decode + diarize + align.
    let n_speakers = speaker_count.max(1);
    let (lines, count, words) = tauri::async_runtime::spawn_blocking(move || {
        let pcm = decode::decode_to_pcm_16k_mono(&audio_path)
            .map_err(|e| AppError::Internal(format!("decode: {}", e.message)))?;
        let abort = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let opts = DiarizeOpts { num_speakers: Some(n_speakers) };
        let diar_segs = diarize(&pcm, &diar_seg, &diar_emb, &opts, abort)
            .map_err(|e| AppError::Internal(format!("diarize: {}", e.message)))?;
        let texts: Vec<TextSegment> = segs
            .iter()
            .map(|(t0, t1, text)| TextSegment {
                start_ms: (*t0 * 1000) as u32,
                end_ms: (*t1 * 1000) as u32,
                text: text.clone(),
            })
            .collect();
        let aligned = align(&texts, &diar_segs);
        let max_spk = aligned.iter().map(|a| a.speaker).max().unwrap_or(0);
        let count = (max_spk as usize) + 1;
        let mut words = 0i64;
        let lines: Vec<LineInput> = segs
            .iter()
            .zip(aligned.iter())
            .map(|((t0, t1, text), a)| {
                words += smart_noter_whisper::transcribe::word_count(text) as i64;
                LineInput {
                    t_seconds: *t0,
                    end_seconds: *t1,
                    t_display: smart_noter_whisper::transcribe::fmt_timestamp(*t0 as u32),
                    text_es: text.clone(),
                    speaker_idx: a.speaker as usize,
                }
            })
            .collect();
        Ok::<_, AppError>((lines, count, words))
    })
    .await
    .map_err(|e| AppError::Internal(format!("rediarize task: {e}")))??;
    // 5. Re-persist atomically (reuses the write path).
    replace_lines(&state.pool, &meeting_id, &lines, count, words)
        .await
        .map_err(AppError::from)?;
    // 6. Refresh FTS (best-effort — text unchanged, but keep parity with transcribe).
    let _ = smart_noter_db::repos::search_repo::upsert_meeting(&state.pool, &meeting_id).await;
    Ok(())
}
```
Notes to reconcile at compile time: `get_audio`'s exact module/signature (grep `fn get_audio` in `meetings_repo.rs` — used at transcription.rs:108); `AppError` conversions from `DbError` (an `impl From<DbError> for AppError` likely exists — if not, map it); `app_data(&app)` is the helper used at transcription.rs:135. `TextSegment`/`align`/`DiarizeOpts`/`diarize` are imported at the top of transcription.rs (679-680) and in the diarize crate.

- [ ] **Step 6: Register the command.** Add `rediarize_meeting` to the `tauri::generate_handler![...]` list (same place `transcribe_meeting` is registered — grep `transcribe_meeting` in `src-tauri/src/lib.rs`).

- [ ] **Step 7: Build + tests.**
Run: `cargo build -p smart-noter && cargo test -p smart-noter-db` → clean + PASS. Clippy `--workspace --all-targets -- -D warnings` clean, `cargo fmt`.

- [ ] **Step 8: Commit.**
```bash
git add src-tauri/crates/db/src/repos/transcript_repo.rs src-tauri/src/commands/transcription.rs src-tauri/src/lib.rs
git commit -m "feat(diarization): rediarize_meeting command (re-run diarization with forced count, keep transcript)"
```

---

## Task B3: Regenerate bindings

**Files:** `src/ipc/bindings.ts` (gitignored — regenerate so the frontend sees `rediarizeMeeting`).

- [ ] **Step 1: Regenerate.**
Run: `cd src-tauri && cargo run --bin specta-export --features generate-bindings` (from repo root prefix the env preamble).
Expected: `src/ipc/bindings.ts` now contains `rediarizeMeeting(meetingId, speakerCount)`. `grep -n rediarize src/ipc/bindings.ts` confirms.

- [ ] **Step 2: Typecheck.** `pnpm tsc --noEmit` clean (the new binding is available; nothing consumes it yet — that's Task B4). No commit needed (gitignored).

---

## Task B4: "Re-asignar hablantes" control + confirm + hook

**Files:** Create `src/features/meeting-detail/useRediarize.ts`; modify `src/features/meeting-detail/tabs/TranscriptTab.tsx` (+ its test); `src/i18n/locales/{es,en}.json`.

- [ ] **Step 1: The hook.** Create `src/features/meeting-detail/useRediarize.ts`:
```ts
import { toast } from '@/components/primitives/Toast/Toast';
import { useT } from '@/i18n/useT';
import { errorMessage, toAppError } from '@/ipc/error';
import { baseApi } from '@/store/api/base';
import { invoke } from '@tauri-apps/api/core';
import { useState } from 'react';
import { useDispatch } from 'react-redux';

/** Re-runs diarization on an existing meeting with a forced speaker count,
 *  keeping the transcript text. Invalidates the meeting cache on success. */
export function useRediarize(meetingId: string) {
  const dispatch = useDispatch();
  const { t } = useT();
  const [running, setRunning] = useState(false);

  const rediarize = async (speakerCount: number) => {
    if (running) return;
    setRunning(true);
    try {
      await invoke('rediarize_meeting', { meetingId, speakerCount });
      dispatch(baseApi.util.invalidateTags([{ type: 'Meeting', id: meetingId }]));
    } catch (err) {
      const ae = toAppError(err);
      toast.error(t('audioErrorTitle'), {
        id: `rediarize-error:${ae.code}`,
        description: errorMessage(ae, t),
      });
    } finally {
      setRunning(false);
    }
  };
  return { running, rediarize };
}
```

- [ ] **Step 2: Failing test for the control.** In `TranscriptTab.test.tsx`, add (follow the file's render setup with a `meeting` that has `transcript` lines and an `audioPath`):
```tsx
  it('re-assign speakers: sets a count, confirms, and calls rediarize_meeting', async () => {
    // render TranscriptTab with a meeting that HAS transcript lines and audio available
    await userEvent.type(screen.getByLabelText('N.º de hablantes'), '2');
    await userEvent.click(screen.getByRole('button', { name: 'Re-asignar hablantes' }));
    // confirm dialog appears; accept it
    await userEvent.click(screen.getByRole('button', { name: /Rehacer|Confirmar|Re-run/i }));
    expect(invokeMock).toHaveBeenCalledWith('rediarize_meeting', { meetingId: meeting.id, speakerCount: 2 });
  });
```

- [ ] **Step 3: Run — fail** (`pnpm test -- TranscriptTab`): no such control.

- [ ] **Step 4: Add the control to `TranscriptTab.tsx`.** Import the hook + a confirm dialog (reuse the project's Modal/confirm primitive — grep for an existing confirm dialog pattern, e.g. a `Modal` with confirm/cancel). Add near the card head (the `lines.length > 0` region so it only shows for meetings with a transcript), gated on audio availability (`meeting.audioPath`/`hasAudio` — check the `MeetingDetail` shape; disable + tooltip when absent):
```tsx
  const { running, rediarize } = useRediarize(meeting.id);
  const [reN, setReN] = useState<number>(2);
  const [confirmOpen, setConfirmOpen] = useState(false);
  const hasAudio = Boolean(meeting.audioPath); // adjust to the real MeetingDetail field
```
Control JSX (in the card head's right side, next to the existing "Seleccionar líneas" button):
```tsx
          {lines.length > 0 && (
            <div className={styles.rediarizeRow}>
              <input
                type="number"
                min={1}
                max={10}
                aria-label={t('rediarizeCountLabel')}
                value={reN}
                onChange={(e) => setReN(Math.max(1, Math.min(10, Number.parseInt(e.target.value, 10) || 1)))}
                disabled={!hasAudio || running}
              />
              <Button
                variant="default"
                disabled={!hasAudio || running}
                title={hasAudio ? undefined : t('rediarizeNoAudio')}
                onClick={() => setConfirmOpen(true)}
              >
                {running ? t('rediarizeRunning') : t('rediarizeCta')}
              </Button>
            </div>
          )}
```
Confirm dialog (always shown before re-diarizing — it's destructive; reuse the project's Modal):
```tsx
      <Modal
        open={confirmOpen}
        onClose={() => setConfirmOpen(false)}
        title={t('rediarizeConfirmTitle')}
        footer={
          <>
            <Button variant="default" onClick={() => setConfirmOpen(false)}>{t('cancel')}</Button>
            <Button variant="primary" onClick={() => { setConfirmOpen(false); void rediarize(reN); }}>
              {t('rediarizeConfirmCta')}
            </Button>
          </>
        }
      >
        {t('rediarizeConfirmBody')}
      </Modal>
```

- [ ] **Step 5: i18n.** Add to BOTH locales:
  - `rediarizeCountLabel`: ES `"N.º de hablantes"`, EN `"Number of speakers"`.
  - `rediarizeCta`: ES `"Re-asignar hablantes"`, EN `"Re-assign speakers"`.
  - `rediarizeRunning`: ES `"Re-asignando…"`, EN `"Re-assigning…"`.
  - `rediarizeNoAudio`: ES `"Requiere el audio de la grabación"`, EN `"Requires the recording audio"`.
  - `rediarizeConfirmTitle`: ES `"Rehacer la diarización"`, EN `"Redo diarization"`.
  - `rediarizeConfirmBody`: ES `"Esto reemplaza las asignaciones de hablante actuales, incluidas correcciones manuales. El texto del transcript no cambia."`, EN `"This replaces the current speaker assignments, including manual corrections. The transcript text is unchanged."`.
  - `rediarizeConfirmCta`: ES `"Rehacer"`, EN `"Redo"`.

- [ ] **Step 6: Run — pass + suite + typecheck.**
Run: `pnpm test -- TranscriptTab` → PASS. `pnpm test` full + `pnpm tsc --noEmit` clean.

- [ ] **Step 7: Commit.**
```bash
git add src/features/meeting-detail/useRediarize.ts src/features/meeting-detail/tabs/TranscriptTab.tsx src/features/meeting-detail/tabs/TranscriptTab.test.tsx src/i18n
git commit -m "feat(diarization): Re-assign speakers control with forced count + confirm (rediarize)"
```

---

## Hardware smoke (after all tasks)

From a release build:
1. **A (tail):** re-transcribe a meeting whose speech runs to the end; check the log line `whisper transcription tail check` shows `last_end_ms ≈ pcm_ms` (not 3–12 s short), and the transcript now includes the tail.
2. **B (save-time):** record with a known participant count set in the save modal → diarization splits into that many speakers.
3. **B (re-diarize):** open a meeting with bad auto-diarization, set N, "Re-asignar hablantes", confirm → speakers re-split correctly, transcript text unchanged, fast (no whisper re-run). Verify the control is disabled when the audio was deleted.

---

## Self-Review

- **Spec coverage:** A no-speech relax (A1) ✓; A force native language (A2) ✓; A verify instrumentation (A1 log) ✓; B save-time input (B1) ✓; B rediarize command reusing replace_lines (B2) ✓; B transcript control + always-confirm + no-WAV disable (B4) ✓; bindings (B3) ✓; i18n (B1, B4) ✓; error handling (no audio / no models / no transcript → typed errors in B2) ✓.
- **Placeholders:** the two compile-reconcile notes (whisper setter name in A1; `get_audio`/`AppError::from`/`meeting.audioPath` field in B2/B4) are real API-name verifications, not TODOs — the code + intent are complete; the implementer confirms exact names via `cargo build`/the `MeetingDetail` type.
- **Type consistency:** `rediarize_meeting(meetingId, speakerCount)` matches across B2 (command), B3 (binding), B4 (hook/test). `LineInput`/`replace_lines`/`segments_for` signatures consistent. `speakerHint`/`speakerCount` naming: `speakerHint` is the save-time nav value (B1); `speaker_count`/`speakerCount` is the forced count in the command/hook (B2/B4) — distinct on purpose.
