//! `CaptureSession` is the source of truth for the audio recording state machine.
//! Audio callbacks, writer thread, meter thread and the Tauri commands all
//! manipulate it through these methods. The methods do not block on I/O.

use crate::error::AudioError;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Type, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CaptureMode {
    System,
    Mic,
    Mix,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Type, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AudioFormat {
    Wav,
    Flac,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaptureState {
    Idle,
    Preview {
        device_id: String,
    },
    Recording {
        session_id: String,
        paused: bool,
    },
    Stopped {
        session_id: String,
        tmp_path: PathBuf,
        bytes: u64,
        duration_sec: u32,
    },
}

pub struct CaptureSession {
    state: CaptureState,
}

impl Default for CaptureSession {
    fn default() -> Self {
        Self {
            state: CaptureState::Idle,
        }
    }
}

impl CaptureSession {
    pub fn state(&self) -> &CaptureState {
        &self.state
    }

    pub fn begin_preview(&mut self, device_id: String) -> Result<(), AudioError> {
        match self.state {
            CaptureState::Idle => {
                self.state = CaptureState::Preview { device_id };
                Ok(())
            }
            _ => Err(AudioError::AlreadyRecording),
        }
    }

    pub fn end_preview(&mut self) {
        if matches!(self.state, CaptureState::Preview { .. }) {
            self.state = CaptureState::Idle;
        }
    }

    pub fn begin_recording(&mut self, session_id: String) -> Result<(), AudioError> {
        match self.state {
            CaptureState::Idle | CaptureState::Preview { .. } => {
                self.state = CaptureState::Recording {
                    session_id,
                    paused: false,
                };
                Ok(())
            }
            _ => Err(AudioError::AlreadyRecording),
        }
    }

    pub fn pause(&mut self) -> Result<(), AudioError> {
        match &mut self.state {
            CaptureState::Recording { paused, .. } if !*paused => {
                *paused = true;
                Ok(())
            }
            _ => Err(AudioError::NotRecording),
        }
    }

    pub fn resume(&mut self) -> Result<(), AudioError> {
        match &mut self.state {
            CaptureState::Recording { paused, .. } if *paused => {
                *paused = false;
                Ok(())
            }
            _ => Err(AudioError::NotRecording),
        }
    }

    pub fn cancel_recording(&mut self) {
        if matches!(self.state, CaptureState::Recording { .. }) {
            self.state = CaptureState::Idle;
        }
    }

    pub fn is_paused(&self) -> bool {
        matches!(self.state, CaptureState::Recording { paused: true, .. })
    }

    pub fn current_session_id(&self) -> Option<&str> {
        match &self.state {
            CaptureState::Recording { session_id, .. } => Some(session_id),
            CaptureState::Stopped { session_id, .. } => Some(session_id),
            _ => None,
        }
    }

    pub fn stop(
        &mut self,
        tmp_path: PathBuf,
        bytes: u64,
        duration_sec: u32,
    ) -> Result<(), AudioError> {
        match &self.state {
            CaptureState::Recording { session_id, .. } => {
                self.state = CaptureState::Stopped {
                    session_id: session_id.clone(),
                    tmp_path,
                    bytes,
                    duration_sec,
                };
                Ok(())
            }
            _ => Err(AudioError::NotRecording),
        }
    }

    pub fn take_finished(&mut self) -> Option<(String, PathBuf, u64, u32)> {
        match std::mem::replace(&mut self.state, CaptureState::Idle) {
            CaptureState::Stopped {
                session_id,
                tmp_path,
                bytes,
                duration_sec,
            } => Some((session_id, tmp_path, bytes, duration_sec)),
            other => {
                self.state = other;
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    #[test]
    fn idle_to_recording_to_stopped_happy_path() {
        let mut s = CaptureSession::default();
        s.begin_recording("sess-1".into()).unwrap();
        assert!(matches!(s.state, CaptureState::Recording { .. }));
        s.stop(p("/tmp/x.wav"), 1024, 5).unwrap();
        assert!(matches!(s.state, CaptureState::Stopped { .. }));
    }

    #[test]
    fn cannot_start_recording_while_already_recording() {
        let mut s = CaptureSession::default();
        s.begin_recording("sess-1".into()).unwrap();
        let err = s.begin_recording("sess-2".into()).unwrap_err();
        assert!(matches!(err, AudioError::AlreadyRecording));
    }

    #[test]
    fn pause_then_resume() {
        let mut s = CaptureSession::default();
        s.begin_recording("sess-1".into()).unwrap();
        s.pause().unwrap();
        assert!(s.is_paused());
        s.resume().unwrap();
        assert!(!s.is_paused());
    }

    #[test]
    fn double_pause_errors() {
        let mut s = CaptureSession::default();
        s.begin_recording("sess-1".into()).unwrap();
        s.pause().unwrap();
        let err = s.pause().unwrap_err();
        assert!(matches!(err, AudioError::NotRecording));
    }

    #[test]
    fn stop_without_recording_errors() {
        let mut s = CaptureSession::default();
        let err = s.stop(p("/x"), 0, 0).unwrap_err();
        assert!(matches!(err, AudioError::NotRecording));
    }

    #[test]
    fn take_finished_returns_payload_once() {
        let mut s = CaptureSession::default();
        s.begin_recording("sess-1".into()).unwrap();
        s.stop(p("/tmp/x.wav"), 999, 7).unwrap();
        let first = s.take_finished().unwrap();
        assert_eq!(first.0, "sess-1");
        assert_eq!(first.2, 999);
        assert!(s.take_finished().is_none());
        assert_eq!(s.state, CaptureState::Idle);
    }

    #[test]
    fn cancel_recording_from_recording_returns_to_idle() {
        let mut s = CaptureSession::default();
        s.begin_recording("sess-1".into()).unwrap();
        s.cancel_recording();
        assert_eq!(s.state, CaptureState::Idle);
    }

    #[test]
    fn cancel_recording_is_no_op_when_idle() {
        let mut s = CaptureSession::default();
        s.cancel_recording();
        assert_eq!(s.state, CaptureState::Idle);
    }

    #[test]
    fn cancel_recording_is_no_op_when_stopped() {
        let mut s = CaptureSession::default();
        s.begin_recording("sess-1".into()).unwrap();
        s.stop(std::path::PathBuf::from("/tmp/x.wav"), 1024, 5)
            .unwrap();
        s.cancel_recording();
        assert!(matches!(s.state, CaptureState::Stopped { .. }));
    }

    #[test]
    fn preview_lifecycle_does_not_block_recording_start() {
        let mut s = CaptureSession::default();
        s.begin_preview("dev-1".into()).unwrap();
        s.begin_recording("sess-1".into()).unwrap();
        assert!(matches!(s.state, CaptureState::Recording { .. }));
    }

    #[test]
    fn cancel_recording_works_from_paused() {
        let mut s = CaptureSession::default();
        s.begin_recording("sess-1".into()).unwrap();
        s.pause().unwrap();
        s.cancel_recording();
        assert_eq!(s.state, CaptureState::Idle);
    }

    #[test]
    fn preview_can_be_replaced_after_end_preview_roundtrip() {
        let mut s = CaptureSession::default();
        s.begin_preview("dev-1".into()).unwrap();
        // The command layer self-heals by ending the stale preview first…
        s.end_preview();
        // …so a new preview begins cleanly (last-wins).
        s.begin_preview("dev-2".into()).unwrap();
        assert!(matches!(s.state, CaptureState::Preview { ref device_id } if device_id == "dev-2"));
    }
}
