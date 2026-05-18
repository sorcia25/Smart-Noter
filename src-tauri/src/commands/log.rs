use smart_noter_core::AppError;
use tracing::{error, info, warn};

#[tauri::command]
#[specta::specta]
pub fn log_frontend_error(
    level: String,
    message: String,
    stack: Option<String>,
) -> Result<(), AppError> {
    match level.as_str() {
        "error" => error!(target: "frontend", "{message}\n{}", stack.unwrap_or_default()),
        "warn" => warn!(target: "frontend", "{message}"),
        _ => info!(target: "frontend", "{message}"),
    }
    Ok(())
}
