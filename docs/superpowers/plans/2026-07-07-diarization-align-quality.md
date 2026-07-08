# Diarization Quality (align overlap-flatten + merge/select-all) — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Que la diarización deje de fusionar voces distintas en un solo hablante (bug del colapso en `align`), y exponer herramientas rápidas de corrección (fusionar hablantes, seleccionar todo de un hablante) + aviso cuando re-diarizar no encuentra más voces.

**Architecture:** Tres helpers **puros** nuevos en el crate `diarize` (`flatten_overlaps`, `fill_zero_durations`, `remap_contiguous`) atacan la causa raíz sin tocar la lógica de `align`. `flatten_overlaps` corre dentro de `diarize()` (beneficia ambas rutas: transcripción inicial y re-diarización). `rediarize_meeting` usa los otros dos y devuelve el conteo. El frontend usa ese conteo para el toast de no-op, y expone `merge_speakers` (ya existe en backend) + un atajo de selección por hablante.

**Tech Stack:** Rust (sherpa-rs diarize, sqlx/SQLite, specta) · React/TS (RTK Query, vitest) · Tauri 2.

**Convenciones de build (OBLIGATORIO en cada comando cargo/git):** anteponer el preamble de entorno y correr cargo desde `src-tauri/`:
```bash
export PATH="$HOME/.cargo/bin:/c/Program Files (x86)/Microsoft Visual Studio/2022/BuildTools/Common7/IDE/CommonExtensions/Microsoft/CMake/CMake/bin:$PATH"
export LIBCLANG_PATH="C:/Program Files/LLVM/bin"
cd "C:/Users/erick/Projects/Smart Noter/src-tauri"
```
Los tests del crate `diarize` NO cargan sherpa/modelos (son lógica pura) → corren rápido y sin DLLs.

---

## File Structure

- **Create:** `src-tauri/crates/diarize/src/overlap.rs` — `flatten_overlaps` (resuelve solapes de segmentos de diarización).
- **Modify:** `src-tauri/crates/diarize/src/align.rs` — añadir `fill_zero_durations` (pre-align) y `remap_contiguous` (post-align), ambas puras y testeadas.
- **Modify:** `src-tauri/crates/diarize/src/lib.rs` — exportar `overlap` + las funciones nuevas.
- **Modify:** `src-tauri/crates/diarize/src/diarize.rs` — llamar `flatten_overlaps` al final de `diarize()`.
- **Modify:** `src-tauri/src/commands/transcription.rs` — `rediarize_meeting`: usar los helpers, arreglar duración-cero, devolver `count`.
- **Regenerate:** `src/ipc/bindings.ts` — firma de `rediarize_meeting` cambia a devolver `number`.
- **Modify:** `src/features/meeting-detail/useRediarize.ts` — recibir `prevSpeakerCount`, usar el count devuelto, toast de no-op.
- **Modify:** `src/features/meeting-detail/tabs/TranscriptTab.tsx` — pasar `prevSpeakerCount`; modal "Fusionar hablantes" (B1); atajo seleccionar-todo (B2).
- **Modify:** `src/i18n/locales/es.json` + `src/i18n/locales/en.json` — claves nuevas; luego `pnpm generate:i18n-keys`.

---

## Task 1: `flatten_overlaps` + wire en `diarize()`

**Files:**
- Create: `src-tauri/crates/diarize/src/overlap.rs`
- Modify: `src-tauri/crates/diarize/src/lib.rs`
- Modify: `src-tauri/crates/diarize/src/diarize.rs`

- [ ] **Step 1: Escribir `overlap.rs` con la función + tests (TDD, todo junto porque es puro y pequeño).**

