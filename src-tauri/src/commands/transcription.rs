use crate::commands::ai::run_summary;
use crate::commands::provider_factory;
use crate::state::{AppState, DownloadHandle, SummaryHandle, TranscriptionHandle};
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
    speaker_count_hint: Option<u32>,
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
    // Resolve the transcription provider + decrypted key (cloud) up front. This
    // also yields the `AppSettings`, so we don't fetch them twice. For local it
    // returns an empty key. Errors (missing API key, secrets failure) surface as
    // a ConfigError before any work is spawned.
    let (provider, settings, key) =
        match provider_factory::resolve_transcription_provider(&state.pool).await {
            Ok(t) => t,
            Err(msg) => {
                clear();
                return Err(terr(TranscriptionErrorCode::ConfigError, msg));
            }
        };
    let is_local = provider == "local";

    let app_dir = match app_data(&app) {
        Ok(d) => d,
        Err(e) => {
            clear();
            return Err(e);
        }
    };
    // The local whisper GGUF is only required when transcribing locally. Cloud
    // providers upload the WAV and need no on-device STT model. (Diarization is
    // always local when enabled and is checked separately below.)
    let model_path: Option<std::path::PathBuf> = if is_local {
        let model_id = settings.transcription_model.clone();
        match models::model_path(&app_dir, &model_id) {
            Some(p) if p.is_file() => Some(p),
            _ => {
                clear();
                return Err(terr(TranscriptionErrorCode::ModelNotDownloaded, model_id));
            }
        }
    } else {
        None
    };

    // Diarization is gated by the persisted toggle and needs BOTH ONNX models present.
    let diarize_on = settings.identify_speakers;
    let diar_seg = diar_models::model_path(&app_dir, "segmentation");
    let diar_emb = diar_models::model_path(&app_dir, "embedding");
    let diar_models_ready = diarize_on
        && diar_seg.as_ref().map(|p| p.is_file()).unwrap_or(false)
        && diar_emb.as_ref().map(|p| p.is_file()).unwrap_or(false);

    // Spawn the blocking job. Persistence (async sqlx) runs via block_on.
    let pool = state.pool.clone();
    let slot = state.transcription.clone();
    let abort = handle.abort.clone();
    let pct = handle.pct.clone();
    let app2 = app.clone();
    let mid = meeting_id.clone();
    // `provider` / `settings` / `key` are captured by the `move` closure below and
    // consumed only in its cloud branch (local needs none of them).
    // Clones for auto-summary chain (used after transcription:completed).
    let auto_summary_llm = state.llm.clone();
    let auto_summary_slot = state.summary.clone();
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

        // Wanted diarization but models aren't downloaded: tell the UI (toast),
        // then proceed with single-speaker transcription (never block the transcript).
        if diarize_on && !diar_models_ready {
            let _ = app2.emit(
                "diarization:degraded",
                FailedEvent {
                    meeting_id: mid.clone(),
                    code: "ModelNotDownloaded".into(),
                    message: "diarization models not downloaded".into(),
                },
            );
        }

        // Transcribe via the resolved provider. The LOCAL branch is byte-identical
        // to before (direct engine call, same `'static + Send` move-closure progress
        // and `Arc<AtomicBool>` abort). The CLOUD branch uploads the WAV through the
        // `Transcriber` trait, whose progress is a `&mut dyn FnMut(u32)`. Both yield a
        // `Vec<Segment>` so the diarize/align/persist tail below is UNCHANGED.
        let segments: Vec<smart_noter_whisper::transcribe::Segment> =
            if let Some(model_path) = model_path.as_ref() {
                // ---- LOCAL: real on-device whisper engine ----
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
                match transcribe(&pcm, model_path, &opts, progress, abort.clone()) {
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
                }
            } else {
                // ---- CLOUD: build the transcriber + upload the WAV ----
                let transcriber =
                    match provider_factory::cloud_transcriber(&provider, &settings, &key) {
                        Ok(t) => t,
                        Err(msg) => {
                            let _ = app2.emit(
                                "transcription:failed",
                                FailedEvent {
                                    meeting_id: mid.clone(),
                                    code: "ConfigError".into(),
                                    message: msg,
                                },
                            );
                            finish(&slot);
                            return;
                        }
                    };

                // Borrow-closure progress (runs inline; no `'static`/`Send` bound needed,
                // unlike the engine's move-closure above). Mirrors the local pct.store + emit.
                let mut progress_cb = |p: u32| {
                    pct.store(p, Ordering::Relaxed);
                    let _ = app2.emit(
                        "transcription:progress",
                        ProgressEvent {
                            meeting_id: mid.clone(),
                            pct: p,
                        },
                    );
                };

                let input = smart_noter_core::traits::TranscribeInput {
                    wav_path: audio_path.clone(),
                    lang: Some(settings.native_language.clone()),
                };
                match transcriber.transcribe(&input, &mut progress_cb, &abort) {
                    // Field-identical map: TranscribedLine -> Segment.
                    Ok(lines) => lines
                        .into_iter()
                        .map(|l| smart_noter_whisper::transcribe::Segment {
                            start_ms: l.start_ms,
                            end_ms: l.end_ms,
                            text: l.text,
                        })
                        .collect(),
                    // The cloud adapter returns Err("cancelado") on abort — treat it as a
                    // cancel, matching the local cancel path (emit cancelled, not failed).
                    Err(msg) if msg == "cancelado" => {
                        let _ = app2.emit(
                            "transcription:cancelled",
                            CancelledEvent {
                                meeting_id: mid.clone(),
                            },
                        );
                        finish(&slot);
                        return;
                    }
                    Err(msg) => {
                        let _ = app2.emit(
                            "transcription:failed",
                            FailedEvent {
                                meeting_id: mid.clone(),
                                code: "TranscriptionError".into(),
                                message: msg,
                            },
                        );
                        finish(&slot);
                        return;
                    }
                }
            };

        // Decide speakers. With diarization requested AND models present, diarize
        // over the SAME pcm + align each text segment to the max-overlap speaker;
        // otherwise (or on failure) fall back to a single speaker S1.
        let mut speaker_count = 1usize;
        let mut speaker_idx: Vec<usize> = vec![0; segments.len()];
        if diar_models_ready {
            let seg_model = diar_seg.clone().unwrap();
            let emb_model = diar_emb.clone().unwrap();
            let opts = DiarizeOpts {
                num_speakers: speaker_count_hint,
            };
            match diarize(&pcm, &seg_model, &emb_model, &opts, abort.clone()) {
                Ok(diar_segs) => {
                    let texts: Vec<TextSegment> = segments
                        .iter()
                        .map(|s| TextSegment {
                            start_ms: s.start_ms,
                            end_ms: s.end_ms,
                            text: s.text.clone(),
                        })
                        .collect();
                    let aligned = align(&texts, &diar_segs);
                    let max_spk = aligned.iter().map(|a| a.speaker).max().unwrap_or(0);
                    speaker_count = (max_spk as usize) + 1;
                    speaker_idx = aligned.iter().map(|a| a.speaker as usize).collect();
                }
                Err(e) if e.code == DiarizationErrorCode::Cancelled => {
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
                    // Degrade to single speaker but tell the UI (toast). Transcript is kept.
                    let _ = app2.emit(
                        "diarization:degraded",
                        FailedEvent {
                            meeting_id: mid.clone(),
                            code: format!("{:?}", e.code),
                            message: e.message,
                        },
                    );
                }
            }
        }

        // sherpa's compute() has no abort hook, so a cancel during the (CPU-bound)
        // diarization phase only lands here. Honor it before persisting.
        if abort.load(Ordering::Relaxed) {
            let _ = app2.emit(
                "transcription:cancelled",
                CancelledEvent {
                    meeting_id: mid.clone(),
                },
            );
            finish(&slot);
            return;
        }

        // Map segments -> lines + word_count (speaker-aware).
        let mut lines = Vec::with_capacity(segments.len());
        let mut words = 0u32;
        for (i, s) in segments.iter().enumerate() {
            let t_seconds = (s.start_ms / 1000) as i64;
            let end_seconds = (s.end_ms / 1000) as i64;
            let t_display = smart_noter_whisper::transcribe::fmt_timestamp(t_seconds as u32);
            words += smart_noter_whisper::transcribe::word_count(&s.text);
            lines.push(LineInput {
                t_seconds,
                end_seconds,
                t_display,
                text_es: s.text.clone(),
                speaker_idx: speaker_idx[i],
            });
        }

        // Persist (async) -- block on the Tauri runtime.
        let persisted = tauri::async_runtime::block_on(replace_lines(
            &pool,
            &mid,
            &lines,
            speaker_count,
            words as i64,
        ));
        match persisted {
            Ok(()) => {
                // Refresh the search index for this meeting (best-effort).
                if let Err(e) = tauri::async_runtime::block_on(
                    smart_noter_db::repos::search_repo::upsert_meeting(&pool, &mid),
                ) {
                    tracing::warn!("fts upsert after transcription failed for {mid}: {e}");
                }
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

                // ── Auto-summary chain (best-effort, never fails the transcription) ──
                // Load settings to check if auto_generate_summary is enabled.
                let auto_settings = tauri::async_runtime::block_on(
                    smart_noter_db::repos::settings_repo::get(&pool),
                );
                let should_auto_summarize = auto_settings
                    .map(|s| s.auto_generate_summary)
                    .unwrap_or(false);

                if should_auto_summarize {
                    // Reserve the summary slot (best-effort — if a manual summary is
                    // already running, skip without erroring the transcription).
                    let auto_abort = Arc::new(AtomicBool::new(false));
                    let reserved = {
                        let mut summary_guard = auto_summary_slot.lock();
                        if summary_guard.is_none() {
                            *summary_guard = Some(SummaryHandle {
                                meeting_id: mid.clone(),
                                abort: auto_abort.clone(),
                            });
                            true
                        } else {
                            tracing::info!(
                                meeting_id = %mid,
                                "auto-summary skipped: a summary job is already running"
                            );
                            false
                        }
                    };

                    if reserved {
                        // Lazy-load the LLM via the same shared helper the generate_summary
                        // command uses. It is idempotent (no-op if already loaded) and
                        // returns NotFound if the GGUF isn't downloaded — in which case we
                        // skip silently (the user hasn't set up the model yet). n_gpu_layers
                        // mirrors the command: 0 (CPU-only) until a settings field exists.
                        let n_gpu_layers: u32 = 0;
                        let load_result = match app2.path().app_data_dir() {
                            Ok(dir) => crate::commands::ai::ensure_llm_loaded(
                                &auto_summary_llm,
                                &dir,
                                n_gpu_layers,
                            ),
                            Err(e) => Err(AppError::Internal(format!("app_data_dir: {e}"))),
                        };

                        match load_result {
                            Ok(()) => {
                                // Spawn on its own thread so the transcription thread
                                // proceeds to finish(&slot) immediately — the user can
                                // start another recording's transcription without waiting
                                // ~60s for the summary to complete.
                                // run_summary owns summary_slot and calls finish() itself.
                                let pool2 = pool.clone();
                                let app3 = app2.clone();
                                let mid2 = mid.clone();
                                let llm2 = auto_summary_llm.clone();
                                let slot2 = auto_summary_slot.clone();
                                std::thread::spawn(move || {
                                    run_summary(pool2, app3, mid2, llm2, slot2, auto_abort);
                                });
                            }
                            Err(e) => {
                                // Model absent or load failed: log and clear the slot we
                                // reserved. Never fails the (already-completed) transcription.
                                tracing::info!(
                                    meeting_id = %mid,
                                    "auto-summary skipped: {e}"
                                );
                                *auto_summary_slot.lock() = None;
                            }
                        }
                    }
                }
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

use smart_noter_diarize::align::TextSegment;
use smart_noter_diarize::models as diar_models;
use smart_noter_diarize::{align, diarize, DiarizationErrorCode, DiarizeOpts};

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
