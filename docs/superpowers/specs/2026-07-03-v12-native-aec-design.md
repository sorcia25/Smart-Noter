# v1.2 — AEC nativo de Windows (drift-aware) — Diseño

**Goal:** Cancelar el eco de las bocinas en las grabaciones en modo Mix delegando la cancelación al AEC nativo de Windows (Voice Clarity, modo Communications), de modo que el clock drift loopback↔mic —que derrotó al cancelador lineal custom de v1.1— lo maneje el SO.

**Arquitectura:** Capturar el micrófono con WASAPI raw en modo `AudioCategory_Communications` (crate `windows` 0.59), fijando el endpoint de render del loopback como referencia del AEC vía `IAcousticEchoCancellationControl::SetEchoCancellationRenderEndpoint`. El SO entrega un stream de mic ya cancelado (near-end) con el drift compensado internamente; nosotros lo mezclamos con el far-end del loopback intacto. El mixer/writer/meter y toda la ruta de loopback quedan sin cambios: la fuente nueva del mic alimenta el mismo canal `Sender<Vec<f32>>`.

**Tech Stack:** Rust · Tauri 2 · crate `windows` 0.59 (`Win32_Media_Audio`) · WASAPI (`IAudioClient`, `IAudioCaptureClient`, `IAcousticEchoCancellationControl`) · el crate `wasapi` 0.16 (loopback, sin cambios) · `cpal` (ruta AEC-off) · React/TS (toggle).

---

## 1. Contexto y motivación

v1.1 implementó un AEC lineal custom (`aec-rs`/SpeexDSP a 16 kHz en el carril del mic, referencia = loopback). **Falló en hardware real:** el loopback (WASAPI) y el mic (cpal) corren en relojes de hardware independientes que driftean ~0.09 %, así que el retardo del eco se mueve (medido +56 → +70 ms en 25 s) y el cancelador lineal nunca engancha (ERLE oscilando 6–20 dB, eco audible). Tres intentos de compensación (preprocess, delay-comp fijo, drift-comp adaptativa por cross-correlación) chocaron con el mismo muro. El AEC se difirió a v1.2 dormido (código intacto, `aec_enabled` default OFF, toggle oculto).

**Investigación de v1.2 (2026-07-03, multiagente, fuentes primarias):**

- **WebRTC AEC3** *detecta* drift (`ClockdriftDetector`) pero **no lo compensa** — solo aprieta su supresor no-lineal (enmascara, no cancela). El fix recomendado por los propios mantenedores de WebRTC es **resamplear ambas señales a un reloj común ANTES del cancelador**. Además ningún crate de Rust lo buildea en MSVC de forma turnkey (tonarino = solo Linux/macOS; la lib C++ vía CMake de get-wrecked está sin mantenimiento desde M124 + parches manuales). **Ruta descartada.**
- **Windows ya trae el AEC que necesitamos: "Voice Clarity"** — un APO de cancelación de eco + supresión de ruido + realce de voz por software, **activado por defecto**, sin hardware especial, disponible para cualquier app que abra su captura en modo `AudioCategory_Communications`. Es el mismo AEC en el que se apoyan Teams/Zoom. **Como el SO es dueño de ambos relojes, el motor de audio le entrega al cancelador frames de mic y de referencia ya timestamped y resampleados → el drift es problema del SO.** Esto retira el problema de raíz.

## 2. Decisión de enfoque

| | **Ruta A — AEC nativo Windows (ELEGIDA)** | Ruta B — resampler + SpeexDSP | Ruta C — WebRTC AEC3 |
|---|---|---|---|
| Drift | ✅ lo maneja el SO | ✅ lo manejamos nosotros | ✅ (re-resolviendo lo que A da gratis) |
| Código | Poco (mic WASAPI + ~15 líneas AEC) | Medio (resampler adaptativo estable) | Mucho (FFI a lib C++ sin CI + parches) |
| Build MSVC | ✅ nativo | ✅ ya funciona | ❌ hostil / sin crate |
| Riesgo | **Bajo** | Medio | Alto |

**Elegida: Ruta A.** Menos código, retira el drift de raíz, tecnología de producción probada, sin C++. `IAcousticEchoCancellationControl` existe desde build 22621 (22H2); la máquina objetivo corre build 26200 (25H2), holgado; expuesto directo en `windows` 0.59.

