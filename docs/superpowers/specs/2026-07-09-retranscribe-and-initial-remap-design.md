# Re-transcribir + remap en la transcripción inicial — Diseño

**Goal:** Añadir un botón "Re-transcribir" (re-correr whisper sobre una reunión existente) y aplicar el re-mapeo de hablantes a contiguos también en la transcripción inicial (hoy solo lo hace la re-diarización).

**Alcance:** dos follow-ups acotados de calidad de transcripción, una rama, un spec/plan/PR (v1.2.2). Ambos ya identificados al cerrar v1.2.1.

**Tech Stack:** Rust (Tauri command) · React/TS · Tauri 2.

---

## Contexto (confirmado leyendo el código)

- **`transcribe_meeting`** (`transcription.rs`) transcribe (local whisper o cloud) → `segments`; si hay modelos de diarización, `diarize` + `align`; persiste con **`replace_lines`** (`transcription.rs:417`) — el MISMO helper atómico que `rediarize_meeting`, que **borra y recrea** el transcript (líneas + participantes). Por tanto **re-transcribir reemplaza limpio** todo el transcript, incluidas las correcciones manuales de hablantes.
- **`useTranscription.start(speakerCountHint?)`** ya llama `invoke('transcribe_meeting', { meetingId, speakerCountHint: speakerCountHint ?? null })` y maneja los eventos (`progress`/`completed`/`failed`/`cancelled`), el re-attach a un job en curso, y trata `TranscriptionBusy` como benigno.
- **`TranscriptTab.tsx`** ya tiene: el input de conteo (`reN`/`reNValue`, `number|null`), `hasAudio` (de `get_meeting_audio`), `status` (de `useTranscription`), y el control de re-diarizar. El botón "Transcribir" actual solo aparece cuando `lines.length === 0`.
- **`transcribe_meeting:357-360`** deriva `speaker_count = max(speaker)+1` y NO re-mapea (a diferencia de `rediarize_meeting`, ya arreglado en v1.2.1). `remap_contiguous` está exportado de `smart_noter_diarize`.

## Follow-up 1 — Botón "Re-transcribir"

**UI (`TranscriptTab.tsx`):** un botón "Re-transcribir" en `cardHeadRight` (junto a "Re-asignar hablantes"), visible con `lines.length > 0`.
- **Deshabilitado** si `!hasAudio` (tooltip explicando que requiere el audio) o si `status === 'running'`.
- Mientras corre, el botón muestra `t('retranscribeRunning')` + `pct` (p. ej. "Re-transcribiendo… 42%").
- **Al hacer clic:** abrir un `Modal` de confirmación (mismo patrón que el de re-diarizar) con el texto: *"Esto vuelve a transcribir desde el audio y **reemplaza el transcript completo**, incluidas las correcciones de hablantes. El audio no cambia."*
- **Confirmar:** `void start(reN === null ? null : reNValue)` — usa el input de conteo existente (vacío = auto-detecta la diarización; número = fuerza ese conteo). No se añade backend nuevo: reutiliza `transcribe_meeting` + `useTranscription`.

**Por qué reusa el flujo existente:** `transcribe_meeting` ya reemplaza vía `replace_lines`, ya emite progreso/eventos, y `useTranscription` ya los consume (incluido el refresh del cache RTK al `completed`). El botón solo es un nuevo punto de entrada + confirmación.

**No colisiona** con el auto-trigger (que exige `lines.length === 0`), ni con doble-clic (`TranscriptionBusy` es benigno).

**i18n:** `retranscribeCta` ("Re-transcribir"), `retranscribeRunning` ("Re-transcribiendo…"), `retranscribeNoAudio` ("Requiere el audio de la grabación"), `retranscribeConfirmTitle` ("Volver a transcribir"), `retranscribeConfirmBody`, `retranscribeConfirmCta` ("Re-transcribir").

## Follow-up 2 — Remap en la transcripción inicial

En `transcribe_meeting` (`transcription.rs:357-360`), reemplazar la derivación `max(speaker)+1` por el re-mapeo a contiguos, igual que `rediarize_meeting`:
```rust
let aligned = align(&texts, &diar_segs);
let raw: Vec<u32> = aligned.iter().map(|a| a.speaker).collect();
let (speakers, count) = smart_noter_diarize::remap_contiguous(&raw);
speaker_count = count;
speaker_idx = speakers.iter().map(|&s| s as usize).collect();
```
Elimina los hablantes fantasma (participantes vacíos) en grabaciones nuevas cuando sherpa emite labels no contiguos. **`fill_zero_durations` NO aplica aquí** — los `segments` vienen de whisper con `start_ms`/`end_ms` reales (no hay líneas de ancho cero como en la ruta de re-diarización, que las reconstruye desde segundos de la DB).

## Manejo de errores / riesgos

- **Re-transcribir sin audio:** el botón se deshabilita (mismo patrón que re-diarizar). Si el audio se borró, `transcribe_meeting` ya emite `failed` (toast).
- **Destructividad:** cubierta por el diálogo de confirmación (reemplaza correcciones manuales). Es la misma semántica que re-diarizar, un paso más (rehace también el texto).
- **Follow-up 2** cambia la diarización inicial de grabaciones NUEVAS (las existentes no cambian hasta re-transcribir/re-diarizar). Riesgo bajo: `remap_contiguous` es puro y ya testeado; el conteo pasa de "max+1" a "nº real de hablantes usados".

## Pruebas

- **Frontend (`TranscriptTab`):** el botón "Re-transcribir" aparece con transcript; deshabilitado sin audio; abre el modal; confirmar llama `transcribe_meeting` con `speakerCountHint` derivado del input (número cuando `reN` tiene valor, `null` cuando está vacío). Seguir el patrón de mock de los tests existentes (mock de `invoke`, store real).
- **Backend:** `remap_contiguous` ya tiene tests unitarios en el crate `diarize`; el cambio en `transcribe_meeting` es mecánico (el mismo patrón ya revisado en `rediarize_meeting`) → se valida con `cargo check` + el smoke.

## Smoke (release)

1. Abrir una reunión con transcript + audio → "Re-transcribir" → confirmar → el transcript se reemplaza (whisper corre, progreso visible, se refresca al terminar).
2. Re-transcribir con un número en el input → respeta el conteo forzado; con el input vacío → auto-detecta.
3. Grabar algo nuevo → la transcripción inicial no deja hablantes vacíos (remap).

## Archivos

- `src/features/meeting-detail/tabs/TranscriptTab.tsx` — botón "Re-transcribir" + modal de confirmación.
- `src-tauri/src/commands/transcription.rs` — remap en la diarización inicial (líneas ~357-360).
- `src/i18n/locales/{es,en}.json` — strings de re-transcribir (+ `pnpm generate:i18n-keys`).
