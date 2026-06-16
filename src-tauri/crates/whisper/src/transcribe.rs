#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    pub start_ms: u32,
    pub end_ms: u32,
    pub text: String,
}

/// whisper.cpp segment timestamps are in centiseconds (10 ms units).
pub fn cs_to_seconds(centiseconds: i64) -> u32 {
    (centiseconds.max(0) / 100) as u32
}

pub fn fmt_timestamp(t_seconds: u32) -> String {
    let h = t_seconds / 3600;
    let m = (t_seconds % 3600) / 60;
    let s = t_seconds % 60;
    format!("{h:02}:{m:02}:{s:02}")
}

pub fn word_count(text: &str) -> u32 {
    text.split_whitespace().count() as u32
}

/// Tuning knobs for one transcription run.
#[derive(Debug, Clone)]
pub struct TranscribeOpts {
    pub n_threads: i32,
    /// `None` → auto-detect language; `Some("es")` to force.
    pub language: Option<String>,
}

impl Default for TranscribeOpts {
    fn default() -> Self {
        Self {
            n_threads: 4,
            language: None,
        }
    }
}

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use crate::error::{TranscriptionError, TranscriptionErrorCode};

fn terr(code: TranscriptionErrorCode, m: impl Into<String>) -> TranscriptionError {
    TranscriptionError {
        code,
        message: m.into(),
    }
}

/// Run Whisper over `pcm` (16 kHz mono f32). `progress(pct)` is called as whisper
/// advances; `abort` is polled — returning `true` cancels the run.
pub fn transcribe(
    pcm: &[f32],
    model_path: &Path,
    opts: &TranscribeOpts,
    mut progress: impl FnMut(u32) + Send + 'static,
    abort: Arc<AtomicBool>,
) -> Result<Vec<Segment>, TranscriptionError> {
    let ctx = WhisperContext::new_with_params(
        model_path.to_string_lossy().as_ref(),
        WhisperContextParameters::default(),
    )
    .map_err(|e| terr(TranscriptionErrorCode::ModelLoadFailed, e.to_string()))?;
    let mut state = ctx
        .create_state()
        .map_err(|e| terr(TranscriptionErrorCode::ModelLoadFailed, e.to_string()))?;

    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_n_threads(opts.n_threads);
    params.set_translate(false);
    params.set_language(Some(opts.language.as_deref().unwrap_or("auto")));
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    params.set_progress_callback_safe(move |p: i32| progress(p.clamp(0, 100) as u32));
    {
        let abort = abort.clone();
        params.set_abort_callback_safe(move || abort.load(Ordering::Relaxed));
    }

    state
        .full(params, pcm)
        .map_err(|e| terr(TranscriptionErrorCode::InferenceFailed, e.to_string()))?;

    if abort.load(Ordering::Relaxed) {
        return Err(terr(
            TranscriptionErrorCode::Cancelled,
            "transcription cancelled",
        ));
    }

    // whisper-rs 0.16 segment API (verified via the Task 0.2 spike):
    //   full_n_segments() -> i32 (infallible); get_segment(i) -> Option<WhisperSegment>;
    //   seg.to_str_lossy() -> Result<Cow<str>>; seg.start_timestamp()/end_timestamp() -> i64 (centiseconds).
    let n = state.full_n_segments();
    let mut out = Vec::with_capacity(n.max(0) as usize);
    for i in 0..n {
        let Some(seg) = state.get_segment(i) else {
            continue;
        };
        let text = seg
            .to_str_lossy()
            .map_err(|e| terr(TranscriptionErrorCode::InferenceFailed, e.to_string()))?
            .trim()
            .to_string();
        if text.is_empty() {
            continue;
        }
        let t0 = seg.start_timestamp(); // centiseconds
        let t1 = seg.end_timestamp();
        out.push(Segment {
            start_ms: (t0.max(0) * 10) as u32,
            end_ms: (t1.max(0) * 10) as u32,
            text,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn centiseconds_to_t_seconds_truncates() {
        assert_eq!(cs_to_seconds(0), 0);
        assert_eq!(cs_to_seconds(450), 4); // 4.5 s → 4
        assert_eq!(cs_to_seconds(6000), 60);
    }

    #[test]
    fn t_display_is_hh_mm_ss() {
        assert_eq!(fmt_timestamp(0), "00:00:00");
        assert_eq!(fmt_timestamp(4), "00:00:04");
        assert_eq!(fmt_timestamp(3661), "01:01:01");
    }

    #[test]
    fn word_count_counts_whitespace_separated_tokens() {
        assert_eq!(word_count("hola que tal"), 3);
        assert_eq!(word_count("  uno   dos  "), 2);
        assert_eq!(word_count(""), 0);
    }
}
