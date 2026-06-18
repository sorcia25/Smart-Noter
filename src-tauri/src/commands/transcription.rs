use crate::state::{AppState, DownloadHandle, TranscriptionHandle};
use serde::Serialize;
use smart_noter_core::AppError;
use smart_noter_db::repos::transcript_repo::{replace_lines, LineInput};
use smart_noter_whisper::error::{TranscriptionError, TranscriptionErrorCode};
use smart_noter_whisper::transcribe::{transcribe, TranscribeOpts};
use smart_noter_whisper::{decode, models};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use tauri::{Emitter, Manager};

fn app_data(app: &tauri::AppHandle) -> Result<std::path::PathBuf, AppError> {
    app.path()
        .app_data_dir()
        .map_err(|e| AppError::Internal(format!("app_data_dir: {e}")))
}

fn terr(code: TranscriptionErrorCode, m: impl Into<String>) -> AppError {
    AppError::from(TranscriptionError {
        code,
        message: m.into(),
    })
}

// ---- event payloads (events are untyped in this codebase; just Serialize) ----
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ProgressEvent {
    meeting_id: String,
    pct: u32,
}
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct SegmentEvent {
    meeting_id: String,
    t: String,
    text: String,
}
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct CompletedEvent {
    meeting_id: String,
    line_count: u32,
    word_count: u32,
}
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct FailedEvent {
    meeting_id: String,
    code: String,
    message: String,
}
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct CancelledEvent {
    meeting_id: String,
}

#[derive(Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptionState {
    pub meeting_id: String,
    pub pct: u32,
}

#[derive(Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct WhisperModelInfo {
    pub id: String,
    pub name: String,
    pub size_mb: u32,
    pub downloaded: bool,
}

