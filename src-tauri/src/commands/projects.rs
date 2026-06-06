use crate::error::{AppError, AppResult};
use crate::models::{Project, ProjectInput};
use crate::state::AppState;
use rusqlite::{params, Connection};
use tauri::State;

fn load_tags(conn: &Connection, project_id: i64) -> AppResult<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT t.name FROM tags t
         JOIN project_tags pt ON pt.tag_id = t.id
         WHERE pt.project_id = ?1
         ORDER BY t.name",
    )?;
    let rows = stmt.query_map([project_id], |r| r.get::<_, String>(0))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

fn set_tags(conn: &Connection, project_id: i64, tags: &[String]) -> AppResult<()> {
    conn.execute("DELETE FROM project_tags WHERE project_id = ?1", [project_id])?;
    for raw in tags {
        let name = raw.trim();
        if name.is_empty() {
            continue;
        }
        conn.execute("INSERT OR IGNORE INTO tags(name) VALUES (?1)", [name])?;
        let tag_id: i64 = conn.query_row("SELECT id FROM tags WHERE name = ?1", [name], |r| r.get(0))?;
        conn.execute(
            "INSERT OR IGNORE INTO project_tags(project_id, tag_id) VALUES (?1, ?2)",
            params![project_id, tag_id],
        )?;
    }
    Ok(())
}

fn row_to_project(conn: &Connection, id: i64) -> AppResult<Project> {
    let mut p = conn.query_row(
        "SELECT id, name, description, status, color, icon, path, repo_path,
                pinned, sort_order, created_at, updated_at
         FROM projects WHERE id = ?1",
        [id],
        |r| {
            Ok(Project {
                id: r.get(0)?,
                name: r.get(1)?,
                description: r.get(2)?,
                status: r.get(3)?,
                color: r.get(4)?,
                icon: r.get(5)?,
                path: r.get(6)?,
                repo_path: r.get(7)?,
                pinned: r.get::<_, i64>(8)? != 0,
                sort_order: r.get(9)?,
                tags: Vec::new(),
                created_at: r.get(10)?,
                updated_at: r.get(11)?,
            })
        },
    )?;
    p.tags = load_tags(conn, id)?;
    Ok(p)
}

fn lock<'a>(state: &'a State<AppState>) -> AppResult<std::sync::MutexGuard<'a, Connection>> {
    state.db.lock().map_err(|_| AppError::internal("db mutex poisoned"))
}

#[tauri::command]
pub fn projects_list(state: State<AppState>) -> AppResult<Vec<Project>> {
    let conn = lock(&state)?;
    let mut stmt = conn.prepare(
        "SELECT id FROM projects
         ORDER BY pinned DESC, sort_order ASC, name COLLATE NOCASE ASC",
    )?;
    let ids = stmt.query_map([], |r| r.get::<_, i64>(0))?;
    let mut out = Vec::new();
    for id in ids {
        out.push(row_to_project(&conn, id?)?);
    }
    Ok(out)
}

#[tauri::command]
pub fn projects_get(state: State<AppState>, id: i64) -> AppResult<Project> {
    let conn = lock(&state)?;
    row_to_project(&conn, id)
}

#[tauri::command]
pub fn projects_create(state: State<AppState>, input: ProjectInput) -> AppResult<Project> {
    let name = input.name.trim();
    if name.is_empty() {
        return Err(AppError { kind: crate::error::ErrorKind::Validation, message: "Имя проекта не может быть пустым".into() });
    }
    let conn = lock(&state)?;
    let next_sort: i64 = conn.query_row("SELECT COALESCE(MAX(sort_order), 0) + 1 FROM projects", [], |r| r.get(0))?;
    conn.execute(
        "INSERT INTO projects(name, description, status, color, icon, path, repo_path, pinned, sort_order)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, ?8)",
        params![
            name,
            input.description,
            input.status.as_deref().unwrap_or("active"),
            input.color,
            input.icon,
            input.path,
            input.repo_path,
            next_sort,
        ],
    )?;
    let id = conn.last_insert_rowid();
    set_tags(&conn, id, &input.tags)?;
    row_to_project(&conn, id)
}

