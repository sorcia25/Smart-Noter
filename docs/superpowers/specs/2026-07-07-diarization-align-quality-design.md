# Diarization Quality — Aflorar hablantes + corrección rápida — Diseño

**Goal:** Que la diarización (automática y con conteo forzado) deje de fusionar voces distintas en un solo hablante, y dar herramientas rápidas para pulir el resto a mano.

**Alcance:** una rama (continúa en `feat/transcript-diarization-quality`), un spec/plan/PR. Dos bloques:
- **A — Motor** (backend): arreglar el colapso de `align` + duración-cero + que `rediarize_meeting` devuelva el conteo.
- **B — Corrección en bloque** (frontend, backend ya existe): fusionar hablantes + seleccionar todas las líneas de un hablante + aviso de no-op.

**Tech Stack:** Rust (sherpa-rs diarize, SQLite/sqlx) · React/TS · Tauri 2.

---

## Contexto y causa raíz (CONFIRMADA)

El usuario reportó que al re-diarizar forzando el número de participantes, dos personas distintas quedaban como un solo hablante y "sin cambio alguno". Una investigación en vivo (app real, `RUST_LOG=debug`) descartó el pipeline:

- El conteo forzado **sí** llega al backend; `rediarize_meeting` **sí** re-diariza, re-persiste (`DELETE`+`INSERT` de participants/lines) y la UI **sí** refresca (el `invalidateTags([{type:'Meeting', id}])` de `useRediarize.ts:21` coincide con `getMeeting`'s `providesTags` en `meetings.api.ts:12`). Nada de esto es el bug.
- **sherpa honra el conteo:** correr `diarize` sobre el audio real con `num_speakers=Some(7)` produce más clusters que en auto (medido: auto→3, forzado-7→5 sobre dos grabaciones).

**El colapso ocurre en `align`** (`align.rs:68` `pick_speaker`), cuya regla es "gana el hablante con **mayor solapamiento total**". Cuando sherpa emite un hablante que **domina** la línea de tiempo (segmentos que la cubren casi entera) y las otras voces como **sub-segmentos cortos SOLAPADOS/anidados** dentro de ese span, cada línea de texto solapa más con el dominante → **todas se le asignan → colapsa a 1 hablante**.

Evidencia (meeting "Prueba 6", `m-20260616-18e52fbb`, forzado 7):
```
SHERPA: 3 distinct {0,4,5}
  [30..19690]S0  [20280..26220]S0  [27149..47770]S0      <- S0 cubre toda la línea de tiempo
  [39569..41205]S5  [41948..45525]S5  [44969..45357]S4   <- S5/S4 ANIDADOS dentro de [27149..47770]S0
ALIGN -> 1 distinct {0}   =>  replace_lines speaker_count = 1   => "sin cambio"
```
Contraste (meeting "40tnts2.0", `m-20260618-616ee38b`, forzado 7): sherpa produce 5 hablantes **espaciados** (no anidados) → `align` reparte a 3-4 → **sí** cambia. La diferencia es puramente la **topología de solapamiento** de los segmentos de sherpa.

**Defecto secundario (empeora el colapso) — intervalos de duración cero:** `segments_for` (`transcript_repo.rs:109`) mapea `end_seconds` NULL (transcripts viejos de Sub-3a) a `t1 = t0`; `rediarize_meeting` (`transcription.rs:599-600`) construye entonces `end_ms == start_ms`. `overlap_ms` (`align.rs:32`) devuelve 0 para un intervalo de punto → `align` degrada al fallback de "segmento más cercano por hueco". Afecta a los meetings con `end_seconds` NULL en todas sus líneas.

**La UI de corrección manual YA existe** y es amplia (descubierto al inspeccionar `TranscriptTab.tsx`): reasignación por línea (menú `⋮`) y **reasignación en bloque** (modo "Seleccionar líneas" → checkboxes → "Aplicar reasignación"), vía `useReassignLinesMutation` + `useCreateSpeakerMutation`. Además, **`merge_speakers` ya existe end-to-end** — comando `merge_speakers(into, from)` (`meetings.rs:73`) + repo atómico (`participants_repo.rs:98`: mueve líneas, borra el hablante vacío, recomputa stats) + `useMergeSpeakersMutation`. Lo único que falta de la corrección en bloque es **exponer la fusión en la UI** y un atajo de **seleccionar-todo-de-un-hablante**.

---

## Parte A — Motor (backend)

### A1 — Aplanar solapamientos antes de alinear (pieza central)

Nuevo módulo `src-tauri/crates/diarize/src/overlap.rs` con una función **pura** (sin sherpa, sin modelos, testeable directo):

```
flatten_overlaps(segments: &[DiarSegment]) -> Vec<DiarSegment>
```

- Convierte segmentos que se **pisan** en el tiempo en una **partición sin solapes** que cubre los mismos rangos.
- **Regla:** en cada tramo cubierto por más de un segmento, gana el de **menor duración original** (el sub-turno corto es más específico que el span que lo envuelve). Empates de duración → menor `speaker` (determinismo).
- **Algoritmo:** barrido (sweep) sobre los puntos de corte (todos los `start_ms`/`end_ms` ordenados y deduplicados); para cada intervalo elemental `[p_i, p_{i+1})`, elegir entre los segmentos que lo cubren el de menor duración original; fusionar intervalos elementales adyacentes con el mismo `speaker`. `n` es pequeño (~10-30 segmentos).
- **No-op** cuando no hay solapes: entrada ya particionada → salida idéntica (misma partición, mismos speakers). Esto garantiza **cero regresión** en diarización limpia (turnos secuenciales, caso presencial).

**Wire:** llamar `flatten_overlaps` al final de `diarize()` (`diarize.rs`), **después** de convertir sherpa-segundos → ms y **antes** de devolver. Así ambas rutas que consumen `diarize()` — la transcripción inicial (`transcribe_meeting`) y la re-diarización (`rediarize_meeting`) — reciben segmentos aplanados sin tocar `align`. `align` queda intacto.

### A2 — Duración-cero (líneas sin fin)

En `rediarize_meeting` (`transcription.rs`), al construir los `TextSegment` desde `segs` (lista ya ordenada por tiempo), si una línea queda con `end <= start`, usar como fin el **inicio de la siguiente** línea; para la **última** línea sin fin, usar la duración del `pcm` (`pcm.len() / 16_000`). Así toda línea tiene una duración real para el solapamiento. (El `pcm` ya está disponible en ese `spawn_blocking`.)

### A3 — `rediarize_meeting` devuelve el conteo resultante

Cambiar la firma de `rediarize_meeting` de `Result<(), AppError>` a `Result<u32, AppError>`, devolviendo `count` (nº de hablantes tras alinear). Lo consume la Parte B (aviso de no-op). Regenerar `bindings.ts`.

### A4 — Re-mapear speakers a labels contiguos (evita hablantes fantasma)

`align` puede devolver speaker labels **no contiguos** (sherpa usa p. ej. `{0,4,5}`), y `rediarize_meeting` deriva hoy `count = max(speaker)+1` (`transcription.rs:605`) → crea participantes **vacíos** (S2–S4 sin líneas). Al aflorar más hablantes con A1 este artefacto sería más visible. Re-mapear los speakers realmente usados a `[0..k)` contiguos (preservando el orden de primera aparición) antes de construir los `LineInput` y `count`, de modo que no queden hablantes fantasma y `count` = nº real de hablantes usados. (Nota: `transcribe_meeting` comparte el patrón de derivación; si el re-mapeo se factoriza como helper puro, aplicarlo también ahí en el plan si es trivial.)

---

## Parte B — Corrección en bloque (frontend; backend ya existe)

### B1 — Fusionar hablantes (UI)

`merge_speakers` + `useMergeSpeakersMutation` ya existen. Añadir un control en `TranscriptTab.tsx`: un botón "Fusionar hablantes" (junto a "Re-asignar hablantes" / "Seleccionar líneas") que abre un `Modal` con dos selectores — **fusionar [origen] dentro de [destino]** — poblados con `meeting.participants`, y un botón confirmar. Al confirmar: `mergeSpeakers({ into: destino, from: origen })`. Deshabilitar confirmar si origen == destino o falta alguno. (El `merge_speakers` del repo ya es no-op si `into == from`.)

### B2 — Seleccionar todas las líneas de un hablante (client-side)

En modo selección (`selectMode`), añadir por cada hablante un atajo (chip/botón con su nombre) que hace `setSelected(new Set(lines.filter(l => l.speakerId === sid).map(l => l.id)))`. Reutiliza la barra "Aplicar reasignación" existente. Sin backend nuevo.

### C — Aviso de no-op (frontend)

`useRediarize` recibe el `count` que ahora devuelve A3. El componente captura el nº de hablantes **previo** (distintos `speakerId` en `meeting.transcript` antes de re-diarizar) y, tras terminar, si el `count` nuevo **no aumentó**, muestra un toast informativo (no error): *"No se detectaron más voces; puedes ajustar a mano (fusionar / reasignar)."* Reutiliza el mecanismo de toast que `useRediarize` ya usa para errores.

---

## Manejo de errores / riesgos

- **`flatten_overlaps` — heurística "más corto gana":** en habla muy solapada podría favorecer un turno espurio muy corto. Mitigado porque (a) es estrictamente mejor que el colapso actual, (b) el smoke lo valida sobre audio real, (c) la corrección manual (fusionar/reasignar) es la red de seguridad. No se añade umbral de duración mínima salvo que el smoke lo pida (YAGNI).
- **Conteo forzado = "hasta N", no "exactamente N":** el resultado tendrá los hablantes que sherpa+flatten+align logren distinguir, que puede ser < N si el audio no da para más (típico en la mezcla mic+bocinas mono). El aviso de no-op (C) comunica esto.
- **Firma de `rediarize_meeting` cambia** → regenerar `bindings.ts` (gitignored; regenerar desde `src-tauri/`).
- **A1 afecta la diarización inicial de NUEVAS grabaciones** (las existentes no cambian hasta re-diarizarse). Riesgo bajo: `flatten_overlaps` es no-op en audio limpio.

## Pruebas

- **`diarize` crate (unit, sin modelos):** `flatten_overlaps` — (1) topología dominante+anidada estilo Prueba 6 → los sub-turnos afloran; (2) secuencial sin solapes → salida idéntica (no-op); (3) solape parcial de dos segmentos → el más corto gana el tramo compartido; (4) empate de duración → menor speaker. + un test de `align` sobre la salida aplanada que confirma que reparte.
- **Backend (unit):** duración-cero — `rediarize_meeting`'s construcción de `TextSegment` con `end<=start` sintetiza la duración desde la siguiente línea / fin del pcm. (Testear la función de construcción de forma aislada con fixtures de `segs`.)
- **Backend (unit):** re-mapeo a contiguos (A4) — speakers alineados `{0,4,5}` → `{0,1,2}` con `count=3` (sin huecos), preservando orden de primera aparición.
- **Frontend:** fusionar (mutation mockeada, modal, confirm deshabilitado si origen==destino), seleccionar-todo (marca las líneas del hablante), toast de no-op (count devuelto ≤ previo).

## Smoke (en *release*, no dev-debug)

1. Re-diarizar una grabación real con voz dominante (mezcla mic+bocinas) → afloran más hablantes que antes (ya no colapsa a 1).
2. Fusionar dos hablantes → se unen y desaparece el vacío; stats recomputadas.
3. Seleccionar-todo de un hablante → marca sus líneas; "Aplicar reasignación" las mueve.
4. Re-diarizar una grabación sin más voces distinguibles → aparece el toast de no-op.

## Archivos

- `src-tauri/crates/diarize/src/overlap.rs` (nuevo) — `flatten_overlaps` + tests.
- `src-tauri/crates/diarize/src/lib.rs` — exportar `overlap`.
- `src-tauri/crates/diarize/src/diarize.rs` — llamar `flatten_overlaps` al final de `diarize()`.
- `src-tauri/src/commands/transcription.rs` — `rediarize_meeting`: duración-cero (A2) + devolver `count` (A3).
- `src/features/meeting-detail/tabs/TranscriptTab.tsx` — fusionar (B1) + seleccionar-todo (B2).
- `src/features/meeting-detail/useRediarize.ts` — count → toast de no-op (C).
- `src/ipc/bindings.ts` — regenerar (firma de `rediarize_meeting`).
- `src/i18n/locales/{es,en}.json` — strings nuevos (fusionar, seleccionar-todo, aviso no-op).
```