#[tauri::command]
#[specta::specta]
pub async fn transcribe_meeting(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
    meeting_id: String,
) -> Result<(), AppError> {
    // Reserve the single slot atomically.
    let handle = TranscriptionHandle {
        meeting_id: meeting_id.clone(),
        abort: Arc::new(AtomicBool::new(false)),
        pct: Arc::new(AtomicU32::new(0)),
    };
    {
        let mut slot = state.transcription.lock();
        if slot.is_some() {
            return Err(terr(
                TranscriptionErrorCode::TranscriptionBusy,
                "a transcription is already running",
            ));
        }
        *slot = Some(handle.clone());
    }
    // Helper to clear the slot on any early return.
    let clear = || {
        *state.transcription.lock() = None;
    };

    // Async pre-checks (no Mutex guard held across awaits).
    let audio = smart_noter_db::repos::meeting_assets_repo::MeetingAssetsRepo(&state.pool)
        .get_audio(&meeting_id)
        .await;
    let audio_path = match audio {
        Ok(Some(a)) => std::path::PathBuf::from(a.path),
        Ok(None) => {
            clear();
            return Err(AppError::NotFound(format!("no audio for {meeting_id}")));
        }
        Err(e) => {
            clear();
            return Err(e);
        }
    };
    let settings = smart_noter_db::repos::settings_repo::get(&state.pool)
        .await
        .map_err(|e| AppError::Database(e.to_string()));
    let settings = match settings {
        Ok(s) => s,
        Err(e) => {
            clear();
            return Err(e);
        }
    };
    let model_id = settings.transcription_model.clone();
    let app_dir = match app_data(&app) {
        Ok(d) => d,
        Err(e) => {
            clear();
            return Err(e);
        }
    };
    let model_path = match models::model_path(&app_dir, &model_id) {
        Some(p) if p.is_file() => p,
        _ => {
            clear();
            return Err(terr(TranscriptionErrorCode::ModelNotDownloaded, model_id));
        }
    };

    // Spawn the blocking job. Persistence (async sqlx) runs via block_on.
    let pool = state.pool.clone();
    let slot = state.transcription.clone();
    let abort = handle.abort.clone();
    let pct = handle.pct.clone();
    let app2 = app.clone();
    let mid = meeting_id.clone();
    std::thread::spawn(move || {
        let finish = |slot: &Arc<parking_lot::Mutex<Option<TranscriptionHandle>>>| {
            *slot.lock() = None;
        };

        let pcm = match decode::decode_to_pcm_16k_mono(&audio_path) {
            Ok(p) => p,
            Err(e) => {
                let _ = app2.emit(
                    "transcription:failed",
                    FailedEvent {
                        meeting_id: mid.clone(),
                        code: format!("{:?}", e.code),
                        message: e.message,
                    },
                );
                finish(&slot);
                return;
            }
        };

        let app3 = app2.clone();
        let mid3 = mid.clone();
        let pct2 = pct.clone();
        let progress = move |p: u32| {
            pct2.store(p, Ordering::Relaxed);
            let _ = app3.emit(
                "transcription:progress",
                ProgressEvent {
                    meeting_id: mid3.clone(),
                    pct: p,
                },
            );
        };

        let opts = TranscribeOpts::default();
        let segments = match transcribe(&pcm, &model_path, &opts, progress, abort.clone()) {
            Ok(s) => s,
            Err(e) if e.code == TranscriptionErrorCode::Cancelled => {
                let _ = app2.emit(
                    "transcription:cancelled",
                    CancelledEvent {
                        meeting_id: mid.clone(),
                    },
                );
                finish(&slot);
                return;
            }
            Err(e) => {
                let _ = app2.emit(
                    "transcription:failed",
                    FailedEvent {
                        meeting_id: mid.clone(),
                        code: format!("{:?}", e.code),
                        message: e.message,
                    },
                );
                finish(&slot);
                return;
            }
        };

        // Map segments -> lines + word_count. (Single speaker S1 for now; the
        // diarization branch that assigns real speakers is added in a later task.)
        let mut lines = Vec::with_capacity(segments.len());
        let mut words = 0u32;
        for s in &segments {
            let t_seconds = (s.start_ms / 1000) as i64;
            let end_seconds = (s.end_ms / 1000) as i64;
            let t_display = smart_noter_whisper::transcribe::fmt_timestamp(t_seconds as u32);
            words += smart_noter_whisper::transcribe::word_count(&s.text);
            lines.push(LineInput {
                t_seconds,
                end_seconds,
                t_display,
                text_es: s.text.clone(),
                speaker_idx: 0,
            });
        }

        // Persist (async) -- block on the Tauri runtime.
        let persisted =
            tauri::async_runtime::block_on(replace_lines(&pool, &mid, &lines, 1, words as i64));
        match persisted {
            Ok(()) => {
                for l in &lines {
                    let _ = app2.emit(
                        "transcription:segment",
                        SegmentEvent {
                            meeting_id: mid.clone(),
                            t: l.t_display.clone(),
                            text: l.text_es.clone(),
                        },
                    );
                }
                let _ = app2.emit(
                    "transcription:completed",
                    CompletedEvent {
                        meeting_id: mid.clone(),
                        line_count: lines.len() as u32,
                        word_count: words,
                    },
                );
            }
            Err(e) => {
                let _ = app2.emit(
                    "transcription:failed",
                    FailedEvent {
                        meeting_id: mid.clone(),
                        code: "DatabaseError".into(),
                        message: e.to_string(),
                    },
                );
            }
        }
        finish(&slot);
    });

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn cancel_transcription(
    state: tauri::State<'_, AppState>,
    meeting_id: String,
) -> Result<(), AppError> {
    if let Some(h) = state.transcription.lock().as_ref() {
        if h.meeting_id == meeting_id {
            h.abort.store(true, Ordering::Relaxed);
        }
    }
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn get_transcription_state(state: tauri::State<'_, AppState>) -> Option<TranscriptionState> {
    state
        .transcription
        .lock()
        .as_ref()
        .map(|h| TranscriptionState {
            meeting_id: h.meeting_id.clone(),
            pct: h.pct.load(Ordering::Relaxed),
        })
}

#[tauri::command]
#[specta::specta]
pub fn list_whisper_models(app: tauri::AppHandle) -> Result<Vec<WhisperModelInfo>, AppError> {
    let dir = app_data(&app)?;
    Ok(models::list(&dir)
        .into_iter()
        .map(|m| WhisperModelInfo {
            id: m.id.to_string(),
            name: m.display_name.to_string(),
            size_mb: m.size_mb,
            downloaded: m.downloaded,
        })
        .collect())
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct DownloadProgressEvent {
    id: String,
    pct: u32,
    bytes_downloaded: u64,
    bytes_total: u64,
}
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct DownloadDoneEvent {
    id: String,
}
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct DownloadFailEvent {
    id: String,
    code: String,
    message: String,
}

#[tauri::command]
#[specta::specta]
pub fn download_whisper_model(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
    id: String,
) -> Result<(), AppError> {
    let handle = DownloadHandle {
        id: id.clone(),
        abort: Arc::new(AtomicBool::new(false)),
    };
    {
        let mut slot = state.download.lock();
        if slot.is_some() {
            return Err(terr(
                TranscriptionErrorCode::DownloadBusy,
                "a download is already running",
            ));
        }
        *slot = Some(handle.clone());
    }
    let dir = app_data(&app)?;
    let slot = state.download.clone();
    let abort = handle.abort.clone();
    let app2 = app.clone();
    let id2 = id.clone();
    std::thread::spawn(move || {
        let app3 = app2.clone();
        let id3 = id2.clone();
        let progress = move |pct: u32, dl: u64, total: u64| {
            let _ = app3.emit(
                "whisper-download:progress",
                DownloadProgressEvent {
                    id: id3.clone(),
                    pct,
                    bytes_downloaded: dl,
                    bytes_total: total,
                },
            );
        };
        let is_cancelled = {
            let a = abort.clone();
            move || a.load(Ordering::Relaxed)
        };
        match models::download(&dir, &id2, progress, is_cancelled) {
            Ok(()) => {
                let _ = app2.emit(
                    "whisper-download:completed",
                    DownloadDoneEvent { id: id2.clone() },
                );
            }
            Err(e) => {
                let _ = app2.emit(
                    "whisper-download:failed",
                    DownloadFailEvent {
                        id: id2.clone(),
                        code: format!("{:?}", e.code),
                        message: e.message,
                    },
                );
            }
        }
        *slot.lock() = None;
    });
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn delete_whisper_model(app: tauri::AppHandle, id: String) -> Result<(), AppError> {
    let dir = app_data(&app)?;
    models::delete(&dir, &id).map_err(AppError::from)
}

// ---- diarization model commands ----

use smart_noter_diarize::models as diar_models;

#[derive(Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DiarizationModelInfo {
    pub id: String,
    pub name: String,
    pub size_mb: u32,
    pub downloaded: bool,
}

#[tauri::command]
#[specta::specta]
pub fn list_diarization_models(
    app: tauri::AppHandle,
) -> Result<Vec<DiarizationModelInfo>, AppError> {
    let dir = app_data(&app)?;
    Ok(diar_models::list(&dir)
        .into_iter()
        .map(|m| DiarizationModelInfo {
            id: m.id.to_string(),
            name: m.display_name.to_string(),
            size_mb: m.size_mb,
            downloaded: m.downloaded,
        })
        .collect())
}

#[tauri::command]
#[specta::specta]
pub fn download_diarization_model(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
    id: String,
) -> Result<(), AppError> {
    let handle = DownloadHandle {
        id: id.clone(),
        abort: Arc::new(AtomicBool::new(false)),
    };
    {
        let mut slot = state.download.lock();
        if slot.is_some() {
            return Err(terr(
                TranscriptionErrorCode::DownloadBusy,
                "a download is already running",
            ));
        }
        *slot = Some(handle.clone());
    }
    let dir = app_data(&app)?;
    let slot = state.download.clone();
    let abort = handle.abort.clone();
    let app2 = app.clone();
    let id2 = id.clone();
    std::thread::spawn(move || {
        let app3 = app2.clone();
        let id3 = id2.clone();
        let progress = move |pct: u32, dl: u64, total: u64| {
            let _ = app3.emit(
                "diarization-download:progress",
                DownloadProgressEvent {
                    id: id3.clone(),
                    pct,
                    bytes_downloaded: dl,
                    bytes_total: total,
                },
            );
        };
        let is_cancelled = {
            let a = abort.clone();
            move || a.load(Ordering::Relaxed)
        };
        match diar_models::download(&dir, &id2, progress, is_cancelled) {
            Ok(()) => {
                let _ = app2.emit(
                    "diarization-download:completed",
                    DownloadDoneEvent { id: id2.clone() },
                );
            }
            Err(e) => {
                let _ = app2.emit(
                    "diarization-download:failed",
                    DownloadFailEvent {
                        id: id2.clone(),
                        code: format!("{:?}", e.code),
                        message: e.message,
                    },
                );
            }
        }
        *slot.lock() = None;
    });
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn delete_diarization_model(app: tauri::AppHandle, id: String) -> Result<(), AppError> {
    let dir = app_data(&app)?;
    diar_models::delete(&dir, &id).map_err(AppError::from)
}