**Ruta B queda como fallback en la repisa** (el `EchoCanceller` SpeexDSP + la receta de drift-estimation por timestamps de dispositivo quedan documentados) por si el smoke de A revela que los efectos del modo Communications son inaceptables. Ruta C se descarta por completo.

## 3. Arquitectura y flujo de datos

**Principio:** cambia solo *de dónde* salen las muestras del mic. El mixer, writer, meter, waveform, silence-fill y todo el loopback son agnósticos a la fuente y no se tocan.

**Con AEC activado (Mix + bocinas):**

```
┌─ Carril A (sistema) ──────────────────────────────┐
│  WASAPI loopback  →  48k mono  ─────────────┐     │   (SIN CAMBIOS)
│  (endpoint de render X)                     │     │
└──────────────┬──────────────────────────────┼─────┘
               │ su endpoint-ID                │
               │ = referencia del AEC          ▼
               ▼                            ┌────────┐
┌─ Carril B (mic) — NUEVO ──────────┐      │ Mixer  │→ writer → WAV
│  IAudioClient (windows 0.59)      │      │ (sync +│
│  · SetClientProperties(           │      │  mezcla)│  = tu voz limpia
│      eCategory = Communications)  │      └────────┘    + far-end
│  · Initialize (mix format, 48k)   │         ▲          (sin doblado)
│  · GetService(IAcousticEcho…) →   │         │
│    SetEchoCancellationRender-     │  mic YA cancelado
│    Endpoint(endpoint-ID de X)     │─────────┘
│  El SO cancela el eco + maneja    │
│  el drift (dueño de ambos relojes)│
└───────────────────────────────────┘
```

El SO quita del mic exactamente el audio del endpoint de render X (el far-end que sale por las bocinas), con drift compensado. Nosotros re-sumamos el far-end limpio desde el loopback. Resultado: **tu voz + far-end, sin eco doblado.**

**Con AEC desactivado (audífonos):** el Carril B usa el `cpal` actual (mic crudo, 48k completo, cero procesamiento). Se conservan **ambas** rutas de captura del mic y se elige según el toggle.

**Enganches:**
- La **referencia del AEC = el mismo endpoint de render que ya resolvemos para el loopback** (incluido el sentinel `DEFAULT_RENDER_LOOPBACK` resuelto al abrir). Ya está disponible vía `resolve_render_device`.
- **Ganancia vs v1.1:** el mic queda a **48 kHz completo** (el modo Communications usa `GetMixFormat`, no fuerza 16 kHz). v1.1 band-limitaba la voz a 16 kHz.

## 4. Secuencia exacta de llamadas (WASAPI + COM)

Verificada contra el sample oficial de Microsoft (`Windows-classic-samples/Samples/AcousticEchoCancellation`) y la referencia de la API. Orden obligatorio:

1. **Elegir el endpoint de render de referencia.** El ID string del endpoint de render X (el que se loopback-captura). `NULL` = deja que Windows auto-elija el render por defecto.
2. **Activar el `IAudioClient` de captura (mic):** `mmDevice->Activate(IAudioClient, CLSCTX_INPROC_SERVER, ...)`.
3. **Fijar la categoría a Communications ANTES de `Initialize`** (esto jala el APO de AEC a la cadena de captura):
   ```
   AudioClientProperties props = { .cbSize = sizeof(...), .eCategory = AudioCategory_Communications };
   audioClient->SetClientProperties(&props);   // debe preceder a Initialize
   ```
4. **Usar el mix format; NO usar RAW** (RAW omite todo el procesamiento adaptativo, incluido el AEC):
   ```
   audioClient->GetMixFormat(&wfx);   // típico 48 kHz float, NO forzado a 16k/mono
   audioClient->Initialize(SHARED, EVENTCALLBACK, dur, 0, wfx, NULL);
   ```
5. **Tras `Initialize`, pedir el control de AEC y fijar la referencia:**
   ```
   if SUCCEEDED(audioClient->GetService(&IAcousticEchoCancellationControl aec)) {  // E_NOINTERFACE = no fatal
       aec->SetEchoCancellationRenderEndpoint(renderEndpointId);   // NULL = auto-elige
   }
   ```
6. `audioClient->Start()`, luego leer el mic **ya cancelado** de `IAudioCaptureClient`.

**Firma en `windows` 0.59:** `unsafe fn SetEchoCancellationRenderEndpoint<P0>(&self, endpointid: P0) -> Result<()> where P0: Param<PCWSTR>` (`ID` inválido → `E_INVALIDARG`; `NULL` → auto). Interfaz en `windows::Win32::Media::Audio`, header `audioclient.h`, mínimo build 22621.

