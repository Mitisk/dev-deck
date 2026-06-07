use crate::error::{AppError, AppResult, ErrorKind};
use crate::models::Link;
use crate::state::AppState;
use rusqlite::{params, Connection};
use std::sync::MutexGuard;
use tauri::State;

fn lock<'a>(state: &'a State<AppState>) -> AppResult<MutexGuard<'a, Connection>> {
    state.db.lock().map_err(|_| AppError::internal("db mutex poisoned"))
}

fn row_to_link(conn: &Connection, id: i64) -> AppResult<Link> {
    Ok(conn.query_row(
        "SELECT id, project_id, label, url, icon, sort_order FROM links WHERE id = ?1",
        [id],
        |r| Ok(Link {
            id: r.get(0)?,
            project_id: r.get(1)?,
            label: r.get(2)?,
            url: r.get(3)?,
            icon: r.get(4)?,
            sort_order: r.get(5)?,
        }),
    )?)
}

#[tauri::command]
pub fn links_list(state: State<AppState>, project_id: i64) -> AppResult<Vec<Link>> {
    let conn = lock(&state)?;
    let mut stmt = conn.prepare(
        "SELECT id FROM links WHERE project_id = ?1 ORDER BY sort_order ASC, id ASC",
    )?;
    let ids = stmt.query_map([project_id], |r| r.get::<_, i64>(0))?;
    let mut out = Vec::new();
    for id in ids {
        out.push(row_to_link(&conn, id?)?);
    }
    Ok(out)
}

#[tauri::command]
pub fn links_create(state: State<AppState>, project_id: i64, label: String, url: String, icon: Option<String>) -> AppResult<Link> {
    if label.trim().is_empty() || url.trim().is_empty() {
        return Err(AppError { kind: ErrorKind::Validation, message: "Название и URL обязательны".into() });
    }
    let conn = lock(&state)?;
    let next: i64 = conn.query_row("SELECT COALESCE(MAX(sort_order),0)+1 FROM links WHERE project_id=?1", [project_id], |r| r.get(0))?;
    conn.execute(
        "INSERT INTO links(project_id, label, url, icon, sort_order) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![project_id, label.trim(), url.trim(), icon, next],
    )?;
    row_to_link(&conn, conn.last_insert_rowid())
}

#[tauri::command]
pub fn links_update(state: State<AppState>, id: i64, label: String, url: String, icon: Option<String>) -> AppResult<Link> {
    if label.trim().is_empty() || url.trim().is_empty() {
        return Err(AppError { kind: ErrorKind::Validation, message: "Название и URL обязательны".into() });
    }
    let conn = lock(&state)?;
    conn.execute(
        "UPDATE links SET label=?2, url=?3, icon=?4 WHERE id=?1",
        params![id, label.trim(), url.trim(), icon],
    )?;
    row_to_link(&conn, id)
}

#[tauri::command]
pub fn links_delete(state: State<AppState>, id: i64) -> AppResult<()> {
    let conn = lock(&state)?;
    conn.execute("DELETE FROM links WHERE id=?1", [id])?;
    Ok(())
}
