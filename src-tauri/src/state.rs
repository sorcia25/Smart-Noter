use parking_lot::Mutex;
use smart_noter_audio::capture::recorder::Recorder;
use smart_noter_audio::capture::session::CaptureSession;
use sqlx::SqlitePool;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub capture_session: Arc<Mutex<CaptureSession>>,
    pub recorder: Arc<Mutex<Option<Recorder>>>,
}