```rust
//! Flatten overlapping diarization segments into a non-overlapping partition.
use crate::align::DiarSegment;

/// Resolve temporal overlaps in sherpa's diarization output. Where two or more
/// segments cover the same instant, the SHORTEST original segment wins that slice
/// (a short nested turn is more specific than the long span that encloses it);
/// ties in duration break to the lower speaker number. Adjacent output slices with
/// the same speaker are merged. If the input has no overlaps, the output is
/// identical to the input (no-op) — this is why it's safe on clean diarization.
pub fn flatten_overlaps(segments: &[DiarSegment]) -> Vec<DiarSegment> {
    if segments.len() < 2 {
        return segments.to_vec();
    }
    let mut cuts: Vec<u32> = Vec::with_capacity(segments.len() * 2);
    for s in segments {
        cuts.push(s.start_ms);
        cuts.push(s.end_ms);
    }
    cuts.sort_unstable();
    cuts.dedup();

    let mut out: Vec<DiarSegment> = Vec::new();
    for w in cuts.windows(2) {
        let (lo, hi) = (w[0], w[1]);
        if lo >= hi {
            continue;
        }
        // Among segments covering [lo,hi), pick the shortest original;
        // tie in duration → lower speaker.
        let mut best: Option<&DiarSegment> = None;
        for s in segments {
            if s.start_ms <= lo && hi <= s.end_ms {
                match best {
                    Some(b) => {
                        let cand = s.end_ms - s.start_ms;
                        let cur = b.end_ms - b.start_ms;
                        if cand < cur || (cand == cur && s.speaker < b.speaker) {
                            best = Some(s);
                        }
                    }
                    None => best = Some(s),
                }
            }
        }
        let Some(b) = best else {
            continue; // slice not covered by any segment (a true gap) — drop it
        };
        if let Some(last) = out.last_mut() {
            if last.speaker == b.speaker && last.end_ms == lo {
                last.end_ms = hi;
                continue;
            }
        }
        out.push(DiarSegment {
            start_ms: lo,
            end_ms: hi,
            speaker: b.speaker,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    fn d(start_ms: u32, end_ms: u32, speaker: u32) -> DiarSegment {
        DiarSegment { start_ms, end_ms, speaker }
    }

    #[test]
    fn no_overlap_is_identity() {
        let segs = vec![d(0, 1000, 0), d(1000, 2000, 1), d(2000, 3000, 0)];
        assert_eq!(flatten_overlaps(&segs), segs);
    }

    #[test]
    fn nested_short_turn_surfaces() {
        // S0 spans the whole timeline; S1 is a short nested turn inside it
        // (the Prueba 6 collapse pattern).
        let segs = vec![d(0, 10000, 0), d(4000, 5000, 1)];
        let out = flatten_overlaps(&segs);
        assert_eq!(out, vec![d(0, 4000, 0), d(4000, 5000, 1), d(5000, 10000, 0)]);
    }

    #[test]
    fn partial_overlap_shorter_wins_shared_slice() {
        // A [0..6000] len 6000, B [4000..7000] len 3000 → shared [4000..6000] goes to B.
        let segs = vec![d(0, 6000, 0), d(4000, 7000, 1)];
        let out = flatten_overlaps(&segs);
        assert_eq!(out, vec![d(0, 4000, 0), d(4000, 7000, 1)]);
    }

    #[test]
    fn equal_duration_tie_breaks_to_lower_speaker_then_merges() {
        // Both len 2000, overlap on [1000..2000] → lower speaker (0) wins;
        // [1000..2000]S0 merges with [2000..3000]S0.
        let segs = vec![d(0, 2000, 1), d(1000, 3000, 0)];
        let out = flatten_overlaps(&segs);
        assert_eq!(out, vec![d(0, 1000, 1), d(1000, 3000, 0)]);
    }
}
```

- [ ] **Step 2: Exportar en `lib.rs`.** Añadir tras la línea `pub use align::{align, AlignedLine, DiarSegment};`:

```rust
pub mod overlap;
pub use overlap::flatten_overlaps;
```

- [ ] **Step 3: Correr los tests, deben pasar.**

Run: `cargo test -p smart-noter-diarize overlap`
Expected: PASS (4 tests).

- [ ] **Step 4: Wire en `diarize()`.** En `src-tauri/crates/diarize/src/diarize.rs`, la cola de la función construye `segments` con un `.collect()` y hace `Ok(segments)`. Cambiar para aplanar antes de devolver:

