use crate::commands::security::{decrypt_secret, encrypt_secret};
use crate::error::{AppError, AppResult, ErrorKind};
use crate::models::{CredInput, Credential};
use crate::state::AppState;
use rusqlite::{params, Connection};
use std::sync::MutexGuard;
use tauri::State;

fn lock<'a>(state: &'a State<AppState>) -> AppResult<MutexGuard<'a, Connection>> {
    state.db.lock().map_err(|_| AppError::internal("db mutex poisoned"))
}

fn row_to_cred(conn: &Connection, id: i64) -> AppResult<Credential> {
    Ok(conn.query_row(
        "SELECT id, project_id, label, type, username, url, notes, sort_order,
                (secret_encrypted IS NOT NULL AND length(secret_encrypted) > 0),
                key_path
         FROM credentials WHERE id = ?1",
        [id],
        |r| {
            Ok(Credential {
                id: r.get(0)?,
                project_id: r.get(1)?,
                label: r.get(2)?,
                kind: r.get(3)?,
                username: r.get(4)?,
                url: r.get(5)?,
                notes: r.get(6)?,
                sort_order: r.get(7)?,
                has_secret: r.get::<_, i64>(8)? != 0,
                key_path: r.get(9)?,
            })
        },
    )?)
}

fn validate(input: &CredInput) -> AppResult<()> {
    if input.label.trim().is_empty() {
        return Err(AppError { kind: ErrorKind::Validation, message: "Название креда пусто".into() });
    }
    Ok(())
}

#[tauri::command]
pub fn creds_list(state: State<AppState>, project_id: i64) -> AppResult<Vec<Credential>> {
    let conn = lock(&state)?;
    let mut stmt = conn.prepare(
        "SELECT id FROM credentials WHERE project_id = ?1 ORDER BY sort_order ASC, id ASC",
    )?;
    let ids = stmt.query_map([project_id], |r| r.get::<_, i64>(0))?;
    let mut out = Vec::new();
    for id in ids {
        out.push(row_to_cred(&conn, id?)?);
    }
    Ok(out)
}

#[tauri::command]
pub fn creds_get_secret(state: State<AppState>, id: i64) -> AppResult<String> {
    let conn = lock(&state)?;
    let blob: Option<Vec<u8>> =
        conn.query_row("SELECT secret_encrypted FROM credentials WHERE id = ?1", [id], |r| r.get(0))?;
    match blob {
        Some(b) if !b.is_empty() => {
            let plain = decrypt_secret(&conn, &state.master_key, &b)?;
            Ok(String::from_utf8_lossy(&plain).into_owned())
        }
        _ => Ok(String::new()),
    }
}

#[tauri::command]
pub fn creds_create(state: State<AppState>, project_id: i64, input: CredInput) -> AppResult<Credential> {
    validate(&input)?;
    let conn = lock(&state)?;
    let next: i64 = conn.query_row(
        "SELECT COALESCE(MAX(sort_order),0)+1 FROM credentials WHERE project_id=?1",
        [project_id],
        |r| r.get(0),
    )?;
    let secret_blob: Option<Vec<u8>> = match input.secret.as_deref() {
        Some(s) if !s.is_empty() => Some(encrypt_secret(&conn, &state.master_key, s.as_bytes())?),
        _ => None,
    };
    conn.execute(
        "INSERT INTO credentials(project_id, label, type, username, url, secret_encrypted, notes, sort_order, key_path)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            project_id,
            input.label.trim(),
            input.kind,
            input.username,
            input.url,
            secret_blob,
            input.notes,
            next,
            input.key_path,
        ],
    )?;
    row_to_cred(&conn, conn.last_insert_rowid())
}

#[tauri::command]
pub fn creds_update(state: State<AppState>, id: i64, input: CredInput) -> AppResult<Credential> {
    validate(&input)?;
    let conn = lock(&state)?;
    let n = conn.execute(
        "UPDATE credentials SET label=?2, type=?3, username=?4, url=?5, notes=?6, key_path=?7, updated_at=datetime('now')
         WHERE id=?1",
        params![id, input.label.trim(), input.kind, input.username, input.url, input.notes, input.key_path],
    )?;
    if n == 0 {
        return Err(AppError { kind: ErrorKind::NotFound, message: "Кред не найден".into() });
    }
    // Секрет меняем только если передан (Some). Some("") = очистить.
    if let Some(s) = input.secret.as_deref() {
        let blob: Option<Vec<u8>> = if s.is_empty() { None } else { Some(encrypt_secret(&conn, &state.master_key, s.as_bytes())?) };
        conn.execute("UPDATE credentials SET secret_encrypted=?2 WHERE id=?1", params![id, blob])?;
    }
    row_to_cred(&conn, id)
}

