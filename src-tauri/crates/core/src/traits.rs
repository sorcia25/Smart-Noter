use crate::models::ai::{Chunk, MeetingAnalysis};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

/// Input fed to the Summarizer for a single meeting.
pub struct AnalysisInput {
    /// Ordered (t_seconds, speaker_label, text) triples from the transcript.
    /// `t_seconds` is rendered as a `[mm:ss]` marker so the LLM can anchor
    /// decisions/blockers/actions/highlights to an audio timestamp.
    pub transcript: Vec<(u32, String, String)>,
    /// The meeting's template section names.
    pub template_sections: Vec<String>,
    /// Output language: "es" | "en".
    pub lang: String,
}

/// Analyzes a meeting transcript and produces a structured summary.
///
/// Execution is synchronous (spawn in a thread). `progress` is called with
/// 0–100 to update the UI; `abort` is checked between steps.
/// Error type is `String` so `core` stays dependency-free; implementors map
/// their internal error type to string.
pub trait Summarizer: Send + Sync {
    fn analyze(
        &self,
        input: &AnalysisInput,
        progress: &mut dyn FnMut(u32),
        abort: &AtomicBool,
    ) -> Result<MeetingAnalysis, String>;
}

/// Provides embedding and streaming-answer capabilities for the RAG chat feature.
///
/// Execution is synchronous (spawn in a thread). `abort` is checked between tokens.
pub trait ChatEngine: Send + Sync {
    /// Embed a batch of text chunks, returning one `Vec<f32>` per input.
    fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, String>;

    /// Answer `question` given `context` chunks, streaming tokens to `on_token`.
    fn answer(
        &self,
        question: &str,
        context: &[Chunk],
        lang: &str,
        on_token: &mut dyn FnMut(&str),
        abort: &AtomicBool,
    ) -> Result<(), String>;
}

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
