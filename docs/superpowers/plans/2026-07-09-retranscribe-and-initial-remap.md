# Re-transcribir + remap inicial — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Botón "Re-transcribir" (re-corre whisper sobre una reunión existente, reusando `transcribe_meeting` + el input de conteo) y aplicar `remap_contiguous` en la diarización de la transcripción inicial (hoy `max+1`).

**Architecture:** El botón es un nuevo punto de entrada al flujo existente (`useTranscription.start` → `transcribe_meeting`, que ya reemplaza vía `replace_lines`); solo se añade UI + confirmación. El remap es un cambio mecánico de 3 líneas en `transcribe_meeting`, con el helper puro ya existente y testeado.

**Tech Stack:** Rust (Tauri command) · React/TS (RTK Query, vitest) · Tauri 2.

**Preamble para cargo/git (OBLIGATORIO; correr cargo desde `src-tauri/`):**
```bash
export PATH="$HOME/.cargo/bin:/c/Program Files (x86)/Microsoft Visual Studio/2022/BuildTools/Common7/IDE/CommonExtensions/Microsoft/CMake/CMake/bin:$PATH"
export LIBCLANG_PATH="C:/Program Files/LLVM/bin"
```

---

## File Structure

- **Modify:** `src-tauri/src/commands/transcription.rs` — remap en la diarización inicial (`~357-360`).
- **Modify:** `src/features/meeting-detail/tabs/TranscriptTab.tsx` — botón "Re-transcribir" + modal de confirmación.
- **Modify:** `src/i18n/locales/es.json` + `en.json` — strings de re-transcribir (+ `pnpm generate:i18n-keys`).
- **Test:** `src/features/meeting-detail/tabs/TranscriptTab.retranscribe.test.tsx` (crear).

---

## Task 1: Remap en la transcripción inicial (backend)

**Files:**
- Modify: `src-tauri/src/commands/transcription.rs`

**Contexto:** en `transcribe_meeting`, tras `align`, el bloque en `~357-360` deriva el conteo como `max(speaker)+1` y usa los speakers crudos:
```rust
let aligned = align(&texts, &diar_segs);
let max_spk = aligned.iter().map(|a| a.speaker).max().unwrap_or(0);
speaker_count = (max_spk as usize) + 1;
speaker_idx = aligned.iter().map(|a| a.speaker as usize).collect();
```
`speaker_count` y `speaker_idx` ya están declarados antes (`let mut speaker_count = 1usize;` / `let mut speaker_idx: Vec<usize> = vec![0; segments.len()];`). `smart_noter_diarize::remap_contiguous(&[u32]) -> (Vec<u32>, usize)` ya existe y está testeado.

- [ ] **Step 1: Reemplazar la derivación por el remap.** Mantener el `let aligned = align(&texts, &diar_segs);` existente **sin cambios** y reemplazar SOLO las tres líneas siguientes (`let max_spk = …;`, `speaker_count = …;`, `speaker_idx = …;`) por:

```rust
// Remap non-contiguous sherpa labels to [0..k) so first-time transcription
// doesn't leave phantom empty participants (mirrors rediarize_meeting).
let raw: Vec<u32> = aligned.iter().map(|a| a.speaker).collect();
let (speakers, count) = smart_noter_diarize::remap_contiguous(&raw);
speaker_count = count;
speaker_idx = speakers.iter().map(|&s| s as usize).collect();
```
(`fill_zero_durations` NO se usa aquí: los `segments` vienen de whisper con `start_ms`/`end_ms` reales. No re-declarar `aligned`.)

- [ ] **Step 2: Compilar.**

Run: `cargo check -p smart-noter`
Expected: OK, sin warnings nuevos (p. ej. no debe quedar un `max_spk` sin usar).

- [ ] **Step 3: Commit.**

```bash
git add src-tauri/src/commands/transcription.rs
git commit -m "feat(transcribe): remap speaker labels in initial diarization (no phantom participants)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 2: Botón "Re-transcribir" + modal (frontend)

**Files:**
- Modify: `src/features/meeting-detail/tabs/TranscriptTab.tsx`
- Modify: `src/i18n/locales/es.json` + `en.json`
- Test: `src/features/meeting-detail/tabs/TranscriptTab.retranscribe.test.tsx` (crear)

**Contexto (`TranscriptTab.tsx`, ya en el archivo):** `const { status, pct, start, cancel } = useTranscription(meeting.id);`; el input de conteo `reN` (`number|null`) con `reNValue = Math.max(1, Math.min(10, reN ?? 1))`; `hasAudio` (de un `get_meeting_audio` effect); el header `cardHeadRight` (dentro de `lines.length > 0`) con los botones de re-diarizar / fusionar / seleccionar; y un `<Modal open={confirmOpen} …>` de re-diarizar al final del JSX (patrón a espejar). `Modal` y `Button` ya están importados. `start(speakerCountHint?)` llama `transcribe_meeting` con `speakerCountHint ?? null`.

- [ ] **Step 1: i18n.** En `es.json` (junto a las claves `rediarize*`):
```json
  "retranscribeCta": "Re-transcribir",
  "retranscribeRunning": "Re-transcribiendo…",
  "retranscribeNoAudio": "Requiere el audio de la grabación",
  "retranscribeConfirmTitle": "Volver a transcribir",
  "retranscribeConfirmBody": "Esto vuelve a transcribir desde el audio y reemplaza el transcript completo, incluidas las correcciones de hablantes. El audio no cambia.",
  "retranscribeConfirmCta": "Re-transcribir",
