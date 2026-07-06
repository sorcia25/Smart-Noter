//! Capture session state machine + worker thread pipelines.

pub mod meter;
pub(crate) mod mic_comms;
pub mod mixer;
pub mod recorder;
pub mod session;
pub mod stream;
pub mod writer;

pub use recorder::{ElapsedEvent, LevelEvent, Recorder, WaveformEvent};
pub use session::{AudioFormat, CaptureMode, CaptureSession, CaptureState};
