use crate::error::AppResult;
use crate::watch;

#[tauri::command]
pub fn watch_resync(app: tauri::AppHandle) -> AppResult<()> {
    watch::resync(&app)
}
