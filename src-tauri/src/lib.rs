mod commands;
mod db;
mod error;
mod models;
mod state;

use state::AppState;
use std::sync::Mutex;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // Каталог данных: %APPDATA%\com.devdeck.app\ (по identifier)
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            std::fs::create_dir_all(dir.join("backups"))?;
            let conn = db::open(&dir.join("devdeck.db"))
                .map_err(|e| format!("db init failed: {}", e.message))?;
            app.manage(AppState { db: Mutex::new(conn) });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::health::db_health,
            commands::projects::projects_list,
            commands::projects::projects_get,
            commands::projects::projects_create,
            commands::projects::projects_update,
            commands::projects::projects_delete,
            commands::projects::projects_archive,
            commands::projects::project_set_pinned,
            commands::projects::project_set_sort,
            commands::actions::open_path,
            commands::actions::open_in_editor,
            commands::actions::open_terminal,
            commands::actions::open_url,
            commands::git::git_status,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
