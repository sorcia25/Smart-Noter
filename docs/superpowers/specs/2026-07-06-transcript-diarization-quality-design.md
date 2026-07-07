# Transcript & Diarization Quality — Diseño

**Goal:** Arreglar que whisper corte los últimos 3-12s de algunos transcripts, y permitir al usuario forzar el número de hablantes de la diarización (proactivo al guardar + correctivo con una re-diarización rápida).

**Alcance:** dos mejoras independientes en una rama (`feat/transcript-diarization-quality`), un solo spec/plan/PR:
- **A** — fix del corte de cola de whisper (bug, causa raíz confirmada).
- **B** — input de número de participantes + re-diarización solo (feature).

**Tech Stack:** Rust (whisper-rs/whisper.cpp, sherpa-rs diarize, SQLite) · React/TS · Tauri 2.

---

## Contexto

Ambos surgieron durante el smoke de v1.2. Ninguno es bug del AEC, pero **la supresión de ruido del AEC nativo de v1.2 AMPLIFICA A** (atenúa la voz baja del final → dispara más seguido el gate de no-speech de whisper). El usuario confirmó que la grabación tiene la cola completa (el corte es de transcripción, no de captura).

---

## Parte A — Corte de cola de whisper

### Causa raíz (CONFIRMADA contra el source vendored de whisper.cpp)

El gate `is_no_speech` (whisper.cpp:7585-7603): por cada ventana de decodificación de 30s, si `no_speech_prob > no_speech_thold` (default **0.6**) **Y** `avg_logprobs < logprob_thold` (**-1.0**), se **descarta TODO el texto de esa ventana** (nunca entra a `result_all`, así que nunca llega al loop de extracción en `transcribe.rs`). La última ventana con voz baja al final cae en el gate → se pierden 3-12s. Variable porque depende de dónde queda el habla dentro de esos últimos ≤30s.

- `transcribe.rs` está **idéntico desde Sub-3a** → es comportamiento latente de whisper.cpp, no regresión nuestra.
- **Amplificador:** la ruta local fuerza `language="auto"` (`TranscribeOpts::default` → `set_language(Some("auto"))`) aunque `settings.native_language="es"`; el idioma nativo **solo** se pasa en la ruta nube (`transcription.rs:294`), nunca local. Auto-detect + baja confianza en cola en español hace más fácil cruzar el `avg_logprobs < -1.0`.
- **Descartado por el agente:** abort temprano (el trampoline lee un `AtomicBool` que sigue false; un abort saldría como `Cancelled`), VAD (off por default), token_timestamps (off), el loop de Rust (correcto), decode (lee el WAV completo).

### Fix (2 cambios chicos)

1. **Relajar el gate** — en `transcribe.rs`, `FullParams::set_no_speech_thold(0.9)` (agregar campo a `TranscribeOpts` con default 0.9, o setearlo directo). La cola callada ya no se descarta. El skip existente de `text.is_empty()` (transcribe.rs:140) evita emitir líneas vacías; alucinación en silencio real es improbable (el gate solo afecta ventanas que SÍ decodificaron texto).
2. **Forzar idioma nativo en la ruta local** — pasar `settings.native_language` a `TranscribeOpts.language` en `transcription.rs:234` en vez de `TranscribeOpts::default()` (que da "auto"). Sube la confianza en español; independientemente correcto (auto-detect por-grabación es un desperdicio cuando hay idioma nativo configurado).
3. **Instrumentación de verificación** (log permanente a nivel `tracing::info`): tras `state.full(...)` (~transcribe.rs:129), loguear `full_n_segments`, el `end_ms` del último segmento, y `pcm.len() as f64 / 16000.0 * 1000.0`. En el smoke confirmar que `last_end_ms ≈ pcm_duration`.

### Archivos (A)
- `src-tauri/crates/whisper/src/transcribe.rs` — `TranscribeOpts` (+ campo `no_speech_thold`), `FullParams` config, log.
- `src-tauri/src/commands/transcription.rs:234` — pasar `settings.native_language` a la ruta local.

---

## Parte B — Input de participantes + re-diarización

### B1 — Input al guardar (cero backend nuevo)

En `StopConfirmModal.tsx`: agregar un input numérico **opcional** "¿Cuántos participantes?" (vacío = auto; rango 1-10). Su valor → `speakerHint` en el nav-state → `transcribe_meeting(speakerCountHint)` → `DiarizeOpts { num_speakers: Some(N) }` → `num_clusters=N`. **El plumbing ya existe** (`StopConfirmModal` ya navega con `{ justRecorded, speakerHint }`); solo hay que poblar `speakerHint` desde el input.

### B2 — Re-diarización solo (comando backend nuevo + control en el transcript)