## 5. Componentes y estructura de archivos

**La clave del encaje:** el nuevo mic alimenta el **mismo canal `Sender<Vec<f32>>`** que hoy usa `cpal`. Río abajo no se toca nada.

**➕ Crear — `src-tauri/crates/audio/src/capture/mic_comms.rs`** (única pieza nueva de verdad):
- Abre `IAudioClient` del endpoint de mic con el crate `windows` 0.59.
- `SetClientProperties(Communications)` antes de `Initialize`.
- `GetMixFormat` → `Initialize` shared + event-driven.
- `GetService::<IAcousticEchoCancellationControl>()` → `SetEchoCancellationRenderEndpoint(ref)`. `E_NOINTERFACE` no-fatal.
- Loop de captura por evento → decode f32 → downmix mono → resample a 48 kHz (reutiliza el helper existente) → al canal. Espeja el patrón de `wasapi_loopback_loop` en `stream.rs`.
- Expone el flag de stop y una vía para re-fijar la referencia (o usa `NULL` en follow-default, ver §6).

**✏️ Modificar:**
- `src-tauri/crates/audio/src/capture/recorder.rs` — al lanzar el Carril B, elegir fuente según `aec_enabled`: `mic_comms` (AEC on) vs `open_mic`/`cpal` (off). Pasarle la referencia (ID de render o `NULL`). **Quitar** la llamada `mixer.enable_aec(...)`.
- `src-tauri/crates/audio/src/capture/stream.rs` — hospedar/exponer el spawn del nuevo mic junto a `open_mic`.
- `src/features/pre-record/PreRecordPage.tsx` — **des-ocultar** el toggle "Cancelar eco de bocinas" (v1.1 lo escondió).
- `src/features/pre-record/PreRecordPage.test.tsx` — invertir el test de v1.1: el toggle está **visible** y graba con `aecEnabled=true` cuando está on.
- `src-tauri/src/commands/audio.rs` + `src-tauri/crates/core/src/models/settings.rs` — `aec_enabled` ya está threaded; se mantiene. El default de `aec_enabled` sigue **false** durante el desarrollo y se voltea a **true** en el commit de ship (post-smoke).

**🚫 Sin tocar:** `mixer.rs`, `writer.rs`, `meter.rs`, `session.rs`, y toda la ruta de loopback — agnósticos al origen del mic.

**🗑️ Remoción diferida (post-smoke):** `capture/echo_canceller.rs` + deps `aec-rs`/`aec-rs-sys` en `Cargo.toml` + la integración `enable_aec`/`a_delayed` del mixer. Se dejan dormidos durante el desarrollo de A como fallback de la Ruta B; se remueven en un commit de limpieza **solo cuando el smoke de A confirme** que funciona.

**Deps:** el crate `windows` 0.59 ya está en el árbol; asegurar las features `Win32_Media_Audio` / `Win32_System_Com` / `Win32_Foundation`.

## 6. Referencia del AEC (auto-seguimiento)

- **Follow default** (sentinel "lo que suene"): `SetEchoCancellationRenderEndpoint(NULL)` → el SO sigue el render por defecto **solo** → *cero* coordinación con el device-following de v1.1.
- **Endpoint fijado** por el usuario: pasar el ID explícito. Si ese endpoint desaparece → re-fijar o caer a `NULL`, con log.

## 7. Política de efectos

El modo Communications trae la cadena completa: **AEC + supresión de ruido + AGC + realce de voz (Voice Clarity)**. Decisión de v1.2: **abrazar la cadena completa tal cual, sin toggles por-efecto (YAGNI).** Para una app de transcripción + resumen la limpieza extra es neto positivo (mejor Whisper); el far-end de la mezcla viene del loopback sin procesar (solo se procesa la porción del mic). Salida de escape si el smoke revela sobre-procesamiento: `IAudioEffectsManager` + `AUDIO_EFFECT_TYPE_*` para apagar efectos individuales — diferido, solo si hace falta.

## 8. Manejo de errores y bordes

