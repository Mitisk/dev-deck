use crate::error::{AppError, AppResult, ErrorKind};
use crate::models::Label;
use crate::state::AppState;
use rusqlite::{params, Connection};
use std::sync::MutexGuard;
use tauri::State;

fn lock<'a>(state: &'a State<AppState>) -> AppResult<MutexGuard<'a, Connection>> {
    state.db.lock().map_err(|_| AppError::internal("db mutex poisoned"))
}

fn row_to_label(conn: &Connection, id: i64) -> AppResult<Label> {
    Ok(conn.query_row(
        "SELECT id, project_id, name, color, sort_order FROM labels WHERE id=?1",
        [id],
        |r| Ok(Label { id: r.get(0)?, project_id: r.get(1)?, name: r.get(2)?, color: r.get(3)?, sort_order: r.get(4)? }),
    )?)
}

#[tauri::command]
pub fn labels_list(state: State<AppState>, project_id: i64) -> AppResult<Vec<Label>> {
    let conn = lock(&state)?;
    let mut stmt = conn.prepare("SELECT id FROM labels WHERE project_id=?1 ORDER BY sort_order, id")?;
    let ids = stmt.query_map([project_id], |r| r.get::<_, i64>(0))?;
    let mut out = Vec::new();
    for id in ids { out.push(row_to_label(&conn, id?)?); }
    Ok(out)
}

#[tauri::command]
pub fn label_create(state: State<AppState>, project_id: i64, name: String, color: Option<String>) -> AppResult<Label> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError { kind: ErrorKind::Validation, message: "Название метки пусто".into() });
    }
    let conn = lock(&state)?;
    let next: i64 = conn.query_row("SELECT COALESCE(MAX(sort_order),0)+1 FROM labels WHERE project_id=?1", [project_id], |r| r.get(0))?;
    conn.execute("INSERT INTO labels(project_id,name,color,sort_order) VALUES(?1,?2,?3,?4)", params![project_id, name, color, next])?;
    row_to_label(&conn, conn.last_insert_rowid())
}

#[tauri::command]
pub fn label_update(state: State<AppState>, id: i64, name: String, color: Option<String>) -> AppResult<Label> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError { kind: ErrorKind::Validation, message: "Название метки пусто".into() });
    }
    let conn = lock(&state)?;
    conn.execute("UPDATE labels SET name=?2, color=?3 WHERE id=?1", params![id, name, color])?;
    row_to_label(&conn, id)
}

#[tauri::command]
pub fn label_delete(state: State<AppState>, id: i64) -> AppResult<()> {
    let conn = lock(&state)?;
    conn.execute("DELETE FROM labels WHERE id=?1", [id])?;
    Ok(())
}

/// Заменить набор меток задачи.
#[tauri::command]
pub fn task_set_labels(state: State<AppState>, task_id: i64, label_ids: Vec<i64>) -> AppResult<()> {
    let conn = lock(&state)?;
    conn.execute("DELETE FROM task_labels WHERE task_id=?1", [task_id])?;
    for lid in label_ids {
        conn.execute("INSERT OR IGNORE INTO task_labels(task_id,label_id) VALUES(?1,?2)", params![task_id, lid])?;
    }
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
    fn labels_crud_and_task_assignment() {
        let conn = mem();
        conn.execute("INSERT INTO projects(name,status,sort_order) VALUES('P','active',0)", []).unwrap();
        let pid = conn.last_insert_rowid();
        conn.execute("INSERT INTO labels(project_id,name,color,sort_order) VALUES(?1,'bug','#f00',0)", [pid]).unwrap();
        let l1 = conn.last_insert_rowid();
        conn.execute("INSERT INTO labels(project_id,name,color,sort_order) VALUES(?1,'urgent','#0f0',1)", [pid]).unwrap();
        let l2 = conn.last_insert_rowid();
        conn.execute("INSERT INTO tasks(project_id,title,status,sort_order) VALUES(?1,'T','todo',0)", [pid]).unwrap();
        let tid = conn.last_insert_rowid();

        // назначить две метки
        conn.execute("INSERT INTO task_labels(task_id,label_id) VALUES(?1,?2)", params![tid, l1]).unwrap();
        conn.execute("INSERT INTO task_labels(task_id,label_id) VALUES(?1,?2)", params![tid, l2]).unwrap();
        let cnt: i64 = conn.query_row("SELECT count(*) FROM task_labels WHERE task_id=?1", [tid], |r| r.get(0)).unwrap();
        assert_eq!(cnt, 2);

        // удаление метки каскадит связь
        conn.execute("DELETE FROM labels WHERE id=?1", [l1]).unwrap();
        let cnt2: i64 = conn.query_row("SELECT count(*) FROM task_labels WHERE task_id=?1", [tid], |r| r.get(0)).unwrap();
        assert_eq!(cnt2, 1);
    }
}
