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
        "SELECT id, name, description, status, color, icon, path, repo_path, health_url,
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
                health_url: r.get(8)?,
                pinned: r.get::<_, i64>(9)? != 0,
                sort_order: r.get(10)?,
                tags: Vec::new(),
                created_at: r.get(11)?,
                updated_at: r.get(12)?,
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
        "INSERT INTO projects(name, description, status, color, icon, path, repo_path, health_url, pinned, sort_order)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0, ?9)",
        params![
            name,
            input.description,
            input.status.as_deref().unwrap_or("active"),
            input.color,
            input.icon,
            input.path,
            input.repo_path,
            input.health_url,
            next_sort,
        ],
    )?;
    let id = conn.last_insert_rowid();
    set_tags(&conn, id, &input.tags)?;
    crate::commands::columns::seed_default_columns(&conn, id)?;
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
                path=?7, repo_path=?8, health_url=?9, updated_at=datetime('now')
         WHERE id=?1",
        params![
            id, name, input.description,
            input.status.as_deref().unwrap_or("active"),
            input.color, input.icon, input.path, input.repo_path, input.health_url,
        ],
    )?;
    if n == 0 {
        return Err(AppError { kind: crate::error::ErrorKind::NotFound, message: "Проект не найден".into() });
    }
    set_tags(&conn, id, &input.tags)?;
    row_to_project(&conn, id)
}

/// Удалить проект. Глобальные креды этого проекта переселяем в другой проект,
/// чтобы они пережили удаление. Если других проектов НЕТ — не переселяем
/// (project_id остался бы NULL и сломал row_to_cred): такие креды уходят по
/// FK CASCADE вместе с последним проектом.
fn delete_project_in(conn: &Connection, id: i64) -> AppResult<()> {
    conn.execute(
        "UPDATE credentials
         SET project_id = (SELECT MIN(id) FROM projects WHERE id <> ?1)
         WHERE project_id = ?1 AND is_global = 1
           AND EXISTS (SELECT 1 FROM projects WHERE id <> ?1)",
        [id],
    )?;
    conn.execute("DELETE FROM projects WHERE id = ?1", [id])?;
    Ok(())
}

#[tauri::command]
pub fn projects_delete(state: State<AppState>, id: i64) -> AppResult<()> {
    let conn = lock(&state)?;
    let tx = conn.unchecked_transaction()?;
    delete_project_in(&conn, id)?;
    tx.commit()?;
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

/// Скопировать выбранное изображение в каталог данных приложения и вернуть путь к копии.
/// Используется как иконка проекта (показывается через asset-протокол).
#[tauri::command]
pub fn project_import_icon(app: tauri::AppHandle, src_path: String) -> AppResult<String> {
    use std::time::{SystemTime, UNIX_EPOCH};
    use tauri::Manager;

    let src = std::path::Path::new(&src_path);
    let ext = src
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("png")
        .to_lowercase();
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError { kind: crate::error::ErrorKind::Io, message: format!("Каталог данных: {}", e) })?
        .join("icons");
    std::fs::create_dir_all(&dir)
        .map_err(|e| AppError { kind: crate::error::ErrorKind::Io, message: format!("Папка иконок: {}", e) })?;
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    let dest = dir.join(format!("icon-{}.{}", stamp, ext));
    std::fs::copy(src, &dest)
        .map_err(|e| AppError { kind: crate::error::ErrorKind::Io, message: format!("Копирование иконки: {}", e) })?;
    Ok(dest.to_string_lossy().into_owned())
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
    fn delete_project_reassigns_global_creds() {
        let conn = mem();
        conn.execute("INSERT INTO projects(name,status,sort_order) VALUES('P1','active',0)", []).unwrap();
        let p1 = conn.last_insert_rowid();
        conn.execute("INSERT INTO projects(name,status,sort_order) VALUES('P2','active',0)", []).unwrap();
        let p2 = conn.last_insert_rowid();
        conn.execute("INSERT INTO credentials(project_id,label,type,sort_order,is_global) VALUES(?1,'G','note',0,1)", [p1]).unwrap();
        let g = conn.last_insert_rowid();
        conn.execute("INSERT INTO credentials(project_id,label,type,sort_order,is_global) VALUES(?1,'L','note',0,0)", [p1]).unwrap();
        let l = conn.last_insert_rowid();

        delete_project_in(&conn, p1).unwrap();

        let pcount: i64 = conn.query_row("SELECT count(*) FROM projects WHERE id=?1", [p1], |r| r.get(0)).unwrap();
        assert_eq!(pcount, 0, "P1 удалён");
        let g_home: i64 = conn.query_row("SELECT project_id FROM credentials WHERE id=?1", [g], |r| r.get(0)).unwrap();
        assert_eq!(g_home, p2, "глобальный кред переехал в P2");
        let lcount: i64 = conn.query_row("SELECT count(*) FROM credentials WHERE id=?1", [l], |r| r.get(0)).unwrap();
        assert_eq!(lcount, 0, "локальный кред удалён с проектом");
    }

    #[test]
    fn delete_last_project_removes_orphan_globals() {
        let conn = mem();
        conn.execute("INSERT INTO projects(name,status,sort_order) VALUES('P','active',0)", []).unwrap();
        let p = conn.last_insert_rowid();
        conn.execute("INSERT INTO credentials(project_id,label,type,sort_order,is_global) VALUES(?1,'G','note',0,1)", [p]).unwrap();
        let g = conn.last_insert_rowid();

        delete_project_in(&conn, p).unwrap();

        // нет другого проекта → глобальный кред НЕ остаётся сиротой с NULL,
        // а удаляется по каскаду вместе с последним проектом
        let cnt: i64 = conn.query_row("SELECT count(*) FROM credentials WHERE id=?1", [g], |r| r.get(0)).unwrap();
        assert_eq!(cnt, 0);
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
