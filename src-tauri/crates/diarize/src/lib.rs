//! Local speaker diarization: model management, sherpa-rs pipeline, and a pure aligner.

pub mod error;

pub use error::{DiarizationError, DiarizationErrorCode};

pub mod align;
pub use align::{align, AlignedLine, DiarSegment};

// pub mod models;   — added in Phase 2
// pub mod diarize;  — added in Phase 3
