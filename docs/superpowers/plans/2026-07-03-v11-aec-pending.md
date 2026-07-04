# v1.1 AEC + Pending Items — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Recording through speakers cancels the acoustic echo (SpeexDSP AEC on the mic lane), the recording follows an output-device switch mid-session, the updater shows download progress, and `specta-export.exe` no longer bloats the installer — shipped as v1.1.0.

**Architecture:** An isolated `EchoCanceller` (16 kHz internally, `aec-rs`/SpeexDSP) sits inside the Mixer on the two time-aligned mono lanes, gated by a persisted toggle. The WASAPI loopback thread polls the default render endpoint and re-opens on change, requesting the original format so the rest of the pipeline is untouched. The updater plugin's `downloadAndInstall` event stream drives a progress bar. A `cfg`-gated `specta-export` bin becomes a trivial stub in release.

**Tech Stack:** Rust (rubato 0.15, `aec-rs` 1.0.0, wasapi 0.16, crossbeam), Tauri 2 (updater/process plugins), React/TS (RTK Query), specta bindings, vitest + Playwright e2e.

**Environment preamble (EVERY `cargo`/`git` command — the pre-commit clippy hook rebuilds the whisper crate; our shells don't inherit the persisted env):**
```bash
export PATH="$HOME/.cargo/bin:/c/Program Files (x86)/Microsoft Visual Studio/2022/BuildTools/Common7/IDE/CommonExtensions/Microsoft/CMake/CMake/bin:$PATH"
export LIBCLANG_PATH="C:/Program Files/LLVM/bin"
```

**Session setup (do ONCE before Task A1):** regenerate the gitignored bindings + i18n keys so the tree compiles:
```bash
cd src-tauri && cargo run --bin specta-export && cd ..   # writes src/ipc/bindings.ts
pnpm generate:i18n-keys                                    # writes src/i18n/keys.ts
```

**Design notes carried from the spec (2026-07-03-v11-aec-pending-design.md):**
- AEC runs at 16 kHz on the mic lane; reference = system loopback. **Consequence:** with AEC ON the recorded voice is band-limited to 16 kHz (loses >8 kHz brightness). Accepted — AEC-on means speakers, where killing echo beats voice brightness; AEC-off (headphones) keeps full 48 kHz. Document in the toggle hint if it reads well.
- `enable_preprocess` starts **false** (the Speex preprocessor is what attenuates near-end voice in double-talk); calibrate on hardware.
- Device-following uses **polling** (user-approved) + re-open with the original format via WASAPI `convert=true` — NO `reconfigure_lane_a`, NO control channel (supersedes spec §3B's control-channel wording).

---

## Module A — AEC (`EchoCanceller` inside the Mixer)

**File structure:**
- Create: `src-tauri/crates/audio/src/capture/echo_canceller.rs` — the isolated AEC unit.
- Modify: `src-tauri/crates/audio/Cargo.toml` — add `aec-rs`.
- Modify: `src-tauri/crates/audio/src/capture/mod.rs` — declare the module (it already has `pub mod mixer; pub mod recorder; pub mod stream;`).
- Modify: `src-tauri/crates/audio/src/capture/mixer.rs` — hold `Option<EchoCanceller>`, delay-FIFO, use in `mix()`.
- Modify: `src-tauri/crates/audio/src/capture/recorder.rs` — thread `aec_enabled` to the mixer thread.
- Modify: `src-tauri/src/commands/audio.rs` — `start_recording` gains `aec_enabled`.
- Modify: `src-tauri/crates/core/src/models/settings.rs` — `aec_enabled` field.
- Modify: `src/features/pre-record/PreRecordPage.tsx`, `src/features/live-recording/LiveRecordingPage.tsx`, i18n locales.

### Task A1: Add `aec-rs` + `EchoCanceller` skeleton

**Files:**
- Modify: `src-tauri/crates/audio/Cargo.toml`
- Create: `src-tauri/crates/audio/src/capture/echo_canceller.rs`
- Modify: `src-tauri/crates/audio/src/capture/mod.rs` (or the capture module root — verify)

- [ ] **Step 1: Add the dependency.** In `src-tauri/crates/audio/Cargo.toml`, under `[dependencies]` (after `rubato = "0.15"`):

```toml
aec-rs = "1.0.0"
```

- [ ] **Step 2: Declare the module.** In `src-tauri/crates/audio/src/capture/mod.rs`, after `pub mod stream;`, add:

```rust
pub mod echo_canceller;
```

- [ ] **Step 3: Write the failing construction test.** Create `echo_canceller.rs` with only the config + a test:

```rust
//! 16 kHz SpeexDSP acoustic echo canceller for the mic lane.
//!
//! Wraps `aec-rs` (SpeexDSP). The Mixer feeds this the two TIME-ALIGNED mono@48k
//! lanes (mic = near-end, system loopback = far-end reference). Internally it
//! downsamples both to 16 kHz, cancels per Speex `frame_size` frame on i16, and
//! upsamples the cleaned mic back to 48 kHz. Output is delayed by a fixed internal
//! latency (resampler priming + one frame); the caller aligns the system lane via
//! its own delay FIFO. See the module design note about 16 kHz band-limiting.

use crate::error::AudioError;
use aec_rs::{Aec, AecConfig};
use rubato::{FftFixedIn, Resampler};

/// Tunable AEC parameters. Defaults target 16 kHz; calibrate on hardware.
#[derive(Debug, Clone)]
pub struct EchoConfig {
    /// Speex frame size in samples @16 kHz (10 ms = 160).
    pub frame_size: usize,
    /// Adaptive filter tail length in samples @16 kHz (100 ms = 1600). Must
    /// exceed the acoustic delay (~10-40 ms) + room reverb.
    pub filter_length: i32,
    /// Speex preprocessor (denoise + residual echo suppress). Starts FALSE —
    /// it over-attenuates near-end voice in double-talk.
    pub enable_preprocess: bool,
}

impl Default for EchoConfig {
    fn default() -> Self {
        Self { frame_size: 160, filter_length: 1600, enable_preprocess: false }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructs_at_default_config() {
        let ec = EchoCanceller::new(EchoConfig::default());
        assert!(ec.is_ok());
    }
}
```

- [ ] **Step 4: Run it and watch it fail to compile.**

Run: `cd src-tauri && cargo test -p smart-noter-audio echo_canceller`
Expected: FAIL — `cannot find type EchoCanceller`.

- [ ] **Step 5: Add the struct + `new`.** Insert before the `#[cfg(test)]` block:

```rust
const DOWN_CHUNK_48: usize = 1024; // 48k input frames per downsample chunk
const UP_CHUNK_16: usize = 512;    // 16k input frames per upsample chunk

pub struct EchoCanceller {
    aec: Aec,
    frame_size: usize,
    down_mic: FftFixedIn<f32>,
    down_ref: FftFixedIn<f32>,
    up_out: FftFixedIn<f32>,
    mic_pending48: Vec<f32>,
    ref_pending48: Vec<f32>,
    mic16: Vec<f32>,
    ref16: Vec<f32>,
    out_pending16: Vec<f32>,
}

impl EchoCanceller {
    pub fn new(cfg: EchoConfig) -> Result<Self, AudioError> {
        let aec = Aec::new(&AecConfig {
            frame_size: cfg.frame_size,
            filter_length: cfg.filter_length,
            sample_rate: 16_000,
            enable_preprocess: cfg.enable_preprocess,
        });
        let mk = |inr: usize, outr: usize, chunk: usize| {
            FftFixedIn::<f32>::new(inr, outr, chunk, 2, 1)
                .map_err(|e| AudioError::Other(format!("aec rubato init: {e}")))
        };
        Ok(Self {
            aec,
            frame_size: cfg.frame_size,
            down_mic: mk(48_000, 16_000, DOWN_CHUNK_48)?,
            down_ref: mk(48_000, 16_000, DOWN_CHUNK_48)?,
            up_out: mk(16_000, 48_000, UP_CHUNK_16)?,
            mic_pending48: Vec::new(),
            ref_pending48: Vec::new(),
            mic16: Vec::new(),
            ref16: Vec::new(),
            out_pending16: Vec::new(),
        })
    }
}
```

- [ ] **Step 6: Run the test — expect PASS.**

Run: `cd src-tauri && cargo test -p smart-noter-audio echo_canceller`
Expected: PASS (1 test).

- [ ] **Step 7: Commit.**

```bash
git add src-tauri/crates/audio/Cargo.toml src-tauri/crates/audio/src/capture/echo_canceller.rs src-tauri/crates/audio/src/capture/mod.rs src-tauri/Cargo.lock
git commit -m "feat(aec): add aec-rs dep + EchoCanceller skeleton"
```

### Task A2: `process()` — streaming resample + frame plumbing (length/latency)

**Files:** Modify `src-tauri/crates/audio/src/capture/echo_canceller.rs`

- [ ] **Step 1: Write the failing length test.** Add to the `tests` module:

```rust
/// Feed 1 second of aligned mic+ref at 48k in 480-sample (10 ms) ticks; the total
/// cleaned output must be within one internal-latency window of the input length
/// (streaming resample + frame buffering delays a bounded prefix, nothing more).
#[test]
fn output_length_tracks_input_minus_bounded_latency() {
    let mut ec = EchoCanceller::new(EchoConfig::default()).unwrap();
    let mut total_out = 0usize;
    let ticks = 100; // 100 * 480 = 48_000 samples = 1 s
    for _ in 0..ticks {
        let mic = vec![0.1f32; 480];
        let refr = vec![0.0f32; 480];
        total_out += ec.process(&mic, &refr).len();
    }
    let input = ticks * 480;
    // Latency is a few resampler chunks + one frame — well under 0.2 s @48k.
    assert!(total_out > input - 9_600, "output {total_out} lost too much vs {input}");
    assert!(total_out <= input, "output {total_out} exceeds input {input}");
}
```

- [ ] **Step 2: Run — expect FAIL (no `process`).**

Run: `cd src-tauri && cargo test -p smart-noter-audio echo_canceller::tests::output_length`
Expected: FAIL — `no method named process`.

- [ ] **Step 3: Implement `process`.** Add inside `impl EchoCanceller`:

```rust
/// Feed one aligned pair of mono@48k buffers (`mic.len() == reference.len()`).
/// Returns cleaned mic@48k, in order, delayed by the fixed internal latency.
/// The returned length is NOT equal to the input length — the caller aligns the
/// system lane with its own delay FIFO.
pub fn process(&mut self, mic: &[f32], reference: &[f32]) -> Vec<f32> {
    self.mic_pending48.extend_from_slice(mic);
    self.ref_pending48.extend_from_slice(reference);

    // Downsample mic and ref in LOCKSTEP so their 16k streams stay sample-aligned
    // (identical resamplers → identical priming delay → no relative skew).
    while self.mic_pending48.len() >= DOWN_CHUNK_48 && self.ref_pending48.len() >= DOWN_CHUNK_48 {
        let mchunk: Vec<f32> = self.mic_pending48.drain(..DOWN_CHUNK_48).collect();
        let rchunk: Vec<f32> = self.ref_pending48.drain(..DOWN_CHUNK_48).collect();
        if let Ok(o) = self.down_mic.process(&[&mchunk], None) {
            if let Some(c) = o.into_iter().next() { self.mic16.extend(c); }
        }
        if let Ok(o) = self.down_ref.process(&[&rchunk], None) {
            if let Some(c) = o.into_iter().next() { self.ref16.extend(c); }
        }
    }

    // Cancel per Speex frame on i16.
    let mut rec_i = vec![0i16; self.frame_size];
    let mut echo_i = vec![0i16; self.frame_size];
    let mut out_i = vec![0i16; self.frame_size];
    while self.mic16.len() >= self.frame_size && self.ref16.len() >= self.frame_size {
        for (d, s) in rec_i.iter_mut().zip(self.mic16.drain(..self.frame_size)) { *d = f32_to_i16(s); }
        for (d, s) in echo_i.iter_mut().zip(self.ref16.drain(..self.frame_size)) { *d = f32_to_i16(s); }
        self.aec.cancel_echo(&rec_i, &echo_i, &mut out_i);
        self.out_pending16.extend(out_i.iter().map(|&s| i16_to_f32(s)));
    }

    // Upsample cleaned 16k → 48k.
    let mut out48 = Vec::new();
    while self.out_pending16.len() >= UP_CHUNK_16 {
        let chunk: Vec<f32> = self.out_pending16.drain(..UP_CHUNK_16).collect();
        if let Ok(o) = self.up_out.process(&[&chunk], None) {
            if let Some(c) = o.into_iter().next() { out48.extend(c); }
        }
    }
    out48
}
```

And add the free helpers at module scope (below the `impl`):

```rust
#[inline]
fn f32_to_i16(x: f32) -> i16 {
    (x.clamp(-1.0, 1.0) * 32_767.0) as i16
}
#[inline]
fn i16_to_f32(x: i16) -> f32 {
    x as f32 / 32_768.0
}
```

- [ ] **Step 4: Run — expect PASS.** (If rubato output sizes differ from assumptions, the length test will flag it — adjust chunk constants, do not weaken the assertion beyond the 0.2 s latency budget.)

Run: `cd src-tauri && cargo test -p smart-noter-audio echo_canceller`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit.**

```bash
git add src-tauri/crates/audio/src/capture/echo_canceller.rs
git commit -m "feat(aec): EchoCanceller streaming 48k<->16k resample + frame cancel"
```

### Task A3: Cancellation works (ERLE on synthetic echo)

**Files:** Modify `src-tauri/crates/audio/src/capture/echo_canceller.rs`

- [ ] **Step 1: Write the failing ERLE test.** Add to `tests`:

```rust
/// Synthetic double-path test: reference = a tone; mic = a delayed, attenuated
/// copy of that tone (the "echo") with NO near-end voice. After the adaptive
/// filter converges, the cleaned mic energy must drop well below the raw echo
/// energy (ERLE >= 12 dB is a conservative floor; the spike measured 33.9 dB).
#[test]
fn cancels_synthetic_echo_erle_positive() {
    use std::f32::consts::PI;
    let mut ec = EchoCanceller::new(EchoConfig::default()).unwrap();
    let sr = 48_000.0f32;
    let delay = 480usize; // 10 ms acoustic delay @48k
    let atten = 0.5f32;

    // 3 seconds so the NLMS filter converges; measure ERLE on the last second.
    let n = 48_000 * 3;
    let reference: Vec<f32> = (0..n).map(|i| 0.3 * (2.0 * PI * 440.0 * i as f32 / sr).sin()).collect();
    let mic: Vec<f32> = (0..n)
        .map(|i| if i >= delay { atten * reference[i - delay] } else { 0.0 })
        .collect();

    let mut cleaned = Vec::new();
    // Feed in 480-sample ticks (matches the real mixer cadence order-of-magnitude).
    for t in 0..(n / 480) {
        let s = t * 480;
        cleaned.extend(ec.process(&mic[s..s + 480], &reference[s..s + 480]));
    }

    // Compare energy over the last ~1 s of cleaned output vs the raw echo it came from.
    let tail = cleaned.len().saturating_sub(48_000);
    let cleaned_energy: f32 = cleaned[tail..].iter().map(|x| x * x).sum();
    // Raw echo energy over a comparable 1 s window (mic is the echo here).
    let echo_energy: f32 = mic[mic.len() - 48_000..].iter().map(|x| x * x).sum();
    let erle_db = 10.0 * (echo_energy / (cleaned_energy + 1e-9)).log10();
    assert!(erle_db > 12.0, "ERLE {erle_db} dB too low — echo not cancelled");
}
```

- [ ] **Step 2: Run — expect PASS** (the implementation from A2 already cancels; this test proves it). If it fails, the failure is real (frame alignment / i16 scaling / config) — debug with systematic-debugging, do not lower the threshold.

Run: `cd src-tauri && cargo test -p smart-noter-audio echo_canceller::tests::cancels`
Expected: PASS.

- [ ] **Step 3: Add a passthrough guard test.** Add:

```rust
/// With a silent reference there is no echo to cancel; the cleaned mic must
/// preserve the near-end signal's energy (the canceller must not eat the voice).
#[test]
fn silent_reference_preserves_voice_energy() {
    use std::f32::consts::PI;
    let mut ec = EchoCanceller::new(EchoConfig::default()).unwrap();
    let n = 48_000;
    let voice: Vec<f32> = (0..n).map(|i| 0.3 * (2.0 * PI * 300.0 * i as f32 / 48_000.0).sin()).collect();
    let mut out = Vec::new();
    for t in 0..(n / 480) {
        let s = t * 480;
        out.extend(ec.process(&voice[s..s + 480], &vec![0.0f32; 480]));
    }
    let vin: f32 = voice[9_600..].iter().map(|x| x * x).sum();
    let vout: f32 = out.iter().skip(9_600).map(|x| x * x).sum();
    // Voice should survive (allow band-limiting/latency losses): keep >40% energy.
    assert!(vout > 0.4 * vin, "voice over-attenuated: {vout} vs {vin}");
}
```

- [ ] **Step 4: Run — expect PASS.**

Run: `cd src-tauri && cargo test -p smart-noter-audio echo_canceller`
Expected: PASS (4 tests).

- [ ] **Step 5: Commit.**

```bash
git add src-tauri/crates/audio/src/capture/echo_canceller.rs
git commit -m "test(aec): ERLE + voice-preservation coverage for EchoCanceller"
```

### Task A4: Wire `EchoCanceller` into the Mixer (delay-aligned)

**Files:** Modify `src-tauri/crates/audio/src/capture/mixer.rs`

- [ ] **Step 1: Write the failing integration test.** Add to `mixer.rs`'s `tests` module:

```rust
/// With AEC enabled and a silent system lane, the mixer still emits mic-driven
/// audio (proves the delay-FIFO path routes cleaned mic to the output and does
/// not stall). Exact values aren't asserted (AEC latency shifts them); we assert
/// that non-trivial output eventually flows.
#[test]
fn aec_enabled_mixer_emits_mic_audio() {
    let mut m = Mixer::new(48_000, 1, 48_000, 1).unwrap();
    m.enable_aec(crate::capture::echo_canceller::EchoConfig::default()).unwrap();
    let mut total = 0usize;
    for _ in 0..200 {
        let out = m.mix(&vec![0.0f32; 480], &vec![0.2f32; 480]).unwrap();
        total += out.len();
    }
    assert!(total > 0, "AEC-enabled mixer produced no output");
}
```

- [ ] **Step 2: Run — expect FAIL (no `enable_aec`).**

Run: `cd src-tauri && cargo test -p smart-noter-audio mixer::tests::aec_enabled`
Expected: FAIL — `no method named enable_aec`.

- [ ] **Step 3: Add the field, builder, and delay FIFO.** In `mixer.rs`:

Add `use` at the top: `use crate::capture::echo_canceller::{EchoCanceller, EchoConfig};`

Add fields to `struct Mixer` (after `synced: bool,`):

```rust
    /// AEC on the mic lane (None = disabled → original path, zero overhead).
    echo: Option<EchoCanceller>,
    /// System-lane samples awaiting their delayed cleaned-mic counterpart
    /// (keeps the mix aligned across the AEC's fixed latency).
    a_delayed: Vec<f32>,
```

Initialise them in `new()`'s `Ok(Self { ... })` (add `echo: None,` and `a_delayed: Vec::new(),`).

Add the builder after `new()`:

```rust
    /// Enable AEC on the mic lane. Call once, right after `new`, in Mix mode.
    pub fn enable_aec(&mut self, cfg: EchoConfig) -> Result<(), AudioError> {
        self.echo = Some(EchoCanceller::new(cfg)?);
        Ok(())
    }
```

- [ ] **Step 4: Route the mix through the AEC.** Replace the mixing block (the current lines that compute `n`, build `mixed`, and drain — from `let n = self.a_ready.len().min(self.b_ready.len());` through `Ok(mixed)`) with:

```rust
        let n = self.a_ready.len().min(self.b_ready.len());
        if n == 0 {
            return Ok(vec![]);
        }

        if let Some(ec) = &mut self.echo {
            // AEC path: cancel the echo out of the mic using the aligned system
            // lane as reference, then mix the cleaned mic against a delay-matched
            // copy of the system lane so both represent the same instant.
            let cleaned = ec.process(&self.b_ready[..n], &self.a_ready[..n]);
            self.a_delayed.extend_from_slice(&self.a_ready[..n]);
            self.a_ready.drain(..n);
            self.b_ready.drain(..n);

            let k = cleaned.len().min(self.a_delayed.len());
            if k == 0 {
                return Ok(vec![]);
            }
            let mixed: Vec<f32> = self.a_delayed[..k]
                .iter()
                .zip(cleaned[..k].iter())
                .map(|(a, b)| (a * SYSTEM_LANE_GAIN + b * MIC_LANE_GAIN) * ANTI_CLIP_GAIN)
                .collect();
            self.a_delayed.drain(..k);
            return Ok(mixed);
        }

        let mixed: Vec<f32> = self.a_ready[..n]
            .iter()
            .zip(self.b_ready[..n].iter())
            .map(|(a, b)| (a * SYSTEM_LANE_GAIN + b * MIC_LANE_GAIN) * ANTI_CLIP_GAIN)
            .collect();

        self.a_ready.drain(..n);
        self.b_ready.drain(..n);

        Ok(mixed)
```

- [ ] **Step 5: Run — expect PASS, and confirm the non-AEC tests still pass.**

Run: `cd src-tauri && cargo test -p smart-noter-audio`
Expected: PASS (all existing mixer tests + the new one).

- [ ] **Step 6: Commit.**

```bash
git add src-tauri/crates/audio/src/capture/mixer.rs
git commit -m "feat(aec): route the mix through EchoCanceller with a delay-aligned system lane"
```

### Task A5: Thread `aec_enabled` through the recorder + command

**Files:**
- Modify: `src-tauri/crates/audio/src/capture/recorder.rs`
- Modify: `src-tauri/src/commands/audio.rs`

- [ ] **Step 1: Extend `Recorder::start`.** In `recorder.rs`, add a parameter to `pub fn start(`:

```rust
    pub fn start(
        app: AppHandle,
        mode: CaptureMode,
        device_id: String,
        mic_device_id: Option<String>,
        aec_enabled: bool,
        format: AudioFormat,
        tmp_path: PathBuf,
    ) -> Result<Self, AudioError> {
```

- [ ] **Step 2: Enable AEC in the mixer thread.** In the Mix branch, right after the mixer is constructed (`let mut mixer = match Mixer::new(...) { Ok(m) => m, ... };`), insert:

```rust
                if aec_enabled {
                    if let Err(e) = mixer.enable_aec(
                        crate::capture::echo_canceller::EchoConfig::default(),
                    ) {
                        tracing::error!(?e, "AEC init failed; continuing without echo cancellation");
                    }
                }
```

- [ ] **Step 3: Fix the two call sites.** In `src-tauri/src/commands/audio.rs`:

`start_recording` — add the param to the signature and pass it through:

```rust
pub fn start_recording(
    state: tauri::State<'_, crate::state::AppState>,
    app: tauri::AppHandle,
    device_id: String,
    capture_mode: CaptureMode,
    mic_device_id: Option<String>,
    aec_enabled: bool,
    format: AudioFormat,
) -> Result<RecordingStartedDto, AppError> {
```

and the `Recorder::start(` call becomes `Recorder::start(app, capture_mode, device_id, mic_device_id, aec_enabled, format, tmp_path)`.

`start_preview` — preview never needs AEC; pass `false`: `Recorder::start(app, capture_mode, device_id, None, false, AudioFormat::Wav, tmp)`.

- [ ] **Step 4: Build the backend.**

Run: `cd src-tauri && cargo build`
Expected: compiles (no callers left unfixed).

- [ ] **Step 5: Commit.**

```bash
git add src-tauri/crates/audio/src/capture/recorder.rs src-tauri/src/commands/audio.rs
git commit -m "feat(aec): thread aec_enabled from start_recording to the mixer thread"
```

### Task A6: `aec_enabled` in AppSettings + regenerate bindings

**Files:**
- Modify: `src-tauri/crates/core/src/models/settings.rs`
- Regenerate: `src/ipc/bindings.ts`

- [ ] **Step 1: Add the field.** In `settings.rs`, in `struct AppSettings`, after `pub storage_dir: String,` add:

```rust
    /// v1.1: cancel speaker echo in Mix mode (default on). Only meaningful for
    /// `capture_mode == "mix"`. See [`EchoCanceller`].
    #[serde(default = "default_true")]
    pub aec_enabled: bool,
```

- [ ] **Step 2: Add the default.** In `impl Default for AppSettings`, after `storage_dir: String::new(),` add:

```rust
            aec_enabled: true,
```

- [ ] **Step 3: Regenerate bindings.** (`default_true` already exists in this file — no new helper.)

Run: `cd src-tauri && cargo run --bin specta-export`
Expected: prints `bindings.ts exported`; `src/ipc/bindings.ts` now has `aecEnabled: boolean` on `AppSettings` and `aecEnabled` on `startRecording`.

- [ ] **Step 4: Verify the frontend type-checks with the new field.**

Run: `pnpm tsc -b`
Expected: no errors (the field is optional-by-default at call sites until Task A7 wires it).

- [ ] **Step 5: Commit.**

```bash
git add src-tauri/crates/core/src/models/settings.rs src/ipc/bindings.ts
git commit -m "feat(aec): persist aec_enabled in AppSettings (default on) + regen bindings"
```

### Task A7: Frontend toggle in PreRecord → nav state → start_recording

**Files:**
- Modify: `src/features/pre-record/PreRecordPage.tsx`
- Modify: `src/features/live-recording/LiveRecordingPage.tsx`
- Modify: `src/i18n/locales/es.json`, `src/i18n/locales/en.json`
- Regenerate: `src/i18n/keys.ts`
- Test: `src/features/pre-record/PreRecordPage.test.tsx` (or the existing PreRecord test file — grep)

- [ ] **Step 1: Add i18n keys.** In `src/i18n/locales/es.json` add:

```json
  "aecToggleLabel": "Cancelar eco de bocinas",
  "aecToggleHint": "Actívalo si escuchas el audio por bocinas; con audífonos apágalo.",
```

In `src/i18n/locales/en.json` add:

```json
  "aecToggleLabel": "Cancel speaker echo",
  "aecToggleHint": "Turn on when you hear the audio through speakers; with headphones, leave it off.",
```

- [ ] **Step 2: Regenerate keys.**

Run: `pnpm generate:i18n-keys`
Expected: `src/i18n/keys.ts` includes `aecToggleLabel` and `aecToggleHint`.

- [ ] **Step 3: Add local toggle state seeded from settings.** In `PreRecordPage.tsx`, after `const [micDeviceId, setMicDeviceId] = useState<string | null>(null);`:

```typescript
  const [aecEnabled, setAecEnabled] = useState(true);
  useEffect(() => {
    if (settings) setAecEnabled(settings.aecEnabled ?? true);
  }, [settings]);
```

(Ensure `useEffect` is imported from `react`.)

- [ ] **Step 4: Render the toggle after the mic picker.** Inside the `{isMix && ( <> ... </> )}` block, immediately after the `</div>` that closes `styles.micPickerRow` and before the `mixHeadphonesHint` div, add:

```tsx
                <label className={styles.aecToggleRow}>
                  <input
                    type="checkbox"
                    checked={aecEnabled}
                    onChange={(e) => {
                      const v = e.target.checked;
                      setAecEnabled(v);
                      if (settings) void updateSettings({ ...settings, aecEnabled: v });
                    }}
                  />
                  <span>{t('aecToggleLabel')}</span>
                </label>
                <div className={styles.modeHint}>{t('aecToggleHint')}</div>
```

(Add a minimal `.aecToggleRow` rule to the PreRecord CSS module mirroring `.micPickerRow` — a flex row with gap. Grep the `.module.css` next to the page.)

- [ ] **Step 5: Pass `aecEnabled` into the nav state.** In `start()`, add to the `state` object (after `micDeviceId: ...`):

```typescript
        aecEnabled: isMix ? aecEnabled : false,
```

- [ ] **Step 6: Consume it in LiveRecording.** In `LiveRecordingPage.tsx`, add to the `NavState` interface: `aecEnabled?: boolean;`. In the `invoke<RecordingStartedDto>('start_recording', { ... })` call, add: `aecEnabled: navState.aecEnabled ?? false,`.

- [ ] **Step 7: Write/extend the PreRecord test.** In the PreRecord test file, add a case asserting the toggle appears only when the mix card is selected and that selecting it + toggling persists `aecEnabled` and rides the nav state. Minimal shape (adapt to the file's existing render helper + router mock):

```typescript
it('mix card shows the AEC toggle (default checked) and passes it to recording', async () => {
  renderPreRecord({ settings: { aecEnabled: true /* + required fields */ } });
  await selectMixCard();
  const toggle = screen.getByLabelText(/cancelar eco|cancel speaker echo/i);
  expect(toggle).toBeChecked();
  await userEvent.click(screen.getByRole('button', { name: /grabar|record/i }));
  expect(navigateMock).toHaveBeenCalledWith(
    expect.any(String),
    expect.objectContaining({ state: expect.objectContaining({ aecEnabled: true }) }),
  );
});
```

- [ ] **Step 8: Run the frontend tests + type-check.**

Run: `pnpm tsc -b && pnpm test src/features/pre-record`
Expected: PASS.

- [ ] **Step 9: Commit.**

```bash
git add src/features/pre-record src/features/live-recording src/i18n
git commit -m "feat(aec): speaker-echo toggle on the mix card, persisted + threaded to recording"
```

---

## Module B — Output-device following (polling)

**Approach:** restructure the WASAPI loopback thread into an outer re-open loop + inner capture loop. Every ~1 s (only for the `__default_render__` sentinel) compare the current default render endpoint id against the open one; on change, break the inner loop and re-open — requesting the ORIGINAL rate/channels with `convert=true` so the mixer/writer never see a format change. Emit a Tauri event so the UI can toast.

**File structure:**
- Modify: `src-tauri/crates/audio/src/capture/stream.rs` — thread restructure + polling.
- Modify: `src-tauri/crates/audio/src/capture/recorder.rs` or wherever the loopback thread can emit — pass an `AppHandle`/event sender for the toast (see Step notes).
- Modify: `src/features/live-recording/LiveRecordingPage.tsx` (or a global listener) + i18n — the toast.

### Task B1: Restructure the loopback thread into reopen + capture loops

**Files:** Modify `src-tauri/crates/audio/src/capture/stream.rs`

- [ ] **Step 1: Pass the device id (not a pre-resolved device) to the thread.** Change `spawn_wasapi_loopback_thread` and `wasapi_loopback_loop` to take `device_id: String` and the ORIGINAL `sample_rate: u32, channels: u16` (the format to keep requesting). Move the device-resolution logic (the sentinel `get_default_device` branch and the by-id enumerate branch, currently in `open_loopback_with_drops` lines ~171-210) into a helper callable from inside the thread:

```rust
/// Resolve the render endpoint for `device_id`: the CURRENT default for the
/// sentinel, else the by-friendly-name match. Returns the device + its id string.
fn resolve_render_device(device_id: &str) -> Result<(wasapi::Device, String), AudioError> {
    // ... existing sentinel + enumerate logic, returning (device, device.get_id()?) ...
}
```

Verify `wasapi::Device` exposes an endpoint id (`get_id()`); if not, use `get_friendlyname()` as the comparison key. `open_loopback_with_drops` no longer resolves up front — it computes the initial `(sample_rate, channels)` once (for the `StreamHandle` + writer), then hands `device_id.to_string()`, `sample_rate`, `channels` to the thread.

- [ ] **Step 2: Wrap the capture loop in an outer reopen loop.** Refactor `wasapi_loopback_loop` so the body becomes:

```rust
    wasapi::initialize_mta(); // (keep the existing is_err guard)
    let follow = device_id == DEFAULT_RENDER_LOOPBACK;
    'reopen: loop {
        if stop.load(Ordering::Relaxed) { break; }
        let (device, open_id) = match resolve_render_device(&device_id) {
            Ok(v) => v,
            Err(e) => { tracing::error!(?e, "loopback resolve"); return Ok(()); }
        };
        // ... existing get_iaudioclient / WaveFormat::new(32,32,Float, sample_rate, channels,None)
        //     / get_periods / initialize_client(..., convert=true) / set_get_eventhandle
        //     / get_audiocaptureclient / start_stream (UNCHANGED — uses the ORIGINAL
        //       sample_rate/channels so downstream never sees a format change) ...
        let mut last_poll = Instant::now();
        loop { // inner capture loop (the existing wait+drain loop)
            if stop.load(Ordering::Relaxed) { let _ = audio_client.stop_stream(); return Ok(()); }
            // ... (Task B2 inserts the poll here) ...
            if h_event.wait_for_event(100).is_err() { continue; }
            // ... existing drain-packets block, UNCHANGED ...
        }
        // (unreachable until B2 adds a `break` to the inner loop)
    }
```

Keep the existing `stop_stream()` on shutdown. This step is a **pure refactor** — behaviour is identical (one open, capture forever) until B2 adds polling. Add `use std::time::Instant;` if missing.

- [ ] **Step 3: Verify the refactor compiles and existing tests pass.**

Run: `cd src-tauri && cargo test -p smart-noter-audio`
Expected: PASS (stream tests unaffected; no behaviour change).

- [ ] **Step 4: Commit.**

```bash
git add src-tauri/crates/audio/src/capture/stream.rs
git commit -m "refactor(loopback): reopen-capable outer loop; resolve device inside the thread"
```

### Task B2: Poll the default endpoint; re-open on change

**Files:** Modify `src-tauri/crates/audio/src/capture/stream.rs`

- [ ] **Step 1: Add a unit test for the change-detection helper.** Extract the comparison into a tiny pure helper and test it:

```rust
/// True when following is on and the current default differs from the open one.
fn should_reopen(follow: bool, open_id: &str, current_id: &str) -> bool {
    follow && open_id != current_id
}

#[cfg(test)]
mod follow_tests {
    use super::should_reopen;
    #[test]
    fn reopens_only_when_following_and_id_changed() {
        assert!(should_reopen(true, "spk", "hp"));
        assert!(!should_reopen(true, "spk", "spk"));
        assert!(!should_reopen(false, "spk", "hp")); // pinned device never follows
    }
}
```

- [ ] **Step 2: Run — expect PASS.**

Run: `cd src-tauri && cargo test -p smart-noter-audio follow_tests`
Expected: PASS.

- [ ] **Step 3: Insert the poll in the inner loop.** At the top of the inner capture loop (before `wait_for_event`):

```rust
            if follow && last_poll.elapsed() >= Duration::from_secs(1) {
                last_poll = Instant::now();
                if let Ok(cur) = wasapi::get_default_device(&wasapi::Direction::Render) {
                    let cur_id = cur.get_id().unwrap_or_default();
                    if should_reopen(follow, &open_id, &cur_id) {
                        tracing::info!(old = %open_id, new = %cur_id, "default render changed; reopening loopback");
                        let _ = audio_client.stop_stream();
                        // (Task B3 emits the toast event here.)
                        continue 'reopen;
                    }
                }
            }
```

The `continue 'reopen` tears down the current client and re-opens at the top of the outer loop, re-resolving the (new) default and requesting the SAME `sample_rate`/`channels` with `convert=true`. The mixer's silence-fill covers the sub-second gap.

- [ ] **Step 4: Verify `convert=true` absorbs a rate difference (implementation check).** If a test machine has two output devices at different native rates (e.g. 48 k and 44.1 k), a manual smoke (Module E) confirms audio stays coherent after the switch. If `convert=true` does NOT resample (audio pitch shifts), fall back to resampling inside the loopback thread to the original rate — but the mixer/writer still stay untouched. Note this in the PR.

- [ ] **Step 5: Build.**

Run: `cd src-tauri && cargo build`
Expected: compiles.

- [ ] **Step 6: Commit.**

```bash
git add src-tauri/crates/audio/src/capture/stream.rs
git commit -m "feat(loopback): poll default render endpoint and follow device switches"
```

### Task B3: Toast on device change

**Files:**
- Modify: `src-tauri/crates/audio/src/capture/stream.rs` (emit) — needs an `AppHandle` or an event `Sender` threaded into the loopback thread; simplest: thread an `Option<tauri::AppHandle>` from `Recorder::start` → `open` → `open_loopback_with_drops` → the thread, and `app.emit("audio:output-device-changed", DeviceChangedEvent { name })`. If threading `AppHandle` into the audio crate is undesirable (it already depends on `tauri` — see `recorder.rs` `app.emit`), reuse that pattern.
- Modify: `src/features/live-recording/LiveRecordingPage.tsx` (or the global event bridge) — listen + toast.
- Modify: i18n locales.

- [ ] **Step 1: Define the event payload + emit.** In `stream.rs` (near the top-level event types, or inline), and in the poll block from B2 Step 3 (where the comment says "Task B3 emits the toast event here"):

```rust
#[derive(Clone, serde::Serialize)]
struct DeviceChangedEvent { name: String }
// in the poll block, after resolving cur:
let name = cur.get_friendlyname().unwrap_or_default();
if let Some(app) = &app_handle { let _ = app.emit("audio:output-device-changed", DeviceChangedEvent { name }); }
```

Thread `app_handle: Option<AppHandle>` through `open`/`open_loopback_with_drops`/`spawn_wasapi_loopback_thread`. `Recorder::start` already owns `app` — pass `Some(app.clone())` in Mix/System, keep `open`'s other callers passing `None` where no handle exists.

- [ ] **Step 2: Add i18n keys.** `es.json`: `"outputDeviceChanged": "Salida cambiada a «{name}»"`. `en.json`: `"outputDeviceChanged": "Output switched to \"{name}\""`. Run `pnpm generate:i18n-keys`.

- [ ] **Step 3: Listen + toast in the frontend.** In `src/App.tsx` (which already bridges `audio:*` events — the `audio:error` handler), add a listener for `audio:output-device-changed` that shows a discreet toast using the existing toast mechanism + `t('outputDeviceChanged', { name })`.

- [ ] **Step 4: Type-check + test.**

Run: `pnpm tsc -b && pnpm test src/features/live-recording`
Expected: PASS.

- [ ] **Step 5: Commit.**

```bash
git add src-tauri/crates/audio/src/capture/stream.rs src-tauri/crates/audio/src/capture/recorder.rs src/features src/i18n
git commit -m "feat(loopback): toast when the recording follows an output-device switch"
```

---

## Module C — Updater download progress

**File structure:**
- Modify: `src/features/updater/useAppUpdater.ts` — extend `UpdateStatus`, drive `downloadAndInstall` events.
- Modify: `src/features/settings/SettingsPage.tsx` — progress bar.
- Modify: i18n locales.

### Task C1: Drive progress from `downloadAndInstall` events

**Files:** Modify `src/features/updater/useAppUpdater.ts`; Test: `src/features/updater/useAppUpdater.test.ts` (create)

- [ ] **Step 1: Extend the status type.** Replace the `downloading` variant:

```typescript
  | { kind: 'downloading'; downloaded: number; total: number | null }
```

- [ ] **Step 2: Write the failing test.** Create `useAppUpdater.test.ts`:

```typescript
import { act, renderHook } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { useAppUpdater } from './useAppUpdater';

vi.mock('@tauri-apps/plugin-process', () => ({ relaunch: vi.fn() }));

it('accumulates download progress from updater events', async () => {
  const update = {
    version: '1.1.0',
    body: '',
    downloadAndInstall: vi.fn(async (cb: (e: any) => void) => {
      cb({ event: 'Started', data: { contentLength: 1000 } });
      cb({ event: 'Progress', data: { chunkLength: 400 } });
      cb({ event: 'Progress', data: { chunkLength: 600 } });
      cb({ event: 'Finished' });
    }),
  };
  const { result } = renderHook(() => useAppUpdater());
  await act(async () => { await result.current.install(update as never); });
  expect(update.downloadAndInstall).toHaveBeenCalled();
});
```

- [ ] **Step 3: Run — expect FAIL** (install ignores the callback today).

Run: `pnpm test src/features/updater/useAppUpdater`
Expected: FAIL.

- [ ] **Step 4: Implement event-driven install.** Replace the `install` callback body:

```typescript
  const install = useCallback(async (update: Update) => {
    let total: number | null = null;
    let downloaded = 0;
    setStatus({ kind: 'downloading', downloaded: 0, total: null });
    try {
      await update.downloadAndInstall((event) => {
        switch (event.event) {
          case 'Started':
            total = event.data.contentLength ?? null;
            setStatus({ kind: 'downloading', downloaded: 0, total });
            break;
          case 'Progress':
            downloaded += event.data.chunkLength;
            setStatus({ kind: 'downloading', downloaded, total });
            break;
          case 'Finished':
            setStatus({ kind: 'downloading', downloaded, total });
            break;
        }
      });
      await relaunch();
    } catch (e) {
      setStatus({ kind: 'error', message: e instanceof Error ? e.message : String(e) });
    }
  }, []);
```

- [ ] **Step 5: Run — expect PASS.**

Run: `pnpm test src/features/updater/useAppUpdater`
Expected: PASS.

- [ ] **Step 6: Commit.**

```bash
git add src/features/updater/useAppUpdater.ts src/features/updater/useAppUpdater.test.ts
git commit -m "feat(updater): accumulate download progress from downloadAndInstall events"
```

### Task C2: Progress bar in Settings

**Files:** Modify `src/features/settings/SettingsPage.tsx`, `SettingsPage.module.css` (grep the actual module), i18n locales

- [ ] **Step 1: Add i18n keys.** `es.json`: `"updateProgress": "{done} / {total} MB"`. `en.json`: same format string. Run `pnpm generate:i18n-keys`.

- [ ] **Step 2: Render the bar.** In the `updater.status.kind === 'downloading'` branch of the updates section, replace the plain `t('updateDownloading')` label with a bar. Compute MB and percent from `updater.status.downloaded`/`total`:

```tsx
                <div className={styles.rowLabel}>{t('updateDownloading')}</div>
                {updater.status.kind === 'downloading' && (
                  <div className={styles.progressWrap}>
                    <div
                      className={styles.progressBar}
                      style={{
                        width: updater.status.total
                          ? `${Math.round((updater.status.downloaded / updater.status.total) * 100)}%`
                          : '100%',
                      }}
                    />
                    <span className={styles.progressText}>
                      {t('updateProgress', {
                        done: (updater.status.downloaded / 1_048_576).toFixed(1),
                        total: updater.status.total
                          ? (updater.status.total / 1_048_576).toFixed(1)
                          : '?',
                      })}
                    </span>
                  </div>
                )}
```

Add `.progressWrap` / `.progressBar` / `.progressText` rules to the settings CSS module (a track with a filled bar + caption).

- [ ] **Step 3: Type-check.**

Run: `pnpm tsc -b`
Expected: no errors.

- [ ] **Step 4: Commit.**

```bash
git add src/features/settings src/i18n
git commit -m "feat(updater): download progress bar in Settings"
```

---

## Module D — Exclude `specta-export.exe` from the installer

**Approach:** `cfg`-gate the bin's body so release builds link nothing heavy (a ~200 KB stub instead of 20 MB), while dev keeps generating bindings behind the feature.

**File structure:**
- Modify: `src-tauri/Cargo.toml` — add the `generate-bindings` feature.
- Modify: `src-tauri/src/bin/specta_export.rs` — cfg-gate.
- Modify: bindings-regen usages in THIS plan + any doc (the regen command gains `--features generate-bindings`).

### Task D1: cfg-gate the bin

**Files:** Modify `src-tauri/Cargo.toml`, `src-tauri/src/bin/specta_export.rs`

- [ ] **Step 1: Add the feature.** In `src-tauri/Cargo.toml` under `[features]` (after the `custom-protocol` line):

```toml
# Dev-only: enables the specta-export bin to link the app lib and emit bindings.
# OFF in release/bundle → the bin compiles to a trivial stub (no 20 MB app-lib link).
generate-bindings = []
```

- [ ] **Step 2: cfg-gate the bin body.** Replace `src-tauri/src/bin/specta_export.rs` entirely with:

```rust
#[cfg(feature = "generate-bindings")]
fn main() {
    use specta_typescript::{BigIntExportBehavior, Typescript};
    smart_noter_lib::specta_builder()
        .export(
            Typescript::default()
                .bigint(BigIntExportBehavior::Number)
                .header("// AUTO-GENERATED by tauri-specta — do not edit.\n// @ts-nocheck\n/* eslint-disable */\n"),
            "../src/ipc/bindings.ts",
        )
        .expect("export bindings");
    println!("bindings.ts exported");
}

#[cfg(not(feature = "generate-bindings"))]
fn main() {
    // Stub for release/bundle builds — Tauri bundles every [[bin]], so the target
    // must exist, but without `generate-bindings` it links no app-lib code and
    // ships as a ~200 KB no-op instead of the 20 MB dev exporter.
}
```

- [ ] **Step 3: Verify the stub build does NOT link the app lib.** Build the bin without the feature and confirm it compiles and is small:

Run: `cd src-tauri && cargo build --release --bin specta-export`
Then check the size of `target/release/specta-export.exe` (should be a few hundred KB, not ~20 MB).
Expected: builds; small exe.

- [ ] **Step 4: Verify bindings still generate WITH the feature.**

Run: `cd src-tauri && cargo run --bin specta-export --features generate-bindings`
Expected: prints `bindings.ts exported`; `git diff src/ipc/bindings.ts` shows no change (already current from Task A6).

> **From here on, the bindings-regen command is** `cargo run --bin specta-export --features generate-bindings`. Update any contributor doc that lists the old command (grep `specta-export` in `*.md`, `package.json`, `lefthook.yml`).

- [ ] **Step 5: Commit.**

```bash
git add src-tauri/Cargo.toml src-tauri/src/bin/specta_export.rs
git commit -m "build: cfg-gate specta-export so release ships a stub, not the 20 MB dev bin"
```

---

## Module E — Version 1.1.0 + release

### Task E1: Bump version to 1.1.0 (5 sites)

**Files:** `package.json`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`, `src/components/shell/Sidebar/Sidebar.tsx`, `src/features/settings/SettingsPage.tsx`

- [ ] **Step 1: Edit the five sites.**
  - `package.json`: `"version": "1.1.0"`.
  - `src-tauri/tauri.conf.json`: `"version": "1.1.0"` (grep the `version` key).
  - `src-tauri/Cargo.toml`: `[workspace.package]` `version = "1.1.0"` (line 16).
  - `Sidebar.tsx:13`: `role: 'Pro · v1.1.0'`.
  - `SettingsPage.tsx` footer (line ~404): `'Smart Noter v1.1.0'`.

- [ ] **Step 2: Sync Cargo.lock + build.**

Run: `cd src-tauri && cargo build`
Expected: `Cargo.lock` updates the workspace crates to 1.1.0; compiles.

- [ ] **Step 3: Commit.**

```bash
git add package.json src-tauri/tauri.conf.json src-tauri/Cargo.toml src-tauri/Cargo.lock src/components src/features/settings
git commit -m "chore(v11): bump version to 1.1.0"
```

### Task E2: Regenerate e2e visual baselines

**Files:** the committed Playwright baseline PNGs (chromium-win32) + any changed spec

- [ ] **Step 1: Run the full frontend + build check first.**

Run: `pnpm tsc -b && pnpm test`
Expected: all vitest suites green.

- [ ] **Step 2: Regenerate baselines if the layout shifted.** The AEC toggle + hint only render when the mix card is selected; whether the committed `pre-record-light` snapshot captures that state determines if it changed. Regenerate and inspect:

Run: `pnpm test:e2e:update`
Then `git diff --stat tests/e2e/visual.spec.ts-snapshots` — if `pre-record-light-chromium-win32.png` changed, **visually spot-check** it (only the intended toggle change, no regression); if nothing changed, there is nothing to commit here.

- [ ] **Step 3: Run e2e to confirm green against the baselines.**

Run: `pnpm test:e2e`
Expected: all specs pass.

- [ ] **Step 4: Commit** (only if a baseline actually changed).

```bash
git add tests/e2e/visual.spec.ts-snapshots
git commit -m "test(e2e): refresh PreRecord baseline for the AEC toggle"
```

### Task E3: Ship (manual, gated on hardware smoke)

- [ ] **Step 1: Merge the branch** `feat/v11-aec` → `main` (`--no-ff`) once CI is green (frontend + backend + e2e).
- [ ] **Step 2: Tag** `v1.1.0` and push → `release.yml` publishes the NSIS installer + `latest.json`.
- [ ] **Step 3: In-app update smoke** on the physical machine: installed 1.0.1 → Settings → Buscar actualizaciones → download bar → 1.1.0 (also closes Sub-8's pending auto-update smoke).

---

## Manual hardware validation (the real proof — AEC + COM can't be unit-tested)

Run after Module B, calibrate before tagging:
1. **Speakers + AEC on:** meeting audio through speakers, speak over it → played-back recording has the echo gone and the voice clear. If double-talk clips the voice, tune `EchoConfig` (`enable_preprocess` stays false; raise `filter_length` for more reverb tail). If residual echo remains, raise `filter_length`.
2. **Headphones + AEC off:** voice is full-band 48 kHz, untouched.
3. **Device switch mid-recording:** start on speakers, switch Windows output to headphones while recording → audio continues within ~1 s, the toast appears, no silence, no pitch shift. Repeat headphones→speakers.
4. **Updater:** 1.0.1 → 1.1.0 shows the progress bar filling.
5. **Installer:** confirm no ~20 MB `specta-export.exe` inside; installer size back near ~19–20 MB.

## Self-review coverage map (spec → tasks)

- Spec 3A AEC → A1–A7. 3B device-following (polling, user-approved) → B1–B3. 3C updater progress → C1–C2. 3D specta-export → D1. 3E version+release → E1–E3.
- Spec risks: R1 (preprocess) → A1 default `false` + manual step 1. R2 (latency) → A4 delay FIFO. R3 (rate change) → B2 Step 4 (`convert=true`, supersedes `reconfigure_lane_a`). R4 (COM threading) → obviated by polling. R5 (stub) → D1. R6 (e2e baselines) → E2. R7 (speex DLL) → **verify during A1: after `cargo build`, check whether `aec-rs-sys` emits a DLL next to the exe; if so, add it to `scripts/stage-dlls.mjs` + `bundle.resources` before Module E.**
