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
///
/// `stop_preview` is a **no-op when not in Preview state**, so it is safe for
/// `start_recording` to call it defensively without risking teardown of a live
/// recording.
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

    // Lives in the audio dir with `tmp-preview-` prefix so the startup sweep
    // (Task 4.6) can reclaim it if the app crashed mid-preview.
    let tmp = audio_dir(&app)?.join(format!("tmp-preview-{}.wav", std::process::id()));
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
    // Only clean up if a preview is actually active. This guards callers (in
    // particular start_recording's defense-in-depth invocation) from accidentally
    // tearing down a live recording when invoked while the session is in
    // Recording state. The state check + lock release happen up front so the
    // capture_session lock is never held across rec.stop().
    let is_preview = matches!(
        state.capture_session.lock().state(),
        smart_noter_audio::capture::session::CaptureState::Preview { .. }
    );
    if !is_preview {
        return Ok(());
    }
    state.capture_session.lock().end_preview();
    let rec_opt = state.recorder.lock().take();
    if let Some(rec) = rec_opt {
        let tmp = rec.tmp_path.clone();
        match rec.stop() {
            Ok((path, _, _)) => {
                let _ = std::fs::remove_file(path);
            }
            Err(_) => {
                let _ = std::fs::remove_file(&tmp);
            }
        }
    }
    Ok(())
}

#[derive(serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RecordingStartedDto {
    pub session_id: String,
    pub sample_rate: u32,
    pub channels: u16,
}

/// Start a real recording. The captured samples are written to a tmp file
/// inside `%APPDATA%\com.smartnoter.app\audio\` (`tmp-{session_id}.wav|flac`),
/// then promoted by `finalize_recording` or removed by `discard_recording`.
///
/// - `System` / `Mic`: `device_id` selects the device.
/// - `Mix`: `device_id` selects the **system loopback**; the microphone is
///   always the OS default input device. (See Phase 4 boundary decision #5.)
///
/// Callers MUST `finalize_recording` or `discard_recording` after stopping;
/// a new `start_recording` while a `Stopped` payload is pending will return
/// `AlreadyRecording`. (See Phase 4 boundary decision #3.)
#[tauri::command]
#[specta::specta]
pub fn start_recording(
    state: tauri::State<'_, crate::state::AppState>,
    app: tauri::AppHandle,
    device_id: String,
    capture_mode: CaptureMode,
    format: AudioFormat,
) -> Result<RecordingStartedDto, AppError> {
    let session_id = format!("sess-{}", uuid::Uuid::new_v4());

    // If a preview is running, stop it first (clean transition).
    let _ = stop_preview(state.clone());

    state
        .capture_session
        .lock()
        .begin_recording(session_id.clone())
        .map_err(AppError::from)?;

    let tmp_path = audio_dir(&app)?.join(format!("tmp-{session_id}.{ext}", ext = ext_for(format)));
    match Recorder::start(app, capture_mode, device_id, format, tmp_path) {
        Ok(recorder) => {
            let sample_rate = recorder.stream.sample_rate;
            let channels = recorder.stream.channels;
            *state.recorder.lock() = Some(recorder);
            Ok(RecordingStartedDto {
                session_id,
                sample_rate,
                channels,
            })
        }
        Err(e) => {
            // Recorder failed to open the stream(s); revert state machine.
            state.capture_session.lock().cancel_recording();
            Err(AppError::from(e))
        }
    }
}

/// Pause the active recording. The Recorder keeps its WASAPI/cpal streams
/// open and the samples keep flowing through the writer thread, but the
/// writer skips them (pause is implemented as a "discard samples" flag).
/// Returns `NotRecording` if the session isn't in `Recording { paused: false }`.
#[tauri::command]
#[specta::specta]
pub fn pause_recording(state: tauri::State<'_, crate::state::AppState>) -> Result<(), AppError> {
    state
        .capture_session
        .lock()
        .pause()
        .map_err(AppError::from)?;
    if let Some(rec) = state.recorder.lock().as_ref() {
        rec.pause();
    }
    Ok(())
}

