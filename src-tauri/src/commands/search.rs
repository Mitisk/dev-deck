use crate::error::{AppError, AppResult};
use crate::models::SearchHit;
use crate::state::AppState;
use rusqlite::Connection;
use std::sync::MutexGuard;
use tauri::State;

fn lock<'a>(state: &'a State<AppState>) -> AppResult<MutexGuard<'a, Connection>> {
    state.db.lock().map_err(|_| AppError::internal("db mutex poisoned"))
}

/// Выполнить один поисковый запрос и добавить хиты. `sql` должен выбирать
/// (project_id, project_name, id, title) и принимать один параметр-паттерн.
fn collect(
    conn: &Connection,
    sql: &str,
    pat: &str,
    kind: &str,
    subtitle: &str,
    out: &mut Vec<SearchHit>,
) -> AppResult<()> {
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([pat], |r| {
        Ok(SearchHit {
            kind: kind.to_string(),
            project_id: r.get(0)?,
            project_name: r.get(1)?,
            id: r.get(2)?,
            title: r.get(3)?,
            subtitle: subtitle.to_string(),
        })
    })?;
    for r in rows {
        out.push(r?);
    }
    Ok(())
}

fn search(conn: &Connection, query: &str) -> AppResult<Vec<SearchHit>> {
    let q = query.trim();
    if q.is_empty() {
        return Ok(Vec::new());
    }
    let pat = format!("%{}%", q);
    let mut hits = Vec::new();

    collect(conn,
        "SELECT id, name, id, name FROM projects WHERE name LIKE ?1 ORDER BY name LIMIT 15",
        &pat, "project", "Проект", &mut hits)?;
    collect(conn,
        "SELECT t.project_id, p.name, t.id, t.title FROM tasks t JOIN projects p ON p.id=t.project_id
         WHERE t.title LIKE ?1 OR IFNULL(t.description,'') LIKE ?1 LIMIT 15",
        &pat, "task", "Задача", &mut hits)?;
    collect(conn,
        "SELECT n.project_id, p.name, n.id, IFNULL(NULLIF(n.title,''),'Заметка') FROM notes n JOIN projects p ON p.id=n.project_id
         WHERE IFNULL(n.title,'') LIKE ?1 OR IFNULL(n.content_md,'') LIKE ?1 LIMIT 15",
        &pat, "note", "Заметка", &mut hits)?;
    collect(conn,
        "SELECT l.project_id, p.name, l.id, l.label FROM links l JOIN projects p ON p.id=l.project_id
         WHERE l.label LIKE ?1 OR l.url LIKE ?1 LIMIT 15",
        &pat, "link", "Ссылка", &mut hits)?;
    collect(conn,
        "SELECT c.project_id, p.name, c.id, c.label FROM credentials c JOIN projects p ON p.id=c.project_id
         WHERE c.label LIKE ?1 OR IFNULL(c.username,'') LIKE ?1 LIMIT 15",
        &pat, "cred", "Кред", &mut hits)?;
    collect(conn,
        "SELECT cm.project_id, p.name, cm.id, cm.label FROM commands cm JOIN projects p ON p.id=cm.project_id
         WHERE cm.label LIKE ?1 OR cm.command LIKE ?1 LIMIT 15",
        &pat, "command", "Команда", &mut hits)?;
    collect(conn,
        "SELECT f.project_id, p.name, f.id, f.label FROM files f JOIN projects p ON p.id=f.project_id
         WHERE f.label LIKE ?1 OR f.path LIKE ?1 LIMIT 15",
        &pat, "file", "Файл", &mut hits)?;

    hits.truncate(50);
    Ok(hits)
}

#[tauri::command]
pub fn search_global(state: State<AppState>, query: String) -> AppResult<Vec<SearchHit>> {
    let conn = lock(&state)?;
    search(&conn, &query)
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
    fn finds_across_tables() {
        let conn = mem();
        conn.execute("INSERT INTO projects(name,status,sort_order) VALUES('Aurora','active',0)", []).unwrap();
        let pid = conn.last_insert_rowid();
        conn.execute("INSERT INTO tasks(project_id,title,status,sort_order) VALUES(?1,'Настроить aurora retries','todo',0)", [pid]).unwrap();
        conn.execute("INSERT INTO notes(project_id,title,content_md) VALUES(?1,'Архитектура','текст про aurora')", [pid]).unwrap();

        let hits = search(&conn, "aurora").unwrap();
        // проект по имени, задача по title, заметка по content
        assert!(hits.iter().any(|h| h.kind == "project"));
        assert!(hits.iter().any(|h| h.kind == "task"));
        assert!(hits.iter().any(|h| h.kind == "note"));
        for h in &hits {
            assert_eq!(h.project_id, pid);
        }
    }

    #[test]
    fn empty_query_returns_nothing() {
        let conn = mem();
        assert!(search(&conn, "   ").unwrap().is_empty());
    }
}
