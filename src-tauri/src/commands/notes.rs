use crate::error::{AppError, AppResult};
use crate::models::Note;
use crate::state::AppState;
use rusqlite::{params, Connection};
use std::sync::MutexGuard;
use tauri::State;

fn lock<'a>(state: &'a State<AppState>) -> AppResult<MutexGuard<'a, Connection>> {
    state.db.lock().map_err(|_| AppError::internal("db mutex poisoned"))
}

fn row_to_note(conn: &Connection, id: i64) -> AppResult<Note> {
    Ok(conn.query_row(
        "SELECT id, project_id, title, content_md, updated_at FROM notes WHERE id = ?1",
        [id],
        |r| {
            Ok(Note {
                id: r.get(0)?,
                project_id: r.get(1)?,
                title: r.get(2)?,
                content_md: r.get(3)?,
                updated_at: r.get(4)?,
            })
        },
    )?)
}

fn list_notes(conn: &Connection, project_id: i64) -> AppResult<Vec<Note>> {
    let mut stmt = conn.prepare(
        "SELECT id FROM notes WHERE project_id = ?1 ORDER BY updated_at DESC, id DESC",
    )?;
    let ids = stmt.query_map([project_id], |r| r.get::<_, i64>(0))?;
    let mut out = Vec::new();
    for id in ids {
        out.push(row_to_note(conn, id?)?);
    }
    Ok(out)
}

fn create_note(conn: &Connection, project_id: i64) -> AppResult<Note> {
    conn.execute(
        "INSERT INTO notes(project_id, title, content_md) VALUES (?1, 'Без названия', '')",
        [project_id],
    )?;
    row_to_note(conn, conn.last_insert_rowid())
}

fn update_note(conn: &Connection, id: i64, title: &str, content_md: &str) -> AppResult<()> {
    conn.execute(
        "UPDATE notes SET title=?2, content_md=?3, updated_at=datetime('now') WHERE id=?1",
        params![id, title, content_md],
    )?;
    Ok(())
}

fn delete_note(conn: &Connection, id: i64) -> AppResult<()> {
    conn.execute("DELETE FROM notes WHERE id=?1", [id])?;
    Ok(())
}

#[tauri::command]
pub fn notes_list(state: State<AppState>, project_id: i64) -> AppResult<Vec<Note>> {
    let conn = lock(&state)?;
    list_notes(&conn, project_id)
}

#[tauri::command]
pub fn notes_create(state: State<AppState>, project_id: i64) -> AppResult<Note> {
    let conn = lock(&state)?;
    create_note(&conn, project_id)
}

#[tauri::command]
pub fn notes_update(state: State<AppState>, id: i64, title: String, content_md: String) -> AppResult<()> {
    let conn = lock(&state)?;
    update_note(&conn, id, &title, &content_md)
}

#[tauri::command]
pub fn notes_delete(state: State<AppState>, id: i64) -> AppResult<()> {
    let conn = lock(&state)?;
    delete_note(&conn, id)
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
    fn create_update_list_delete() {
        let conn = mem();
        conn.execute("INSERT INTO projects(name,status,sort_order) VALUES('P','active',0)", []).unwrap();
        let pid = conn.last_insert_rowid();

        let n = create_note(&conn, pid).unwrap();
        assert_eq!(n.title.as_deref(), Some("Без названия"));

        update_note(&conn, n.id, "Архитектура", "# Заголовок\n\nтекст").unwrap();
        let list = list_notes(&conn, pid).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].title.as_deref(), Some("Архитектура"));
        assert_eq!(list[0].content_md.as_deref(), Some("# Заголовок\n\nтекст"));

        delete_note(&conn, n.id).unwrap();
        assert_eq!(list_notes(&conn, pid).unwrap().len(), 0);
    }
}
