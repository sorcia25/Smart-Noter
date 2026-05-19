//! Smart Noter audio capture — WASAPI/cpal-backed crate.

pub mod capture;
pub mod devices;
pub mod error;

pub use devices::{enumerate, AudioDevice, AudioDeviceKind};
pub use error::AudioError;
