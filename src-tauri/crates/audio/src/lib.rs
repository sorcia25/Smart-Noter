//! Smart Noter audio capture — WASAPI/cpal-backed crate.

pub mod error;

pub use error::AudioError;

pub mod devices;
pub use devices::{enumerate, AudioDevice, AudioDeviceKind};
