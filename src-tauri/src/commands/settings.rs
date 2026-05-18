use crate::error::from_db;
use crate::state::AppState;
use smart_noter_core::{models::AppSettings, AppError};
use smart_noter_db::repos::settings_repo;
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn get_settings(state: State<'_, AppState>) -> Result<AppSettings, AppError> {
    settings_repo::get(&state.pool).await.map_err(from_db)
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
