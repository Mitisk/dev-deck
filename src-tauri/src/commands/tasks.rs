use crate::error::{AppError, AppResult, ErrorKind};
use crate::models::{Task, TaskInput};
use crate::state::AppState;
use rusqlite::{params, Connection};
use std::sync::MutexGuard;
use tauri::State;

fn lock<'a>(state: &'a State<AppState>) -> AppResult<MutexGuard<'a, Connection>> {
    state.db.lock().map_err(|_| AppError::internal("db mutex poisoned"))
}

fn row_to_task(conn: &Connection, id: i64) -> AppResult<Task> {
    Ok(conn.query_row(
        "SELECT id, project_id, title, description, status, priority, due_date, sort_order, created_at, completed_at
         FROM tasks WHERE id = ?1",
        [id],
        |r| {
            Ok(Task {
                id: r.get(0)?,
                project_id: r.get(1)?,
                title: r.get(2)?,
                description: r.get(3)?,
                status: r.get(4)?,
                priority: r.get(5)?,
                due_date: r.get(6)?,
                sort_order: r.get(7)?,
                created_at: r.get(8)?,
                completed_at: r.get(9)?,
            })
        },
    )?)
}

fn list_tasks(conn: &Connection, project_id: i64) -> AppResult<Vec<Task>> {
    let mut stmt = conn.prepare(
        "SELECT id FROM tasks WHERE project_id = ?1 ORDER BY sort_order ASC, id ASC",
    )?;
    let ids = stmt.query_map([project_id], |r| r.get::<_, i64>(0))?;
    let mut out = Vec::new();
    for id in ids {
        out.push(row_to_task(conn, id?)?);
    }
    Ok(out)
}

fn create_task(conn: &Connection, project_id: i64, input: TaskInput) -> AppResult<Task> {
    let title = input.title.trim();
    if title.is_empty() {
        return Err(AppError { kind: ErrorKind::Validation, message: "Заголовок задачи пуст".into() });
    }
    let status = input.status.as_deref().unwrap_or("todo");
    let priority = input.priority.unwrap_or(0).clamp(0, 2);
    let next_sort: i64 = conn.query_row(
        "SELECT COALESCE(MAX(sort_order), 0) + 1 FROM tasks WHERE project_id = ?1 AND status = ?2",
        params![project_id, status],
        |r| r.get(0),
    )?;
    conn.execute(
        "INSERT INTO tasks(project_id, title, description, status, priority, due_date, sort_order, completed_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, CASE WHEN ?4='done' THEN datetime('now') ELSE NULL END)",
        params![project_id, title, input.description, status, priority, input.due_date, next_sort],
    )?;
    row_to_task(conn, conn.last_insert_rowid())
}

fn update_task(conn: &Connection, id: i64, input: TaskInput) -> AppResult<Task> {
    let title = input.title.trim();
    if title.is_empty() {
        return Err(AppError { kind: ErrorKind::Validation, message: "Заголовок задачи пуст".into() });
    }
    let status = input.status.as_deref().unwrap_or("todo");
    let priority = input.priority.unwrap_or(0).clamp(0, 2);
    let n = conn.execute(
        "UPDATE tasks SET title=?2, description=?3, status=?4, priority=?5, due_date=?6,
                completed_at = CASE WHEN ?4='done' THEN COALESCE(completed_at, datetime('now')) ELSE NULL END
         WHERE id=?1",
        params![id, title, input.description, status, priority, input.due_date],
    )?;
    if n == 0 {
        return Err(AppError { kind: ErrorKind::NotFound, message: "Задача не найдена".into() });
    }
    row_to_task(conn, id)
}

fn move_task(conn: &Connection, id: i64, status: &str, sort_order: i64) -> AppResult<()> {
    conn.execute(
        "UPDATE tasks SET status=?2, sort_order=?3,
                completed_at = CASE WHEN ?2='done' THEN COALESCE(completed_at, datetime('now')) ELSE NULL END
         WHERE id=?1",
        params![id, status, sort_order],
    )?;
    Ok(())
}

fn delete_task(conn: &Connection, id: i64) -> AppResult<()> {
    conn.execute("DELETE FROM tasks WHERE id = ?1", [id])?;
    Ok(())
}

#[tauri::command]
pub fn tasks_list(state: State<AppState>, project_id: i64) -> AppResult<Vec<Task>> {
    let conn = lock(&state)?;
    list_tasks(&conn, project_id)
}

#[tauri::command]
pub fn tasks_create(state: State<AppState>, project_id: i64, input: TaskInput) -> AppResult<Task> {
    let conn = lock(&state)?;
    create_task(&conn, project_id, input)
}

