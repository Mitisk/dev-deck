use crate::error::AppResult;

#[tauri::command]
pub fn tray_resync(app: tauri::AppHandle) -> AppResult<()> {
    crate::tray::resync(&app)
}
