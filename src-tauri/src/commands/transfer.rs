use crate::commands::security::{decrypt_secret, encrypt_secret};
use crate::error::{AppError, AppResult, ErrorKind};
use crate::models::ImportSummary;
use crate::state::AppState;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{Manager, State};

// ---------- DTO (форма JSON-выгрузки) ----------

#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct ExportTask { title: String, description: Option<String>, status: String, priority: i64, due_date: Option<String>, sort_order: i64 }

#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct ExportItem { text: String, is_done: bool, sort_order: i64 }

#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct ExportChecklist { title: String, sort_order: i64, items: Vec<ExportItem> }

#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct ExportNote { title: Option<String>, content_md: Option<String> }

#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct ExportLink { label: String, url: String, icon: Option<String>, sort_order: i64 }

#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct ExportFile { label: String, path: String, sort_order: i64 }

#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct ExportCommand { label: String, command: String, working_dir: Option<String>, run_in: String, icon: Option<String>, sort_order: i64 }

#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct ExportCred {
    label: String,
    #[serde(rename = "type")] kind: String,
    username: Option<String>, url: Option<String>, notes: Option<String>, sort_order: i64,
    #[serde(skip_serializing_if = "Option::is_none")] secret: Option<String>,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct ExportProject {
    name: String, description: Option<String>, status: String, color: Option<String>,
    icon: Option<String>, path: Option<String>, repo_path: Option<String>, pinned: bool, sort_order: i64,
    tags: Vec<String>,
    tasks: Vec<ExportTask>, checklists: Vec<ExportChecklist>, notes: Vec<ExportNote>,
    links: Vec<ExportLink>, files: Vec<ExportFile>, commands: Vec<ExportCommand>, creds: Vec<ExportCred>,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct ExportDoc { version: i64, exported_at: u64, projects: Vec<ExportProject> }

// ---------- helpers ----------

fn col<T, F>(conn: &Connection, sql: &str, pid: i64, f: F) -> AppResult<Vec<T>>
where F: Fn(&rusqlite::Row) -> rusqlite::Result<T> {
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([pid], |r| f(r))?;
    let mut v = Vec::new();
    for r in rows { v.push(r?); }
    Ok(v)
}

fn build_export(
    conn: &Connection,
    mk: &Mutex<Option<[u8; 32]>>,
    include_secrets: bool,
) -> AppResult<ExportDoc> {
    let mut projects = Vec::new();
    let mut pstmt = conn.prepare(
        "SELECT id, name, description, status, color, icon, path, repo_path, pinned, sort_order FROM projects ORDER BY sort_order, id",
    )?;
    let prows = pstmt.query_map([], |r| Ok((
        r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, Option<String>>(2)?, r.get::<_, String>(3)?,
        r.get::<_, Option<String>>(4)?, r.get::<_, Option<String>>(5)?, r.get::<_, Option<String>>(6)?,
        r.get::<_, Option<String>>(7)?, r.get::<_, i64>(8)?, r.get::<_, i64>(9)?,
    )))?.collect::<Result<Vec<_>, _>>()?;

    for (id, name, description, status, color, icon, path, repo_path, pinned, sort_order) in prows {
        let tags = col(conn, "SELECT t.name FROM tags t JOIN project_tags pt ON pt.tag_id=t.id WHERE pt.project_id=?1 ORDER BY t.name", id, |r| r.get::<_, String>(0))?;
        let tasks = col(conn, "SELECT title,description,status,priority,due_date,sort_order FROM tasks WHERE project_id=?1 ORDER BY sort_order,id", id,
            |r| Ok(ExportTask { title: r.get(0)?, description: r.get(1)?, status: r.get(2)?, priority: r.get(3)?, due_date: r.get(4)?, sort_order: r.get(5)? }))?;
        let mut checklists = Vec::new();
        let cls = col(conn, "SELECT id,title,sort_order FROM checklists WHERE project_id=?1 ORDER BY sort_order,id", id, |r| Ok((r.get::<_,i64>(0)?, r.get::<_,String>(1)?, r.get::<_,i64>(2)?)))?;
        for (cid, ctitle, csort) in cls {
            let items = col(conn, "SELECT text,is_done,sort_order FROM checklist_items WHERE checklist_id=?1 ORDER BY sort_order,id", cid,
                |r| Ok(ExportItem { text: r.get(0)?, is_done: r.get::<_, i64>(1)? != 0, sort_order: r.get(2)? }))?;
            checklists.push(ExportChecklist { title: ctitle, sort_order: csort, items });
        }
        let notes = col(conn, "SELECT title,content_md FROM notes WHERE project_id=?1 ORDER BY id", id, |r| Ok(ExportNote { title: r.get(0)?, content_md: r.get(1)? }))?;
        let links = col(conn, "SELECT label,url,icon,sort_order FROM links WHERE project_id=?1 ORDER BY sort_order,id", id, |r| Ok(ExportLink { label: r.get(0)?, url: r.get(1)?, icon: r.get(2)?, sort_order: r.get(3)? }))?;
        let files = col(conn, "SELECT label,path,sort_order FROM files WHERE project_id=?1 ORDER BY sort_order,id", id, |r| Ok(ExportFile { label: r.get(0)?, path: r.get(1)?, sort_order: r.get(2)? }))?;
        let commands = col(conn, "SELECT label,command,working_dir,run_in,icon,sort_order FROM commands WHERE project_id=?1 ORDER BY sort_order,id", id,
            |r| Ok(ExportCommand { label: r.get(0)?, command: r.get(1)?, working_dir: r.get(2)?, run_in: r.get(3)?, icon: r.get(4)?, sort_order: r.get(5)? }))?;

        // creds: метаданные; секрет — только если include_secrets
        let cred_rows = col(conn, "SELECT id,label,type,username,url,notes,sort_order FROM credentials WHERE project_id=?1 ORDER BY sort_order,id", id,
            |r| Ok((r.get::<_,i64>(0)?, r.get::<_,String>(1)?, r.get::<_,String>(2)?, r.get::<_,Option<String>>(3)?, r.get::<_,Option<String>>(4)?, r.get::<_,Option<String>>(5)?, r.get::<_,i64>(6)?)))?;
        let mut creds = Vec::new();
        for (cid, label, kind, username, url, notes, sort_order) in cred_rows {
            let secret = if include_secrets {
                let blob: Option<Vec<u8>> = conn.query_row("SELECT secret_encrypted FROM credentials WHERE id=?1", [cid], |r| r.get(0))?;
                match blob {
                    Some(b) if !b.is_empty() => Some(String::from_utf8_lossy(&decrypt_secret(conn, mk, &b)?).into_owned()),
                    _ => None,
                }
            } else { None };
            creds.push(ExportCred { label, kind, username, url, notes, sort_order, secret });
        }

        projects.push(ExportProject {
            name, description, status, color, icon, path, repo_path, pinned: pinned != 0, sort_order,
            tags, tasks, checklists, notes, links, files, commands, creds,
        });
    }

    Ok(ExportDoc {
        version: 1,
        exported_at: SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0),
        projects,
    })
}