```rust
    let segments: Vec<DiarSegment> = raw
        .into_iter()
        .map(|s| DiarSegment {
            start_ms: (s.start.max(0.0) * 1000.0) as u32,
            end_ms: (s.end.max(0.0) * 1000.0) as u32,
            speaker: s.speaker.max(0) as u32,
        })
        .collect();
    // sherpa can emit temporally OVERLAPPING segments (a dominant speaker's span
    // with short nested turns inside); align's max-total-overlap rule would then
    // collapse every line onto the dominant speaker. Flatten to a non-overlapping
    // partition first (no-op when there are no overlaps).
    Ok(crate::overlap::flatten_overlaps(&segments))
```
(Ajustar los nombres si el binding local difiere; el objetivo es envolver el `Vec<DiarSegment>` final en `flatten_overlaps` antes de `Ok(...)`.)

- [ ] **Step 5: Compilar el crate.**

Run: `cargo build -p smart-noter-diarize`
Expected: OK, sin warnings nuevos.

- [ ] **Step 6: Commit.**

```bash
git add src-tauri/crates/diarize/src/overlap.rs src-tauri/crates/diarize/src/lib.rs src-tauri/crates/diarize/src/diarize.rs
git commit -m "feat(diarize): flatten overlapping segments before align (shortest-wins)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 2: `fill_zero_durations` (pre-align)

**Files:**
- Modify: `src-tauri/crates/diarize/src/align.rs` (añadir función + tests)
- Modify: `src-tauri/crates/diarize/src/lib.rs` (exportar)

- [ ] **Step 1: Escribir la función + tests en `align.rs`** (al final del archivo, antes o después de `mod tests`; la función va en el cuerpo del módulo, los tests dentro de `mod tests`).

Función (cuerpo del módulo):
```rust
/// Give every text segment a real duration for overlap scoring. Transcript lines
/// persisted at second granularity (or legacy rows with NULL end) can arrive with
/// `end_ms <= start_ms`; such a point interval overlaps no diar segment and forces
/// `align` into its nearest-gap fallback. Fill each degenerate line's end from the
/// NEXT line's start; the last line (or a next line that starts no later) uses
/// `audio_end_ms`, with a 1 ms floor so the interval is never empty.
pub fn fill_zero_durations(texts: &[TextSegment], audio_end_ms: u32) -> Vec<TextSegment> {
    let n = texts.len();
    texts
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let mut end = t.end_ms;
            if end <= t.start_ms {
                let next = if i + 1 < n { texts[i + 1].start_ms } else { audio_end_ms };
                end = if next > t.start_ms { next } else { t.start_ms + 1 };
            }
            TextSegment {
                start_ms: t.start_ms,
                end_ms: end,
                text: t.text.clone(),
            }
        })
        .collect()
}
```

Tests (dentro de `mod tests`, reutilizan el helper `t(...)` ya existente):
```rust
    #[test]
    fn fill_uses_next_line_start_then_audio_end() {
        let texts = vec![t(0, 0, "a"), t(5000, 0, "b"), t(9000, 0, "c")];
        let out = fill_zero_durations(&texts, 12000);
        assert_eq!(out[0].end_ms, 5000);
        assert_eq!(out[1].end_ms, 9000);
        assert_eq!(out[2].end_ms, 12000);
    }

    #[test]
    fn fill_keeps_real_durations() {
        let texts = vec![t(0, 3000, "a")];
        assert_eq!(fill_zero_durations(&texts, 10000)[0].end_ms, 3000);
    }

    #[test]
    fn fill_same_instant_lines_get_1ms_floor() {
        let texts = vec![t(5000, 5000, "a"), t(5000, 5000, "b")];
        let out = fill_zero_durations(&texts, 10000);
        assert_eq!(out[0].end_ms, 5001); // next starts at same 5000 → floor
        assert_eq!(out[1].end_ms, 10000);
    }
```

- [ ] **Step 2: Exportar en `lib.rs`.** Cambiar la línea de export de align a:

```rust
pub use align::{align, fill_zero_durations, AlignedLine, DiarSegment, TextSegment};
```
(Se añade `fill_zero_durations` y `TextSegment` al re-export para que el comando lo use directo.)

- [ ] **Step 3: Correr tests.**

Run: `cargo test -p smart-noter-diarize fill_`
Expected: PASS (3 tests).

- [ ] **Step 4: Commit.**

```bash
git add src-tauri/crates/diarize/src/align.rs src-tauri/crates/diarize/src/lib.rs
git commit -m "feat(diarize): fill_zero_durations for point-interval transcript lines

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 3: `remap_contiguous` (post-align)