/// Resume a paused recording.
/// Returns `NotRecording` if the session isn't in `Recording { paused: true }`.
#[tauri::command]
#[specta::specta]
pub fn resume_recording(state: tauri::State<'_, crate::state::AppState>) -> Result<(), AppError> {
    state
        .capture_session
        .lock()
        .resume()
        .map_err(AppError::from)?;
    if let Some(rec) = state.recorder.lock().as_ref() {
        rec.resume();
    }
    Ok(())
}

fn audio_dir(app: &tauri::AppHandle) -> Result<std::path::PathBuf, AppError> {
    use tauri::Manager;
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::Internal(format!("app_data_dir: {e}")))?
        .join("audio");
    std::fs::create_dir_all(&dir)
        .map_err(|e| AppError::Internal(format!("create audio dir: {e}")))?;
    Ok(dir)
}

fn ext_for(fmt: AudioFormat) -> &'static str {
    match fmt {
        AudioFormat::Wav => "wav",
        AudioFormat::Flac => "flac",
    }
}

#[derive(serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CaptureResult {
    pub session_id: String,
    pub path: String,
    pub bytes: u64,
    pub duration_sec: u32,
}

/// Finish the active recording. Stops the writer + meter threads, transitions
/// the session machine `Recording → Stopped` with the tmp file path, and
/// returns a `CaptureResult { session_id, path, bytes, duration_sec }`.
///
/// The tmp file lives in `%APPDATA%\com.smartnoter.app\audio\` as
/// `tmp-{session_id}.wav|flac`. Callers MUST follow up with either
/// `finalize_recording` (promotes to a Meeting row + asset) or
/// `discard_recording` (deletes the tmp file).
#[tauri::command]
#[specta::specta]
pub fn stop_recording(
    state: tauri::State<'_, crate::state::AppState>,
) -> Result<CaptureResult, AppError> {
    let session_id = state
        .capture_session
        .lock()
        .current_session_id()
        .ok_or(smart_noter_audio::AudioError::NotRecording)
        .map(|s| s.to_string())
        .map_err(AppError::from)?;
    let rec = state
        .recorder
        .lock()
        .take()
        .ok_or(smart_noter_audio::AudioError::NotRecording)
        .map_err(AppError::from)?;
    let tmp = rec.tmp_path.clone();
    let (path, bytes, duration_sec) = rec.stop().map_err(|e| {
        // rec.stop() failed (writer error); remove the partial tmp file so a full
        // disk does not permanently block a retry. Session machine stays in Recording
        // (cancel_recording via discard_recording is the recovery path).
        let _ = std::fs::remove_file(&tmp);
        AppError::from(e)
    })?;
    state
        .capture_session
        .lock()
        .stop(path.clone(), bytes, duration_sec)
        .map_err(AppError::from)?;
    Ok(CaptureResult {
        session_id,
        path: path.display().to_string(),
        bytes,
        duration_sec,
    })
}