fn import_doc(
    conn: &Connection,
    mk: &Mutex<Option<[u8; 32]>>,
    doc: &ExportDoc,
) -> AppResult<usize> {
    for p in &doc.projects {
        conn.execute(
            "INSERT INTO projects(name,description,status,color,icon,path,repo_path,pinned,sort_order)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![p.name, p.description, p.status, p.color, p.icon, p.path, p.repo_path, p.pinned as i64, p.sort_order],
        )?;
        let pid = conn.last_insert_rowid();

        for tag in &p.tags {
            conn.execute("INSERT OR IGNORE INTO tags(name) VALUES(?1)", [tag])?;
            let tid: i64 = conn.query_row("SELECT id FROM tags WHERE name=?1", [tag], |r| r.get(0))?;
            conn.execute("INSERT OR IGNORE INTO project_tags(project_id,tag_id) VALUES(?1,?2)", params![pid, tid])?;
        }
        for t in &p.tasks {
            conn.execute("INSERT INTO tasks(project_id,title,description,status,priority,due_date,sort_order) VALUES(?1,?2,?3,?4,?5,?6,?7)",
                params![pid, t.title, t.description, t.status, t.priority, t.due_date, t.sort_order])?;
        }
        for c in &p.checklists {
            conn.execute("INSERT INTO checklists(project_id,title,sort_order) VALUES(?1,?2,?3)", params![pid, c.title, c.sort_order])?;
            let cid = conn.last_insert_rowid();
            for it in &c.items {
                conn.execute("INSERT INTO checklist_items(checklist_id,text,is_done,sort_order) VALUES(?1,?2,?3,?4)",
                    params![cid, it.text, it.is_done as i64, it.sort_order])?;
            }
        }
        for n in &p.notes {
            conn.execute("INSERT INTO notes(project_id,title,content_md) VALUES(?1,?2,?3)", params![pid, n.title, n.content_md])?;
        }
        for l in &p.links {
            conn.execute("INSERT INTO links(project_id,label,url,icon,sort_order) VALUES(?1,?2,?3,?4,?5)", params![pid, l.label, l.url, l.icon, l.sort_order])?;
        }
        for f in &p.files {
            conn.execute("INSERT INTO files(project_id,label,path,sort_order) VALUES(?1,?2,?3,?4)", params![pid, f.label, f.path, f.sort_order])?;
        }
        for cm in &p.commands {
            conn.execute("INSERT INTO commands(project_id,label,command,working_dir,run_in,icon,sort_order) VALUES(?1,?2,?3,?4,?5,?6,?7)",
                params![pid, cm.label, cm.command, cm.working_dir, cm.run_in, cm.icon, cm.sort_order])?;
        }
        for cr in &p.creds {
            let blob: Option<Vec<u8>> = match cr.secret.as_deref() {
                Some(s) if !s.is_empty() => Some(encrypt_secret(conn, mk, s.as_bytes())?),
                _ => None,
            };
            conn.execute("INSERT INTO credentials(project_id,label,type,username,url,secret_encrypted,notes,sort_order) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
                params![pid, cr.label, cr.kind, cr.username, cr.url, blob, cr.notes, cr.sort_order])?;
        }
    }
    Ok(doc.projects.len())
}

