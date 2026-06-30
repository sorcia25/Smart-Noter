# Sub-6 Module C — Cloud STT Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add cloud speech-to-text (OpenAI + Azure OpenAI Whisper) behind a new `Transcriber` trait, selected by `settings.transcription_provider`, with local sherpa diarization aligned to the cloud lines unchanged. Local stays the default + fallback.

**Architecture:** A `Transcriber` trait in `core` (`wav_path → Vec<TranscribedLine{start_ms,end_ms,text}>`). The local whisper engine is wrapped in a `LocalTranscriber` (decodes the WAV internally). Cloud adapters in `providers/src/stt.rs` upload the audio in ~10-min chunks (`POST /audio/transcriptions`, `verbose_json`), concatenating segments with per-chunk timestamp offsets. A `transcriber()` factory picks local vs cloud; `transcription.rs` calls it instead of the whisper engine directly — diarization + alignment + persistence are untouched.

**Tech Stack:** `reqwest::blocking` (multipart upload), `serde_json` (verbose_json), `hound` (in-memory 16 kHz mono WAV chunks), the existing `whisper`/`diarize` crates, `tiny_http` (adapter tests).

**Spec:** `docs/superpowers/specs/2026-06-30-sub6c-cloud-stt-design.md`. Modules A + B shipped (main==5f11c94).

