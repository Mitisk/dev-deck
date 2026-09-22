//! Папки проектов в сайдбаре: один уровень, только имя и порядок.
//! Удаление папки возвращает её проекты в корень (FK ON DELETE SET NULL).

use crate::error::{AppError, AppResult, ErrorKind};
use crate::models::ProjectGroup;
use crate::state::AppState;
use rusqlite::{params, Connection};
use std::sync::MutexGuard;
use tauri::State;

fn lock<'a>(state: &'a State<AppState>) -> AppResult<MutexGuard<'a, Connection>> {
    state.db.lock().map_err(|_| AppError::internal("db mutex poisoned"))
}

fn row_to_group(conn: &Connection, id: i64) -> AppResult<ProjectGroup> {
    Ok(conn.query_row(
        "SELECT id, name, sort_order FROM project_groups WHERE id=?1",
        [id],
        |r| Ok(ProjectGroup { id: r.get(0)?, name: r.get(1)?, sort_order: r.get(2)? }),
    )?)
}

fn clean_name(raw: &str) -> AppResult<&str> {
    let name = raw.trim();
    if name.is_empty() {
        return Err(AppError { kind: ErrorKind::Validation, message: "Имя папки не может быть пустым".into() });
    }
    Ok(name)
}

fn list_groups(conn: &Connection) -> AppResult<Vec<ProjectGroup>> {
    let mut stmt = conn.prepare("SELECT id FROM project_groups ORDER BY sort_order ASC, id ASC")?;
    let ids = stmt.query_map([], |r| r.get::<_, i64>(0))?;
    let mut out = Vec::new();
    for id in ids {
        out.push(row_to_group(conn, id?)?);
    }
    Ok(out)
}

fn create_group(conn: &Connection, name: &str) -> AppResult<i64> {
    let name = clean_name(name)?;
    let next: i64 = conn.query_row("SELECT COALESCE(MAX(sort_order),0)+1 FROM project_groups", [], |r| r.get(0))?;
    conn.execute("INSERT INTO project_groups(name, sort_order) VALUES(?1, ?2)", params![name, next])?;
    Ok(conn.last_insert_rowid())
}

fn rename_group(conn: &Connection, id: i64, name: &str) -> AppResult<()> {
    let name = clean_name(name)?;
    let n = conn.execute("UPDATE project_groups SET name=?2 WHERE id=?1", params![id, name])?;
    if n == 0 {
        return Err(AppError { kind: ErrorKind::NotFound, message: "Папка не найдена".into() });
    }
    Ok(())
}

fn delete_group(conn: &Connection, id: i64) -> AppResult<()> {
    conn.execute("DELETE FROM project_groups WHERE id=?1", [id])?;
    Ok(())
}

fn reorder_groups(conn: &Connection, ids: &[i64]) -> AppResult<()> {
    for (i, id) in ids.iter().enumerate() {
        conn.execute("UPDATE project_groups SET sort_order=?2 WHERE id=?1", params![id, i as i64])?;
    }
    Ok(())
}

#[tauri::command]
pub fn groups_list(state: State<AppState>) -> AppResult<Vec<ProjectGroup>> {
    let conn = lock(&state)?;
    list_groups(&conn)
}

#[tauri::command]
pub fn groups_create(state: State<AppState>, name: String) -> AppResult<ProjectGroup> {
    let conn = lock(&state)?;
    let id = create_group(&conn, &name)?;
    row_to_group(&conn, id)
}

#[tauri::command]
pub fn groups_rename(state: State<AppState>, id: i64, name: String) -> AppResult<ProjectGroup> {
    let conn = lock(&state)?;
    rename_group(&conn, id, &name)?;
    row_to_group(&conn, id)
}

#[tauri::command]
pub fn groups_delete(state: State<AppState>, id: i64) -> AppResult<()> {
    let conn = lock(&state)?;
    delete_group(&conn, id)
}

#[tauri::command]
pub fn groups_reorder(state: State<AppState>, ids: Vec<i64>) -> AppResult<()> {
    let conn = lock(&state)?;
    let tx = conn.unchecked_transaction()?;
    reorder_groups(&conn, &ids)?;
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
    fn create_list_rename_in_order() {
        let conn = mem();
        let a = create_group(&conn, " Alpha ").unwrap();
        let b = create_group(&conn, "Beta").unwrap();
        let list = list_groups(&conn).unwrap();
        assert_eq!(list.iter().map(|g| g.id).collect::<Vec<_>>(), vec![a, b]);
        assert_eq!(list[0].name, "Alpha", "имя обрезается");

        rename_group(&conn, a, "Gamma").unwrap();
        assert_eq!(row_to_group(&conn, a).unwrap().name, "Gamma");

        assert!(matches!(create_group(&conn, "  "), Err(AppError { kind: ErrorKind::Validation, .. })));
        assert!(matches!(rename_group(&conn, 9999, "X"), Err(AppError { kind: ErrorKind::NotFound, .. })));
    }

    #[test]
    fn reorder_sets_positions() {
        let conn = mem();
        let a = create_group(&conn, "A").unwrap();
        let b = create_group(&conn, "B").unwrap();
        reorder_groups(&conn, &[b, a]).unwrap();
        let list = list_groups(&conn).unwrap();
        assert_eq!(list.iter().map(|g| g.id).collect::<Vec<_>>(), vec![b, a]);
    }

    #[test]
    fn delete_group_moves_projects_to_root() {
        let conn = mem();
        let g = create_group(&conn, "F").unwrap();
        conn.execute("INSERT INTO projects(name,status,sort_order,group_id) VALUES('P','active',0,?1)", [g]).unwrap();
        let p = conn.last_insert_rowid();

        delete_group(&conn, g).unwrap();

        let pcount: i64 = conn.query_row("SELECT count(*) FROM projects WHERE id=?1", [p], |r| r.get(0)).unwrap();
        assert_eq!(pcount, 1, "проект остаётся");
        let gid: Option<i64> = conn.query_row("SELECT group_id FROM projects WHERE id=?1", [p], |r| r.get(0)).unwrap();
        assert_eq!(gid, None, "проект ушёл в корень");
    }
}
