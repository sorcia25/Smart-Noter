//! Local speaker diarization: model management, sherpa-rs pipeline, and a pure aligner.

pub mod error;

pub use error::{DiarizationError, DiarizationErrorCode};

pub mod align;
pub use align::{align, AlignedLine, DiarSegment};

pub mod diarize;
pub mod models;

pub use diarize::{diarize, DiarizeOpts};