```
En `en.json`:
```json
  "retranscribeCta": "Re-transcribe",
  "retranscribeRunning": "Re-transcribing…",
  "retranscribeNoAudio": "Requires the recording audio",
  "retranscribeConfirmTitle": "Re-transcribe",
  "retranscribeConfirmBody": "This re-transcribes from the audio and replaces the entire transcript, including speaker corrections. The audio is unchanged.",
  "retranscribeConfirmCta": "Re-transcribe",
```
Luego: `pnpm generate:i18n-keys` (regenera el gitignored `keys.ts` — NO commitear).

- [ ] **Step 2: Test (TDD)** en `TranscriptTab.retranscribe.test.tsx`, reutilizando el setup de `TranscriptTab.merge.test.tsx` (mock de `invoke` por comando — `get_meeting_audio` devuelve un objeto de audio para que `hasAudio` sea true; `transcribe_meeting` devuelve undefined — store real, fixture `MeetingDetail` con transcript y participantes). El test:
  - Renderiza el tab, hace clic en "Re-transcribir" (`t('retranscribeCta')`) → abre el modal.
  - Confirma → asserts `invoke` fue llamado con `('transcribe_meeting', { meetingId: 'm-1', speakerCountHint: 2 })` (con `reN` en su default 2). Usar `within(screen.getByRole('dialog'))` para el botón de confirmar (comparte texto con el CTA del header).
  - (Opcional) con `get_meeting_audio` → null, el botón "Re-transcribir" queda deshabilitado.

Run: `pnpm test:run src/features/meeting-detail/tabs/TranscriptTab.retranscribe.test.tsx`
Expected: FAIL (el botón no existe aún).

- [ ] **Step 3: Implementar.** En `TranscriptTab.tsx`:
  - Estado nuevo (junto a `confirmOpen`): `const [retranscribeOpen, setRetranscribeOpen] = useState(false);`
  - En `cardHeadRight`, junto al botón de re-diarizar, añadir:
    ```tsx
    <Button
      variant="default"
      disabled={!hasAudio || status === 'running'}
      title={hasAudio ? undefined : t('retranscribeNoAudio')}
      onClick={() => setRetranscribeOpen(true)}
    >
      {status === 'running' ? `${t('retranscribeRunning')} ${pct}%` : t('retranscribeCta')}
    </Button>
    ```
  - Añadir el modal (espejando el de `confirmOpen`), al final del JSX junto a los otros modales:
    ```tsx
    <Modal
      open={retranscribeOpen}
      onClose={() => setRetranscribeOpen(false)}
      title={t('retranscribeConfirmTitle')}
      footer={
        <>
          <Button variant="default" onClick={() => setRetranscribeOpen(false)}>
            {t('cancel')}
          </Button>
          <Button
            variant="primary"
            onClick={() => {
              setRetranscribeOpen(false);
              void start(reN === null ? null : reNValue);
            }}
          >
            {t('retranscribeConfirmCta')}
          </Button>
        </>
      }
    >
      {t('retranscribeConfirmBody')}
    </Modal>
    ```

- [ ] **Step 4: Run the test, expect PASS.**

Run: `pnpm test:run src/features/meeting-detail/tabs/TranscriptTab.retranscribe.test.tsx`
Expected: PASS.

- [ ] **Step 5: Full suite + typecheck + commit.**

Run: `pnpm test:run` y `pnpm exec tsc --noEmit`
Expected: todo verde.
```bash
git add src/features/meeting-detail/tabs/TranscriptTab.tsx src/features/meeting-detail/tabs/TranscriptTab.retranscribe.test.tsx src/i18n/locales/es.json src/i18n/locales/en.json
git commit -m "feat(transcribe): Re-transcribe button (re-run whisper on an existing meeting)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```
(NO commitear `src/i18n/keys.ts` — gitignored.)

---

## Verificación final (smoke, en *release*)

1. Reunión con transcript + audio → "Re-transcribir" → confirmar → el transcript se reemplaza (whisper corre, progreso en el botón, se refresca al terminar).
2. Con un número en el input de conteo → respeta el forzado; input vacío → auto-detecta.
3. Grabar algo nuevo → la transcripción inicial no deja hablantes vacíos (remap).

Cerrar con superpowers:finishing-a-development-branch → bump 1.2.2 + PR + CI + merge + tag + release.