#[tauri::command]
pub fn projects_update(state: State<AppState>, id: i64, input: ProjectInput) -> AppResult<Project> {
    let name = input.name.trim();
    if name.is_empty() {
        return Err(AppError { kind: crate::error::ErrorKind::Validation, message: "Имя проекта не может быть пустым".into() });
    }
    let conn = lock(&state)?;
    let n = conn.execute(
        "UPDATE projects SET name=?2, description=?3, status=?4, color=?5, icon=?6,
                path=?7, repo_path=?8, updated_at=datetime('now')
         WHERE id=?1",
        params![
            id, name, input.description,
            input.status.as_deref().unwrap_or("active"),
            input.color, input.icon, input.path, input.repo_path,
        ],
    )?;
    if n == 0 {
        return Err(AppError { kind: crate::error::ErrorKind::NotFound, message: "Проект не найден".into() });
    }
    set_tags(&conn, id, &input.tags)?;
    row_to_project(&conn, id)
}

#[tauri::command]
pub fn projects_delete(state: State<AppState>, id: i64) -> AppResult<()> {
    let conn = lock(&state)?;
    conn.execute("DELETE FROM projects WHERE id = ?1", [id])?;
    Ok(())
}

#[tauri::command]
pub fn projects_archive(state: State<AppState>, id: i64) -> AppResult<()> {
    let conn = lock(&state)?;
    conn.execute(
        "UPDATE projects SET status='archived', updated_at=datetime('now') WHERE id=?1",
        [id],
    )?;
    Ok(())
}

#[tauri::command]
pub fn project_set_pinned(state: State<AppState>, id: i64, pinned: bool) -> AppResult<()> {
    let conn = lock(&state)?;
    conn.execute(
        "UPDATE projects SET pinned=?2, updated_at=datetime('now') WHERE id=?1",
        params![id, pinned as i64],
    )?;
    Ok(())
}

#[tauri::command]
pub fn project_set_sort(state: State<AppState>, id: i64, sort_order: i64) -> AppResult<()> {
    let conn = lock(&state)?;
    conn.execute(
        "UPDATE projects SET sort_order=?2, updated_at=datetime('now') WHERE id=?1",
        params![id, sort_order],
    )?;
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

    fn insert(conn: &Connection, name: &str, tags: &[&str]) -> i64 {
        conn.execute(
            "INSERT INTO projects(name, status, sort_order) VALUES (?1, 'active', 0)",
            [name],
        ).unwrap();
        let id = conn.last_insert_rowid();
        let owned: Vec<String> = tags.iter().map(|s| s.to_string()).collect();
        set_tags(conn, id, &owned).unwrap();
        id
    }

    #[test]
    fn create_with_tags_and_read_back() {
        let conn = mem();
        let id = insert(&conn, "Aurora", &["rust", "axum"]);
        let p = row_to_project(&conn, id).unwrap();
        assert_eq!(p.name, "Aurora");
        assert_eq!(p.tags, vec!["axum".to_string(), "rust".to_string()]);
        assert_eq!(p.status, "active");
        assert!(!p.pinned);
    }

    #[test]
    fn set_tags_replaces_and_dedups_global() {
        let conn = mem();
        let a = insert(&conn, "A", &["shared", "x"]);
        let b = insert(&conn, "B", &["shared", "y"]);
        let shared_count: i64 = conn
            .query_row("SELECT count(*) FROM tags WHERE name='shared'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(shared_count, 1);
        assert_eq!(row_to_project(&conn, a).unwrap().tags, vec!["shared".to_string(), "x".to_string()]);
        set_tags(&conn, a, &["only".to_string()]).unwrap();
        assert_eq!(row_to_project(&conn, a).unwrap().tags, vec!["only".to_string()]);
        assert_eq!(row_to_project(&conn, b).unwrap().tags, vec!["shared".to_string(), "y".to_string()]);
    }

    #[test]
    fn delete_cascades_project_tags() {
        let conn = mem();
        let id = insert(&conn, "Temp", &["t1", "t2"]);
        conn.execute("DELETE FROM projects WHERE id = ?1", [id]).unwrap();
        let links: i64 = conn
            .query_row("SELECT count(*) FROM project_tags WHERE project_id = ?1", [id], |r| r.get(0))
            .unwrap();
        assert_eq!(links, 0);
    }
}