**Files:**
- Modify: `src-tauri/crates/diarize/src/align.rs`
- Modify: `src-tauri/crates/diarize/src/lib.rs`

- [ ] **Step 1: Función + tests en `align.rs`.**

Función:
```rust
/// Remap speaker labels to a contiguous `[0..k)` range, preserving first-appearance
/// order. `align` can emit non-contiguous labels (sherpa uses e.g. {0,4,5}); the
/// caller derives the participant count from `max(speaker)+1`, which would create
/// phantom empty participants. Returns `(remapped, k)` where `k` is the real number
/// of distinct speakers used.
pub fn remap_contiguous(speakers: &[u32]) -> (Vec<u32>, usize) {
    let mut mapping: Vec<u32> = Vec::new(); // index = new label, value = original
    let out = speakers
        .iter()
        .map(|&s| match mapping.iter().position(|&o| o == s) {
            Some(idx) => idx as u32,
            None => {
                mapping.push(s);
                (mapping.len() - 1) as u32
            }
        })
        .collect();
    (out, mapping.len())
}
```

Tests:
```rust
    #[test]
    fn remap_non_contiguous_to_zero_based() {
        let (out, k) = remap_contiguous(&[0, 4, 4, 5, 0]);
        assert_eq!(out, vec![0, 1, 1, 2, 0]);
        assert_eq!(k, 3);
    }

    #[test]
    fn remap_preserves_first_appearance_order() {
        let (out, k) = remap_contiguous(&[5, 5, 2, 0]);
        assert_eq!(out, vec![0, 0, 1, 2]);
        assert_eq!(k, 3);
    }

    #[test]
    fn remap_empty_is_zero() {
        let (out, k) = remap_contiguous(&[]);
        assert!(out.is_empty());
        assert_eq!(k, 0);
    }
```

- [ ] **Step 2: Exportar en `lib.rs`.**

```rust
pub use align::{align, fill_zero_durations, remap_contiguous, AlignedLine, DiarSegment, TextSegment};
```

- [ ] **Step 3: Correr tests.**

Run: `cargo test -p smart-noter-diarize remap_`
Expected: PASS (3 tests).

- [ ] **Step 4: Commit.**

```bash
git add src-tauri/crates/diarize/src/align.rs src-tauri/crates/diarize/src/lib.rs
git commit -m "feat(diarize): remap_contiguous speaker labels to avoid phantom participants

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 4: `rediarize_meeting` usa los helpers + devuelve el conteo

**Files:**
- Modify: `src-tauri/src/commands/transcription.rs`
- Regenerate: `src/ipc/bindings.ts`

**Contexto:** el cuerpo actual del `spawn_blocking` en `rediarize_meeting` (`transcription.rs:587-622`) construye `texts` (start/end en ms desde segundos), llama `align`, deriva `count = max(speaker)+1`, y arma los `LineInput`. El comando devuelve `Result<(), AppError>` y hace `Ok(())` al final.

- [ ] **Step 1: Actualizar la firma del comando** a `-> Result<u32, AppError>`.

- [ ] **Step 2: Reescribir el bloque de cómputo** (dentro del `spawn_blocking`), usando los helpers nuevos. Reemplazar desde la construcción de `texts` hasta la construcción de `lines`:

```rust
        let audio_end_ms = (pcm.len() as f64 / 16_000.0 * 1000.0) as u32;
        let texts: Vec<TextSegment> = segs
            .iter()
            .map(|(t0, t1, text)| TextSegment {
                start_ms: (*t0 * 1000) as u32,
                end_ms: (*t1 * 1000) as u32,
                text: text.clone(),
            })
            .collect();
        // Legacy/second-granularity lines can be zero-width → give them real spans.
        let texts = smart_noter_diarize::fill_zero_durations(&texts, audio_end_ms);
        let aligned = align(&texts, &diar_segs);
        // align may emit non-contiguous speaker labels; remap so count = real speakers.
        let raw: Vec<u32> = aligned.iter().map(|a| a.speaker).collect();
        let (speakers, count) = smart_noter_diarize::remap_contiguous(&raw);
        let mut words = 0i64;
        let lines: Vec<LineInput> = segs
            .iter()
            .zip(speakers.iter())
            .map(|((t0, t1, text), &spk)| {
                words += smart_noter_whisper::transcribe::word_count(text) as i64;
                LineInput {
                    t_seconds: *t0,
                    end_seconds: *t1,
                    t_display: smart_noter_whisper::transcribe::fmt_timestamp(*t0 as u32),
                    text_es: text.clone(),
                    speaker_idx: spk as usize,
                }
            })
            .collect();
        Ok::<_, AppError>((lines, count, words))
