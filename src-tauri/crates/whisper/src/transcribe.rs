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
    /// Whisper's per-window no-speech gate threshold. whisper.cpp's default 0.6
    /// drops a whole 30 s window when `no_speech_prob` exceeds it (and avg_logprobs
    /// < -1.0), silently losing quiet trailing speech (the last 3–12 s bug, worsened
    /// by the v1.2 AEC noise-suppression). Relaxed to 0.9 so tail windows still emit;
    /// the `text.is_empty()` skip below drops truly-empty output.
    pub no_speech_thold: f32,
    /// Whisper's per-window log-prob confidence threshold, the second arm of the
    /// `is_no_speech` gate (`no_speech_prob > no_speech_thold && avg_logprobs < logprob_thold`).
    /// whisper.cpp's default -1.0 drops the OPENING 30 s window when its no_speech_prob
    /// exceeds even our relaxed 0.9 (clear speech mis-flagged as no-speech, amplified by
    /// the v1.2 AEC's opening AGC/NS ramp). Lowered to -2.0 so only very-low-confidence
    /// (≈garbage) windows are dropped; the `text.is_empty()` skip guards silence.
    pub logprob_thold: f32,
}

impl Default for TranscribeOpts {
    fn default() -> Self {
        Self {
            n_threads: 4,
            language: None,
            no_speech_thold: 0.9,
            logprob_thold: -2.0,
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

/// Raw ggml abort callback. whisper-rs 0.16's `set_abort_callback_safe` is broken:
/// it instantiates the trampoline as `trampoline::<F>` (the concrete closure type)
/// but stores a `*mut Box<dyn FnMut() -> bool>` as user_data, so the trampoline
/// reinterprets the fat-pointer box as the closure, reads garbage memory, and
/// returns a junk bool — which makes whisper spuriously abort with "failed to
/// encode" partway through. We wire the raw callback ourselves instead. `user_data`
/// is a `*const AtomicBool` that outlives the `state.full()` call.
unsafe extern "C" fn abort_trampoline(user_data: *mut std::ffi::c_void) -> bool {
    (*(user_data as *const AtomicBool)).load(Ordering::Relaxed)
}

/// Decide a `state.full()` outcome given the abort flag. whisper.cpp returns an
/// error ("failed to encode") when the abort callback fires mid-run, so the abort
/// flag must take precedence over the raw error — otherwise a user cancel is
/// misreported as an inference failure.
fn classify_full_outcome(
    full_result: Result<(), String>,
    aborted: bool,
) -> Result<(), TranscriptionError> {
    if aborted {
        return Err(terr(
            TranscriptionErrorCode::Cancelled,
            "transcription cancelled",
        ));
    }
    full_result.map_err(|e| terr(TranscriptionErrorCode::InferenceFailed, e))?;
    Ok(())
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
    params.set_no_speech_thold(opts.no_speech_thold);
    params.set_logprob_thold(opts.logprob_thold);
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    params.set_progress_callback_safe(move |p: i32| progress(p.clamp(0, 100) as u32));
    // SAFETY: `abort` (Arc<AtomicBool>) lives for the whole `state.full()` call below,
    // so the pointer handed to whisper stays valid while whisper.cpp polls it. We use
    // the raw setter because `set_abort_callback_safe` is bugged (see abort_trampoline).
    unsafe {
        params.set_abort_callback(Some(abort_trampoline));
        params.set_abort_callback_user_data(Arc::as_ptr(&abort) as *mut std::ffi::c_void);
    }

    let full_outcome = state
        .full(params, pcm)
        .map(|_| ())
        .map_err(|e| e.to_string());
    classify_full_outcome(full_outcome, abort.load(Ordering::Relaxed))?;

    // whisper-rs 0.16 segment API (verified via the Task 0.2 spike):
    //   full_n_segments() -> i32 (infallible); get_segment(i) -> Option<WhisperSegment>;
    //   seg.to_str_lossy() -> Result<Cow<str>>; seg.start_timestamp()/end_timestamp() -> i64 (centiseconds).
    // Head/tail-drop verification (whisper head/tail fix): the first segment's start
    // should be ~0 and the last segment's end should reach ~the pcm duration. If
    // either is off, whisper's no-speech gate dropped the head or tail.
    let n = state.full_n_segments();
    let first_start_ms = (0..n)
        .find_map(|i| state.get_segment(i))
        .map(|s| (s.start_timestamp().max(0) * 10) as u64)
        .unwrap_or(0);
    let last_end_ms = (0..n)
        .rev()
        .find_map(|i| state.get_segment(i))
        .map(|s| (s.end_timestamp().max(0) * 10) as u64)
        .unwrap_or(0);
    let pcm_ms = (pcm.len() as f64 / 16_000.0 * 1000.0) as u64;
    tracing::info!(
        segments = n,
        first_start_ms,
        last_end_ms,
        pcm_ms,
        "whisper transcription head/tail check"
    );

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

    #[test]
    fn abort_during_full_is_cancelled_even_when_full_errors() {
        // whisper.cpp returns "failed to encode" when the abort callback fires
        // mid-run; with the abort flag set, that must be Cancelled, not InferenceFailed.
        let err = classify_full_outcome(Err("failed to encode".into()), true).unwrap_err();
        assert!(matches!(err.code, TranscriptionErrorCode::Cancelled));
    }

    #[test]
    fn genuine_full_error_without_abort_is_inference_failed() {
        let err = classify_full_outcome(Err("ggml exploded".into()), false).unwrap_err();
        assert!(matches!(err.code, TranscriptionErrorCode::InferenceFailed));
    }

    #[test]
    fn successful_full_without_abort_is_ok() {
        assert!(classify_full_outcome(Ok(()), false).is_ok());
    }

    #[test]
    fn transcribe_opts_default_relaxes_no_speech_and_no_forced_language() {
        let o = TranscribeOpts::default();
        assert_eq!(
            o.no_speech_thold, 0.9,
            "relaxed gate keeps quiet tail windows"
        );
        assert!(
            o.language.is_none(),
            "language still defaults to auto unless forced"
        );
    }

    #[test]
    fn transcribe_opts_default_lowers_logprob_thold() {
        let o = TranscribeOpts::default();
        assert_eq!(
            o.logprob_thold, -2.0,
            "loosened confidence arm keeps the clear opening window"
        );
    }
}
