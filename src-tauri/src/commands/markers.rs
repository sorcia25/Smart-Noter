use crate::state::AppState;
use smart_noter_core::{AppError, Marker};
use smart_noter_db::repos::markers_repo::MarkersRepo;
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn list_markers(
    state: State<'_, AppState>,
    meeting_id: String,
) -> Result<Vec<Marker>, AppError> {
    MarkersRepo(&state.pool).list_by_meeting(&meeting_id).await
}

#[tauri::command]
#[specta::specta]
pub async fn create_marker(
    state: State<'_, AppState>,
    meeting_id: String,
    t_seconds: i64,
    label: String,
) -> Result<Marker, AppError> {
    MarkersRepo(&state.pool)
        .create(&meeting_id, t_seconds, "manual", &label, "manual")
        .await
}

#[tauri::command]
#[specta::specta]
pub async fn update_marker(
    state: State<'_, AppState>,
    id: String,
    label: String,
) -> Result<(), AppError> {
    MarkersRepo(&state.pool).update_label(&id, &label).await
}

#[tauri::command]
#[specta::specta]
pub async fn delete_marker(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    MarkersRepo(&state.pool).delete(&id).await
}
