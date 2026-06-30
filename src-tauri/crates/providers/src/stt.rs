use smart_noter_core::traits::{TranscribeInput, TranscribedLine, Transcriber};
use std::io::Cursor;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::http_common::status_to_err;

const SAMPLE_RATE: u32 = 16_000;
const CHUNK_SECS: usize = 600; // ~10 min; ~19 MB as 16-bit mono WAV, under the 25 MB cap

// ---------------------------------------------------------------------------
// Pure helpers
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Audio decoder (WAV + FLAC; providers must NOT depend on the whisper crate)
// ---------------------------------------------------------------------------

/// Decode a WAV or FLAC file to 16 kHz mono f32 PCM.
/// Dispatches on file extension; resamples if needed.
pub(crate) fn decode_audio_16k_mono(path: &std::path::Path) -> Result<Vec<f32>, String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let (raw, rate, channels) = match ext.as_str() {
        "wav" => read_wav(path)?,
        "flac" => read_flac(path)?,
        other => {
            return Err(format!(
                "formato de audio no soportado para STT cloud: {other}"
            ))
        }
    };
    let ch = channels.max(1) as usize;
    let mono: Vec<f32> = if ch == 1 {
        raw
    } else {
        raw.chunks_exact(ch)
            .map(|f| f.iter().sum::<f32>() / ch as f32)
            .collect()
    };
    Ok(if rate == SAMPLE_RATE {
        mono
    } else {
        resample_linear(&mono, rate, SAMPLE_RATE)
    })
}

fn read_wav(path: &std::path::Path) -> Result<(Vec<f32>, u32, u16), String> {
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
    Ok((raw, spec.sample_rate, spec.channels))
}

fn read_flac(path: &std::path::Path) -> Result<(Vec<f32>, u32, u16), String> {
    let mut reader = claxon::FlacReader::open(path).map_err(|e| format!("abrir FLAC: {e}"))?;
    let info = reader.streaminfo();
    let max = (1i64 << (info.bits_per_sample - 1)) as f32;
    let mut samples = Vec::new();
    for s in reader.samples() {
        samples.push(s.map_err(|e| e.to_string())? as f32 / max);
    }
    Ok((samples, info.sample_rate, info.channels as u16))
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

// ---------------------------------------------------------------------------
// Shared chunked upload loop
// ---------------------------------------------------------------------------

/// Shared chunked upload loop. `post_chunk(wav_bytes, lang) -> Result<Value, String>`
/// is the per-provider HTTP call; this handles decode, splitting, offsetting, abort, progress.
fn transcribe_chunked(
    input: &TranscribeInput,
    progress: &mut dyn FnMut(u32),
    abort: &AtomicBool,
    post_chunk: impl Fn(Vec<u8>, Option<&str>) -> Result<serde_json::Value, String>,
) -> Result<Vec<TranscribedLine>, String> {
    let pcm = decode_audio_16k_mono(&input.wav_path)?;
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

// ---------------------------------------------------------------------------
// Adapters
// ---------------------------------------------------------------------------

pub struct OpenAiStt {
    pub api_key: String,
    pub base: String, // default "https://api.openai.com/v1"
}

impl OpenAiStt {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            base: "https://api.openai.com/v1".to_string(),
        }
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
            post_multipart(
                &url,
                &[("Authorization", format!("Bearer {key}"))],
                wav,
                "whisper-1",
                lang,
                "OpenAI",
            )
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
        Self {
            endpoint: endpoint.trim_end_matches('/').to_string(),
            deployment,
            api_key,
        }
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
            post_multipart(
                &url,
                &[("api-key", key.clone())],
                wav,
                "whisper-1",
                lang,
                "Azure",
            )
        })
    }
}

