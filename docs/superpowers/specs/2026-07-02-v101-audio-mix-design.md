# v1.0.1: Audio Mix — clear and visible ("Mezcla nítida y visible") — Design

> **Status:** DESIGN — approved in brainstorming 2026-07-02. First post-1.0 release.
> Ships via the Sub-8 pipeline (tag → GitHub Release → in-app update), which also completes the
> pending end-to-end auto-update smoke.

**Goal:** Recording a meeting captures BOTH the system audio and the user's voice, clearly
balanced, through a discoverable one-click option — shipped as v1.0.1.

**Context (real-hardware diagnosis, 2026-07-02):** The Sub-2 mixer works (first real-mic
validation ever). Two defects: (a) the user's voice is drowned out — both lanes are summed 50/50
and the system lane is hot; (b) with speakers, the system audio echoes — it enters twice (digital
loopback + acoustic speaker-bleed into the mic ~10-40 ms later). (b) fundamentally needs AEC,
which is **out of scope** (the webrtc-audio-processing MSVC spike verdict was hostile — see
memory; deferred to v1.1). With headphones (b) disappears physically. Also: Mix mode is buried in
Settings with confusing override hints, and the mix mic is silently the Windows default
(`stream.rs:109` — "mic picks the system default for now").

## 1. Scope — four modules

| # | Module | What |
|---|---|---|
| A | Lane balance | System lane ×0.6, mic lane ×1.0 in the Mixer; tuned with the user on real hardware before tagging |
| B | Mic selector (backend) | `Recorder::start` + stream open accept an optional mic device id for Mix; default stays the system default mic |
| C | Mix as a first-class card (frontend) | Virtual "Sistema + Micrófono (recomendado)" card in PreRecord + mic picker + headphones hint; override-hint mess removed |
| D | Version 1.0.1 + release | 5 version sites, e2e baselines (PreRecord layout changes!), tag → Release → in-app update smoke on the physical machine |

## 2. Non-goals

- **AEC** (speaker echo removal) — v1.1 (fork/upstream, speex, or Windows-native; see memory).
- Updater download progress; startup update check (deferred at v1.0.0).
- Recording-format work (MP3 quality etc.) or any transcription/AI change.
- Persisting the chosen mix mic as a setting (YAGNI for now: per-recording choice, defaults to
  the system default mic each time).

## 3. Design

### 3A. Lane balance (Mixer)

`src-tauri/crates/audio/src/capture/mixer.rs` mixes `(a + b) * ANTI_CLIP_GAIN` (0.7), lane A =
system loopback, lane B = mic (`recorder.rs` wiring). Change to per-lane gains:

```rust
pub const SYSTEM_LANE_GAIN: f32 = 0.6; // lane A — tame the hot digital loopback
pub const MIC_LANE_GAIN: f32 = 1.0;    // lane B — keep the voice at full level
// in mix():  (a * SYSTEM_LANE_GAIN + b * MIC_LANE_GAIN) * ANTI_CLIP_GAIN
```

- Max output magnitude becomes (0.6+1.0)×0.7 = 1.12 → still clamped by the writer; acceptable.
- Existing mixer tests assert exact `(x + y) * ANTI_CLIP_GAIN` values → update expectations to the
  new formula; add one test asserting the asymmetric gain is applied to the correct lane.
- **The 0.6 value is a starting point.** Before tagging, a manual tuning loop on the user's
  physical machine (record → listen → adjust constant) fixes the final value.

### 3B. Optional mix mic (backend)

- `stream.rs::open(...)` gains `mic_device_id: Option<&str>`; in `CaptureMode::Mix` use it to open
  that input device (fall back to the system default when `None`). Reuses the existing explicit-mic
  open path with the shared-drops counter (mirror of `open_mic_default_with_drops`).
- `Recorder::start(app, capture_mode, device_id, mic_device_id: Option<String>, format, tmp)`
  threads it through (both call sites in `commands/audio.rs`).
- IPC: `start_recording` gains optional `micDeviceId?: string | null` (specta regenerates
  bindings). `start_preview` unchanged (mix previews the system lane, as today).
