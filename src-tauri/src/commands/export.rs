use crate::error::from_db;
use crate::state::AppState;
use smart_noter_core::AppError;
use smart_noter_db::repos::{meeting_assets_repo::MeetingAssetsRepo, meetings_repo};
use smart_noter_export::{to_markdown, to_pdf, wav_or_flac_to_mp3, ExportOpts};
use std::path::PathBuf;
use tauri::State;
use tauri_plugin_dialog::DialogExt;

fn ext_for(fmt: &str) -> &'static str {
    match fmt {
        "audio" => "mp3",
        "pdf" => "pdf",
        _ => "md",
    }
}

/// Replace characters that are illegal in filenames (Windows is the strictest)
/// so the export can actually be written to disk. Spanish meeting titles often
/// contain `:` (e.g. "Reunión: alineación"), which would make `fs::write` fail
/// on Windows. Falls back to "export" if nothing usable remains.
fn sanitize_filename(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '-',
            c if (c as u32) < 0x20 => '-',
            c => c,
        })
        .collect();
    let trimmed = cleaned.trim_matches(['.', ' ', '-']);
    if trimmed.is_empty() {
        "export".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Generate the bytes for one format. `audio` needs the audio path from the DB.
async fn bytes_for(
    pool: &sqlx::SqlitePool,
    fmt: &str,
    detail: &smart_noter_core::MeetingDetail,
    opts: &ExportOpts,
) -> Result<Vec<u8>, AppError> {
    match fmt {
        "md" => Ok(to_markdown(detail, opts).into_bytes()),
        "pdf" => {
            // PDF layout (genpdf) is CPU-bound; offload it like the MP3 encode
            // so a large transcript doesn't block the async runtime's reactor.
            let detail = detail.clone();
            let opts = *opts;
            tauri::async_runtime::spawn_blocking(move || to_pdf(&detail, &opts))
                .await
                .map_err(|e| AppError::Internal(e.to_string()))?
                .map_err(|e| AppError::Internal(e.to_string()))
        }
        "audio" => {
            let asset = MeetingAssetsRepo(pool)
                .get_audio(&detail.id)
                .await?
                .ok_or_else(|| AppError::NotFound(format!("no audio for {}", detail.id)))?;
            let path = PathBuf::from(asset.path);
            // Encoding is CPU-bound; run it off the async runtime's reactor.
            tauri::async_runtime::spawn_blocking(move || wav_or_flac_to_mp3(&path))
                .await
                .map_err(|e| AppError::Internal(e.to_string()))?
                .map_err(|e| AppError::Internal(e.to_string()))
        }
        other => Err(AppError::Validation(format!("unknown format: {other}"))),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn export_meeting(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
    meeting_id: String,
    formats: Vec<String>,
    file_name: String,
    timestamps: bool,
    bilingual: bool,
) -> Result<Vec<String>, AppError> {
    if formats.is_empty() {
        return Err(AppError::Validation("no formats selected".into()));
    }
    // Defend against filesystem-illegal characters before they reach
    // set_file_name / dir.join (the dialog hint and the actual write path).
    let file_name = sanitize_filename(&file_name);

    let detail = meetings_repo::get_detail(&state.pool, &meeting_id)
        .await
        .map_err(from_db)?;

    let opts = ExportOpts {
        timestamps,
        bilingual,
    };

    // Generate every artifact first so the dialog only appears once data is ready.
    let mut artifacts: Vec<(String, Vec<u8>)> = Vec::new(); // (ext, bytes)
    for fmt in &formats {
        let bytes = bytes_for(&state.pool, fmt, &detail, &opts).await?;
        artifacts.push((ext_for(fmt).to_string(), bytes));
    }

    // Single format → "Save as"; multiple → "Select folder".
    let written: Vec<String> = if artifacts.len() == 1 {
        let (ext, bytes) = &artifacts[0];
        let Some(path) = app
            .dialog()
            .file()
            .set_file_name(format!("{file_name}.{ext}"))
            .add_filter(ext.to_uppercase(), &[ext.as_str()])
            .blocking_save_file()
        else {
            return Ok(vec![]); // user cancelled
        };
        let path = path
            .into_path()
            .map_err(|e| AppError::Internal(e.to_string()))?;
        std::fs::write(&path, bytes).map_err(|e| AppError::Internal(e.to_string()))?;
        vec![path.display().to_string()]
    } else {
        let Some(dir) = app.dialog().file().blocking_pick_folder() else {
            return Ok(vec![]); // user cancelled
        };
        let dir = dir
            .into_path()
            .map_err(|e| AppError::Internal(e.to_string()))?;
        let mut out = Vec::new();
        for (ext, bytes) in &artifacts {
            let path = dir.join(format!("{file_name}.{ext}"));
            std::fs::write(&path, bytes).map_err(|e| AppError::Internal(e.to_string()))?;
            out.push(path.display().to_string());
        }
        out
    };

    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::sanitize_filename;

    #[test]
    fn strips_windows_illegal_chars() {
        // Spanish titles routinely contain ':' which is illegal on Windows.
        assert_eq!(
            sanitize_filename("Reunión: alineación"),
            "Reunión- alineación"
        );
        assert_eq!(sanitize_filename("a/b\\c"), "a-b-c");
        assert_eq!(sanitize_filename("q?<>|*\"x"), "q------x");
    }

    #[test]
    fn keeps_accents_and_normal_chars() {
        assert_eq!(
            sanitize_filename("plan-Q2_diseño 2026"),
            "plan-Q2_diseño 2026"
        );
    }

    #[test]
    fn falls_back_when_nothing_usable_remains() {
        assert_eq!(sanitize_filename(":::"), "export");
        assert_eq!(sanitize_filename("   "), "export");
        assert_eq!(sanitize_filename(""), "export");
    }
}
