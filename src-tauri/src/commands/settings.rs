use crate::error::from_db;
use crate::state::AppState;
use smart_noter_core::{models::AppSettings, AppError};
use smart_noter_db::repos::settings_repo;
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn get_settings(state: State<'_, AppState>) -> Result<AppSettings, AppError> {
    let mut s = settings_repo::get(&state.pool).await.map_err(from_db)?;
    // Normalize a legacy persisted model display-name to the catalog id.
    if smart_noter_whisper::models::find(&s.transcription_model).is_none() {
        s.transcription_model = "large-v3".into();
    }
    Ok(s)
}

#[tauri::command]
#[specta::specta]
pub async fn update_settings(
    state: State<'_, AppState>,
    settings: AppSettings,
) -> Result<(), AppError> {
    settings_repo::upsert(&state.pool, &settings)
        .await
        .map_err(from_db)
}
