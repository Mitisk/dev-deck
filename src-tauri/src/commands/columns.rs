use crate::error::{AppError, AppResult, ErrorKind};
use crate::models::TaskColumn;
use crate::state::AppState;
use rusqlite::{params, Connection, OptionalExtension};
use std::sync::MutexGuard;
use tauri::State;

fn lock<'a>(state: &'a State<AppState>) -> AppResult<MutexGuard<'a, Connection>> {
    state.db.lock().map_err(|_| AppError::internal("db mutex poisoned"))
}

/// Засеять дефолтные колонки (To Do/In Progress/Done) для проекта. Вызывается при
/// создании/импорте проекта. Идемпотентно по UNIQUE(project_id,key).
pub fn seed_default_columns(conn: &Connection, project_id: i64) -> AppResult<()> {
    let defaults = [("todo", "To Do", 0i64, 0i64), ("doing", "In Progress", 0, 1), ("done", "Done", 1, 2)];
    for (key, name, is_done, sort) in defaults {
        conn.execute(
            "INSERT OR IGNORE INTO task_columns(project_id,key,name,is_done,sort_order) VALUES(?1,?2,?3,?4,?5)",
            params![project_id, key, name, is_done, sort],
        )?;
    }
    Ok(())
}

/// Является ли колонка (по ключу) «выполненной».
pub fn is_done_column(conn: &Connection, project_id: i64, key: &str) -> bool {
    conn.query_row(
        "SELECT is_done FROM task_columns WHERE project_id=?1 AND key=?2",
        params![project_id, key],
        |r| r.get::<_, i64>(0),
    )
    .optional()
    .ok()
    .flatten()
    .map(|v| v != 0)
    .unwrap_or(false)
}

/// Ключ первой колонки проекта (для дефолтного статуса новой задачи).
pub fn first_column_key(conn: &Connection, project_id: i64) -> AppResult<String> {
    Ok(conn
        .query_row(
            "SELECT key FROM task_columns WHERE project_id=?1 ORDER BY sort_order, id LIMIT 1",
            [project_id],
            |r| r.get::<_, String>(0),
        )
        .optional()?
        .unwrap_or_else(|| "todo".to_string()))
}

fn row_to_column(conn: &Connection, id: i64) -> AppResult<TaskColumn> {
    Ok(conn.query_row(
        "SELECT id, project_id, key, name, is_done, sort_order FROM task_columns WHERE id=?1",
        [id],
        |r| Ok(TaskColumn {
            id: r.get(0)?,
            project_id: r.get(1)?,
            key: r.get(2)?,
            name: r.get(3)?,
            is_done: r.get::<_, i64>(4)? != 0,
            sort_order: r.get(5)?,
        }),
    )?)
}

#[tauri::command]
pub fn columns_list(state: State<AppState>, project_id: i64) -> AppResult<Vec<TaskColumn>> {
    let conn = lock(&state)?;
    let mut stmt = conn.prepare("SELECT id FROM task_columns WHERE project_id=?1 ORDER BY sort_order, id")?;
    let ids = stmt.query_map([project_id], |r| r.get::<_, i64>(0))?;
    let mut out = Vec::new();
    for id in ids { out.push(row_to_column(&conn, id?)?); }
    Ok(out)
}

#[tauri::command]
pub fn column_create(state: State<AppState>, project_id: i64, name: String, is_done: bool) -> AppResult<TaskColumn> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError { kind: ErrorKind::Validation, message: "Название колонки пусто".into() });
    }
    let conn = lock(&state)?;
    let next: i64 = conn.query_row("SELECT COALESCE(MAX(sort_order),0)+1 FROM task_columns WHERE project_id=?1", [project_id], |r| r.get(0))?;
    // временный ключ, затем фиксируем по id
    conn.execute("INSERT INTO task_columns(project_id,key,name,is_done,sort_order) VALUES(?1,'',?2,?3,?4)", params![project_id, name, is_done as i64, next])?;
    let id = conn.last_insert_rowid();
    let key = format!("c{}", id);
    conn.execute("UPDATE task_columns SET key=?2 WHERE id=?1", params![id, key])?;
    row_to_column(&conn, id)
}

