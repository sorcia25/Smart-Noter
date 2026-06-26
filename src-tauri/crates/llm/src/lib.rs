//! Local LLM (llama.cpp) for summary generation, extraction, and RAG chat.
//! Owns GGUF model management. Implements core's Summarizer/ChatEngine traits.
pub mod chat;
pub mod engine;
pub mod models;
pub mod prompt;
pub mod summarize;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AiError {
    #[error("model not found: {0}")]
    ModelMissing(String),
    #[error("llm load failed: {0}")]
    Load(String),
    #[error("inference failed: {0}")]
    Inference(String),
    #[error("download failed: {0}")]
    Download(String),
    #[error("bad model output: {0}")]
    Parse(String),
}