```

- [ ] **Step 3: Devolver el conteo.** Tras `replace_lines(...)` y el refresh de FTS, cambiar el `Ok(())` final por `Ok(count as u32)`. (El `count` sale del tuple del `spawn_blocking`; ya está en scope.)

- [ ] **Step 4: Asegurar los imports.** Verificar que `TextSegment` y `align` estén importados en `transcription.rs` (ya se usa `align` y `TextSegment` en el comando). `fill_zero_durations` / `remap_contiguous` se llaman con path completo `smart_noter_diarize::` (no requiere import nuevo).

- [ ] **Step 5: Compilar.**

Run: `cargo build -p smart-noter --lib` (o `cargo build` del binario)
Expected: OK.

- [ ] **Step 6: Regenerar bindings.** Desde `src-tauri/`:

Run: `cargo run --bin specta-export`
Expected: `src/ipc/bindings.ts` actualizado; la línea de `rediarize_meeting` ahora tipa el retorno como `number` (antes `null`). Verificar con `grep -n "rediarize_meeting" ../src/ipc/bindings.ts`.

- [ ] **Step 7: Commit.**

```bash
git add src-tauri/src/commands/transcription.rs src/ipc/bindings.ts
git commit -m "feat(diarize): rediarize_meeting fills durations, remaps speakers, returns count

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 5: Frontend — count devuelto + toast de no-op (C)

**Files:**
- Modify: `src/features/meeting-detail/useRediarize.ts`
- Modify: `src/features/meeting-detail/tabs/TranscriptTab.tsx`
- Modify: `src/i18n/locales/es.json` + `en.json`
- Test: `src/features/meeting-detail/useRediarize.test.ts` (crear)

- [ ] **Step 1: Añadir claves i18n.** En `es.json`:
```json
  "rediarizeNoChangeTitle": "Sin hablantes nuevos",
  "rediarizeNoChangeBody": "La diarización no encontró más voces distintas. Puedes ajustar a mano: fusionar o reasignar hablantes.",
```
En `en.json`:
```json
  "rediarizeNoChangeTitle": "No new speakers",
  "rediarizeNoChangeBody": "Diarization found no more distinct voices. You can adjust manually: merge or reassign speakers.",
```
Luego: `pnpm generate:i18n-keys` (regenera `src/i18n/keys.ts`).

- [ ] **Step 2: Escribir el test (TDD)** en `useRediarize.test.ts`, siguiendo el patrón de `useTranscription.test.tsx` (mockear `invoke` y `toast`):