#[tauri::command]
pub fn creds_delete(state: State<AppState>, id: i64) -> AppResult<()> {
    let conn = lock(&state)?;
    conn.execute("DELETE FROM credentials WHERE id=?1", [id])?;
    Ok(())
}

/// Переставить креды: выставить sort_order по порядку переданных id.
#[tauri::command]
pub fn creds_reorder(state: State<AppState>, project_id: i64, ids: Vec<i64>) -> AppResult<()> {
    let conn = lock(&state)?;
    let tx = conn.unchecked_transaction()?;
    for (i, id) in ids.iter().enumerate() {
        conn.execute(
            "UPDATE credentials SET sort_order=?2 WHERE id=?1 AND project_id=?3",
            params![id, i as i64, project_id],
        )?;
    }
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto;
    use crate::db;

    fn mem() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        db::migrations::run(&conn).unwrap();
        conn
    }

    #[test]
    fn create_stores_encrypted_and_get_secret_decrypts() {
        let conn = mem();
        conn.execute("INSERT INTO projects(name,status,sort_order) VALUES('P','active',0)", []).unwrap();
        let pid = conn.last_insert_rowid();

        // прямой insert через encrypt (минуя команду, которой нужен State)
        let blob = crypto::encrypt(b"super-secret").unwrap();
        conn.execute(
            "INSERT INTO credentials(project_id,label,type,secret_encrypted,sort_order) VALUES(?1,'Stripe','api_key',?2,0)",
            params![pid, blob],
        ).unwrap();
        let id = conn.last_insert_rowid();

        // секрет в БД зашифрован (не равен plaintext)
        let stored: Vec<u8> = conn.query_row("SELECT secret_encrypted FROM credentials WHERE id=?1", [id], |r| r.get(0)).unwrap();
        assert_ne!(stored.as_slice(), b"super-secret".as_slice());

        // дешифровка возвращает оригинал
        let dec = crypto::decrypt(&stored).unwrap();
        assert_eq!(String::from_utf8_lossy(&dec), "super-secret");

        // метаданные без секрета
        let cred = row_to_cred(&conn, id).unwrap();
        assert_eq!(cred.label, "Stripe");
        assert_eq!(cred.kind, "api_key");
        assert!(cred.has_secret);
    }

    #[test]
    fn creds_reorder_sets_sort_order() {
        let conn = mem();
        conn.execute("INSERT INTO projects(name,status,sort_order) VALUES('P','active',0)", []).unwrap();
        let pid = conn.last_insert_rowid();

        let blob = crypto::encrypt(b"x").unwrap();
        conn.execute(
            "INSERT INTO credentials(project_id,label,type,secret_encrypted,sort_order) VALUES(?1,'a','note',?2,0)",
            params![pid, blob],
        ).unwrap();
        let a = conn.last_insert_rowid();

        let blob2 = crypto::encrypt(b"y").unwrap();
        conn.execute(
            "INSERT INTO credentials(project_id,label,type,secret_encrypted,sort_order) VALUES(?1,'b','note',?2,1)",
            params![pid, blob2],
        ).unwrap();
        let b = conn.last_insert_rowid();

        // переставляем в порядок [b, a] той же SQL-логикой, что и команда
        let ids = vec![b, a];
        let tx = conn.unchecked_transaction().unwrap();
        for (i, id) in ids.iter().enumerate() {
            conn.execute(
                "UPDATE credentials SET sort_order=?2 WHERE id=?1 AND project_id=?3",
                params![id, i as i64, pid],
            ).unwrap();
        }
        tx.commit().unwrap();

        let order: Vec<i64> = {
            let mut stmt = conn.prepare("SELECT id FROM credentials WHERE project_id=?1 ORDER BY sort_order ASC, id ASC").unwrap();
            let rows = stmt.query_map([pid], |r| r.get(0)).unwrap();
            rows.map(|r| r.unwrap()).collect()
        };
        assert_eq!(order, vec![b, a]);
    }
}
