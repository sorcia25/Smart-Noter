use crate::{engine::LocalLlm, prompt::chatml};
use smart_noter_core::ai_prompt::{build_messages, parse_analysis};
use smart_noter_core::models::ai::MeetingAnalysis;
use smart_noter_core::traits::{AnalysisInput, Summarizer};
use std::sync::atomic::AtomicBool;

// ---------------------------------------------------------------------------
// Public functions
// ---------------------------------------------------------------------------

/// Build a ChatML-wrapped prompt for Qwen2.5-Instruct.
///
/// `strict`: when `true` an extra instruction is prepended to the system role
/// reminding the model to emit ONLY the JSON object, no prose. This is used
/// on the second attempt when the first response fails to parse.
pub fn build_prompt(input: &AnalysisInput, strict: bool) -> String {
    let (system, user) = build_messages(input, strict);
    chatml(&system, &user)
}

// ---------------------------------------------------------------------------
// Summarizer implementation
// ---------------------------------------------------------------------------

pub struct LocalSummarizer<'a> {
    pub llm: &'a LocalLlm,
}

impl Summarizer for LocalSummarizer<'_> {
    fn analyze(
        &self,
        input: &AnalysisInput,
        progress: &mut dyn FnMut(u32),
        abort: &AtomicBool,
    ) -> Result<MeetingAnalysis, String> {
        progress(5);
        let prompt = build_prompt(input, false);
        let mut sink = |_: &str| {};
        let raw = self
            .llm
            .generate(&prompt, 1024, &mut sink, abort)
            .map_err(|e| e.to_string())?;
        progress(80);

        tracing::debug!(
            len = raw.len(),
            raw = %raw.chars().take(800).collect::<String>(),
            "LLM summary raw output"
        );

        let analysis = parse_analysis(&raw, &input.lang).or_else(|_| {
            // Retry with a stricter system instruction; rebuild via ChatML so the
            // structure stays intact (prepends "IMPORTANTE" to the system role).
            let strict_prompt = build_prompt(input, true);
            let raw2 = self
                .llm
                .generate(&strict_prompt, 1024, &mut |_| {}, abort)
                .map_err(|e| e.to_string())?;

            tracing::debug!(
                len = raw2.len(),
                raw = %raw2.chars().take(800).collect::<String>(),
                "LLM summary raw output (strict retry)"
            );

            parse_analysis(&raw2, &input.lang)
        })?;

        progress(100);
        Ok(analysis)
    }
}
