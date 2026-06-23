use crate::error::from_db;
use crate::state::AppState;
use smart_noter_core::{
    models::{MeetingDetail, MeetingSummary},
    AppError,
};
use smart_noter_db::repos::{
    actions_repo, blockers_repo, decisions_repo, meetings_repo, participants_repo,
};
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn list_meetings(state: State<'_, AppState>) -> Result<Vec<MeetingSummary>, AppError> {
    meetings_repo::list_summaries(&state.pool)
        .await
        .map_err(from_db)
}

#[tauri::command]
#[specta::specta]
pub async fn get_meeting(
    state: State<'_, AppState>,
    id: String,
) -> Result<MeetingDetail, AppError> {
    meetings_repo::get_detail(&state.pool, &id)
        .await
        .map_err(from_db)
}

#[tauri::command]
#[specta::specta]
pub async fn update_meeting_title(
    state: State<'_, AppState>,
    id: String,
    title_es: String,
    title_en: Option<String>,
) -> Result<(), AppError> {
    meetings_repo::update_title(&state.pool, &id, &title_es, title_en.as_deref())
        .await
        .map_err(from_db)
}

#[tauri::command]
#[specta::specta]
pub async fn toggle_action(
    state: State<'_, AppState>,
    action_id: String,
) -> Result<bool, AppError> {
    actions_repo::toggle(&state.pool, &action_id)
        .await
        .map_err(from_db)
}

#[tauri::command]
#[specta::specta]
pub async fn rename_participant(
    state: State<'_, AppState>,
    participant_id: String,
    name: Option<String>,
) -> Result<(), AppError> {
    participants_repo::rename(&state.pool, &participant_id, name.as_deref())
        .await
        .map_err(from_db)
}

#[tauri::command]
#[specta::specta]
pub async fn merge_speakers(
    state: State<'_, AppState>,
    into: String,
    from: String,
) -> Result<(), AppError> {
    participants_repo::merge_speakers(&state.pool, &into, &from)
        .await
        .map_err(from_db)
}

#[tauri::command]
#[specta::specta]
pub async fn reassign_lines(
    state: State<'_, AppState>,
    line_ids: Vec<i64>,
    speaker_id: String,
) -> Result<(), AppError> {
    participants_repo::reassign_lines(&state.pool, &line_ids, &speaker_id)
        .await
        .map_err(from_db)
}

#[tauri::command]
#[specta::specta]
pub async fn create_speaker(
    state: State<'_, AppState>,
    meeting_id: String,
) -> Result<String, AppError> {
    participants_repo::create_speaker(&state.pool, &meeting_id)
        .await
        .map_err(from_db)
}

#[tauri::command]
#[specta::specta]
pub async fn delete_meeting(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    meetings_repo::soft_delete(&state.pool, &id)
        .await
        .map_err(from_db)
}

#[tauri::command]
#[specta::specta]
pub async fn restore_meeting(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    meetings_repo::restore(&state.pool, &id)
        .await
        .map_err(from_db)
}

#[tauri::command]
#[specta::specta]
pub async fn list_trashed_meetings(
    state: State<'_, AppState>,
) -> Result<Vec<MeetingSummary>, AppError> {
    meetings_repo::list_trashed(&state.pool)
        .await
        .map_err(from_db)
}

#[tauri::command]
#[specta::specta]
pub async fn purge_meeting(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    let paths = meetings_repo::purge(&state.pool, &id)
        .await
        .map_err(from_db)?;
    for p in paths {
        if let Err(e) = std::fs::remove_file(&p) {
            tracing::warn!("purge_meeting: could not delete audio file {p}: {e}");
        }
    }
    Ok(())
}

// ---- actions ----
#[tauri::command]
#[specta::specta]
pub async fn create_action(
    state: State<'_, AppState>,
    meeting_id: String,
    text: String,
    owner_participant_id: Option<String>,
    due: Option<String>,
) -> Result<String, AppError> {
    actions_repo::create(
        &state.pool,
        &meeting_id,
        &text,
        owner_participant_id.as_deref(),
        due.as_deref(),
    )
    .await
    .map_err(from_db)
}

#[tauri::command]
#[specta::specta]
pub async fn update_action(
    state: State<'_, AppState>,
    action_id: String,
    text: String,
    owner_participant_id: Option<String>,
    due: Option<String>,
) -> Result<(), AppError> {
    actions_repo::update(
        &state.pool,
        &action_id,
        &text,
        owner_participant_id.as_deref(),
        due.as_deref(),
    )
    .await
    .map_err(from_db)
}

#[tauri::command]
#[specta::specta]
pub async fn delete_action(state: State<'_, AppState>, action_id: String) -> Result<(), AppError> {
    actions_repo::delete(&state.pool, &action_id)
        .await
        .map_err(from_db)
}

// ---- decisions ----
#[tauri::command]
#[specta::specta]
pub async fn create_decision(
    state: State<'_, AppState>,
    meeting_id: String,
    text: String,
) -> Result<i64, AppError> {
    decisions_repo::create(&state.pool, &meeting_id, &text)
        .await
        .map_err(from_db)
}

#[tauri::command]
#[specta::specta]
pub async fn update_decision(
    state: State<'_, AppState>,
    id: i64,
    text: String,
) -> Result<(), AppError> {
    decisions_repo::update(&state.pool, id, &text)
        .await
        .map_err(from_db)
}

#[tauri::command]
#[specta::specta]
pub async fn delete_decision(state: State<'_, AppState>, id: i64) -> Result<(), AppError> {
    decisions_repo::delete(&state.pool, id)
        .await
        .map_err(from_db)
}

// ---- blockers ----
#[tauri::command]
#[specta::specta]
pub async fn create_blocker(
    state: State<'_, AppState>,
    meeting_id: String,
    text: String,
) -> Result<i64, AppError> {
    blockers_repo::create(&state.pool, &meeting_id, &text)
        .await
        .map_err(from_db)
}

#[tauri::command]
#[specta::specta]
pub async fn update_blocker(
    state: State<'_, AppState>,
    id: i64,
    text: String,
) -> Result<(), AppError> {
    blockers_repo::update(&state.pool, id, &text)
        .await
        .map_err(from_db)
}

#[tauri::command]
#[specta::specta]
pub async fn delete_blocker(state: State<'_, AppState>, id: i64) -> Result<(), AppError> {
    blockers_repo::delete(&state.pool, id)
        .await
        .map_err(from_db)
}
