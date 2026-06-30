# Sub-6 Module C — Cloud STT (transcription) Design

> Refines the Module-C sketch in the master spec
> `docs/superpowers/specs/2026-06-27-sub6-cloud-providers-design.md` with the
> decisions made in the 2026-06-30 brainstorming. Modules A (provider infra) and
> B (cloud LLM) are SHIPPED (origin/main==5f11c94). This is the third and final
> Sub-6 module.

## Goal

Add cloud speech-to-text behind a new `Transcriber` trait so a meeting's audio can
be transcribed by **OpenAI** or **Azure OpenAI (Whisper)** instead of the local
whisper.cpp engine, selected per the existing `settings.transcription_provider`.
Speaker **diarization stays local** (sherpa) and is aligned to the cloud lines by
timestamps — exactly as the local pipeline already aligns whisper lines to sherpa
segments. **Local stays the default and the fallback.**

## Locked decisions (brainstorming 2026-06-30)

| Decision | Choice |
|----------|--------|
| Azure STT API | **Azure OpenAI Whisper deployment** (OpenAI-shaped `/audio/transcriptions`), reusing the Azure endpoint + `api-key` + `http_common` plumbing from Module B — NOT Azure AI Speech (separate stack). |
| Long audio (OpenAI 25 MB upload cap) | **Chunking**: decode → 16 kHz mono WAV → split into ~10-min windows (under 25 MB), transcribe each, **concatenate with a per-chunk timestamp offset**. Fixed-boundary cuts for MVP; silence-aware cutting is a follow-up. |
| `Transcriber` input | `wav_path: PathBuf` (so cloud can upload the file; the local impl decodes internally). |
| STT model per provider | OpenAI `whisper-1` (const); Azure = its **whisper deployment** name (configurable, separate from the LLM deployment); local = existing `transcription_model`. Stored in a new `transcription_models: BTreeMap` (same pattern as Module B's `provider_models`). |
| STT config UI | Extend the existing **TranscriptionPanel** with a `transcription_provider` selector + Azure STT deployment field. The API key is shared per-provider (configured in the AI ProviderPanel); show a hint if missing. |

## Architecture

### The `Transcriber` trait (new, `core/src/traits.rs`)

```rust
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

pub struct TranscribeInput {
    pub wav_path: PathBuf,
    pub lang: Option<String>,   // hint; None = auto-detect
}

pub struct TranscribedLine {
    pub start_ms: u32,
    pub end_ms: u32,
    pub text: String,
}

pub trait Transcriber: Send + Sync {
    fn transcribe(
        &self,
        input: &TranscribeInput,
        progress: &mut dyn FnMut(u32),
        abort: &AtomicBool,
    ) -> Result<Vec<TranscribedLine>, String>;
}
```

`TranscribedLine` mirrors the whisper engine's existing `Segment{start_ms,end_ms,text}`
and the `TextSegment` the diarization aligner already consumes — so cloud lines feed
the existing `align()` unchanged.

### Local refactor (`crates/whisper`)

A `LocalTranscriber` (new, in `crates/whisper`) implements `Transcriber`: it
`decode::decode_to_pcm_16k_mono(&wav_path)` then calls the existing
`transcribe(&pcm, &model_path, &opts, progress, abort) -> Vec<Segment>` and maps
`Segment → TranscribedLine`. The engine's PCM-based API is unchanged; only a thin
wrapper is added. `crates/whisper` gains a dependency on `smart-noter-core` for the
trait (it already shares core types).

The transcription **job still decodes the WAV to PCM itself** for diarization
(sherpa needs PCM, independent of the transcriber). For the local provider this
means the WAV is decoded twice (job for diarize + LocalTranscriber for whisper) —
acceptable; decode is cheap relative to inference. (A future optimization could
pass PCM through, but that would split the trait's interface.)

### Cloud adapters (`crates/providers/src/stt.rs`)

`OpenAiStt{ api_key, model, base }` and `AzureStt{ endpoint, deployment, api_key }`,
both impl `Transcriber`. Shared chunking + verbose_json parsing live in a small
helper so the two adapters differ only in URL + auth (mirroring the Module-B
openai/azure split).

- **OpenAI**: `POST {base}/audio/transcriptions`, header `Authorization: Bearer {key}`,
  multipart form: `file` (the WAV chunk), `model=whisper-1`, `response_format=verbose_json`,
  `language={lang}` (if set). Response: `{"segments":[{"start":<sec>,"end":<sec>,"text":...}]}`.
- **Azure**: `POST {endpoint}/openai/deployments/{deployment}/audio/transcriptions?api-version=2024-06-01`,
  header `api-key: {key}`, same multipart + response shape. Endpoint trailing-slash
  normalized (as `AzureProvider::new` does in Module B).

**Chunking** (`stt.rs`, shared by both adapters):
1. `decode_to_pcm_16k_mono(&wav_path)` → `Vec<f32>`.
2. Split into windows of `CHUNK_SECS` (~600 s) → each is `chunk_secs * 16000` samples,
   well under 25 MB as 16-bit WAV (~19 MB at 600 s).
3. For each window: write a 16 kHz mono 16-bit WAV to a temp buffer (reuse the `hound`
   writer the export crate uses), POST it, parse `segments[]`, **add `window_index *
   CHUNK_SECS * 1000` to each line's start/end ms**, collect.
4. `progress` reports `(completed_chunks / total_chunks) * 100`; `abort` is checked
   between chunks (returns `Err("cancelado")` cleanly, like the Module-B adapters).

Non-2xx → `http_common::status_to_err(status, provider)` plus the response body
(the same observability improvement Module B uses). `reqwest::blocking` multipart
(the trait is sync, called from the transcription worker `std::thread`).

### Factory + job integration

- `provider_factory.rs`: generalize provider/key resolution so STT reuses it. Add
  `transcriber(settings, key) -> Result<Box<dyn Transcriber>, String>` matching on
  `settings.transcription_provider` ("local" → not built here, handled inline like
  the LLM local branch; "openai" → `OpenAiStt`; "azure" → `AzureStt`, requires
  `azure_endpoint` + the STT deployment, guarded like the empty-deployment case in B).
  The **API key is the same per-provider DPAPI secret** the LLM uses (`secrets_repo`
  is keyed by provider, not domain) — resolve it via a `model_for`/`resolve`-style
  helper parameterized by the provider string + a `transcription_model_for(provider)`.
- `transcription.rs`: the worker builds a `TranscribeInput{wav_path, lang}` and calls
  the factory's transcriber (local vs cloud per `transcription_provider`) **instead of
  the direct whisper call** at ~line 212. Everything after — diarization on the PCM,
  `align()`, `LineInput` build, `replace_lines` persist, the `transcription:*` events —
  is **unchanged** (it already operates on `start_ms/end_ms/text`). Progress/abort
  thread through the trait. **The up-front whisper-model-file check becomes
  conditional** — only required when `transcription_provider == "local"` (cloud STT
  needs no local GGUF). The sherpa diarization-model check stays whenever
  `identify_speakers` is on, regardless of provider (diarization is always local).

### Settings + UI

- `AppSettings`: add `transcription_models: BTreeMap<String,String>` (`#[serde(default)]`)
  + `transcription_model_for(provider)` (openai→"whisper-1", azure→`transcription_models["azure"]`
  or "" , local→the existing `transcription_model`). Reuse `azure_endpoint` (already
  added in B) for the Azure STT endpoint. Keep `transcription_model` (local GGUF id).
- `TranscriptionPanel.tsx`: add a `transcription_provider` selector (local/openai/azure);
  when cloud, show the model/deployment field (whisper-1 placeholder for OpenAI, the
  deployment name for Azure) + a note that the API key is configured under *Proveedores
  de IA*. Persist via `update_settings` (provider + `transcription_models`), like the
  ProviderPanel does for the AI domain.

## Errors, security, testing

- **Errors:** cloud STT maps HTTP status → Spanish messages (401/429/quota/network),
  surfaced via the existing `transcription:failed` event with the provider name.
  Missing key / missing Azure deployment → clear config errors. Local stays the fallback.
- **Security:** keys remain DPAPI ciphertext (never in Redux/logs); audio is uploaded
  over HTTPS only when a cloud provider is selected (privacy disclaimer already shown).
- **Testing:** STT adapters against a `tiny_http` mock (multipart accepted + canned
  verbose_json) — no real network/keys; chunking (split count + timestamp-offset merge
  over a synthetic multi-window input); factory returns the right impl per
  `transcription_provider`+key and errors cleanly without a key / Azure deployment; the
  existing `align()` tests already cover speaker assignment for any line source.

## File structure

| File | Change |
|------|--------|
| `crates/core/src/traits.rs` | add `Transcriber` + `TranscribeInput` + `TranscribedLine` |
| `crates/whisper/src/...` (+ `Cargo.toml`) | add `LocalTranscriber` impl; depend on `smart-noter-core` |
| `crates/providers/src/stt.rs` (create) | `OpenAiStt` + `AzureStt` + shared chunking/verbose_json |
| `crates/providers/src/lib.rs` | export the STT adapters |
| `crates/core/src/models/settings.rs` | `transcription_models` map + `transcription_model_for` |
| `src/commands/provider_factory.rs` | `transcriber()` + generalize key/model resolution for the STT domain |
| `src/commands/transcription.rs` | call the factory instead of the direct whisper engine |
| `src/features/settings/TranscriptionPanel.tsx` | provider selector + Azure STT deployment field |

## Task breakdown (for the plan)

1. **C1** — `Transcriber` trait in core + `LocalTranscriber` refactor in `crates/whisper`; the
   job still uses local but now through the trait (no behavior change). Tests stay green.
2. **C2** — STT adapters (`stt.rs`): OpenAI + Azure + shared chunking + verbose_json, `tiny_http` tests.
3. **C3** — Factory `transcriber()` + `transcription_models`/`transcription_model_for` settings + wire
   `transcription.rs` to pick local vs cloud; regen bindings.
4. **C4** — `TranscriptionPanel` UI (provider selector + Azure deployment) + i18n; tsc + vitest.
5. **C5** — Real-app smoke (user pastes keys; back up + restore the %APPDATA% DB; remember the
   DEV-DEBUG whisper-in-debug crash — for cloud STT the local whisper isn't invoked, but diarization
   still runs locally on the PCM, so keep `auto_transcribe` off / be ready to run release).

## Open follow-ups (noted, out of scope for MVP)

- Silence-aware chunk boundaries (vs fixed ~10-min cuts) to avoid splitting a word.
- Azure AI Speech as an alternative STT backend (native diarization) if Whisper-deployment
  quality/cost isn't enough.
- Compress chunks (MP3/opus, reusing the export encoder) to cut upload count on long meetings.