#[tauri::command]
pub fn tasks_update(state: State<AppState>, id: i64, input: TaskInput) -> AppResult<Task> {
    let conn = lock(&state)?;
    update_task(&conn, id, input)
}

#[tauri::command]
pub fn tasks_move(state: State<AppState>, id: i64, status: String, sort_order: i64) -> AppResult<()> {
    let conn = lock(&state)?;
    move_task(&conn, id, &status, sort_order)
}

#[tauri::command]
pub fn tasks_delete(state: State<AppState>, id: i64) -> AppResult<()> {
    let conn = lock(&state)?;
    delete_task(&conn, id)
}

fn agenda(conn: &Connection, today: &str) -> AppResult<Vec<crate::models::AgendaItem>> {
    let mut stmt = conn.prepare(
        "SELECT t.project_id, p.name, p.color, t.id, t.title, t.due_date, t.priority
         FROM tasks t JOIN projects p ON p.id = t.project_id
         WHERE t.status != 'done'
           AND t.due_date IS NOT NULL AND t.due_date != '' AND t.due_date <= ?1
           AND p.status != 'archived'
         ORDER BY t.due_date ASC, t.priority DESC
         LIMIT 50",
    )?;
    let rows = stmt.query_map([today], |r| {
        Ok(crate::models::AgendaItem {
            project_id: r.get(0)?,
            project_name: r.get(1)?,
            project_color: r.get(2)?,
            task_id: r.get(3)?,
            title: r.get(4)?,
            due_date: r.get(5)?,
            priority: r.get(6)?,
        })
    })?;
    let mut out = Vec::new();
    for r in rows { out.push(r?); }
    Ok(out)
}

#[tauri::command]
pub fn tasks_agenda(state: State<AppState>, today: String) -> AppResult<Vec<crate::models::AgendaItem>> {
    let conn = lock(&state)?;
    agenda(&conn, &today)
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
        conn.execute("INSERT INTO projects(name, status, sort_order) VALUES ('P', 'active', 0)", []).unwrap();
        conn.last_insert_rowid()
    }

    fn input(title: &str, status: &str) -> TaskInput {
        TaskInput { title: title.into(), description: None, status: Some(status.into()), priority: Some(2), due_date: None }
    }

    #[test]
    fn create_list_move_delete() {
        let conn = mem();
        let pid = project(&conn);
        let a = create_task(&conn, pid, input("A", "todo")).unwrap();
        create_task(&conn, pid, input("B", "doing")).unwrap();
        assert_eq!(list_tasks(&conn, pid).unwrap().len(), 2);
        assert_eq!(a.priority, 2);
        assert!(a.completed_at.is_none());

        move_task(&conn, a.id, "done", 7).unwrap();
        let done = list_tasks(&conn, pid).unwrap().into_iter().find(|t| t.id == a.id).unwrap();
        assert_eq!(done.status, "done");
        assert_eq!(done.sort_order, 7);
        assert!(done.completed_at.is_some());

        delete_task(&conn, a.id).unwrap();
        assert_eq!(list_tasks(&conn, pid).unwrap().len(), 1);
    }

    #[test]
    fn deleted_with_project_cascade() {
        let conn = mem();
        let pid = project(&conn);
        create_task(&conn, pid, input("X", "todo")).unwrap();
        conn.execute("DELETE FROM projects WHERE id = ?1", [pid]).unwrap();
        assert_eq!(list_tasks(&conn, pid).unwrap().len(), 0);
    }

    #[test]
    fn agenda_returns_overdue_and_today_not_done() {
        let conn = mem();
        let pid = project(&conn);
        // просрочена, сегодня, в будущем, выполнена-просрочена
        conn.execute("INSERT INTO tasks(project_id,title,status,due_date,sort_order) VALUES(?1,'overdue','todo','2026-06-01',0)", [pid]).unwrap();
        conn.execute("INSERT INTO tasks(project_id,title,status,due_date,sort_order) VALUES(?1,'today','doing','2026-06-07',1)", [pid]).unwrap();
        conn.execute("INSERT INTO tasks(project_id,title,status,due_date,sort_order) VALUES(?1,'future','todo','2026-12-31',2)", [pid]).unwrap();
        conn.execute("INSERT INTO tasks(project_id,title,status,due_date,sort_order) VALUES(?1,'done','done','2026-06-01',3)", [pid]).unwrap();
        conn.execute("INSERT INTO tasks(project_id,title,status,due_date,sort_order) VALUES(?1,'nodate','todo','',4)", [pid]).unwrap();

        let items = agenda(&conn, "2026-06-07").unwrap();
        let titles: Vec<&str> = items.iter().map(|i| i.title.as_str()).collect();
        assert_eq!(titles, vec!["overdue", "today"]); // future/done/nodate исключены, сортировка по дате
    }
}
