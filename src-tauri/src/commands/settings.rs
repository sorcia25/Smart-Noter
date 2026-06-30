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

/// The effective audio storage directory (absolute), for display in Settings.
#[tauri::command]
#[specta::specta]
pub fn get_storage_dir(state: State<'_, AppState>) -> String {
    state.audio_dir.lock().display().to_string()
}

/// Prompt for a new storage folder, relocate existing audio there, repoint the DB
/// asset paths, persist the choice, and update the in-memory cache. Returns the new
/// path, or None if the user cancelled the picker. Copy -> repoint -> delete so a
/// mid-operation failure never loses a file (originals stay until the DB is updated).
#[tauri::command]
#[specta::specta]
pub async fn set_storage_dir(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<Option<String>, AppError> {
    use smart_noter_db::repos::MeetingAssetsRepo;
    use tauri_plugin_dialog::DialogExt;

    let Some(folder) = app.dialog().file().blocking_pick_folder() else {
        return Ok(None); // user cancelled
    };
    let new_dir = folder
        .into_path()
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let current = state.audio_dir.lock().clone();
    if current == new_dir {
        return Ok(Some(new_dir.display().to_string()));
    }
    std::fs::create_dir_all(&new_dir)
        .map_err(|e| AppError::Internal(format!("create dir: {e}")))?;

    // Plan moves from the recorded audio assets: old absolute path -> new_dir/<file>.
    let assets = MeetingAssetsRepo(&state.pool).list_all_audio().await?;
    let mut moves: Vec<(String, std::path::PathBuf, std::path::PathBuf)> = Vec::new();
    for a in &assets {
        let old = std::path::PathBuf::from(&a.path);
        let Some(fname) = old.file_name() else {
            continue;
        };
        let dst = new_dir.join(fname);
        if old != dst {
            moves.push((a.id.clone(), old, dst));
        }
    }

    // 1. Copy (keep originals). On any failure, remove partial copies and abort
    //    with the DB and original files untouched.
    for (_, old, dst) in &moves {
        if !old.exists() {
            continue;
        }
        if let Err(e) = std::fs::copy(old, dst) {
            for (_, _, d) in &moves {
                let _ = std::fs::remove_file(d);
            }
            return Err(AppError::Internal(format!("copy to new storage: {e}")));
        }
    }
    // 2. Repoint DB asset paths + persist the setting.
    let repo = MeetingAssetsRepo(&state.pool);
    for (id, _, dst) in &moves {
        repo.update_path(id, &dst.display().to_string()).await?;
    }
    let mut settings = settings_repo::get(&state.pool).await.map_err(from_db)?;
    settings.storage_dir = new_dir.display().to_string();
    settings_repo::upsert(&state.pool, &settings)
        .await
        .map_err(from_db)?;
    // 3. Delete originals (best-effort) and update the cached dir.
    for (_, old, _) in &moves {
        let _ = std::fs::remove_file(old);
    }
    *state.audio_dir.lock() = new_dir.clone();
    // Let the asset protocol serve audio from the new dir at runtime — the static
    // config scope only covers the default app_data/audio, so a custom dir would 403.
    use tauri::Manager;
    let _ = app.asset_protocol_scope().allow_directory(&new_dir, false);
    Ok(Some(new_dir.display().to_string()))
}