```ts
import { renderHook, act } from '@testing-library/react';
import { Provider } from 'react-redux';
import { describe, expect, it, vi, beforeEach } from 'vitest';
import { toast } from '@/components/primitives/Toast/Toast';
import { invoke } from '@tauri-apps/api/core';
import { useRediarize } from './useRediarize';
import { store } from '@/store';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('@/components/primitives/Toast/Toast', () => ({
  toast: { info: vi.fn(), error: vi.fn() },
}));

const wrapper = ({ children }: { children: React.ReactNode }) => (
  <Provider store={store}>{children}</Provider>
);

describe('useRediarize no-op toast', () => {
  beforeEach(() => vi.clearAllMocks());

  it('shows an info toast when the resulting count did not increase', async () => {
    vi.mocked(invoke).mockResolvedValue(2); // 2 speakers back
    const { result } = renderHook(() => useRediarize('m-1'), { wrapper });
    await act(async () => {
      await result.current.rediarize(7, 2); // prev was 2, result 2 → no change
    });
    expect(vi.mocked(toast.info)).toHaveBeenCalledTimes(1);
  });

  it('does NOT show the info toast when the count increased', async () => {
    vi.mocked(invoke).mockResolvedValue(5);
    const { result } = renderHook(() => useRediarize('m-1'), { wrapper });
    await act(async () => {
      await result.current.rediarize(7, 2); // prev 2, result 5 → changed
    });
    expect(vi.mocked(toast.info)).not.toHaveBeenCalled();
  });
});
```

- [ ] **Step 3: Correr el test, debe FALLAR** (la firma actual de `rediarize` toma un solo argumento y no lee el retorno).

Run: `pnpm test src/features/meeting-detail/useRediarize.test.ts`
Expected: FAIL.

- [ ] **Step 4: Implementar en `useRediarize.ts`.** Cambiar `rediarize`:

```ts
  const rediarize = async (speakerCount: number, prevSpeakerCount: number) => {
    if (running) return;
    setRunning(true);
    try {
      const count = await invoke<number>('rediarize_meeting', { meetingId, speakerCount });
      dispatch(baseApi.util.invalidateTags([{ type: 'Meeting', id: meetingId }]));
      if (count <= prevSpeakerCount) {
        toast.info(t('rediarizeNoChangeTitle'), {
          id: `rediarize-noop:${meetingId}`,
          description: t('rediarizeNoChangeBody'),
        });
      }
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
```

- [ ] **Step 5: Actualizar la llamada en `TranscriptTab.tsx`.** Calcular el nº de hablantes usados actuales y pasarlo. Cerca del `useMemo` de `byId` añadir:

```tsx
  const usedSpeakerCount = useMemo(
    () => new Set(lines.map((l) => l.speakerId)).size,
    [lines],
  );
```
Y en el `onClick` de confirmar del modal de re-diarizar, cambiar `void rediarize(reNValue)` por `void rediarize(reNValue, usedSpeakerCount)`.

- [ ] **Step 6: Correr el test, debe PASAR.**

Run: `pnpm test src/features/meeting-detail/useRediarize.test.ts`
Expected: PASS.

- [ ] **Step 7: Verificar tipos + commit.**

Run: `pnpm tsc -b` (o el check de tipos del proyecto)
```bash
git add src/features/meeting-detail/useRediarize.ts src/features/meeting-detail/useRediarize.test.ts src/features/meeting-detail/tabs/TranscriptTab.tsx src/i18n/locales/es.json src/i18n/locales/en.json src/i18n/keys.ts
git commit -m "feat(diarize): toast when re-diarize yields no additional speakers

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 6: Frontend — Fusionar hablantes (B1)

**Files:**
- Modify: `src/features/meeting-detail/tabs/TranscriptTab.tsx`
- Modify: `src/i18n/locales/es.json` + `en.json` (+ `pnpm generate:i18n-keys`)
- Test: `src/features/meeting-detail/tabs/TranscriptTab.merge.test.tsx` (crear)

**Contexto:** `useMergeSpeakersMutation` ya existe (`meetings.api.ts`) → `mergeSpeakers({ into, from })`. `merge_speakers` mueve las líneas, borra el hablante vacío y recomputa stats. La clave i18n `speaker.merge` ("Fusionar en…") ya existe; añadimos las del modal.

- [ ] **Step 1: Claves i18n.** `es.json`:
```json
  "speaker.mergeCta": "Fusionar hablantes",
  "speaker.mergeTitle": "Fusionar dos hablantes en uno",
  "speaker.mergeFromLabel": "Fusionar este…",
  "speaker.mergeIntoLabel": "…dentro de este",
```
`en.json`:
```json
  "speaker.mergeCta": "Merge speakers",
  "speaker.mergeTitle": "Merge two speakers into one",
  "speaker.mergeFromLabel": "Merge this…",
  "speaker.mergeIntoLabel": "…into this",
