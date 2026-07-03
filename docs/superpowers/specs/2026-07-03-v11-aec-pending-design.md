# v1.1: AEC + pending items ("AEC + pendientes") — Design

> **Status:** DESIGN — approved in brainstorming 2026-07-03. Second post-1.0 release.
> Ships via the Sub-8 pipeline (tag → GitHub Release → in-app update). Suggested branch:
> `feat/v11-aec`. AEC route was de-risked by a spike on 2026-07-02 (VIABLE — see
> `project_v101_audio_mix_state` memory); this spec does NOT re-spike it.

**Goal:** Recording a meeting through **speakers** captures the user's voice **without the system
audio echoing into the mic**, and the recording keeps working when the user **switches output
device mid-recording** — plus two distribution polish items deferred at v1.0. Shipped as v1.1.0.

**Context:** v1.0.1 balanced the two mix lanes but left the acoustic problem unsolved: with
speakers, the system audio bleeds into the mic ~10–40 ms late and is captured a second time
(digital loopback + acoustic echo). Balance can't fix this — the echo lives inside the mic lane.
The 2026-07-02 spike found `aec-rs` 1.0.0 (SpeexDSP echo canceller) compiles clean on MSVC (16 s,
cmake+bindgen, no meson/abseil hell) and delivers **33.9 dB ERLE @ 16 kHz** with
`cancel_echo(rec=mic, echo=loopback_reference, out)` on i16. Two smaller items ride along: the
loopback stream still pins its endpoint at open (switching speakers→headphones mid-recording
records silence — v1.0.1 gotcha #3), and two Sub-8 leftovers (updater download progress bar;
`specta-export.exe` bloating the installer by ~20 MB).

## 1. Scope — five modules

| # | Module | What |
|---|--------|------|
| A | **AEC** (`EchoCanceller`) — centerpiece | New isolated `EchoCanceller` invoked inside the Mixer on the already-aligned mono/48k lanes; 16 kHz internally (48k↔16k encapsulated). `aec-rs` 1.0.0. A **"Cancelar eco de bocinas"** toggle on the mix card, persisted, default ON |
| B | Output-device following | Poll the default render endpoint in the loopback thread; re-open on change (System + Mix modes, sentinel only), requesting the original format (WASAPI `convert=true`) so the Mixer/writer are untouched; discreet toast |
| C | Updater download progress | `downloadAndInstall` `onEvent` (Started/Progress/Finished) → progress bar + MB/% in Settings |
| D | Exclude `specta-export.exe` | `#[cfg]`-gate the bin's *contents* → a trivial stub (~200 KB) in release instead of the 20 MB dev bin |
| E | Version 1.1.0 + release | 5 version sites, e2e baselines (the AEC toggle changes the mix card), tag → Release → in-app update smoke |

## 2. Non-goals

- **Windows-native AEC** (`IAcousticEchoCancellationControl`) — a future quality tier (22H2+ AND
  driver-APO dependent; uncertain). SpeexDSP is the portable, verified route now.
- **Auto-detecting speakers vs headphones** to flip the AEC toggle — WASAPI form-factor is
  unreliable (USB/Bluetooth/combo endpoints report "Speakers"/"Unknown"). Manual toggle only.
- **Startup auto-update check** — still deferred (manual "Buscar actualizaciones" only, per Sub-8).
- **Stereo AEC / far-end nonlinear processing** — the mono, single-reference path is enough.
- Re-spiking the AEC crate, or any transcription/AI/persistence change.

## 3. Design

### 3A. AEC — `EchoCanceller` inside the Mixer

**Dependency:** `aec-rs = "1.0.0"` in `src-tauri/crates/audio/Cargo.toml`. **Open verification (R7):**
whether `aec-rs-sys` links speexdsp statically (zero new DLLs) or ships a DLL — if a DLL, add it to
`scripts/stage-dlls.mjs` + `bundle.resources` (the established Sub-8 native-DLL pattern).

**New module** `src-tauri/crates/audio/src/capture/echo_canceller.rs`:

```rust
pub struct EchoCanceller { /* aec-rs Aec, rubato down/up resamplers, i16 frame buffers */ }

impl EchoCanceller {
    pub fn new(config: EchoConfig) -> Result<Self, AudioError>;
    /// mic and reference are the aligned mono@48k lanes (same length). Returns cleaned mic@48k.
    pub fn cancel(&mut self, mic_48k: &[f32], reference_48k: &[f32]) -> Vec<f32>;
}
```

Internally: downsample `mic` + `reference` 48k→16k (rubato), f32→i16, buffer to Speex `frame_size`,
`Aec::cancel_echo(rec=mic, echo=reference, out)` per full frame, i16→f32, upsample 16k→48k. It is a
**fixed-latency block**: `cancel` returns the same sample count it was given, delayed by one
internal frame (~16–32 ms); the first call(s) emit priming silence equal to that latency.

