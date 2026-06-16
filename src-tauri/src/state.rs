use parking_lot::Mutex;
use smart_noter_audio::capture::recorder::Recorder;
use smart_noter_audio::capture::session::CaptureSession;
use sqlx::SqlitePool;
use std::sync::atomic::{AtomicBool, AtomicU32};
use std::sync::Arc;

/// Application-wide shared state, accessed through `tauri::State<'_, AppState>`
/// inside commands. `Clone` is derived because every field is cheap to clone
/// (Arc clones bump a refcount, SqlitePool is itself a clone-able handle).
///
/// **Lock ordering invariant.** When a command needs both `capture_session`
/// and `recorder`, it MUST acquire `capture_session` first and release it
/// before touching `recorder`. The two locks are NEVER held simultaneously
/// anywhere in the codebase — see `commands/audio.rs` for the established
/// pattern: `let _ = state.capture_session.lock().<op>(...);` (temporary
/// guard, dropped at end-of-expression) followed by `state.recorder.lock()`.
///
/// **No `MutexGuard` across `.await`.** `parking_lot::Mutex` is blocking;
/// holding a guard across an `.await` in an async command would deadlock the
/// Tauri command-handler thread. Always extract the value (`.take()`, `.clone()`,
/// `.map(|s| s.to_string())`) into an owned binding before any `await` point.
#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub capture_session: Arc<Mutex<CaptureSession>>,
    pub recorder: Arc<Mutex<Option<Recorder>>>,
    pub transcription: Arc<Mutex<Option<TranscriptionHandle>>>,
    pub download: Arc<Mutex<Option<DownloadHandle>>>,
}

/// Live transcription job (one at a time). `pct` is updated by the progress
/// callback so `get_transcription_state` can report it; `abort` is polled by
/// whisper.cpp to cancel.
#[derive(Clone)]
pub struct TranscriptionHandle {
    pub meeting_id: String,
    pub abort: Arc<AtomicBool>,
    pub pct: Arc<AtomicU32>,
}

/// Live model download (one at a time).
#[derive(Clone)]
pub struct DownloadHandle {
    pub id: String,
    pub abort: Arc<AtomicBool>,
}
