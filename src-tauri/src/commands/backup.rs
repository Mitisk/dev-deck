use crate::backup;
use crate::error::{AppError, AppResult, ErrorKind};
use crate::models::BackupInfo;
use crate::state::AppState;
use std::time::UNIX_EPOCH;
use tauri::{Manager, State};

fn backups_dir(app: &tauri::AppHandle) -> AppResult<std::path::PathBuf> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError { kind: ErrorKind::Io, message: format!("Каталог данных: {}", e) })?
        .join("backups");
    Ok(dir)
}

#[tauri::command]
pub fn backup_now(state: State<AppState>, app: tauri::AppHandle) -> AppResult<String> {
    let dir = backups_dir(&app)?;
    let conn = state.db.lock().map_err(|_| AppError::internal("db mutex poisoned"))?;
    let path = backup::make_backup(&conn, &dir)?;
    Ok(path.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn backups_list(app: tauri::AppHandle) -> AppResult<Vec<BackupInfo>> {
    let dir = backups_dir(&app)?;
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for e in rd.flatten() {
            let p = e.path();
            let name = match p.file_name().and_then(|n| n.to_str()) {
                Some(n) if n.starts_with("devdeck-backup-") && n.ends_with(".db") => n.to_string(),
                _ => continue,
            };
            let meta = std::fs::metadata(&p).ok();
            let size_bytes = meta.as_ref().map(|m| m.len()).unwrap_or(0);
            let created_epoch = meta
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            out.push(BackupInfo { name, size_bytes, created_epoch });
        }
    }
    out.sort_by(|a, b| b.created_epoch.cmp(&a.created_epoch));
    Ok(out)
}
