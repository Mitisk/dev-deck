use crate::error::{AppError, AppResult, ErrorKind};
use crate::models::FileShortcut;
use crate::state::AppState;
use rusqlite::{params, Connection};
use std::sync::MutexGuard;
use tauri::State;

fn lock<'a>(state: &'a State<AppState>) -> AppResult<MutexGuard<'a, Connection>> {
    state.db.lock().map_err(|_| AppError::internal("db mutex poisoned"))
}

fn row_to_file(conn: &Connection, id: i64) -> AppResult<FileShortcut> {
    Ok(conn.query_row(
        "SELECT id, project_id, label, path, sort_order, show_terminal FROM files WHERE id = ?1",
        [id],
        |r| Ok(FileShortcut {
            id: r.get(0)?,
            project_id: r.get(1)?,
            label: r.get(2)?,
            path: r.get(3)?,
            sort_order: r.get(4)?,
            show_terminal: r.get::<_, i64>(5)? != 0,
        }),
    )?)
}

#[tauri::command]
pub fn files_list(state: State<AppState>, project_id: i64) -> AppResult<Vec<FileShortcut>> {
    let conn = lock(&state)?;
    let mut stmt = conn.prepare(
        "SELECT id FROM files WHERE project_id = ?1 ORDER BY sort_order ASC, id ASC",
    )?;
    let ids = stmt.query_map([project_id], |r| r.get::<_, i64>(0))?;
    let mut out = Vec::new();
    for id in ids {
        out.push(row_to_file(&conn, id?)?);
    }
    Ok(out)
}

#[tauri::command]
pub fn files_create(state: State<AppState>, project_id: i64, label: String, path: String) -> AppResult<FileShortcut> {
    if label.trim().is_empty() || path.trim().is_empty() {
        return Err(AppError { kind: ErrorKind::Validation, message: "Название и путь обязательны".into() });
    }
    let conn = lock(&state)?;
    let next: i64 = conn.query_row("SELECT COALESCE(MAX(sort_order),0)+1 FROM files WHERE project_id=?1", [project_id], |r| r.get(0))?;
    conn.execute(
        "INSERT INTO files(project_id, label, path, sort_order) VALUES (?1, ?2, ?3, ?4)",
        params![project_id, label.trim(), path.trim(), next],
    )?;
    row_to_file(&conn, conn.last_insert_rowid())
}

#[tauri::command]
pub fn files_update(state: State<AppState>, id: i64, label: String, path: String) -> AppResult<FileShortcut> {
    if label.trim().is_empty() || path.trim().is_empty() {
        return Err(AppError { kind: ErrorKind::Validation, message: "Название и путь обязательны".into() });
    }
    let conn = lock(&state)?;
    conn.execute("UPDATE files SET label=?2, path=?3 WHERE id=?1", params![id, label.trim(), path.trim()])?;
    row_to_file(&conn, id)
}

#[tauri::command]
pub fn files_delete(state: State<AppState>, id: i64) -> AppResult<()> {
    let conn = lock(&state)?;
    conn.execute("DELETE FROM files WHERE id=?1", [id])?;
    Ok(())
}

#[tauri::command]
pub fn files_set_terminal(state: State<AppState>, id: i64, show_terminal: bool) -> AppResult<FileShortcut> {
    let conn = lock(&state)?;
    conn.execute(
        "UPDATE files SET show_terminal=?2 WHERE id=?1",
        params![id, if show_terminal { 1i64 } else { 0 }],
    )?;
    row_to_file(&conn, id)
}