// ---------------------------------------------------------------------------
// HTTP helpers
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use smart_noter_core::traits::TranscribeInput;
    use std::sync::atomic::AtomicBool;

    // -----------------------------------------------------------------------
    // Pure helper tests (no network)
    // -----------------------------------------------------------------------

    #[test]
    fn parse_verbose_json_offsets_timestamps() {
        let body = serde_json::json!({
            "segments": [
                {"start": 0.0,  "end": 1.5, "text": " hola"},
                {"start": 1.5,  "end": 2.0, "text": "mundo "},
                {"start": 2.0,  "end": 2.0, "text": "   "}
            ]
        });
        let lines = parse_verbose_json(&body, 600_000);
        assert_eq!(lines.len(), 2, "blank segment must be filtered out");

        assert_eq!(lines[0].start_ms, 600_000);
        assert_eq!(lines[0].end_ms, 601_500);
        assert_eq!(lines[0].text, "hola");

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
        let pcm: Vec<f32> = vec![0.0; 160]; // 10 ms of silence
        let bytes = pcm_to_wav_bytes(&pcm);
        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(&bytes[8..12], b"WAVE");
    }

    #[test]
    fn decode_audio_rejects_unsupported_extension() {
        let tmp = tempfile::NamedTempFile::with_suffix(".mp3").expect("tempfile");
        let err = decode_audio_16k_mono(tmp.path()).unwrap_err();
        assert!(
            err.contains("no soportado"),
            "expected 'no soportado' in error, got: {err}"
        );
    }

    // -----------------------------------------------------------------------
    // Mock HTTP tests (tiny_http)
    // -----------------------------------------------------------------------

    /// Build a tiny WAV file (16 kHz mono, 100 ms silence) for mock tests.
    fn make_temp_wav() -> tempfile::NamedTempFile {
        let tmp = tempfile::NamedTempFile::with_suffix(".wav").expect("tempfile");
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 16_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut w = hound::WavWriter::create(tmp.path(), spec).expect("wav writer");
        for _ in 0..1_600u32 {
            w.write_sample(0i16).expect("sample");
        }
        w.finalize().expect("finalize");
        tmp
    }

    #[test]
    fn openai_stt_transcribes_single_chunk_via_mock() {
        let server = tiny_http::Server::http("127.0.0.1:0").expect("server");
        let port = server.server_addr().to_ip().expect("ip addr").port();

        let canned = serde_json::json!({
            "segments": [
                {"start": 0.0, "end": 1.0, "text": "hola"},
                {"start": 1.0, "end": 2.0, "text": "mundo"}
            ]
        })
        .to_string();

        // Spawn server thread: accept one multipart POST, reply with canned JSON.
        let server_handle = std::thread::spawn(move || {
            let req = server.recv().expect("recv request");
            let response = tiny_http::Response::from_string(canned).with_header(
                tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..])
                    .unwrap(),
            );
            req.respond(response).expect("respond");
        });

        let tmp = make_temp_wav();
        let stt = OpenAiStt {
            api_key: "k".into(),
            base: format!("http://127.0.0.1:{port}/v1"),
        };
        let input = TranscribeInput {
            wav_path: tmp.path().to_path_buf(),
            lang: None,
        };
        let abort = AtomicBool::new(false);
        let mut _prog = 0u32;
        let lines = stt
            .transcribe(&input, &mut |p| _prog = p, &abort)
            .expect("transcribe");

        server_handle.join().expect("server thread");

        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].text, "hola");
        assert_eq!(lines[0].start_ms, 0);
        assert_eq!(lines[0].end_ms, 1_000);
        assert_eq!(lines[1].text, "mundo");
        assert_eq!(lines[1].start_ms, 1_000);
        assert_eq!(lines[1].end_ms, 2_000);
    }

    #[test]
    fn transcribe_returns_cancelado_when_abort_is_set() {
        let tmp = make_temp_wav();
        let stt = OpenAiStt {
            api_key: "k".into(),
            base: "http://127.0.0.1:1".into(), // unreachable — abort fires first
        };
        let input = TranscribeInput {
            wav_path: tmp.path().to_path_buf(),
            lang: None,
        };
        // Set abort BEFORE the call.
        let abort = AtomicBool::new(true);
        let result = stt.transcribe(&input, &mut |_| {}, &abort);
        assert!(result.is_err());
        let msg = match result {
            Err(e) => e,
            Ok(_) => panic!("expected Err, got Ok"),
        };
        assert!(
            msg.contains("cancelado"),
            "expected 'cancelado' in error, got: {msg}"
        );
    }
}