// ---------- команды ----------

#[tauri::command]
pub fn export_json(state: State<AppState>, include_secrets: bool) -> AppResult<String> {
    let conn = state.db.lock().map_err(|_| AppError::internal("db mutex poisoned"))?;
    let doc = build_export(&conn, &state.master_key, include_secrets)?;
    serde_json::to_string_pretty(&doc).map_err(|e| AppError { kind: ErrorKind::Internal, message: format!("JSON: {}", e) })
}

#[tauri::command]
pub fn export_to_file(state: State<AppState>, app: tauri::AppHandle, include_secrets: bool) -> AppResult<String> {
    let json = export_json(state, include_secrets)?;
    let dir = app.path().app_data_dir()
        .map_err(|e| AppError { kind: ErrorKind::Io, message: format!("Каталог данных: {}", e) })?
        .join("exports");
    std::fs::create_dir_all(&dir).map_err(|e| AppError { kind: ErrorKind::Io, message: format!("Папка экспорта: {}", e) })?;
    let epoch = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let path = dir.join(format!("devdeck-export-{}.json", epoch));
    std::fs::write(&path, json).map_err(|e| AppError { kind: ErrorKind::Io, message: format!("Запись файла: {}", e) })?;
    Ok(path.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn import_json(state: State<AppState>, json: String) -> AppResult<ImportSummary> {
    let doc: ExportDoc = serde_json::from_str(&json)
        .map_err(|e| AppError { kind: ErrorKind::Validation, message: format!("Некорректный JSON: {}", e) })?;
    let conn = state.db.lock().map_err(|_| AppError::internal("db mutex poisoned"))?;
    let n = import_doc(&conn, &state.master_key, &doc)?;
    Ok(ImportSummary { projects: n })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto;
    use crate::db;
    use std::sync::Mutex;

    fn mem() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        db::migrations::run(&conn).unwrap();
        conn
    }

    #[test]
    fn export_import_roundtrip_with_secret() {
        let src = mem();
        src.execute("INSERT INTO projects(name,status,sort_order) VALUES('Aurora','active',0)", []).unwrap();
        let pid = src.last_insert_rowid();
        src.execute("INSERT INTO tasks(project_id,title,status,sort_order) VALUES(?1,'T1','todo',0)", [pid]).unwrap();
        src.execute("INSERT INTO notes(project_id,title,content_md) VALUES(?1,'N','body')", [pid]).unwrap();
        let blob = crypto::encrypt(b"shh").unwrap();
        src.execute("INSERT INTO credentials(project_id,label,type,secret_encrypted,sort_order) VALUES(?1,'Stripe','api_key',?2,0)", params![pid, blob]).unwrap();

        let doc = build_export(&src, &Mutex::new(None), true).unwrap();
        assert_eq!(doc.projects.len(), 1);
        assert_eq!(doc.projects[0].creds[0].secret.as_deref(), Some("shh"));

        let json = serde_json::to_string(&doc).unwrap();
        let parsed: ExportDoc = serde_json::from_str(&json).unwrap();

        let dst = mem();
        let n = import_doc(&dst, &Mutex::new(None), &parsed).unwrap();
        assert_eq!(n, 1);
        let tasks: i64 = dst.query_row("SELECT count(*) FROM tasks", [], |r| r.get(0)).unwrap();
        assert_eq!(tasks, 1);
        // секрет заново зашифрован и расшифровывается
        let stored: Vec<u8> = dst.query_row("SELECT secret_encrypted FROM credentials", [], |r| r.get(0)).unwrap();
        assert_eq!(String::from_utf8_lossy(&crypto::decrypt(&stored).unwrap()), "shh");
    }

    #[test]
    fn export_without_secrets_omits_them() {
        let src = mem();
        src.execute("INSERT INTO projects(name,status,sort_order) VALUES('P','active',0)", []).unwrap();
        let pid = src.last_insert_rowid();
        let blob = crypto::encrypt(b"x").unwrap();
        src.execute("INSERT INTO credentials(project_id,label,type,secret_encrypted,sort_order) VALUES(?1,'C','token',?2,0)", params![pid, blob]).unwrap();
        let doc = build_export(&src, &Mutex::new(None), false).unwrap();
        assert!(doc.projects[0].creds[0].secret.is_none());
    }
}
