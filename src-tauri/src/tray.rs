use crate::error::{AppError, AppResult};
use crate::state::AppState;
use std::sync::Mutex;
use tauri::menu::{IsMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIcon;
use tauri::{AppHandle, Manager, Wry};

/// Хэндл трея для пересборки меню на лету.
#[derive(Default)]
pub struct TrayState {
    pub icon: Mutex<Option<TrayIcon<Wry>>>,
}

/// Закреплённые (не архивные) проекты для меню трея.
fn pinned_projects(app: &AppHandle) -> AppResult<Vec<(i64, String)>> {
    let st = app.state::<AppState>();
    let conn = st
        .db
        .lock()
        .map_err(|_| AppError::internal("db mutex poisoned"))?;
    let mut stmt = conn.prepare(
        "SELECT id, name FROM projects WHERE pinned=1 AND status != 'archived' \
         ORDER BY sort_order ASC, name COLLATE NOCASE ASC LIMIT 8",
    )?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
    let mut v = Vec::new();
    for row in rows {
        v.push(row?);
    }
    Ok(v)
}

/// Собрать меню трея: Открыть / [sep / закреплённые] / Выход.
pub fn build_menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let open_i = MenuItem::with_id(app, "open", "Открыть DevDeck", true, None::<&str>)?;
    let quit_i = MenuItem::with_id(app, "quit", "Выход", true, None::<&str>)?;
    let pinned = pinned_projects(app).unwrap_or_default();
    let proj_items: Vec<MenuItem<Wry>> = pinned
        .iter()
        .map(|(id, name)| MenuItem::with_id(app, format!("proj:{id}"), name, true, None::<&str>))
        .collect::<tauri::Result<_>>()?;
    let sep1 = PredefinedMenuItem::separator(app)?;
    let sep2 = PredefinedMenuItem::separator(app)?;

    let mut items: Vec<&dyn IsMenuItem<Wry>> = vec![&open_i];
    if !proj_items.is_empty() {
        items.push(&sep1);
        for it in &proj_items {
            items.push(it);
        }
        items.push(&sep2);
    }
    items.push(&quit_i);
    Menu::with_items(app, &items)
}

/// Пересобрать меню трея под актуальный набор закреплённых проектов.
pub fn resync(app: &AppHandle) -> AppResult<()> {
    let menu = build_menu(app).map_err(|e| AppError::internal(format!("tray menu: {e}")))?;
    let st = app.state::<TrayState>();
    let guard = st
        .icon
        .lock()
        .map_err(|_| AppError::internal("tray mutex poisoned"))?;
    if let Some(icon) = guard.as_ref() {
        icon.set_menu(Some(menu))
            .map_err(|e| AppError::internal(format!("set_menu: {e}")))?;
    }
    Ok(())
}