/// Promote the `Stopped` payload to a persistent Meeting + MeetingAsset
/// in the database, atomically. Renames the tmp file to a stable
/// `{meeting_id}.{ext}` name in the same audio dir.
///
/// Returns `Validation` if `session_id` doesn't match the pending Stopped
/// payload (caller likely got a stale state out of order). Returns
/// `AppError::Internal("no finished session to finalize")` if no Stopped
/// payload exists.
///
/// On DB insert failure after the file rename has succeeded, the function
/// attempts a compensating rename back to `tmp-{session_id}.{ext}` so the
/// startup sweep (Task 4.6) can reclaim the file. If the compensating
/// rename also fails, the original DB error is returned and the file
/// remains under its final name as an orphan (rare; user must clean up
/// manually).
#[tauri::command]
#[specta::specta]
pub async fn finalize_recording(
    state: tauri::State<'_, crate::state::AppState>,
    app: tauri::AppHandle,
    session_id: String,
    title: String,
    template_id: String,
) -> Result<smart_noter_core::MeetingDetail, AppError> {
    let (sess_id, tmp_path, bytes, duration_sec) = state
        .capture_session
        .lock()
        .take_finished()
        .ok_or_else(|| AppError::Internal("no finished session to finalize".into()))?;
    if sess_id != session_id {
        return Err(AppError::Validation(format!(
            "session_id mismatch: have {sess_id}, got {session_id}"
        )));
    }
    // sess_id is "sess-<32 hex chars>"; bytes 5..13 are the first 8 hex chars of the UUID.
    // UUIDv4 → random 8-char suffix; collisions possible but ~2^-32 / day. Document only.
    let meeting_id = format!(
        "m-{}-{}",
        chrono::Utc::now().format("%Y%m%d"),
        &sess_id[5..13]
    );
    let ext = tmp_path
        .extension()
        .and_then(|s| s.to_str())
        .ok_or_else(|| AppError::Internal(format!("tmp_path missing extension: {tmp_path:?}")))?;
    let final_path = audio_dir(&app)?.join(format!("{meeting_id}.{ext}"));
    std::fs::rename(&tmp_path, &final_path)
        .map_err(|e| AppError::Internal(format!("rename {tmp_path:?}: {e}")))?;

    let mime = match ext.to_ascii_lowercase().as_str() {
        "wav" => Some("audio/wav".to_string()),
        "flac" => Some("audio/flac".to_string()),
        _ => None,
    };
    let now = chrono::Utc::now().to_rfc3339();
    let meeting = smart_noter_core::MeetingDetail {
        id: meeting_id.clone(),
        title: smart_noter_core::Bilingual {
            es: title.clone(),
            en: None,
        },
        template: template_id,
        date: now.clone(),
        duration_sec: duration_sec as i64,
        device_used: None,
        word_count: 0,
        summary: None,
        participants: vec![],
        actions: vec![],
        decisions: vec![],
        blockers: vec![],
        transcript: vec![],
    };
    let asset = smart_noter_core::MeetingAsset {
        id: format!("a-{}", uuid::Uuid::new_v4()),
        meeting_id: meeting_id.clone(),
        kind: "audio".into(),
        path: final_path.display().to_string(),
        bytes: bytes as i64,
        mime_type: mime,
        created_at: now,
    };
    match smart_noter_db::repos::MeetingsRepo(&state.pool)
        .create_with_asset(&meeting, &asset)
        .await
    {
        Ok(()) => Ok(meeting),
        Err(db_err) => {
            // Compensating rename so the next startup sweep (Task 4.6) can clean up
            // the tmp file. If the compensating rename also fails, accept the orphan
            // and propagate the original DB error (the second failure isn't actionable).
            let _ = std::fs::rename(&final_path, &tmp_path);
            Err(db_err)
        }
    }
}

/// Tear down any in-flight recording or pending Stopped payload, deleting
/// the tmp file from disk. Idempotent: safe to call even if no recording
/// is active. Also clears any active preview state (defense in depth).
///
/// This is the escape hatch from the `Stopped → AlreadyRecording` trap:
/// after `stop_recording`, callers that choose not to finalize MUST call
/// `discard_recording` before they can call `start_recording` again
/// (see Phase 4 boundary decision #3 / `start_recording` doc-comment).
///
/// Also recovers from the inconsistent state left when `stop_recording` fails
/// mid-flight (recorder extracted + `rec.stop()` errors, leaving the session
/// machine in `Recording` with `recorder = None`). `cancel_recording()` is
/// silent and idempotent: it only flips `Recording → Idle`; Stopped and Idle
/// are left unchanged.
#[tauri::command]
#[specta::specta]
pub fn discard_recording(state: tauri::State<'_, crate::state::AppState>) -> Result<(), AppError> {
    // Recorder cleanup (separate lock, drops before fs I/O on tmp path).
    let rec_opt = state.recorder.lock().take();
    if let Some(rec) = rec_opt {
        let tmp = rec.tmp_path.clone();
        match rec.stop() {
            Ok((path, _, _)) => {
                let _ = std::fs::remove_file(path);
            }
            Err(_) => {
                let _ = std::fs::remove_file(&tmp);
            }
        }
    }

    // Session-machine cleanup: take any Stopped payload first (needs lock), then
    // run the two no-op methods (end_preview, cancel_recording) under the same lock.
    let tmp_to_remove = {
        let mut sess = state.capture_session.lock();
        let stopped_path = sess.take_finished().map(|(_, tmp_path, _, _)| tmp_path);
        sess.end_preview(); // safe no-op if not in Preview
        sess.cancel_recording(); // safe no-op if not in Recording — recovers from rec.stop() failure
        stopped_path
    };
    if let Some(tmp_path) = tmp_to_remove {
        let _ = std::fs::remove_file(tmp_path);
    }
    Ok(())
}
