use crate::error::{AppError, AppResult, ErrorKind};
use crate::models::{Checklist, ChecklistItem};
use crate::state::AppState;
use rusqlite::{params, Connection};
use std::sync::MutexGuard;
use tauri::State;

fn lock<'a>(state: &'a State<AppState>) -> AppResult<MutexGuard<'a, Connection>> {
    state.db.lock().map_err(|_| AppError::internal("db mutex poisoned"))
}

fn load_items(conn: &Connection, checklist_id: i64) -> AppResult<Vec<ChecklistItem>> {
    let mut stmt = conn.prepare(
        "SELECT id, checklist_id, text, is_done, sort_order FROM checklist_items
         WHERE checklist_id = ?1 ORDER BY sort_order ASC, id ASC",
    )?;
    let rows = stmt.query_map([checklist_id], |r| {
        Ok(ChecklistItem {
            id: r.get(0)?,
            checklist_id: r.get(1)?,
            text: r.get(2)?,
            is_done: r.get::<_, i64>(3)? != 0,
            sort_order: r.get(4)?,
        })
    })?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

fn list_checklists(conn: &Connection, project_id: i64) -> AppResult<Vec<Checklist>> {
    let mut stmt = conn.prepare(
        "SELECT id, project_id, title, sort_order FROM checklists
         WHERE project_id = ?1 ORDER BY sort_order ASC, id ASC",
    )?;
    let rows = stmt
        .query_map([project_id], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, String>(2)?, r.get::<_, i64>(3)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let mut out = Vec::new();
    for (id, pid, title, sort_order) in rows {
        out.push(Checklist { id, project_id: pid, title, sort_order, items: load_items(conn, id)? });
    }
    Ok(out)
}

fn create_checklist(conn: &Connection, project_id: i64, title: &str) -> AppResult<Checklist> {
    let title = title.trim();
    if title.is_empty() {
        return Err(AppError { kind: ErrorKind::Validation, message: "Название чеклиста пусто".into() });
    }
    let next: i64 = conn.query_row(
        "SELECT COALESCE(MAX(sort_order),0)+1 FROM checklists WHERE project_id=?1",
        [project_id],
        |r| r.get(0),
    )?;
    conn.execute(
        "INSERT INTO checklists(project_id, title, sort_order) VALUES (?1, ?2, ?3)",
        params![project_id, title, next],
    )?;
    let id = conn.last_insert_rowid();
    Ok(Checklist { id, project_id, title: title.to_string(), sort_order: next, items: Vec::new() })
}

fn update_checklist(conn: &Connection, id: i64, title: &str) -> AppResult<()> {
    let title = title.trim();
    if title.is_empty() {
        return Err(AppError { kind: ErrorKind::Validation, message: "Название чеклиста пусто".into() });
    }
    conn.execute("UPDATE checklists SET title=?2 WHERE id=?1", params![id, title])?;
    Ok(())
}

fn delete_checklist(conn: &Connection, id: i64) -> AppResult<()> {
    conn.execute("DELETE FROM checklists WHERE id=?1", [id])?;
    Ok(())
}

fn add_item(conn: &Connection, checklist_id: i64, text: &str) -> AppResult<ChecklistItem> {
    let text = text.trim();
    if text.is_empty() {
        return Err(AppError { kind: ErrorKind::Validation, message: "Пункт пуст".into() });
    }
    let next: i64 = conn.query_row(
        "SELECT COALESCE(MAX(sort_order),0)+1 FROM checklist_items WHERE checklist_id=?1",
        [checklist_id],
        |r| r.get(0),
    )?;
    conn.execute(
        "INSERT INTO checklist_items(checklist_id, text, is_done, sort_order) VALUES (?1, ?2, 0, ?3)",
        params![checklist_id, text, next],
    )?;
    Ok(ChecklistItem { id: conn.last_insert_rowid(), checklist_id, text: text.to_string(), is_done: false, sort_order: next })
}

fn update_item(conn: &Connection, id: i64, text: &str) -> AppResult<()> {
    let text = text.trim();
    if text.is_empty() {
        return Err(AppError { kind: ErrorKind::Validation, message: "Пункт пуст".into() });
    }
    conn.execute("UPDATE checklist_items SET text=?2 WHERE id=?1", params![id, text])?;
    Ok(())
}

fn toggle_item(conn: &Connection, id: i64, is_done: bool) -> AppResult<()> {
    conn.execute("UPDATE checklist_items SET is_done=?2 WHERE id=?1", params![id, is_done as i64])?;
    Ok(())
}

fn delete_item(conn: &Connection, id: i64) -> AppResult<()> {
    conn.execute("DELETE FROM checklist_items WHERE id=?1", [id])?;
    Ok(())
}

#[tauri::command]
pub fn checklists_list(state: State<AppState>, project_id: i64) -> AppResult<Vec<Checklist>> {
    let conn = lock(&state)?;
    list_checklists(&conn, project_id)
}

#[tauri::command]
pub fn checklists_create(state: State<AppState>, project_id: i64, title: String) -> AppResult<Checklist> {
    let conn = lock(&state)?;
    create_checklist(&conn, project_id, &title)
}

#[tauri::command]
pub fn checklists_update(state: State<AppState>, id: i64, title: String) -> AppResult<()> {
    let conn = lock(&state)?;
    update_checklist(&conn, id, &title)
}

#[tauri::command]
pub fn checklists_delete(state: State<AppState>, id: i64) -> AppResult<()> {
    let conn = lock(&state)?;
    delete_checklist(&conn, id)
}

#[tauri::command]
pub fn checklist_items_add(state: State<AppState>, checklist_id: i64, text: String) -> AppResult<ChecklistItem> {
    let conn = lock(&state)?;
    add_item(&conn, checklist_id, &text)
}

#[tauri::command]
pub fn checklist_items_update(state: State<AppState>, id: i64, text: String) -> AppResult<()> {
    let conn = lock(&state)?;
    update_item(&conn, id, &text)
}

#[tauri::command]
pub fn checklist_items_toggle(state: State<AppState>, id: i64, is_done: bool) -> AppResult<()> {
    let conn = lock(&state)?;
    toggle_item(&conn, id, is_done)
}

#[tauri::command]
pub fn checklist_items_delete(state: State<AppState>, id: i64) -> AppResult<()> {
    let conn = lock(&state)?;
    delete_item(&conn, id)
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

    fn project(conn: &Connection) -> i64 {
        conn.execute("INSERT INTO projects(name, status, sort_order) VALUES ('P','active',0)", []).unwrap();
        conn.last_insert_rowid()
    }

    #[test]
    fn checklist_with_items_roundtrip() {
        let conn = mem();
        let pid = project(&conn);
        let cl = create_checklist(&conn, pid, "Релиз").unwrap();
        add_item(&conn, cl.id, "Тег").unwrap();
        let it2 = add_item(&conn, cl.id, "Changelog").unwrap();
        toggle_item(&conn, it2.id, true).unwrap();

        let lists = list_checklists(&conn, pid).unwrap();
        assert_eq!(lists.len(), 1);
        assert_eq!(lists[0].title, "Релиз");
        assert_eq!(lists[0].items.len(), 2);
        let done = lists[0].items.iter().filter(|i| i.is_done).count();
        assert_eq!(done, 1);
    }

    #[test]
    fn delete_checklist_cascades_items() {
        let conn = mem();
        let pid = project(&conn);
        let cl = create_checklist(&conn, pid, "X").unwrap();
        add_item(&conn, cl.id, "a").unwrap();
        delete_checklist(&conn, cl.id).unwrap();
        let cnt: i64 = conn
            .query_row("SELECT count(*) FROM checklist_items WHERE checklist_id=?1", [cl.id], |r| r.get(0))
            .unwrap();
        assert_eq!(cnt, 0);
    }
}
