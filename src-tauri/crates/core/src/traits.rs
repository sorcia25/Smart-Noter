use crate::models::ai::{Chunk, MeetingAnalysis};
use std::sync::atomic::AtomicBool;

/// Input fed to the Summarizer for a single meeting.
pub struct AnalysisInput {
    /// Ordered pairs of (speaker_label, text) from the transcript.
    pub transcript: Vec<(String, String)>,
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