**Conventions (verified):** `crates/whisper` already depends on `smart-noter-core` + `hound`. `Segment{start_ms:u32,end_ms:u32,text:String}`. `decode_to_pcm_16k_mono(&Path) -> Result<Vec<f32>, TranscriptionError>`. Trait error type is `String`. Env preamble (PATH+LIBCLANG_PATH) on every cargo/git command. `cargo fmt` from inside `src-tauri/`. lefthook fmt+clippy — never `--no-verify`. `git add src-tauri/`/`src/` (NOT `-A`; untracked `.claude/` stays out). `bindings.ts` gitignored (regen via specta-export, don't commit).

**Env preamble (prepend to EVERY cargo/git command):**
```bash
export PATH="$HOME/.cargo/bin:/c/Program Files (x86)/Microsoft Visual Studio/2022/BuildTools/Common7/IDE/CommonExtensions/Microsoft/CMake/CMake/bin:$PATH"
export LIBCLANG_PATH="C:/Program Files/LLVM/bin"
cd "C:/Users/erick/Projects/Smart Noter"
```

---

## File Structure

| File | Responsibility |
|------|----------------|
| `crates/core/src/traits.rs` (modify) | add `Transcriber` trait + `TranscribeInput` + `TranscribedLine` |
| `crates/whisper/src/local_transcriber.rs` (create) | `LocalTranscriber` impl `Transcriber` (decode + engine + map) |
| `crates/whisper/src/lib.rs` (modify) | `pub mod local_transcriber;` + re-export |
| `crates/providers/Cargo.toml` (modify) | reqwest `multipart` feature + `hound` dep |
| `crates/providers/src/stt.rs` (create) | `OpenAiStt` + `AzureStt` + shared chunking + verbose_json parse |
| `crates/providers/src/lib.rs` (modify) | export STT adapters |
| `crates/core/src/models/settings.rs` (modify) | `transcription_models` map + `transcription_model_for` |
| `src/commands/provider_factory.rs` (modify) | `transcriber()` + STT key/model resolution |
| `src/commands/transcription.rs` (modify) | call factory (local vs cloud); conditional whisper-model check |
| `src/features/settings/TranscriptionPanel.tsx` (modify) | `transcription_provider` selector + Azure deployment field |

---

## Task C1: `Transcriber` trait + `LocalTranscriber`

**Files:** `crates/core/src/traits.rs`, `crates/whisper/src/local_transcriber.rs` (create), `crates/whisper/src/lib.rs`.

This adds the trait and wraps the local engine. The transcription job is NOT touched yet (that's C3) — local behavior is unchanged because nothing calls `LocalTranscriber` until C3.

- [ ] **Step 1 — Add the trait + types to `core/src/traits.rs`.** Append:
```rust
use std::path::PathBuf;

/// Input for a single transcription request.
pub struct TranscribeInput {
    pub wav_path: PathBuf,
    pub lang: Option<String>, // hint; None = auto-detect
}

/// One transcribed line with millisecond timestamps. Mirrors the local whisper
/// `Segment` and the diarization aligner's `TextSegment` so any transcriber's
/// output feeds `align()` unchanged.
pub struct TranscribedLine {
    pub start_ms: u32,
    pub end_ms: u32,
    pub text: String,
}

/// Produces timestamped lines from a meeting's audio. Execution is synchronous
/// (spawned in a worker thread); `progress` is 0–100, `abort` is checked
/// cooperatively. Error type is `String` so `core` stays dependency-free.
pub trait Transcriber: Send + Sync {
    fn transcribe(
        &self,
        input: &TranscribeInput,
        progress: &mut dyn FnMut(u32),
        abort: &AtomicBool,
    ) -> Result<Vec<TranscribedLine>, String>;
}
```
(`AtomicBool` is already imported at the top of `traits.rs` — confirm; if not, add `use std::sync::atomic::AtomicBool;`.)

- [ ] **Step 2 — Build core.**
```bash
cargo build -p smart-noter-core --manifest-path src-tauri/Cargo.toml
```
Expected: compiles.

- [ ] **Step 3 — Create `crates/whisper/src/local_transcriber.rs`.**
```rust
use crate::decode::decode_to_pcm_16k_mono;
use crate::transcribe::{transcribe, Segment, TranscribeOpts};
use smart_noter_core::traits::{TranscribeInput, TranscribedLine, Transcriber};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

/// Local whisper.cpp transcriber. Decodes the WAV to 16 kHz mono PCM and runs the
/// engine; maps each `Segment` to a `TranscribedLine`.
pub struct LocalTranscriber {
    pub model_path: PathBuf,
    pub n_threads: i32,
}

fn segment_to_line(s: Segment) -> TranscribedLine {
    TranscribedLine { start_ms: s.start_ms, end_ms: s.end_ms, text: s.text }
}

impl Transcriber for LocalTranscriber {
    fn transcribe(
        &self,
        input: &TranscribeInput,
        progress: &mut dyn FnMut(u32),
        abort: &AtomicBool,
    ) -> Result<Vec<TranscribedLine>, String> {
        let pcm = decode_to_pcm_16k_mono(&input.wav_path).map_err(|e| e.message)?;
        let opts = TranscribeOpts { n_threads: self.n_threads, language: input.lang.clone() };
        // The engine takes an owned Arc<AtomicBool>; bridge by polling the borrowed
        // flag into a fresh Arc the engine can hold.
        let engine_abort = Arc::new(AtomicBool::new(abort.load(std::sync::atomic::Ordering::Relaxed)));
        let segments = transcribe(&pcm, &self.model_path, &opts, |p| progress(p), engine_abort)
            .map_err(|e| e.message)?;
        Ok(segments.into_iter().map(segment_to_line).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segment_maps_to_line_preserving_fields() {
        let s = Segment { start_ms: 1500, end_ms: 3200, text: "hola".into() };
        let l = segment_to_line(s);
        assert_eq!(l.start_ms, 1500);
        assert_eq!(l.end_ms, 3200);
        assert_eq!(l.text, "hola");
    }
}
```
NOTE on `abort`: the engine's `transcribe` wants `Arc<AtomicBool>` but the trait gives `&AtomicBool`. The snapshot bridge above is a known limitation — abort set AFTER `transcribe` begins won't reach the engine through this Arc. For C1 this preserves behavior (the job's local path will pass the real Arc in C3). **In C3, when wiring the job, pass the job's actual `Arc<AtomicBool>` to a `LocalTranscriber` variant OR keep the local branch calling the engine directly with the real Arc.** Flag this in your report so C3 handles it. (Simplest C3 resolution: the local branch in `transcription.rs` keeps calling the engine `transcribe(&pcm, ..., abort.clone())` directly — `LocalTranscriber` is used only as the trait object for the *cloud-shaped* factory signature, and for local the job stays direct. Decide in C3.)

- [ ] **Step 4 — Register + build + test whisper.** In `crates/whisper/src/lib.rs` add `pub mod local_transcriber;` and `pub use local_transcriber::LocalTranscriber;`.
```bash
cargo test -p smart-noter-whisper --manifest-path src-tauri/Cargo.toml
```
Expected: existing whisper tests + the new `segment_maps_to_line_preserving_fields` pass.

- [ ] **Step 5 — fmt + commit.**
```bash
(cd src-tauri && cargo fmt)
git add src-tauri/
git commit -m "feat(sub6c): Transcriber trait + LocalTranscriber wrapper"
```

---

## Task C2: STT cloud adapters + chunking

**Files:** `crates/providers/Cargo.toml`, `crates/providers/src/stt.rs` (create), `crates/providers/src/lib.rs`.

- [ ] **Step 1 — Cargo.toml deps.** In `crates/providers/Cargo.toml`, change the reqwest line to add `multipart`, and add `hound`:
```toml
reqwest = { version = "0.12", default-features = false, features = ["blocking", "json", "rustls-tls", "multipart"] }
hound = "3.5"
```
(Keep `smart-noter-core`, `serde`, `serde_json`, and dev-dep `tiny_http`.)

- [ ] **Step 2 — Write the shared helpers + chunking with failing tests first.** Create `crates/providers/src/stt.rs`:
```rust
use smart_noter_core::traits::{TranscribeInput, TranscribedLine, Transcriber};
use std::io::Cursor;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::http_common::status_to_err;

const SAMPLE_RATE: u32 = 16_000;
const CHUNK_SECS: usize = 600; // ~10 min; ~19 MB as 16-bit mono WAV, under the 25 MB cap

/// Encode a 16 kHz mono f32 PCM slice to an in-memory 16-bit WAV (for multipart upload).
pub(crate) fn pcm_to_wav_bytes(pcm: &[f32]) -> Vec<u8> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: SAMPLE_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut cursor = Cursor::new(Vec::<u8>::new());
    {
        let mut w = hound::WavWriter::new(&mut cursor, spec).expect("wav writer");
        for &s in pcm {
            let v = (s.clamp(-1.0, 1.0) * 32767.0) as i16;
            w.write_sample(v).expect("wav sample");
        }
        w.finalize().expect("wav finalize");
    }
    cursor.into_inner()
}

/// Parse an OpenAI/Azure `verbose_json` transcription response into lines, adding
/// `offset_ms` to every timestamp (so chunk N's lines sit at their real position).
pub(crate) fn parse_verbose_json(body: &serde_json::Value, offset_ms: u32) -> Vec<TranscribedLine> {
    body["segments"]
        .as_array()
        .map(|segs| {
            segs.iter()
                .filter_map(|s| {
                    let start = s["start"].as_f64()?;
                    let end = s["end"].as_f64()?;
                    let text = s["text"].as_str()?.trim().to_string();
                    if text.is_empty() {
                        return None;
                    }
                    Some(TranscribedLine {
                        start_ms: (start * 1000.0) as u32 + offset_ms,
                        end_ms: (end * 1000.0) as u32 + offset_ms,
                        text,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Number of CHUNK_SECS windows for a PCM length (at SAMPLE_RATE).
pub(crate) fn chunk_count(pcm_len: usize) -> usize {
    let per = CHUNK_SECS * SAMPLE_RATE as usize;
    pcm_len.div_ceil(per)
}

pub(crate) fn client() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(300))
        .build()
        .expect("client")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_verbose_json_offsets_timestamps() {
        let body = serde_json::json!({
            "segments": [
                {"start": 0.0, "end": 1.5, "text": " hola"},
                {"start": 1.5, "end": 2.0, "text": "mundo "},
                {"start": 2.0, "end": 2.0, "text": "   "}  // blank → dropped
            ]
        });
        let lines = parse_verbose_json(&body, 600_000); // 10-min offset
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].start_ms, 600_000);
        assert_eq!(lines[0].end_ms, 601_500);
        assert_eq!(lines[0].text, "hola");
        assert_eq!(lines[1].start_ms, 601_500);
        assert_eq!(lines[1].text, "mundo");
    }

    #[test]
    fn chunk_count_splits_by_ten_minutes() {
        let per = CHUNK_SECS * SAMPLE_RATE as usize;
        assert_eq!(chunk_count(0), 0);
        assert_eq!(chunk_count(per), 1);
        assert_eq!(chunk_count(per + 1), 2);
        assert_eq!(chunk_count(per * 3), 3);
    }

    #[test]
    fn pcm_to_wav_bytes_has_riff_header() {
        let bytes = pcm_to_wav_bytes(&[0.0, 0.5, -0.5, 1.0]);
        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(&bytes[8..12], b"WAVE");
    }
}
```
Run them: `cargo test -p smart-noter-providers --manifest-path src-tauri/Cargo.toml stt::` — expect PASS (these are the pure helpers).

- [ ] **Step 3 — Add the two adapters + the shared chunked-transcribe loop** to `stt.rs`:
```rust
/// Shared chunked upload loop. `post_chunk(wav_bytes, lang) -> Result<serde_json::Value, String>`
/// is the per-provider HTTP call; this handles decode, splitting, offsetting, abort, progress.
fn transcribe_chunked(
    input: &TranscribeInput,
    progress: &mut dyn FnMut(u32),
    abort: &AtomicBool,
    post_chunk: impl Fn(Vec<u8>, Option<&str>) -> Result<serde_json::Value, String>,
) -> Result<Vec<TranscribedLine>, String> {
    // Decode via the whisper crate's decoder — providers already depends on core only,
    // so re-decode here with hound directly to avoid a whisper dep: read the WAV.
    let pcm = crate::stt::decode_wav_16k_mono(&input.wav_path)?;
    let per = CHUNK_SECS * SAMPLE_RATE as usize;
    let total = chunk_count(pcm.len());
    let mut lines = Vec::new();
    for (i, chunk) in pcm.chunks(per).enumerate() {
        if abort.load(Ordering::Relaxed) {
            return Err("cancelado".to_string());
        }
        let wav = pcm_to_wav_bytes(chunk);
        let body = post_chunk(wav, input.lang.as_deref())?;
        let offset_ms = (i * CHUNK_SECS * 1000) as u32;
        lines.extend(parse_verbose_json(&body, offset_ms));
        progress(((i + 1) * 100 / total.max(1)) as u32);
    }
    Ok(lines)
}

pub struct OpenAiStt {
    pub api_key: String,
    pub base: String, // default "https://api.openai.com/v1"
}
impl OpenAiStt {
    pub fn new(api_key: String) -> Self {
        Self { api_key, base: "https://api.openai.com/v1".to_string() }
    }
}
impl Transcriber for OpenAiStt {
    fn transcribe(
        &self,
        input: &TranscribeInput,
        progress: &mut dyn FnMut(u32),
        abort: &AtomicBool,
    ) -> Result<Vec<TranscribedLine>, String> {
        let key = self.api_key.clone();
        let url = format!("{}/audio/transcriptions", self.base);
        transcribe_chunked(input, progress, abort, |wav, lang| {
            post_multipart(&url, &[("Authorization", format!("Bearer {key}"))], wav, "whisper-1", lang, "OpenAI")
        })
    }
}

pub struct AzureStt {
    pub endpoint: String,
    pub deployment: String,
    pub api_key: String,
}
impl AzureStt {
    pub fn new(endpoint: String, deployment: String, api_key: String) -> Self {
        Self { endpoint: endpoint.trim_end_matches('/').to_string(), deployment, api_key }
    }
}
impl Transcriber for AzureStt {
    fn transcribe(
        &self,
        input: &TranscribeInput,
        progress: &mut dyn FnMut(u32),
        abort: &AtomicBool,
    ) -> Result<Vec<TranscribedLine>, String> {
        let key = self.api_key.clone();
        let url = format!(
            "{}/openai/deployments/{}/audio/transcriptions?api-version=2024-06-01",
            self.endpoint, self.deployment
        );
        transcribe_chunked(input, progress, abort, |wav, lang| {
            // Azure ignores the model field (deployment is in the URL); send a placeholder.
            post_multipart(&url, &[("api-key", key.clone())], wav, "whisper-1", lang, "Azure")
        })
    }
}

/// One multipart POST → verbose_json Value. `headers` are (name, value) auth pairs.
fn post_multipart(
    url: &str,
    headers: &[(&str, String)],
    wav: Vec<u8>,
    model: &str,
    lang: Option<&str>,
    provider: &str,
) -> Result<serde_json::Value, String> {
    let part = reqwest::blocking::multipart::Part::bytes(wav)
        .file_name("chunk.wav")
        .mime_str("audio/wav")
        .map_err(|e| e.to_string())?;
    let mut form = reqwest::blocking::multipart::Form::new()
        .part("file", part)
        .text("model", model.to_string())
        .text("response_format", "verbose_json".to_string());
    if let Some(l) = lang {
        form = form.text("language", l.to_string());
    }
    let mut req = client().post(url);
    for (k, v) in headers {
        req = req.header(*k, v);
    }
    let resp = req
        .multipart(form)
        .send()
        .map_err(|e| format!("sin conexión con {provider}: {e}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().unwrap_or_default();
        let trunc: String = body.chars().take(300).collect();
        return Err(format!("{} — {}", status_to_err(status, provider), trunc));
    }
    resp.json::<serde_json::Value>().map_err(|e| e.to_string())
}

/// Decode a WAV file to 16 kHz mono f32 PCM. Providers can't depend on the whisper
/// crate (would create a cycle via core), so read WAV directly with hound and
/// downmix/resample minimally. (FLAC is decoded by the local path only; cloud STT
/// requires WAV input — the capture pipeline writes WAV by default.)
pub(crate) fn decode_wav_16k_mono(path: &std::path::Path) -> Result<Vec<f32>, String> {
    let mut reader = hound::WavReader::open(path).map_err(|e| format!("abrir WAV: {e}"))?;
    let spec = reader.spec();
    let raw: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader
            .samples::<f32>()
            .collect::<Result<_, _>>()
            .map_err(|e| e.to_string())?,
        hound::SampleFormat::Int => {
            let max = (1i64 << (spec.bits_per_sample - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|s| s.map(|v| v as f32 / max))
                .collect::<Result<_, _>>()
                .map_err(|e| e.to_string())?
        }
    };
    let ch = spec.channels.max(1) as usize;
    let mono: Vec<f32> = if ch == 1 {
        raw
    } else {
        raw.chunks_exact(ch).map(|f| f.iter().sum::<f32>() / ch as f32).collect()
    };
    Ok(if spec.sample_rate == SAMPLE_RATE {
        mono
    } else {
        resample_linear(&mono, spec.sample_rate, SAMPLE_RATE)
    })
}

fn resample_linear(input: &[f32], from: u32, to: u32) -> Vec<f32> {
    if input.is_empty() || from == to {
        return input.to_vec();
    }
    let ratio = to as f64 / from as f64;
    let out_len = ((input.len() as f64) * ratio).round() as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let src = i as f64 / ratio;
        let i0 = src.floor() as usize;
        let i1 = (i0 + 1).min(input.len() - 1);
        let frac = (src - i0 as f64) as f32;
        out.push(input[i0] * (1.0 - frac) + input[i1] * frac);
    }
    out
}
```
NOTE: `decode_wav_16k_mono` duplicates the whisper decoder's WAV path because `providers` must not depend on `whisper` (which depends on `core`, and `providers` on `core` — adding `providers → whisper` is fine acyclically, BUT keeping providers lean avoids pulling whisper-rs/ggml into the providers build). If the reviewer prefers, `providers` MAY depend on `smart-noter-whisper` and call `decode_to_pcm_16k_mono` instead (handles FLAC too) — decide in review; the duplication is small and WAV-only is acceptable since capture writes WAV.

- [ ] **Step 4 — Adapter tests against a `tiny_http` mock.** Add to `stt.rs` tests: bind `127.0.0.1:0`, build `OpenAiStt{ base: "http://127.0.0.1:{port}/v1", .. }`, write a small temp WAV (use `hound` like `decode.rs`'s test helper), spawn a server thread that accepts the multipart POST and returns canned `verbose_json`, assert `transcribe` returns the expected lines. Cover: single-chunk transcription parses lines; an abort flag set before the call returns `Err("cancelado")`. (A multi-chunk test would need a >10-min WAV — skip the real multi-chunk upload; the offset logic is already unit-tested via `parse_verbose_json`.)

- [ ] **Step 5 — Export + build + test + commit.** In `crates/providers/src/lib.rs` add `pub mod stt;` + `pub use stt::{AzureStt, OpenAiStt};`.
```bash
cargo test -p smart-noter-providers --manifest-path src-tauri/Cargo.toml
(cd src-tauri && cargo fmt)
git add src-tauri/
git commit -m "feat(sub6c): OpenAI + Azure STT adapters with chunked multipart upload"
```

---

## Task C3: Factory + settings + transcription.rs wiring

**Files:** `crates/core/src/models/settings.rs`, `src/commands/provider_factory.rs`, `src/commands/transcription.rs`, `src/commands/mod.rs` (if needed).

- [ ] **Step 1 — Settings: per-provider transcription model.** In `settings.rs`, add after `transcription_model`:
```rust
    #[serde(default)]
    pub transcription_models: std::collections::BTreeMap<String, String>,
```
add to `Default`: `transcription_models: std::collections::BTreeMap::new(),`. Add an accessor:
```rust
impl AppSettings {
    /// The STT model/deployment for a cloud transcription provider. OpenAI is always
    /// `whisper-1`; Azure uses its configured whisper deployment; local uses the GGUF id.
    pub fn transcription_model_for(&self, provider: &str) -> String {
        match provider {
            "openai" => "whisper-1".to_string(),
            "azure" => self
                .transcription_models
                .get("azure")
                .filter(|m| !m.is_empty())
                .cloned()
                .unwrap_or_default(),
            _ => self.transcription_model.clone(), // local GGUF id
        }
    }
}
```
Add a test:
```rust
#[test]
fn transcription_model_for_per_provider() {
    let mut s = AppSettings::default();
    assert_eq!(s.transcription_model_for("openai"), "whisper-1");
    assert_eq!(s.transcription_model_for("azure"), "");
    assert_eq!(s.transcription_model_for("local"), s.transcription_model);
    s.transcription_models.insert("azure".into(), "my-whisper".into());
    assert_eq!(s.transcription_model_for("azure"), "my-whisper");
}
```
Run `cargo test -p smart-noter-core` — green.

- [ ] **Step 2 — Factory `transcriber()`.** In `provider_factory.rs`, add (reusing `resolve_provider`'s key-decrypt pattern — note STT uses `transcription_provider`, NOT `ai_provider`, but the SAME per-provider DPAPI key):
```rust
use smart_noter_providers::{AzureStt, OpenAiStt};
use smart_noter_core::traits::Transcriber;

/// Build a CLOUD transcriber for a non-"local" transcription provider. The decrypted
/// `key` is the same per-provider secret the LLM uses. Errors on missing config.
pub fn cloud_transcriber(
    provider: &str,
    settings: &AppSettings,
    key: &str,
) -> Result<Box<dyn Transcriber>, String> {
    match provider {
        "openai" => Ok(Box::new(OpenAiStt::new(key.to_string()))),
        "azure" => {
            if settings.azure_endpoint.trim().is_empty() {
                return Err("configura el endpoint de Azure en Configuración".to_string());
            }
            let deployment = settings.transcription_model_for("azure");
            if deployment.is_empty() {
                return Err("configura el deployment de Whisper de Azure en Configuración".to_string());
            }
            Ok(Box::new(AzureStt::new(settings.azure_endpoint.clone(), deployment, key.to_string())))
        }
        other => Err(format!("proveedor de transcripción desconocido: {other}")),
    }
}

/// Resolve (provider, settings, key) for the TRANSCRIPTION domain (mirrors
/// `resolve_provider` but reads `transcription_provider`).
pub async fn resolve_transcription_provider(
    pool: &sqlx::SqlitePool,
) -> Result<(String, AppSettings, String), String> {
    let settings = settings_repo::get(pool).await.map_err(|e| format!("settings: {e}"))?;
    let provider = settings.transcription_provider.clone();
    let key = if provider == "local" {
        String::new()
    } else {
        match secrets_repo::get(pool, &provider).await.map_err(|e| format!("secrets: {e}"))? {
            Some(ct) => crate::secrets::decrypt(&ct).map_err(|e| format!("no se pudo leer la API key: {e}"))?,
            None => return Err("configura la API key del proveedor en Configuración".to_string()),
        }
    };
    Ok((provider, settings, key))
}
```
Add factory tests mirroring the LLM ones: `cloud_transcriber("openai", &s, "k")` is `Ok`; `"azure"` with empty endpoint → `Err` containing "endpoint"; `"azure"` with endpoint but no deployment → `Err` containing "deployment"; unknown → `Err`.

- [ ] **Step 3 — Wire `transcription.rs`.** Read the current job (lines ~78-444). Make these changes:
  1. **Conditional whisper-model check:** the up-front model-file validation (~lines 105-144) must only run when `settings.transcription_provider == "local"`. For cloud providers, skip it (no GGUF needed). Keep the sherpa diarization-model check as-is (diarization is always local when `identify_speakers`).
  2. **Resolve the transcription provider** inside the worker (or before spawn, async): `let (provider, settings, key) = block_on(provider_factory::resolve_transcription_provider(&pool))` — on `Err`, emit `transcription:failed` with code `"ConfigError"` and return.
  3. **Replace the direct whisper call** (~line 212 `transcribe(&pcm, &model_path, &opts, progress, abort)`) with a provider branch:
     - **local:** keep the existing direct engine call `transcribe(&pcm, &model_path, &opts, progress, abort.clone())` (preserves the real-Arc abort; LocalTranscriber's Arc-snapshot limitation noted in C1 means the direct call is preferred for local).
     - **cloud:** `let t = provider_factory::cloud_transcriber(&provider, &settings, &key)?; let input = TranscribeInput { wav_path: audio_path.clone(), lang: Some("es".into()) }; t.transcribe(&input, &mut progress_cb, &abort)` → `Vec<TranscribedLine>`. Map `TranscribedLine{start_ms,end_ms,text}` → the existing `Segment{start_ms,end_ms,text}` (or adapt the downstream code to take `TranscribedLine` — they're structurally identical; simplest is to build the `Vec<Segment>` the rest of the job already uses, OR build `TextSegment` for `align` directly). Keep `progress`/`abort` semantics.
  4. **Diarization + alignment + persistence are UNCHANGED.** The cloud lines (start_ms/end_ms/text) feed the same `TextSegment`→`align()`→`speaker_idx`→`LineInput`→`replace_lines` path. For cloud, the job still decodes the PCM (for diarization) when `identify_speakers` is on.
  Verify the local path still produces identical output (run the existing transcription tests if any; otherwise build + the smoke in C5 covers it).

- [ ] **Step 4 — Build, the suites, regen bindings, commit.**
```bash
cargo build -p smart-noter --manifest-path src-tauri/Cargo.toml
cargo test -p smart-noter-core -p smart-noter-db -p smart-noter -p smart-noter-providers -p smart-noter-whisper --manifest-path src-tauri/Cargo.toml
cargo run --bin specta-export --manifest-path src-tauri/Cargo.toml   # transcription_models added → regen
(cd src-tauri && cargo fmt)
git add src-tauri/
git commit -m "feat(sub6c): transcriber factory + per-provider STT model + transcription.rs wiring"
```

---

## Task C4: TranscriptionPanel UI (provider selection + Azure deployment)

**Files:** `src/features/settings/TranscriptionPanel.tsx`, `src/i18n/locales/{es,en}.json`, `src/i18n/keys.ts`.

- [ ] **Step 1 — Read `TranscriptionPanel.tsx`** to learn its current structure (it manages `transcription_model` for local via `useGetSettingsQuery`/`useUpdateSettingsMutation`). Add:
  - A `transcription_provider` selector (`local` | `openai` | `azure`) bound to `settings.transcriptionProvider`, persisted via `updateSettings`.
  - When provider is `openai`: show a read-only note that the model is `whisper-1` and the key is configured under *Proveedores de IA*.
  - When provider is `azure`: show an input for the Whisper **deployment** bound to `settings.transcriptionModels.azure` (persist by merging into `transcriptionModels`), plus the same key/endpoint note (endpoint reused from `azureEndpoint`).
  - When `local`: keep the existing model selector.
  Strings via `t()`. Follow the `ProviderPanel.tsx` patterns (the per-provider model + `updateSettings({ ...settings, ... })` merge, the active-provider sync). Add i18n keys: `transcriptionProviderLabel`, `azureWhisperDeployment`, `sttKeyHint` (ES + EN + keys.ts).

- [ ] **Step 2 — Typecheck, test, commit.**
```bash
npx tsc --noEmit
npx vitest run
git add src/
git commit -m "feat(sub6c): transcription provider selector + Azure Whisper deployment UI"
```

---

## Task C5: Real-app smoke (controller-run)

- [ ] **Step 1 — Back up the %APPDATA% DB** (`com.smartnoter.app/db.sqlite{,-shm,-wal}`) with checksums.
- [ ] **Step 2 — Run the dev app.** REMEMBER the DEV-DEBUG whisper crash: cloud STT does NOT invoke local whisper, BUT diarization still runs sherpa locally on the PCM, and `auto_transcribe` could fire the LOCAL whisper engine on a meeting — keep `autoTranscribe=false` (DB) during the smoke, or be ready to run release. Free port 1420 / kill `smart-noter.exe` between relaunches (PowerShell).
- [ ] **Step 3 — Smoke** (user pastes real keys): set transcription provider = OpenAI, a meeting WITH audio (no existing transcript) → transcribe → confirm cloud lines + (if `identify_speakers`) speaker assignment from local sherpa. Watch logs for the multipart upload to `api.openai.com` and `replace_lines`. Repeat for Azure (endpoint + Whisper deployment). Test a meeting > 10 min to exercise chunking (multiple uploads + correct timestamp offsets — lines past 10 min should have sensible timestamps). Restore the DB after.

---

## Self-Review

**Spec coverage:** ✅ `Transcriber` trait + types (C1), local wrap (C1), OpenAI + Azure adapters + chunking + verbose_json (C2), factory + per-provider STT model + `transcription_provider` wiring + conditional whisper check (C3), TranscriptionPanel UI (C4), smoke incl. chunking (C5). Diarization/alignment/persistence reuse the existing path (C3 leaves them unchanged). Matches the Module-C spec.

**Type consistency:** `TranscribedLine{start_ms:u32,end_ms:u32,text:String}` matches whisper `Segment` + diarize `TextSegment` (all u32). `transcription_model_for` / `transcription_models` consistent across settings + factory. `cloud_transcriber`/`resolve_transcription_provider` mirror the Module-B `cloud_summarizer`/`resolve_provider` shapes.

**Open items flagged for execution:** (1) C1's `LocalTranscriber` abort uses an Arc snapshot — C3's local branch should call the engine directly with the real `Arc<AtomicBool>` (documented in C1 Step 3 + C3 Step 3). (2) C2 `decode_wav_16k_mono` duplicates the whisper WAV decoder to keep `providers` off the whisper crate — reviewer may swap to a `smart-noter-whisper` dep (WAV-only is fine since capture writes WAV; FLAC meetings would need the dep or local STT). (3) Cloud STT maps `TranscribedLine` into the job's existing `Segment`/`TextSegment` flow — C3 picks the cleanest adaptation.
