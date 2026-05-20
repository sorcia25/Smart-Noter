use tauri::Manager;
use tauri_specta::{collect_commands, Builder};

pub mod commands;
pub mod error;
pub mod events;
pub mod state;

use crate::state::AppState;

/// Delete any `tmp-*` files left in `audio_dir` by a previous crash, partial
/// finalize, or compensating-rename failure. Called once at startup before the
/// async DB block so orphans are removed before any new recording can begin.
fn sweep_orphan_tmp_files(audio_dir: &std::path::Path) {
    if let Ok(entries) = std::fs::read_dir(audio_dir) {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with("tmp-") {
                let _ = std::fs::remove_file(e.path());
                tracing::info!("swept orphan {name}");
            }
        }
    }
}

pub fn specta_builder() -> Builder {
    Builder::<tauri::Wry>::new().commands(collect_commands![
        commands::audio::discard_recording,
        commands::audio::finalize_recording,
        commands::audio::pause_recording,
        commands::audio::resume_recording,
        commands::audio::start_preview,
        commands::audio::start_recording,
        commands::audio::stop_preview,
        commands::audio::stop_recording,
        commands::meetings::list_meetings,
        commands::meetings::get_meeting,
        commands::meetings::update_meeting_title,
        commands::meetings::toggle_action,
        commands::meetings::rename_participant,
        commands::templates::list_templates,
        commands::templates::set_default_template,
        commands::devices::list_audio_devices,
        commands::settings::get_settings,
        commands::settings::update_settings,
        commands::log::log_frontend_error,
    ])
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = specta_builder();

    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::default().build())
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);

            // Sweep orphaned tmp-* files before any DB or UI work.
            if let Ok(app_data) = app.handle().path().app_data_dir() {
                sweep_orphan_tmp_files(&app_data.join("audio"));
            }

            let app_handle = app.handle().clone();
            tauri::async_runtime::block_on(async move {
                let app_data = app_handle.path().app_data_dir().expect("app_data_dir");
                std::fs::create_dir_all(&app_data).ok();
                let db_path = app_data.join("db.sqlite");
                let pool = smart_noter_db::init_pool(&db_path)
                    .await
                    .expect("init pool");

                // Write embedded seed to disk and seed if empty
                let seed_path = app_data.join("seed_data.json");
                if !seed_path.exists() {
                    let bytes = include_bytes!("../crates/db/seed_data.json");
                    std::fs::write(&seed_path, bytes).expect("write seed json");
                }
                smart_noter_db::seed::seed_if_empty(&pool, &seed_path)
                    .await
                    .expect("seed");

                app_handle.manage(AppState {
                    pool,
                    capture_session: std::sync::Arc::new(parking_lot::Mutex::new(
                        smart_noter_audio::capture::session::CaptureSession::default(),
                    )),
                    recorder: std::sync::Arc::new(parking_lot::Mutex::new(None)),
                });
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