```
Luego `pnpm generate:i18n-keys`.

- [ ] **Step 2: Test (TDD)** en `TranscriptTab.merge.test.tsx`. Enfoque (igual que `useTranscription.test.tsx`): **mockear `invoke`** de `@tauri-apps/api/core` por comando (la mutation RTK Query pasa por ese transport) y renderizar con el **store real** en un `<Provider>`; NO mockear la mutation en sí.

```tsx
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { Provider } from 'react-redux';
import { describe, expect, it, vi, beforeEach } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { store } from '@/store';
import { TranscriptTab } from './TranscriptTab';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

// Mock TranscriptTab's invoke calls per-command: get_meeting_audio (useEffect)
// returns null; merge_speakers returns undefined.
function mockInvoke() {
  vi.mocked(invoke).mockImplementation((cmd: string) => {
    if (cmd === 'get_meeting_audio') return Promise.resolve(null);
    return Promise.resolve(undefined as never);
  });
}

// Minimal MeetingDetail fixture: 2 participants + 3 lines (2 of p1, 1 of p2).
// Fill required fields per src/ipc/bindings.ts MeetingDetail/Participant/TranscriptLine.
const meeting = {/* id: 'm-1', participants: [{id:'p1',...},{id:'p2',...}], transcript: [...] */} as any;

describe('TranscriptTab merge speakers', () => {
  beforeEach(() => { vi.clearAllMocks(); mockInvoke(); });

  it('merges the chosen source into the chosen target', async () => {
    render(<Provider store={store}><MemoryRouter><TranscriptTab meeting={meeting} /></MemoryRouter></Provider>);
    fireEvent.click(screen.getByText(/Fusionar hablantes|Merge speakers/));
    // pick from=p1, into=p2 in the two <select>s, then confirm
    // ...
    await waitFor(() =>
      expect(vi.mocked(invoke)).toHaveBeenCalledWith('merge_speakers', { into: 'p2', from: 'p1' }),
    );
  });
});
```
Casos: (a) botón visible con ≥2 participantes; (b) confirmar llama `invoke('merge_speakers', { into, from })`; (c) confirmar **deshabilitado** si origen == destino. El fixture `meeting` se completa con los campos requeridos por los tipos de `bindings.ts` (mirar `MeetingDetail`, `Participant`, `TranscriptLine`).

- [ ] **Step 3: Correr el test, debe FALLAR** (no existe el botón/modal aún).

Run: `pnpm test src/features/meeting-detail/tabs/TranscriptTab.merge.test.tsx`
Expected: FAIL.

- [ ] **Step 4: Implementar.** En `TranscriptTab.tsx`:
  - Importar `useMergeSpeakersMutation`.
  - `const [mergeSpeakers] = useMergeSpeakersMutation();`
  - Estado: `const [mergeOpen, setMergeOpen] = useState(false); const [mergeFrom, setMergeFrom] = useState(''); const [mergeInto, setMergeInto] = useState('');`
  - Botón en `cardHeadRight` (visible si `meeting.participants.length >= 2`): `<Button variant="default" onClick={() => setMergeOpen(true)}>{t('speaker.mergeCta')}</Button>`.
  - `<Modal open={mergeOpen} onClose={() => setMergeOpen(false)} title={t('speaker.mergeTitle')} footer={...}>` con dos `<select>` poblados de `meeting.participants` (usar `speakerLabel(p, lang)` como texto de cada opción), etiquetados con `speaker.mergeFromLabel` / `speaker.mergeIntoLabel`.
  - Confirmar: `disabled={!mergeFrom || !mergeInto || mergeFrom === mergeInto}`; onClick:
    ```tsx
    await mergeSpeakers({ into: mergeInto, from: mergeFrom });
    setMergeOpen(false); setMergeFrom(''); setMergeInto('');
    ```

- [ ] **Step 5: Correr el test, debe PASAR.**

Run: `pnpm test src/features/meeting-detail/tabs/TranscriptTab.merge.test.tsx`
Expected: PASS.

- [ ] **Step 6: Tipos + commit.**

Run: `pnpm tsc -b`
```bash
git add src/features/meeting-detail/tabs/TranscriptTab.tsx src/features/meeting-detail/tabs/TranscriptTab.merge.test.tsx src/i18n/locales/es.json src/i18n/locales/en.json src/i18n/keys.ts
git commit -m "feat(diarize): merge-speakers modal in the transcript tab

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 7: Frontend — Seleccionar todas las líneas de un hablante (B2)

