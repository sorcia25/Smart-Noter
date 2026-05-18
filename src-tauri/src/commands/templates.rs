use crate::error::from_db;
use crate::state::AppState;
use smart_noter_core::{models::Template, AppError};
use smart_noter_db::repos::templates_repo;
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn list_templates(state: State<'_, AppState>) -> Result<Vec<Template>, AppError> {
    templates_repo::list_all(&state.pool).await.map_err(from_db)
}

#[tauri::command]
#[specta::specta]
pub async fn set_default_template(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    templates_repo::set_default(&state.pool, &id)
        .await
        .map_err(from_db)
}