**Comando nuevo `rediarize_meeting(meetingId: String, speaker_count: u32) -> Result<(), AppError>`** en `transcription.rs`:
- Resolver el WAV de la reunión (misma resolución que `get_meeting_audio`); si no existe → error `AudioMissing` (audio borrado).
- Decodificar el WAV → `pcm` (reutiliza `decode`).
- Cargar los segmentos del transcript existente de la DB (`transcript_lines`: `start_ms`/`end_ms`/`text`); si no hay transcript → error.
- Exigir modelos de diarización presentes (igual que `transcribe_meeting`); si no → `diarization:degraded`.
- `diarize(pcm, seg_model, emb_model, DiarizeOpts { num_speakers: Some(speaker_count) })` + `align(texts, diar_segs)` → nuevo `speaker_idx`. **No re-corre whisper** (rápido, conserva el texto).
- Actualizar la DB de forma **atómica** (transacción): resetear los hablantes de la reunión (crear N participantes `Sujeto 1..N` + re-asignar todas las líneas). **Reemplaza** las asignaciones previas (incluidas reasignaciones/renombres manuales).
- Emitir eventos de progreso/fin (reusar `transcription:progress`/`completed` o `rediarize:*`) para que el UI refresque + invalide el cache RTK del `Meeting`.

**Control en `TranscriptTab.tsx`** — "Re-asignar hablantes" (input N + botón):
- Al hacer clic: **siempre** mostrar un diálogo de confirmación ("Esto rehace la diarización y reemplaza las asignaciones de hablante actuales, incluidas correcciones manuales"). Simple e inequívoco (re-diarizar es destructivo). *Opcional (solo si el modelo ya expone una señal fácil de "línea/hablante tocado manualmente"): volverlo condicional a que existan cambios manuales — a decidir en el plan tras inspeccionar el repo de DB; si no hay señal, se queda en siempre-confirmar.*
- Llama `rediarize_meeting(meetingId, N)`; muestra estado "corriendo"; al terminar invalida el query del meeting (refresca el transcript).
- **Deshabilitado** si el audio no está disponible (sin WAV), con tooltip que lo explica.

### Decisión de diseño (aprobada)
Re-diarizar es una acción explícita de "rehacer la diarización" → **resetea** todas las asignaciones de hablante, protegida por un diálogo de confirmación (siempre; ver arriba).

### Archivos (B)
- `src/features/live-recording/StopConfirmModal/StopConfirmModal.tsx` (+ test) — input al guardar.
- `src-tauri/src/commands/transcription.rs` — comando `rediarize_meeting`.
- `src-tauri/crates/db/` — método de repo para reset+reasignar hablantes de una reunión (transacción) + query de segmentos si no existe.
- `src/features/meeting-detail/tabs/TranscriptTab.tsx` — control de re-asignar (+ detección de cambios manuales para el confirm).
- `src/ipc/bindings.ts` — regenerar (comando nuevo, gitignored).
- `src/i18n/locales/{es,en}.json` — strings nuevos.

---

## Manejo de errores
- **Sin WAV** (audio borrado) → `rediarize_meeting` regresa error; el control se deshabilita + tooltip.
- **Modelos de diarización ausentes** → mismo path `diarization:degraded` (toast).
- **Sin transcript** en la reunión → error/no-op (el control solo aparece con transcript).
- **Cancelación** → reusar el patrón de abort (`AtomicBool`) si aplica.
- La actualización de la DB en `rediarize_meeting` es **transaccional** (reset + reasignación juntas o nada).

## Pruebas
- **Frontend:** `StopConfirmModal` — el input pobla `speakerHint` en el nav-state; `TranscriptTab` — el control de re-asignar (comando mockeado) + el diálogo de confirmación cuando hay cambios manuales.
- **Backend:** `rediarize_meeting` — la lógica de reset+reasignación en la DB es testeable con fixtures (segmentos + un `speaker_idx` dado); la diarización en sí (sherpa, necesita modelos) se valida en el smoke.
- **Smoke:** A — re-transcribir las reuniones problema y confirmar que la cola regresa (vía la instrumentación `last_end_ms ≈ pcm_duration`). B — grabar → fijar N al guardar → mejor diarización; y re-diarizar una reunión existente con N corregido → mejor split de voces.

## Riesgos
- **A:** relajar `no_speech_thold` podría emitir una línea alucinada en una cola genuinamente silenciosa (mitigado por el skip de `text.is_empty()` + el gate solo afecta ventanas que decodificaron texto). El smoke valida.
- **B:** re-diarizar resetea cambios manuales (mitigado por el diálogo de confirmación). El reset+reasignación en DB debe ser atómico.
