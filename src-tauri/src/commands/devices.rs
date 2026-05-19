#[tauri::command]
#[specta::specta]
pub fn list_audio_devices(
) -> Result<Vec<smart_noter_audio::AudioDevice>, smart_noter_core::AppError> {
    smart_noter_audio::enumerate().map_err(Into::into)
}
