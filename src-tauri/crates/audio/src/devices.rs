//! Device enumeration. Combines WASAPI render endpoints (for loopback capture)
//! with cpal input devices (for microphones). Produces a `Vec<AudioDevice>`
//! with deterministic ids so the same physical device gets the same id across
//! sessions.

use crate::error::AudioError;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::hash::{Hash, Hasher};

#[derive(Debug, Clone, Type, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AudioDeviceKind {
    Loopback,
    Input,
}

#[derive(Debug, Clone, Type, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioDevice {
    pub id: String,
    pub name: String,
    pub kind: AudioDeviceKind,
    pub sample_rate: u32,
    pub channels: u16,
    pub is_default: bool,
    pub recommended: bool,
}

/// Stable hash for an endpoint identifier (WASAPI endpoint ID or cpal device name).
/// Used as the device `id` so the same device gets the same id across runs.
pub fn stable_id_for(endpoint: &str, kind: AudioDeviceKind) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    endpoint.hash(&mut hasher);
    let suffix = match kind {
        AudioDeviceKind::Loopback => "L",
        AudioDeviceKind::Input => "I",
    };
    format!("d-{suffix}-{:016x}", hasher.finish())
}

/// Enumerate all available capture devices.
pub fn enumerate() -> Result<Vec<AudioDevice>, AudioError> {
    let mut out = Vec::new();
    out.extend(enumerate_loopback()?);
    out.extend(enumerate_input()?);
    Ok(out)
}

fn enumerate_loopback() -> Result<Vec<AudioDevice>, AudioError> {
    use wasapi::{get_default_device, DeviceCollection, Direction};

    let default_endpoint_id = get_default_device(&Direction::Render)
        .ok()
        .and_then(|d| d.get_id().ok());

    let collection = DeviceCollection::new(&Direction::Render).map_err(|e| match e {
        // wasapi 0.16.0: WasapiError wraps windows_core::Error; the Windows variant
        // carries an HRESULT. Other variants are unreachable today for this call,
        // but route them through Other so we never silently lose the message.
        wasapi::WasapiError::Windows(inner) => AudioError::WasapiInit {
            hresult: inner.code().0,
        },
        other => AudioError::Other(format!("WASAPI: {other}")),
    })?;

    let mut out = Vec::new();
    for i in 0..collection.get_nbr_devices().unwrap_or(0) {
        let device = match collection.get_device_at_index(i) {
            Ok(d) => d,
            Err(_) => continue,
        };
        let name = device
            .get_friendlyname()
            .unwrap_or_else(|_| "Unknown".into());
        let endpoint_id = device.get_id().unwrap_or_else(|_| name.clone());
        let id = stable_id_for(&endpoint_id, AudioDeviceKind::Loopback);
        let format = device
            .get_iaudioclient()
            .and_then(|c| c.get_mixformat())
            .ok();
        let (sr, ch) = format
            .map(|f| (f.get_samplespersec(), f.get_nchannels()))
            .unwrap_or((48_000, 2));
        out.push(AudioDevice {
            id,
            name,
            kind: AudioDeviceKind::Loopback,
            sample_rate: sr,
            channels: ch,
            is_default: Some(&endpoint_id) == default_endpoint_id.as_ref(),
            recommended: true, // loopback is the primary use case
        });
    }
    Ok(out)
}

fn enumerate_input() -> Result<Vec<AudioDevice>, AudioError> {
    use cpal::traits::{DeviceTrait, HostTrait};
    let host = cpal::default_host();
    let default_input = host.default_input_device().and_then(|d| d.name().ok());

    let mut out = Vec::new();
    let devices = host
        .input_devices()
        .map_err(|e| AudioError::Other(format!("cpal input_devices: {e}")))?;
    for d in devices {
        let name = d.name().unwrap_or_else(|_| "Unknown".into());
        let config = d.default_input_config().ok();
        let (sr, ch) = config
            .map(|c| (c.sample_rate().0, c.channels()))
            .unwrap_or((48_000, 1));
        out.push(AudioDevice {
            id: stable_id_for(&name, AudioDeviceKind::Input),
            is_default: default_input.as_ref() == Some(&name),
            name,
            kind: AudioDeviceKind::Input,
            sample_rate: sr,
            channels: ch,
            recommended: false,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_id_is_deterministic() {
        let a = stable_id_for("{0.0.1.00000000}.{abc}", AudioDeviceKind::Loopback);
        let b = stable_id_for("{0.0.1.00000000}.{abc}", AudioDeviceKind::Loopback);
        assert_eq!(a, b);
        assert!(a.starts_with("d-L-"));
    }

    #[test]
    fn stable_id_differs_per_kind() {
        let l = stable_id_for("dev-x", AudioDeviceKind::Loopback);
        let i = stable_id_for("dev-x", AudioDeviceKind::Input);
        assert_ne!(l, i);
    }

    /// Smoke test — enumerate returns at least the default render endpoint
    /// on Windows hosts. Excluded from default `cargo test` since CI runners
    /// may not have audio hardware.
    #[cfg(feature = "audio-integration")]
    #[test]
    fn enumerate_returns_nonempty_on_windows() {
        let devices = enumerate().unwrap();
        assert!(!devices.is_empty());
    }
}
