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
    /// The local LLM singleton. `LocalLlm::load()` calls `LlamaBackend::init()` which
    /// can only run ONCE per process — a second call errors. We lazy-load on first use
    /// and hold the loaded instance here forever. When `Some`, reuse it; never reload.
    pub llm: Arc<Mutex<Option<smart_noter_llm::engine::LocalLlm>>>,
    /// Active summary job (one at a time). Mirrors the transcription handle pattern.
    pub summary: Arc<Mutex<Option<SummaryHandle>>>,
    /// Active LLM model download (one at a time, shares the same DownloadHandle type).
    pub llm_download: Arc<Mutex<Option<DownloadHandle>>>,
    /// Active RAG chat job (one at a time). Mirrors the summary handle pattern.
    pub chat: Arc<Mutex<Option<ChatHandle>>>,
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

/// Live summary job (one at a time). Abort is polled cooperatively by the LLM
/// generate loop (via the AtomicBool passed to LocalLlm::generate).
#[derive(Clone)]
pub struct SummaryHandle {
    pub meeting_id: String,
    pub abort: Arc<AtomicBool>,
}

/// Live RAG chat job (one at a time). Abort is polled cooperatively by the LLM
/// generate loop (via the AtomicBool passed to LocalLlm::generate).
#[derive(Clone)]
pub struct ChatHandle {
    pub meeting_id: String,
    pub abort: Arc<AtomicBool>,
}
