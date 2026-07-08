//! Local speaker diarization: model management, sherpa-rs pipeline, and a pure aligner.

pub mod error;

pub use error::{DiarizationError, DiarizationErrorCode};

pub mod align;
pub use align::{
    align, fill_zero_durations, remap_contiguous, AlignedLine, DiarSegment, TextSegment,
};

pub mod overlap;
pub use overlap::flatten_overlaps;

pub mod diarize;
pub mod models;

pub use diarize::{diarize, DiarizeOpts};
