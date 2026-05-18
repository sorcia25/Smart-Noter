use crate::error::from_db;
use crate::state::AppState;
use smart_noter_core::{
    models::{MeetingDetail, MeetingSummary},
    AppError,
};
use smart_noter_db::repos::{actions_repo, meetings_repo, participants_repo};
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
