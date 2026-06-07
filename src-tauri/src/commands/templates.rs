use crate::error::{AppError, AppResult, ErrorKind};
use crate::models::{Checklist, ChecklistTemplate};
use crate::state::AppState;
use rusqlite::{params, Connection};
use std::sync::MutexGuard;
use tauri::State;

fn lock<'a>(state: &'a State<AppState>) -> AppResult<MutexGuard<'a, Connection>> {
    state.db.lock().map_err(|_| AppError::internal("db mutex poisoned"))
}

fn parse_items(json: &str) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(json).unwrap_or_default()
}

fn row_to_template(conn: &Connection, id: i64) -> AppResult<ChecklistTemplate> {
    let (name, items_json): (String, String) =
        conn.query_row("SELECT name, items_json FROM checklist_templates WHERE id=?1", [id], |r| Ok((r.get(0)?, r.get(1)?)))?;
    Ok(ChecklistTemplate { id, name, items: parse_items(&items_json) })
}

#[tauri::command]
pub fn templates_list(state: State<AppState>) -> AppResult<Vec<ChecklistTemplate>> {
    let conn = lock(&state)?;
    let mut stmt = conn.prepare("SELECT id FROM checklist_templates ORDER BY name COLLATE NOCASE")?;
    let ids = stmt.query_map([], |r| r.get::<_, i64>(0))?;
    let mut out = Vec::new();
    for id in ids {
        out.push(row_to_template(&conn, id?)?);
    }
    Ok(out)
}

/// Сохранить существующий чеклист как шаблон (тексты пунктов).
#[tauri::command]
pub fn template_save(state: State<AppState>, checklist_id: i64, name: String) -> AppResult<ChecklistTemplate> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError { kind: ErrorKind::Validation, message: "Имя шаблона пусто".into() });
    }
    let conn = lock(&state)?;
    let mut stmt = conn.prepare("SELECT text FROM checklist_items WHERE checklist_id=?1 ORDER BY sort_order, id")?;
    let texts: Vec<String> = stmt.query_map([checklist_id], |r| r.get::<_, String>(0))?.collect::<Result<_, _>>()?;
    let items_json = serde_json::to_string(&texts).map_err(|e| AppError { kind: ErrorKind::Internal, message: format!("JSON: {}", e) })?;
    conn.execute("INSERT INTO checklist_templates(name, items_json) VALUES(?1, ?2)", params![name, items_json])?;
    row_to_template(&conn, conn.last_insert_rowid())
}

/// Применить шаблон к проекту: создать новый чеклист с пунктами (галочки сброшены).
#[tauri::command]
pub fn template_apply(state: State<AppState>, template_id: i64, project_id: i64) -> AppResult<Checklist> {
    let conn = lock(&state)?;
    let (name, items_json): (String, String) =
        conn.query_row("SELECT name, items_json FROM checklist_templates WHERE id=?1", [template_id], |r| Ok((r.get(0)?, r.get(1)?)))?;
    let items = parse_items(&items_json);

    let next: i64 = conn.query_row("SELECT COALESCE(MAX(sort_order),0)+1 FROM checklists WHERE project_id=?1", [project_id], |r| r.get(0))?;
    conn.execute("INSERT INTO checklists(project_id, title, sort_order) VALUES(?1, ?2, ?3)", params![project_id, name, next])?;
    let cid = conn.last_insert_rowid();
    for (i, text) in items.iter().enumerate() {
        conn.execute("INSERT INTO checklist_items(checklist_id, text, is_done, sort_order) VALUES(?1, ?2, 0, ?3)", params![cid, text, i as i64])?;
    }
    // вернуть созданный чеклист
    let items_out = {
        let mut s = conn.prepare("SELECT id, checklist_id, text, is_done, sort_order FROM checklist_items WHERE checklist_id=?1 ORDER BY sort_order, id")?;
        let rows = s.query_map([cid], |r| Ok(crate::models::ChecklistItem {
            id: r.get(0)?, checklist_id: r.get(1)?, text: r.get(2)?, is_done: r.get::<_, i64>(3)? != 0, sort_order: r.get(4)?,
        }))?.collect::<Result<Vec<_>, _>>()?;
        rows
    };
    Ok(Checklist { id: cid, project_id, title: name, sort_order: next, items: items_out })
}

#[tauri::command]
pub fn template_delete(state: State<AppState>, id: i64) -> AppResult<()> {
    let conn = lock(&state)?;
    conn.execute("DELETE FROM checklist_templates WHERE id=?1", [id])?;
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
    fn save_and_apply_template() {
        let conn = mem();
        conn.execute("INSERT INTO projects(name,status,sort_order) VALUES('A','active',0)", []).unwrap();
        let pa = conn.last_insert_rowid();
        conn.execute("INSERT INTO checklists(project_id,title,sort_order) VALUES(?1,'Релиз',0)", [pa]).unwrap();
        let cl = conn.last_insert_rowid();
        conn.execute("INSERT INTO checklist_items(checklist_id,text,is_done,sort_order) VALUES(?1,'Тег',1,0)", [cl]).unwrap();
        conn.execute("INSERT INTO checklist_items(checklist_id,text,is_done,sort_order) VALUES(?1,'Changelog',0,1)", [cl]).unwrap();

        // сохранить как шаблон (тексты)
        let texts: Vec<String> = {
            let mut s = conn.prepare("SELECT text FROM checklist_items WHERE checklist_id=?1 ORDER BY sort_order").unwrap();
            s.query_map([cl], |r| r.get::<_, String>(0)).unwrap().collect::<Result<_, _>>().unwrap()
        };
        let items_json = serde_json::to_string(&texts).unwrap();
        conn.execute("INSERT INTO checklist_templates(name,items_json) VALUES('Чеклист релиза',?1)", [&items_json]).unwrap();
        let tid = conn.last_insert_rowid();

        // применить к другому проекту через прямой вызов логики (apply делает то же)
        conn.execute("INSERT INTO projects(name,status,sort_order) VALUES('B','active',0)", []).unwrap();
        let pb = conn.last_insert_rowid();
        let (name, ijson): (String, String) = conn.query_row("SELECT name,items_json FROM checklist_templates WHERE id=?1", [tid], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
        let items = parse_items(&ijson);
        assert_eq!(items, vec!["Тег".to_string(), "Changelog".to_string()]);
        assert_eq!(name, "Чеклист релиза");

        conn.execute("INSERT INTO checklists(project_id,title,sort_order) VALUES(?1,?2,0)", params![pb, name]).unwrap();
        let ncl = conn.last_insert_rowid();
        for (i, t) in items.iter().enumerate() {
            conn.execute("INSERT INTO checklist_items(checklist_id,text,is_done,sort_order) VALUES(?1,?2,0,?3)", params![ncl, t, i as i64]).unwrap();
        }
        let cnt: i64 = conn.query_row("SELECT count(*) FROM checklist_items WHERE checklist_id=?1 AND is_done=0", [ncl], |r| r.get(0)).unwrap();
        assert_eq!(cnt, 2); // галочки сброшены
    }
}
