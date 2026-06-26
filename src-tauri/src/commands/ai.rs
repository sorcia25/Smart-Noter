//! AI summary commands: generate_summary, cancel_summary, update_summary_text,
//! get_summary_state, plus download/delete/list commands for LLM models.
//!
//! ## LLM singleton design
//!
//! `LocalLlm::load()` calls `LlamaBackend::init()`, which can run **only once per
//! process** (a second call returns `BackendAlreadyInitialized`).  We store the
//! loaded `LocalLlm` in `AppState.llm: Arc<Mutex<Option<LocalLlm>>>`.  On first
//! use we load it and keep it — on subsequent calls we reuse the held instance.
//! The `Arc<Mutex<...>>` lets us clone the Arc into the spawned thread, lock it
//! there, and call `generate` / `analyze` while still holding the lock, then drop
//! the lock when inference finishes.  Because `LocalLlm` has `unsafe impl Send +
//! Sync` (declared in engine.rs), this is safe.

use crate::error::from_db;
use crate::state::{AppState, DownloadHandle, SummaryHandle};
use serde::Serialize;
use smart_noter_core::traits::{AnalysisInput, Summarizer};
use smart_noter_core::{AppError, Bilingual};
use smart_noter_db::repos::{
    actions_repo, blockers_repo, decisions_repo, meetings_repo, templates_repo,
};
use smart_noter_llm::{engine::LocalLlm, models as llm_models, summarize::LocalSummarizer};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{Emitter, Manager};

// ---------------------------------------------------------------------------
// app_data helper (mirrors transcription.rs)
// ---------------------------------------------------------------------------

fn app_data(app: &tauri::AppHandle) -> Result<std::path::PathBuf, AppError> {
    app.path()
        .app_data_dir()
        .map_err(|e| AppError::Internal(format!("app_data_dir: {e}")))
}