**Speex config (`AecConfig`), initial values to calibrate on hardware:**
- `sample_rate = 16_000`, `frame_size` ≈ 256 (16 ms), `filter_length` ≈ 2048 (128 ms tail — must
  exceed the ~10–40 ms acoustic delay + room reverb).
- `enable_preprocess`: **start OFF** (or with residual-echo-suppress only). The spike's
  double-talk voice attenuation (~-9 dB) comes from the Speex *preprocessor*, not the canceller;
  keeping it off protects near-end voice. This is a tuning knob, documented, not a blocker (R1).

**Integration in `mixer.rs`** (`mix()`, around the gain line 254-264): when AEC is enabled, before
mixing compute `b_clean = ec.cancel(&self.b_ready[..n], &self.a_ready[..n])` and mix
`a_ready * SYSTEM_LANE_GAIN + b_clean * MIC_LANE_GAIN`. Lane A (system) stays untouched at 48k — it
is the remote participants via loopback, already echo-free. The Mixer owns an `Option<EchoCanceller>`
(None when disabled → today's path, zero overhead). The fixed AEC latency offsets the cleaned voice
vs the system lane by ~1 frame; imperceptible for a meeting recording, so accepted (R2; delaying
lane A by the same latency is the fallback if it ever matters).

**Toggle wiring** (mirror of v1.0.1's mic-selector threading):
- Backend: `aec_enabled: bool` flows `start_recording → Recorder::start → mixer thread → Mixer`.
  Only meaningful in `CaptureMode::Mix`.
- Settings: new persisted field `aecEnabled: boolean` (default `true`), added to the settings
  schema with the existing default/migration pattern.
- IPC: `start_recording` gains `aecEnabled: boolean`. specta regenerates `bindings.ts`.
- Frontend: a toggle **"Cancelar eco de bocinas"** on the mix card (`PreRecordPage.tsx`, near the
  mic picker at 206-225), visible only when the mix card is selected; reads/writes
  `settings.aecEnabled`; the value rides the navigation state to LiveRecording → `start_recording`.
- i18n (both locales): `aecToggleLabel` ("Cancelar eco de bocinas" / "Cancel speaker echo"),
  `aecToggleHint` ("Actívalo con bocinas; con audífonos apágalo"). Regenerate `keys.ts`.

### 3B. Output-device following (polling)

Today the loopback resolves the default render endpoint once at stream-open (sentinel
`DEFAULT_RENDER_LOOPBACK` → `wasapi::get_default_device`, `stream.rs:28,171-180`) and then pins it.
wasapi 0.16 does NOT expose `IMMNotificationClient` for default-device changes (its `EventCallbacks`
are audio-session events), so following uses **polling** (user-approved 2026-07-03) instead of a COM
notification client — simpler and lower-risk, same observable behavior.

- Restructure `wasapi_loopback_loop` into an outer re-open loop + the existing inner capture loop.
  Applies while a loopback stream is alive — **System and Mix modes** (Mic-only has no loopback).
- **Only when the device id is the sentinel** `__default_render__`. A user-pinned device means
  "record exactly this endpoint" → do not follow.
- Every ~1 s the inner loop compares the current default render endpoint id
  (`wasapi::get_default_device`) against the open one; on change it stops the old `IAudioClient` and
  re-opens at the top of the outer loop. The Mixer's **silence-fill already covers the sub-second
  gap**. Detection latency ≤1 s is irrelevant while the user is switching devices.
- **Rate/channel change (supersedes the earlier control-channel design):** the re-open requests the
  ORIGINAL rate/channels with WASAPI `convert=true` (already used at `stream.rs`), so WASAPI absorbs
  any new-endpoint format difference and the Mixer + writer never see a change — no
  `reconfigure_lane_a`, no control channel. If `convert=true` turns out not to resample, the
  fallback resamples inside the loopback thread to the original rate (still untouching the Mixer).
- **Toast:** emit a Tauri event `audio:output-device-changed { name }`; the frontend (`App.tsx`)
  shows a discreet toast ("Salida cambiada a «X»").

### 3C. Updater download progress

The updater's `downloadAndInstall` accepts an `onEvent` callback emitting `Started { contentLength }`
/ `Progress { chunkLength }` / `Finished`. Today "Buscar actualizaciones" (Settings) downloads with
no feedback. Change the download call in the Settings update flow to:
- `Started` → store total, show bar at 0 %.
- `Progress` → accumulate `downloaded += chunkLength`; bar = `downloaded / total`.
- `Finished` → 100 %, then the existing restart prompt (plugin `process` relaunch).

UI: a progress bar + "X.X / Y.Y MB (Z %)" in the existing updates section of Settings. No backend
change; no new decisions.

### 3D. Exclude `specta-export.exe`

The 20 MB weight comes from the bin linking the app lib (specta + all types + transitive sys
crates). Prior attempts failed: `required-features` (bundler still requires the exe to exist);
moving it to its own crate (compiles, but the exe dies `0xC0000135` at dev runtime — transitive sys
linking). New angle — gate the **contents**, keep the target existing:

- `src-tauri/src/bin/specta_export.rs` (the `[[bin]] specta-export`, `Cargo.toml:54-56`): put the
  whole `use`-of-the-app-lib + binding generation behind `#[cfg(feature = "generate-bindings")]`;
  without the feature, `fn main() {}`. The `[[bin]]` always exists (bundler happy), but in release
  (feature off) it links nothing heavy → a **~200 KB stub** instead of 20 MB.
- Add a `generate-bindings` feature to the existing `[features]` in `src-tauri/Cargo.toml` (line 58),
  **not** enabled in the release/bundle build. Dev regenerates bindings with
  `cargo run --bin specta-export --features generate-bindings` (update the bindings-regen
  doc/command accordingly).
- **Refinement:** if the bundler tolerates the exe being absent on disk, also delete it in
  `stage-dlls.mjs` (the `beforeBundleCommand` hook) → 0 KB. Implementation validates which applies.
- Verify installer size before/after (target: well under the 30 MB budget, ideally back toward the
  19.5 MB of v1.0.0).

### 3E. Version 1.1.0 + release

- Bump **1.0.1 → 1.1.0** in the 5 sites: `package.json`, `src-tauri/tauri.conf.json`,
  `src-tauri/Cargo.toml` (workspace) + `Cargo.lock` sync, `Sidebar.tsx` (`Pro · v1.1.0`),
  `SettingsPage.tsx` footer.
- **e2e visual baselines:** the AEC toggle adds an element to the mix card → the `pre-record`
  baseline will exceed the 0.02 threshold. Regenerate (`pnpm test:e2e:update`), spot-check, commit
  — known trap, handled inside the branch.
- Ship: merge → main CI green → tag `v1.1.0` → `release.yml` publishes → the physical machine
  updates from Settings → Buscar actualizaciones (also completes Sub-8's still-pending
  1.0.0→1.0.1→1.1.0 in-app update smoke).

## 4. Testing

- **AEC (Rust, `EchoCanceller` in isolation):** synthesize near-end voice + a delayed attenuated
  copy of a reference as the "mic"; assert `cancel()` yields **ERLE ≥ 25 dB** (spike measured 33.9).
  Passthrough test: reference = silence → cleaned mic ≈ input (no damage). Resample round-trip
  sanity. `cargo test --workspace`.
- **Mixer (Rust):** `reconfigure_lane_a` produces correct output at the new rate; AEC-disabled path
  unchanged (existing mix tests stay green).
- **Frontend (vitest):** mix card renders the toggle only when selected; toggle reads/writes
  `aecEnabled` and rides navigation to `start_recording`; updater flow — mocked
  `downloadAndInstall` emitting Started/Progress/Finished drives the bar.
- **e2e:** specs green with the regenerated PreRecord baseline.
- **Manual (user, physical machine)** — the real validation, since COM/acoustics can't be unit
  tested:
  - Speakers + AEC on → echo gone, voice clear; tune `enable_preprocess`/`filter_length`;
    double-talk acceptable.
  - Headphones + AEC off → voice intact (no attenuation).
  - Start recording on speakers, switch to headphones mid-recording → audio continues, toast shows,
    no silence; recording sample-rate stays coherent.
  - Updater 1.0.1 → 1.1.0 shows the progress bar.
  - Installer ships no 20 MB `specta-export.exe`.

## 5. Risks

| # | Risk | Mitigation |
|---|------|------------|
| R1 | Speex preprocessor over-attenuates near-end voice in double-talk (~-9 dB) | Start with preprocessor OFF (canceller only); tune on hardware; the toggle lets the user disable AEC entirely with headphones |
| R2 | `EchoCanceller` fixed latency offsets cleaned voice vs system lane | ~1 frame (16–32 ms), imperceptible for a meeting; accepted. Fallback: delay lane A by the same latency |
| R3 | Output-device switch changes sample rate | Re-open requests the ORIGINAL format with WASAPI `convert=true`; Mixer/writer untouched. Fallback: resample in the loopback thread |
| R4 | Polling adds latency / misses a fast switch | ~1 s poll; silence-fill covers the gap; a device switch is a human action, so sub-second detection is ample |
| R5 | `specta-export` stub still ships (~200 KB) | Acceptable vs 20 MB; delete-in-`beforeBundle` refinement if the bundler tolerates absence |
| R6 | PreRecord e2e baseline churn from the toggle | Regen + spot-check inside the branch (known trap) |
| R7 | `aec-rs-sys` may ship a speexdsp DLL (new installer dependency) | Verify early; if so, bundle it via `stage-dlls.mjs` + `bundle.resources` (Sub-8 pattern) |
| R8 | Following only fires for the sentinel; pinned-device users still pin | By design — a pinned device means "record exactly this endpoint" |
