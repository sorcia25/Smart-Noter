//! Local Whisper transcription: model management, audio decode, and inference.

pub mod decode;
pub mod error;
pub mod models;
pub mod transcribe;

pub use error::{TranscriptionError, TranscriptionErrorCode};
pub use transcribe::Segment;