// ---------------------------------------------------------------------------
// Event payloads
// ---------------------------------------------------------------------------

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct SummaryProgressEvent {
    meeting_id: String,
    pct: u32,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct SummaryCompletedEvent {
    meeting_id: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct SummaryFailedEvent {
    meeting_id: String,
    code: String,
    message: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct LlmDownloadProgressEvent {
    id: String,
    pct: u32,
    bytes_downloaded: u64,
    bytes_total: u64,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct LlmDownloadDoneEvent {
    id: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct LlmDownloadFailEvent {
    id: String,
    code: String,
    message: String,
}

// ---------------------------------------------------------------------------
// Lazy-load helper: get (or create) the singleton LocalLlm
// ---------------------------------------------------------------------------

/// Returns a clone of the `Arc<Mutex<Option<LocalLlm>>>` after ensuring the
/// inner `Option` holds a loaded model.  If the model GGUF is absent this
/// returns `Err(AppError::NotFound(...))`.
///
/// **The Mutex is locked only for the duration of the load, then dropped.**
/// The caller receives the Arc and can lock it independently to run inference.
fn ensure_llm_loaded(
    llm_arc: &Arc<parking_lot::Mutex<Option<LocalLlm>>>,
    app_data_dir: &std::path::Path,
    n_gpu_layers: u32,
) -> Result<(), AppError> {
    let mut slot = llm_arc.lock();
    if slot.is_some() {
        // Already loaded — nothing to do.
        return Ok(());
    }

    let model_id = "qwen2.5-3b-instruct-q4";
    let model_path = llm_models::model_path(app_data_dir, model_id);
    if !model_path.is_file() {
        return Err(AppError::NotFound(format!(
            "LLM model not downloaded: {model_id}. Download it first via list_llm_models / download_llm_model."
        )));
    }

    let llm = LocalLlm::load(&model_path, n_gpu_layers)
        .map_err(|e| AppError::Internal(format!("LLM load failed: {e}")))?;
    *slot = Some(llm);
    Ok(())
}

// ---------------------------------------------------------------------------
// run_summary — the shared orchestrator
// ---------------------------------------------------------------------------

/// Run the full summary pipeline for `meeting_id`.
///
/// This function is designed to be called from inside a `std::thread::spawn`
/// closure (like `transcription.rs`).  All async DB calls use
/// `tauri::async_runtime::block_on`.
///
/// It:
/// 1. Loads the meeting detail (transcript + participants).
/// 2. Resolves speaker labels (participant name or fallback to "S{n}").
/// 3. Loads the template sections for the meeting's template_id.
/// 4. Builds `AnalysisInput` and calls `LocalSummarizer::analyze`, emitting
///    `summary:progress` events.
/// 5. On success: persists the summary, deletes AI items, re-inserts them
///    from `MeetingAnalysis`, emits `summary:completed`.
/// 6. On failure / abort: emits `summary:failed`.
/// 7. Clears the `SummaryHandle` slot when done.
pub fn run_summary(
    pool: sqlx::SqlitePool,
    app: tauri::AppHandle,
    meeting_id: String,
    llm_arc: Arc<parking_lot::Mutex<Option<LocalLlm>>>,
    summary_slot: Arc<parking_lot::Mutex<Option<SummaryHandle>>>,
    abort: Arc<AtomicBool>,
) {
    let finish = |slot: &Arc<parking_lot::Mutex<Option<SummaryHandle>>>| {
        *slot.lock() = None;
    };

    // Helper closures for emitting events.
    let emit_progress = |pct: u32| {
        let _ = app.emit(
            "summary:progress",
            SummaryProgressEvent {
                meeting_id: meeting_id.clone(),
                pct,
            },
        );
    };
    let emit_completed = || {
        let _ = app.emit(
            "summary:completed",
            SummaryCompletedEvent {
                meeting_id: meeting_id.clone(),
            },
        );
    };
    let emit_failed = |code: &str, message: &str| {
        let _ = app.emit(
            "summary:failed",
            SummaryFailedEvent {
                meeting_id: meeting_id.clone(),
                code: code.to_string(),
                message: message.to_string(),
            },
        );
    };

    // 1. Load meeting detail (transcript + participants).
    let detail = match tauri::async_runtime::block_on(meetings_repo::get_detail(&pool, &meeting_id))
    {
        Ok(d) => d,
        Err(e) => {
            emit_failed("DatabaseError", &e.to_string());
            finish(&summary_slot);
            return;
        }
    };

    if abort.load(Ordering::Relaxed) {
        emit_failed("Cancelled", "cancelled before inference");
        finish(&summary_slot);
        return;
    }

    // 2. Build speaker label map: participant_id → display name.
    //    Falls back to the participant's label ("S1", "S2"...) if no name set.
    let label_map: std::collections::HashMap<String, String> = detail
        .participants
        .iter()
        .map(|p| {
            let display = p
                .name
                .clone()
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| p.label.clone());
            (p.id.clone(), display)
        })
        .collect();

    // 3. Build the transcript pairs (speaker_label, text_es).
    let transcript_pairs: Vec<(String, String)> = detail
        .transcript
        .iter()
        .map(|line| {
            let label = label_map
                .get(&line.speaker_id)
                .cloned()
                .unwrap_or_else(|| line.speaker_id.clone());
            (label, line.text.es.clone())
        })
        .collect();

    // 4. Load template sections.
    let template_id = detail.template.clone();
    let all_templates = match tauri::async_runtime::block_on(templates_repo::list_all(&pool)) {
        Ok(t) => t,
        Err(e) => {
            emit_failed("DatabaseError", &format!("templates: {e}"));
            finish(&summary_slot);
            return;
        }
    };
    let template_sections: Vec<String> = all_templates
        .iter()
        .find(|t| t.id == template_id)
        .map(|t| t.sections.clone())
        .unwrap_or_default();
    if template_sections.is_empty() {
        tracing::warn!(meeting_id = %meeting_id, template = %template_id, "no template sections found; summary will be unstructured");
    }

    // App is Spanish-primary, so summaries are generated in Spanish for now.
    // TODO(future): derive from settings.language once multi-language summary is supported.
    let lang = "es".to_string();

    let input = AnalysisInput {
        transcript: transcript_pairs,
        template_sections,
        lang,
    };

    emit_progress(5);

    // 5. Run inference.  We lock the LLM for the entire inference duration so
    //    that no second summary can share the backend simultaneously.
    let analysis = {
        let llm_guard = llm_arc.lock();
        let llm = match llm_guard.as_ref() {
            Some(l) => l,
            None => {
                emit_failed("ModelNotLoaded", "LLM slot was empty at inference time");
                finish(&summary_slot);
                return;
            }
        };
        let summarizer = LocalSummarizer { llm };
        let abort_ref: &AtomicBool = &abort;
        let mut progress_cb = |pct: u32| {
            if abort_ref.load(Ordering::Relaxed) {
                return;
            }
            emit_progress(pct);
        };
        summarizer.analyze(&input, &mut progress_cb, abort_ref)
    };

    if abort.load(Ordering::Relaxed) {
        emit_failed("Cancelled", "cancelled during inference");
        finish(&summary_slot);
        return;
    }

    let analysis = match analysis {
        Ok(a) => a,
        Err(e) => {
            emit_failed("InferenceError", &e);
            finish(&summary_slot);
            return;
        }
    };

    emit_progress(90);

    // 6. Persist: update summary, replace AI decisions/blockers/actions.
    let persist_result = tauri::async_runtime::block_on(async {
        meetings_repo::update_summary(&pool, &meeting_id, &analysis.summary).await?;

        decisions_repo::delete_ai(&pool, &meeting_id).await?;
        for text in &analysis.decisions {
            decisions_repo::create_with_source(&pool, &meeting_id, text, "ai").await?;
        }

        blockers_repo::delete_ai(&pool, &meeting_id).await?;
        for text in &analysis.blockers {
            blockers_repo::create_with_source(&pool, &meeting_id, text, "ai").await?;
        }

        actions_repo::delete_ai(&pool, &meeting_id).await?;
        for action in &analysis.actions {
            actions_repo::create_with_source(
                &pool,
                &meeting_id,
                &action.text,
                action.owner_hint.as_deref(),
                action.due.as_deref(),
                "ai",
            )
            .await?;
        }

        // TODO(Task10): chunk + embed the transcript for RAG and persist chunks.

        Ok::<(), smart_noter_db::DbError>(())
    });

    match persist_result {
        Ok(()) => {
            emit_progress(100);
            emit_completed();
        }
        Err(e) => {
            emit_failed("DatabaseError", &e.to_string());
        }
    }

    finish(&summary_slot);
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

#[tauri::command]
#[specta::specta]
pub async fn generate_summary(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
    meeting_id: String,
) -> Result<(), AppError> {
    // Reserve the single summary slot.
    let handle = SummaryHandle {
        meeting_id: meeting_id.clone(),
        abort: Arc::new(AtomicBool::new(false)),
    };
    {
        let mut slot = state.summary.lock();
        if slot.is_some() {
            return Err(AppError::Validation(
                "a summary is already running".to_string(),
            ));
        }
        *slot = Some(handle.clone());
    }

    // Helper to clear the slot on early return (before thread spawn).
    let clear = || {
        *state.summary.lock() = None;
    };

    // n_gpu_layers: AppSettings has no GPU-layers field yet — use 0 (CPU-only) conservatively.
    // Task 9 (settings UI) will add this field; until then the LLM always runs on CPU.
    let n_gpu_layers: u32 = 0;

    let app_dir = match app_data(&app) {
        Ok(d) => d,
        Err(e) => {
            clear();
            return Err(e);
        }
    };

    // Lazy-load the LLM (may already be loaded from a previous call).
    let llm_arc = state.llm.clone();
    if let Err(e) = ensure_llm_loaded(&llm_arc, &app_dir, n_gpu_layers) {
        clear();
        return Err(e);
    }

    // Clone everything needed by the thread.
    let pool = state.pool.clone();
    let summary_slot = state.summary.clone();
    let abort = handle.abort.clone();
    let mid = meeting_id.clone();
    let app2 = app.clone();

    std::thread::spawn(move || {
        run_summary(pool, app2, mid, llm_arc, summary_slot, abort);
    });

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn cancel_summary(
    state: tauri::State<'_, AppState>,
    meeting_id: String,
) -> Result<(), AppError> {
    if let Some(h) = state.summary.lock().as_ref() {
        if h.meeting_id == meeting_id {
            h.abort.store(true, Ordering::Relaxed);
        }
    }
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn update_summary_text(
    state: tauri::State<'_, AppState>,
    meeting_id: String,
    summary: Bilingual,
) -> Result<(), AppError> {
    meetings_repo::update_summary(&state.pool, &meeting_id, &summary)
        .await
        .map_err(from_db)
}

#[tauri::command]
#[specta::specta]
pub fn get_summary_state(state: tauri::State<'_, AppState>) -> Result<Option<String>, AppError> {
    Ok(state.summary.lock().as_ref().map(|h| h.meeting_id.clone()))
}

// ---------------------------------------------------------------------------
// LLM model management commands
// ---------------------------------------------------------------------------

#[derive(serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LlmModelInfo {
    pub id: String,
    pub name: String,
    pub size_mb: u32,
    pub downloaded: bool,
}

#[tauri::command]
#[specta::specta]
pub fn list_llm_models(app: tauri::AppHandle) -> Result<Vec<LlmModelInfo>, AppError> {
    let dir = app_data(&app)?;
    Ok(llm_models::list(&dir)
        .into_iter()
        .map(|m| LlmModelInfo {
            id: m.id.to_string(),
            name: m.display_name.to_string(),
            size_mb: m.size_mb,
            downloaded: m.downloaded,
        })
        .collect())
}

#[tauri::command]
#[specta::specta]
pub fn download_llm_model(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
    id: String,
) -> Result<(), AppError> {
    let handle = DownloadHandle {
        id: id.clone(),
        abort: Arc::new(AtomicBool::new(false)),
    };
    {
        let mut slot = state.llm_download.lock();
        if slot.is_some() {
            return Err(AppError::Validation(
                "an LLM model download is already running".to_string(),
            ));
        }
        *slot = Some(handle.clone());
    }
    let dir = match app_data(&app) {
        Ok(d) => d,
        Err(e) => {
            *state.llm_download.lock() = None;
            return Err(e);
        }
    };
    let slot = state.llm_download.clone();
    let abort = handle.abort.clone();
    let app2 = app.clone();
    let id2 = id.clone();
    std::thread::spawn(move || {
        let app3 = app2.clone();
        let id3 = id2.clone();
        let progress = move |pct: u32, dl: u64, total: u64| {
            let _ = app3.emit(
                "llm-download:progress",
                LlmDownloadProgressEvent {
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
        match llm_models::download(&dir, &id2, progress, is_cancelled) {
            Ok(()) => {
                let _ = app2.emit(
                    "llm-download:completed",
                    LlmDownloadDoneEvent { id: id2.clone() },
                );
            }
            Err(e) => {
                let _ = app2.emit(
                    "llm-download:failed",
                    LlmDownloadFailEvent {
                        id: id2.clone(),
                        code: "DownloadError".into(),
                        message: e.to_string(),
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
pub fn cancel_llm_download(state: tauri::State<'_, AppState>, id: String) -> Result<(), AppError> {
    if let Some(h) = state.llm_download.lock().as_ref() {
        if h.id == id {
            h.abort.store(true, Ordering::Relaxed);
        }
    }
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn delete_llm_model(app: tauri::AppHandle, id: String) -> Result<(), AppError> {
    let dir = app_data(&app)?;
    llm_models::delete(&dir, &id).map_err(|e| AppError::Internal(e.to_string()))
}