- Error path: if the selected mic fails to open, the existing `AudioError::DeviceNotFound` /
  `WasapiInit` toast flow applies unchanged (no new error types).

### 3C. Mix as a first-class card (PreRecord)

`src/features/pre-record/PreRecordPage.tsx` currently derives the mode from the selected device
kind + the buried `settings.captureMode === 'mix'` (with `mixRecordHint` / `mixOverrideHint`).
New model:

- A **virtual card** is prepended to the device grid — sentinel id `MIX_CARD_ID = '__mix__'`,
  styled like a device card: title **"Sistema + Micrófono"** with a "Recomendado" tag, description
  "Graba la reunión y tu voz". Selecting it:
  - shows a **mic `<select>`** below (devices with `kind === 'input'`; first option "Micrófono
    predeterminado de Windows" = `null`), and
  - shows the honest hint: 💡 "Con audífonos la calidad es máxima (evita el eco de las bocinas)".
- Mode derivation becomes explicit: card selected → `recordMode = 'mix'` (+ `micDeviceId` from the
  picker); a loopback device → `'system'`; an input device → `'mic'`. The `mixOverrideHint` /
  `mixRecordHint` pair and their i18n keys are **removed**.
- Preview while the card is selected: system loopback preview (today's mix behavior — preview
  meter shows the meeting audio; the default loopback device is used).
- `settings.captureMode === 'mix'` now simply **preselects the card** on page load (its role as a
  recording-time override disappears). `'system'`/`'mic'` keep preselecting the matching first
  device as today.
- Navigation state to LiveRecording carries `micDeviceId`; LiveRecording passes it to
  `start_recording`.
- i18n (both locales): `mixCardTitle`, `mixCardDesc`, `mixCardRecommended` (reuse existing
  "Recomendado" tag key if one exists), `mixMicLabel`, `mixMicDefault`, `mixHeadphonesHint`;
  remove `mixRecordHint`, `mixOverrideHint`. Regenerate `keys.ts`.

### 3D. Version + release

- Bump **1.0.0 → 1.0.1** in the 5 sites: `package.json`, `src-tauri/tauri.conf.json`,
  `src-tauri/Cargo.toml` (workspace, line 16) + `Cargo.lock` sync, `Sidebar.tsx:13`
  (`Pro · v1.0.1`), `SettingsPage.tsx` footer (`Smart Noter v1.0.1`).
- **e2e visual baselines:** the PreRecord layout gains a card + picker → `pre-record-light`
  baseline WILL exceed the 0.02 threshold. Regenerate (`pnpm test:e2e:update`), visually
  spot-check, commit — the known trap, handled inside the branch, not discovered on CI.
- Ship: merge → main CI green → tag `v1.0.1` → `release.yml` publishes → **the physical machine
  updates from Settings → Buscar actualizaciones** (completing the pending auto-update smoke).

## 4. Testing

- **Mixer (Rust):** updated exact-value tests + new lane-asymmetry test; `cargo test --workspace`.
- **Frontend (vitest):** PreRecord — card renders first; selecting it exposes the mic picker +
  hint and navigates with `captureMode: 'mix'` + chosen `micDeviceId`; loopback/input cards keep
  their modes; `settings.captureMode === 'mix'` preselects the card. Update the existing
  `captureMode derivation` test block accordingly.
- **e2e:** 19 specs green with regenerated PreRecord baseline.
- **Manual (user, physical machine):** the tuning loop (3A) + final smoke: mix recording with
  chosen mic sounds balanced; headphones recording has no echo; in-app update 1.0.0 → 1.0.1.

## 5. Risks

| # | Risk | Mitigation |
|---|---|---|
| R1 | 0.6 balance wrong on real hardware | Tuning loop with the user before tagging; constant is one line |
| R2 | Selected mic fails to open mid-setup | Existing AudioError toast path; user picks another mic |
| R3 | PreRecord baseline churn | Regen + spot-check inside the branch (known trap) |
| R4 | Windows default-mic fallback surprises (webcam mic chosen by OS) | The picker makes the choice visible; default option labeled explicitly |