**Files:**
- Modify: `src/features/meeting-detail/tabs/TranscriptTab.tsx`
- Modify: `src/i18n/locales/es.json` + `en.json` (+ `pnpm generate:i18n-keys`)
- Test: `src/features/meeting-detail/tabs/TranscriptTab.selectall.test.tsx` (crear)

**Contexto:** el modo selección (`selectMode`, `selected: Set<number>`) y la barra "Aplicar reasignación" ya existen. Solo añadimos un atajo que, en modo selección, marca todas las líneas de un hablante de un clic.

- [ ] **Step 1: Clave i18n.** `es.json`: `"speaker.selectAllOf": "Seleccionar sus líneas"`; `en.json`: `"speaker.selectAllOf": "Select their lines"`. Luego `pnpm generate:i18n-keys`.

- [ ] **Step 2: Test (TDD)** en `TranscriptTab.selectall.test.tsx`: con `selectMode` activo, al hacer clic en el chip del hablante `p1`, las casillas de las líneas de `p1` quedan marcadas (y las de `p2` no). Aserción sobre los checkboxes marcados.

- [ ] **Step 3: Correr, debe FALLAR.**

Run: `pnpm test src/features/meeting-detail/tabs/TranscriptTab.selectall.test.tsx`
Expected: FAIL.

- [ ] **Step 4: Implementar.** En `TranscriptTab.tsx`:
  - Helper: 
    ```tsx
    function selectAllOf(speakerId: string) {
      setSelected(new Set(lines.filter((l) => l.speakerId === speakerId).map((l) => l.id)));
    }
    ```
  - En el bloque `selectMode && (...)` (junto al toolbar de selección), renderizar una fila de chips, uno por `meeting.participants`, cada uno:
    ```tsx
    <button
      key={p.id}
      className={styles.selectAllChip}
      type="button"
      title={t('speaker.selectAllOf')}
      onClick={() => selectAllOf(p.id)}
    >
      {speakerLabel(p, lang)}
    </button>
    ```
  - Añadir una clase `.selectAllChip` mínima en `TranscriptTab.module.css` (reutilizar el estilo de chips existente si lo hay).

- [ ] **Step 5: Correr, debe PASAR.**

Run: `pnpm test src/features/meeting-detail/tabs/TranscriptTab.selectall.test.tsx`
Expected: PASS.

- [ ] **Step 6: Suite completa de tests frontend + backend + commit.**

Run: `pnpm test` (frontend) y `cargo test -p smart-noter-diarize` (backend)
Expected: todo verde.
```bash
git add src/features/meeting-detail/tabs/TranscriptTab.tsx src/features/meeting-detail/tabs/TranscriptTab.module.css src/features/meeting-detail/tabs/TranscriptTab.selectall.test.tsx src/i18n/locales/es.json src/i18n/locales/en.json src/i18n/keys.ts
git commit -m "feat(diarize): select-all-lines-of-a-speaker shortcut

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Verificación final (smoke, en *release*)

Tras todas las tareas, compilar el instalador release y validar sobre audio real (dev-debug crashea whisper/sherpa; correr desde el binario release):
1. Re-diarizar una grabación con voz dominante → **afloran más hablantes** (ya no colapsa a 1).
2. Re-diarizar una grabación sin más voces → **toast** "Sin hablantes nuevos".
3. **Fusionar** dos hablantes → se unen, el vacío desaparece, stats recomputadas.
4. **Seleccionar sus líneas** (modo selección) → marca las del hablante; "Aplicar reasignación" las mueve.
5. Confirmar que la diarización inicial de una grabación NUEVA sigue razonable (no regresión por el flatten).

No marcar el trabajo como completo hasta que el smoke pase (usar superpowers:finishing-a-development-branch para cerrar la rama).
