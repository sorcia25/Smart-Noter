//! Microphone capture through the Windows-native AEC (Communications signal
//! processing mode). Opens an `IAudioClient` in `AudioCategory_Communications`
//! and sets the loopback render endpoint as the echo-cancellation reference via
//! `IAcousticEchoCancellationControl`. The OS delivers an already-echo-cancelled
//! mic stream and compensates the loopback/mic clock drift internally.
//!
//! The captured f32 samples are pushed to the same `Sender<Vec<f32>>` the cpal
//! mic path uses, so the mixer/writer downstream are agnostic to the source.

use crate::capture::session::CaptureMode;
use crate::capture::stream::DEFAULT_RENDER_LOOPBACK;

/// Whether the microphone should be captured through the OS AEC (Communications
/// mode) instead of raw cpal. Only Mix mode has speaker echo to cancel.
// No caller yet — the COM capture code that uses this lands in a later v1.2 task.
#[allow(dead_code)]
pub(crate) fn use_comms_mic(mode: CaptureMode, aec_enabled: bool) -> bool {
    matches!(mode, CaptureMode::Mix) && aec_enabled
}

/// Whether the AEC reference should auto-follow the default render endpoint.
/// True for the "record whatever is playing" sentinel — we pass NULL to
/// `SetEchoCancellationRenderEndpoint` and let Windows track the default render
/// (so no manual re-set is needed when the output device changes). A pinned
/// loopback id resolves to that concrete endpoint instead.
// No caller yet — the COM capture code that uses this lands in a later v1.2 task.
#[allow(dead_code)]
pub(crate) fn aec_reference_is_auto(loopback_device_id: &str) -> bool {
    loopback_device_id == DEFAULT_RENDER_LOOPBACK
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comms_mic_only_for_mix_with_aec() {
        assert!(use_comms_mic(CaptureMode::Mix, true));
        assert!(!use_comms_mic(CaptureMode::Mix, false));
        assert!(!use_comms_mic(CaptureMode::Mic, true));
        assert!(!use_comms_mic(CaptureMode::System, true));
    }

    #[test]
    fn sentinel_reference_is_auto_else_pinned() {
        assert!(aec_reference_is_auto(DEFAULT_RENDER_LOOPBACK));
        assert!(!aec_reference_is_auto("some-endpoint-id"));
    }
}