#[tauri::command]
pub fn column_update(state: State<AppState>, id: i64, name: String, is_done: bool) -> AppResult<TaskColumn> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError { kind: ErrorKind::Validation, message: "Название колонки пусто".into() });
    }
    let conn = lock(&state)?;
    conn.execute("UPDATE task_columns SET name=?2, is_done=?3 WHERE id=?1", params![id, name, is_done as i64])?;
    // если стала done — проставить completed_at задачам в ней; иначе снять
    let (pid, key): (i64, String) = conn.query_row("SELECT project_id, key FROM task_columns WHERE id=?1", [id], |r| Ok((r.get(0)?, r.get(1)?)))?;
    if is_done {
        conn.execute("UPDATE tasks SET completed_at=COALESCE(completed_at, datetime('now')) WHERE project_id=?1 AND status=?2", params![pid, key])?;
    } else {
        conn.execute("UPDATE tasks SET completed_at=NULL WHERE project_id=?1 AND status=?2", params![pid, key])?;
    }
    row_to_column(&conn, id)
}

#[tauri::command]
pub fn column_delete(state: State<AppState>, id: i64) -> AppResult<()> {
    let conn = lock(&state)?;
    let (pid, key): (i64, String) = conn.query_row("SELECT project_id, key FROM task_columns WHERE id=?1", [id], |r| Ok((r.get(0)?, r.get(1)?)))?;
    let count: i64 = conn.query_row("SELECT count(*) FROM task_columns WHERE project_id=?1", [pid], |r| r.get(0))?;
    if count <= 1 {
        return Err(AppError { kind: ErrorKind::Validation, message: "Нельзя удалить последнюю колонку".into() });
    }
    // перенести задачи в первую оставшуюся колонку
    let target: String = conn.query_row(
        "SELECT key FROM task_columns WHERE project_id=?1 AND id!=?2 ORDER BY sort_order, id LIMIT 1",
        params![pid, id],
        |r| r.get(0),
    )?;
    conn.execute("UPDATE tasks SET status=?3 WHERE project_id=?1 AND status=?2", params![pid, key, target])?;
    conn.execute("DELETE FROM task_columns WHERE id=?1", [id])?;
    Ok(())
}

/// Переставить колонки: выставить sort_order по порядку переданных id.
#[tauri::command]
pub fn columns_reorder(state: State<AppState>, project_id: i64, ids: Vec<i64>) -> AppResult<()> {
    let conn = lock(&state)?;
    let tx = conn.unchecked_transaction()?;
    for (i, id) in ids.iter().enumerate() {
        conn.execute(
            "UPDATE task_columns SET sort_order=?2 WHERE id=?1 AND project_id=?3",
            params![id, i as i64, project_id],
        )?;
    }
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    fn mem() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        db::migrations::run(&conn).unwrap();
        conn
    }

    #[test]
    fn reorder_columns_sets_sort_order() {
        let conn = mem();
        conn.execute("INSERT INTO projects(name,status,sort_order) VALUES('P','active',0)", []).unwrap();
        let pid = conn.last_insert_rowid();
        seed_default_columns(&conn, pid).unwrap();
        let ids: Vec<i64> = {
            let mut s = conn.prepare("SELECT id FROM task_columns WHERE project_id=?1 ORDER BY sort_order").unwrap();
            let r = s.query_map([pid], |r| r.get::<_, i64>(0)).unwrap();
            r.map(|x| x.unwrap()).collect()
        };
        let rev: Vec<i64> = ids.iter().rev().cloned().collect();
        let tx = conn.unchecked_transaction().unwrap();
        for (i, id) in rev.iter().enumerate() {
            conn.execute("UPDATE task_columns SET sort_order=?2 WHERE id=?1 AND project_id=?3", params![id, i as i64, pid]).unwrap();
        }
        tx.commit().unwrap();
        let first: i64 = conn.query_row("SELECT id FROM task_columns WHERE project_id=?1 ORDER BY sort_order LIMIT 1", [pid], |r| r.get(0)).unwrap();
        assert_eq!(first, rev[0]);
    }
}
