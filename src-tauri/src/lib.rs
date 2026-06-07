mod backup;
mod commands;
mod crypto;
mod db;
mod error;
mod models;
mod state;
mod tray;
mod watch;

use state::AppState;
use std::sync::Mutex;
use tauri::tray::TrayIconBuilder;
use tauri::Emitter;
use tauri::Manager;
use tauri::WindowEvent;

fn show_main(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.set_focus();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // Каталог данных: %APPDATA%\com.devdeck.app\ (по identifier)
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            std::fs::create_dir_all(dir.join("backups"))?;
            let conn = db::open(&dir.join("devdeck.db"))
                .map_err(|e| format!("db init failed: {}", e.message))?;
            backup::maybe_auto_backup(&conn, &dir.join("backups"));
            app.manage(AppState { db: Mutex::new(conn), master_key: Mutex::new(None) });
            app.manage(watch::WatchState::default());
            app.manage(tray::TrayState::default());
            app.manage(commands::cmds::RunningState::default());

            // --- системный трей ---
            let menu = tray::build_menu(app.handle())?;

            let tray_icon = TrayIconBuilder::with_id("main-tray")
                .icon(app.default_window_icon().expect("есть иконка окна").clone())
                .tooltip("DevDeck")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| {
                    let id = event.id.as_ref();
                    match id {
                        "open" => show_main(app),
                        "quit" => app.exit(0),
                        _ => {
                            if let Some(rest) = id.strip_prefix("proj:") {
                                if let Ok(pid) = rest.parse::<i64>() {
                                    show_main(app);
                                    let _ = app.emit("tray-open-project", pid);
                                }
                            }
                        }
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    use tauri::tray::TrayIconEvent;
                    if let TrayIconEvent::Click { button, button_state, .. } = event {
                        if button == tauri::tray::MouseButton::Left
                            && button_state == tauri::tray::MouseButtonState::Up
                        {
                            show_main(tray.app_handle());
                        }
                    }
                })
                .build(app)?;

            // сохранить хэндл трея для пересборки меню (закреплённые проекты)
            if let Ok(mut g) = app.state::<tray::TrayState>().icon.lock() {
                *g = Some(tray_icon);
            }

            // --- закрытие окна = сворачивание в трей ---
            if let Some(window) = app.get_webview_window("main") {
                let w = window.clone();
                window.on_window_event(move |event| {
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = w.hide();
                    }
                });
            }

            // --- слежение за папками проектов (notify) ---
            let _ = watch::resync(app.handle());

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::health::db_health,
            commands::monitor::health_check,
            commands::projects::projects_list,
            commands::projects::projects_get,
            commands::projects::projects_create,
            commands::projects::projects_update,
            commands::projects::projects_delete,
            commands::projects::projects_archive,
            commands::projects::project_set_pinned,
            commands::projects::project_set_sort,
            commands::projects::project_import_icon,
            commands::actions::open_path,
            commands::actions::open_in_editor,
            commands::actions::open_terminal,
            commands::actions::open_url,
            commands::git::git_status,
            commands::git::git_fetch,
            commands::git::git_pull,
            commands::git::git_push,
            commands::git::git_commit_all,
            commands::git::dashboard_attention,
            commands::tasks::tasks_list,
            commands::tasks::tasks_create,
            commands::tasks::tasks_update,
            commands::tasks::tasks_move,
            commands::tasks::tasks_reorder,
            commands::tasks::tasks_delete_completed,
            commands::tasks::tasks_delete,
            commands::tasks::tasks_agenda,
            commands::labels::labels_list,
            commands::labels::label_create,
            commands::labels::label_update,
            commands::labels::label_delete,
            commands::labels::task_set_labels,
            commands::labels::labels_reorder,
            commands::columns::columns_list,
            commands::columns::column_create,
            commands::columns::column_update,
            commands::columns::column_delete,
            commands::columns::columns_reorder,
            commands::checklists::checklists_list,
            commands::checklists::checklists_create,
            commands::checklists::checklists_update,
            commands::checklists::checklists_delete,
            commands::checklists::checklist_items_add,
            commands::checklists::checklist_items_update,
            commands::checklists::checklist_items_toggle,
            commands::checklists::checklist_items_delete,
            commands::creds::creds_list,
            commands::creds::creds_get_secret,
            commands::creds::creds_create,
            commands::creds::creds_update,
            commands::creds::creds_delete,
            commands::notes::notes_list,
            commands::notes::notes_create,
            commands::notes::notes_update,
            commands::notes::notes_delete,
            commands::links::links_list,
            commands::links::links_create,
            commands::links::links_update,
            commands::links::links_delete,
            commands::files::files_list,
            commands::files::files_create,
            commands::files::files_update,
            commands::files::files_delete,
            commands::actions::open_shortcut,
            commands::cmds::commands_list,
            commands::cmds::commands_create,
            commands::cmds::commands_update,
            commands::cmds::commands_delete,
            commands::cmds::command_run,
            commands::cmds::command_run_bg,
            commands::cmds::command_stop,
            commands::cmds::command_running,
            commands::search::search_global,
            commands::backup::backup_now,
            commands::backup::backups_list,
            commands::templates::templates_list,
            commands::templates::template_save,
            commands::templates::template_apply,
            commands::templates::template_delete,
            commands::transfer::export_json,
            commands::transfer::export_to_file,
            commands::transfer::export_to_path,
            commands::transfer::import_from_path,
            commands::transfer::import_json,
            commands::security::crypto_status,
            commands::security::master_enable,
            commands::security::master_disable,
            commands::security::master_change,
            commands::security::master_unlock,
            commands::security::master_lock,
            commands::security::user_get,
            commands::security::user_set,
            commands::watch::watch_resync,
            commands::tray::tray_resync,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
