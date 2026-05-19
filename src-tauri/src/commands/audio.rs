use smart_noter_audio::capture::recorder::Recorder;
use smart_noter_audio::capture::session::{AudioFormat, CaptureMode};
use smart_noter_core::AppError;

/// Start a live preview of the chosen audio source. The captured samples are
/// written to a discarded tmp WAV that `stop_preview` removes.
///
/// - `System` / `Mic`: `device_id` selects the device.
/// - `Mix`: `device_id` selects the **system loopback**; the microphone is
///   always the OS default input device. (See Phase 4 boundary decision #5.)
///
/// Callers MUST invoke `stop_preview` before `start_recording`: both commands
/// share a single `Recorder` slot in `AppState`, so a recording attempt while
/// a preview is live would race for exclusive WASAPI/cpal stream handles.
#[tauri::command]
#[specta::specta]
pub fn start_preview(
    state: tauri::State<'_, crate::state::AppState>,
    app: tauri::AppHandle,
    device_id: String,
    capture_mode: CaptureMode,
) -> Result<(), AppError> {
    state
        .capture_session
        .lock()
        .begin_preview(device_id.clone())
        .map_err(AppError::from)?;

    let tmp = std::env::temp_dir().join(format!("sn-preview-{}.wav", std::process::id()));
    match Recorder::start(app, capture_mode, device_id, AudioFormat::Wav, tmp) {
        Ok(recorder) => {
            *state.recorder.lock() = Some(recorder);
            Ok(())
        }
        Err(e) => {
            // Recorder failed to open the stream(s); roll session back to Idle
            // so the user can retry without an AlreadyRecording error.
            state.capture_session.lock().end_preview();
            Err(AppError::from(e))
        }
    }
}

#[tauri::command]
#[specta::specta]
pub fn stop_preview(state: tauri::State<'_, crate::state::AppState>) -> Result<(), AppError> {
    state.capture_session.lock().end_preview();
    if let Some(rec) = state.recorder.lock().take() {
        if let Ok((path, _, _)) = rec.stop() {
            let _ = std::fs::remove_file(path);
        }
    }
    Ok(())
}