Arreglando el nit de v1.1 (nunca grabar sin cancelar mientras el toggle dice "on"):
- `GetService(IAcousticEchoCancellationControl)` → **`E_NOINTERFACE`**: no-fatal. El APO de AEC igual corre; el SO auto-elige referencia. Log + seguir.
- **No se puede abrir el cliente comms del mic** (`SetClientProperties`/`Initialize` fallan): fallback a `cpal` crudo **+ toast al usuario** ("AEC no disponible, grabando mic sin cancelación"). Nunca silencioso.
- **Formato**: `GetMixFormat` no-48k / multicanal → downmix+resample lo absorbe (igual que el loopback).
- **Device-following**: referencia `NULL` se auto-sigue; endpoint fijado que desaparece → re-fijar o `NULL`, con log.
- **Stop/teardown**: espeja el patrón de flag del hilo de loopback.

**Alcance del toggle:** el AEC solo aplica en modo **Mix** (mic + sistema). Se queda en la tarjeta de Mix, como en v1.1. Mic-only y System-only no lo ofrecen.

## 9. Pruebas y validación

El AEC del SO es dependiente del hardware/SO → **no unit-testeable**; la validación real es el smoke. Lógica pura que sí se cubre con tests (TDD):

**Unit tests (CI):**
- Selección de fuente del mic: `(aec_enabled, capture_mode)` → comms-mode vs `cpal`.
- Resolución de referencia: follow-default → `NULL`; endpoint fijado → ID explícito.
- Fallback: comms-open falla → `cpal` + señal al usuario (mockeando el resultado de apertura).
- Downmix/resample del mic comms → reutiliza helpers ya testeados.
- Frontend (`PreRecordPage.test.tsx`): toggle visible + graba con `aecEnabled=true`.

**No cubre CI** (runners headless): la apertura WASAPI comms-mode + la cancelación misma → van al smoke.

**Smoke en hardware (la validación de verdad):**
1. Mix + bocinas + AEC-on: reproducir audio + hablar → grabación = voz + far-end, **sin doblado**.
2. ⭐ **Duración larga (5–10 min):** confirmar que el eco NO reaparece con el tiempo — *esto es lo que mató a v1.1* (falló a ~25 s por drift). El smoke debe correr largo.
3. Audífonos + AEC-off: voz cruda 48k completa.
4. Cambio de dispositivo de salida a mitad (follow-default): la referencia se auto-sigue, eco sigue cancelado, sin glitch.
5. `E_NOINTERFACE` / mic raro: fallback + toast.
6. Calidad de transcripción: ¿el mic limpio mejora/empeora Whisper?
7. Playback: ¿la porción del mic procesada suena aceptable?

## 10. Riesgos y preguntas abiertas

- **Riesgo principal:** que el AEC del SO no cancele bien para el hardware/endpoint específico del usuario (caso render no-integrado / distinto clock domain). Mitigación: es el AEC de producción de Teams/Zoom; el smoke de duración larga lo confirma. Odds muchísimo mejores que el DSP custom de v1.1.
- **`GetService` → `E_NOINTERFACE`** en el mic real: manejado (AEC igual corre, sin fijar referencia). Verificar en el smoke.
- **Sobre-procesamiento** de la cadena completa en el playback/transcripción: verificar en el smoke; escape vía per-effect toggles si hace falta.
- **Refactor de captura del mic** (cpal → WASAPI raw): acotado al Carril B, pero toca código testeado; los tests de selección de fuente + el smoke lo cubren.

## 11. Fuentes primarias

- [IAcousticEchoCancellationControl](https://learn.microsoft.com/en-us/windows/win32/api/audioclient/nn-audioclient-iacousticechocancellationcontrol) · [SetEchoCancellationRenderEndpoint](https://learn.microsoft.com/en-us/windows/win32/api/audioclient/nf-audioclient-iacousticechocancellationcontrol-setechocancellationrenderendpoint)
- [Sample oficial AcousticEchoCancellation](https://github.com/microsoft/Windows-classic-samples/blob/main/Samples/AcousticEchoCancellation/README.md)
- [Windows 11 APO/AEC framework (timestamps + resampling de la referencia)](https://learn.microsoft.com/en-us/windows-hardware/drivers/audio/windows-11-apis-for-audio-processing-objects) · [Audio signal processing modes (RAW omite AEC)](https://learn.microsoft.com/en-us/windows-hardware/drivers/audio/audio-signal-processing-modes)
- [Meet Voice Clarity](https://techcommunity.microsoft.com/blog/surfaceitpro/meet-voice-clarity/1419014)
- [Binding en windows-rs](https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/Media/Audio/struct.IAcousticEchoCancellationControl.html)
