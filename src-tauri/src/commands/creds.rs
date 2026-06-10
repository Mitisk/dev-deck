use crate::commands::security::{decrypt_secret, encrypt_secret};
use crate::error::{AppError, AppResult, ErrorKind};
use crate::models::{CredInput, Credential};
use crate::state::AppState;
use rusqlite::{params, Connection, OptionalExtension};
use std::process::Command;
use std::sync::MutexGuard;
use tauri::State;

fn lock<'a>(state: &'a State<AppState>) -> AppResult<MutexGuard<'a, Connection>> {
    state.db.lock().map_err(|_| AppError::internal("db mutex poisoned"))
}

fn row_to_cred(conn: &Connection, id: i64) -> AppResult<Credential> {
    Ok(conn.query_row(
        "SELECT id, project_id, label, type, username, url, notes, sort_order,
                (secret_encrypted IS NOT NULL AND length(secret_encrypted) > 0),
                key_path, is_global
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
                is_global: r.get::<_, i64>(10)? != 0,
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

/// id кредов проекта: локальные (project_id) + все глобальные. Порядок:
/// локальные — по credentials.sort_order; глобальные — по cred_order проекта,
/// без строки — в конец.
fn list_cred_ids(conn: &Connection, project_id: i64) -> AppResult<Vec<i64>> {
    let mut stmt = conn.prepare(
        "SELECT c.id
         FROM credentials c
         LEFT JOIN cred_order o ON o.cred_id = c.id AND o.project_id = ?1
         WHERE c.project_id = ?1 OR c.is_global = 1
         ORDER BY (CASE WHEN c.is_global = 1
                        THEN COALESCE(o.sort_order, 1000000000)
                        ELSE c.sort_order END) ASC,
                  c.id ASC",
    )?;
    let rows = stmt.query_map([project_id], |r| r.get::<_, i64>(0))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

#[tauri::command]
pub fn creds_list(state: State<AppState>, project_id: i64) -> AppResult<Vec<Credential>> {
    let conn = lock(&state)?;
    let ids = list_cred_ids(&conn, project_id)?;
    let mut out = Vec::new();
    for id in ids {
        out.push(row_to_cred(&conn, id)?);
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

/// Применить порядок: локальные пишут в credentials.sort_order, глобальные — в cred_order.
fn reorder_creds(conn: &Connection, project_id: i64, ids: &[i64]) -> AppResult<()> {
    for (i, id) in ids.iter().enumerate() {
        let is_global: bool = conn.query_row(
            "SELECT is_global FROM credentials WHERE id=?1",
            params![id],
            |r| r.get::<_, i64>(0),
        )? != 0;
        if is_global {
            conn.execute(
                "INSERT INTO cred_order(cred_id, project_id, sort_order) VALUES(?1,?2,?3)
                 ON CONFLICT(cred_id, project_id) DO UPDATE SET sort_order=excluded.sort_order",
                params![id, project_id, i as i64],
            )?;
        } else {
            conn.execute(
                "UPDATE credentials SET sort_order=?2 WHERE id=?1 AND project_id=?3",
                params![id, i as i64, project_id],
            )?;
        }
    }
    Ok(())
}

/// Переставить креды: выставить sort_order по порядку переданных id.
#[tauri::command]
pub fn creds_reorder(state: State<AppState>, project_id: i64, ids: Vec<i64>) -> AppResult<()> {
    let conn = lock(&state)?;
    let tx = conn.unchecked_transaction()?;
    reorder_creds(&conn, project_id, &ids)?;
    tx.commit()?;
    Ok(())
}

/// Разбить ssh-цель на host и хвостовой :port (порт — только если все цифры).
fn split_host_port(target: &str) -> (&str, Option<&str>) {
    if let Some(idx) = target.rfind(':') {
        let (h, p) = (&target[..idx], &target[idx + 1..]);
        // Порт извлекаем, только если хвост — цифры И слева не «голый» IPv6
        // (в голом IPv6 есть `:`; bracketed-форма `[..]` заканчивается на `]`).
        let host_ok = !h.contains(':') || h.ends_with(']');
        if !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()) && host_ok {
            return (h, Some(p));
        }
    }
    (target, None)
}

/// argv для putty (без имени программы). Политика «ключ → опц. пароль»:
/// есть key_path → `-i key` (пароль НЕ передаём); иначе есть пароль → `-pw`.
fn build_putty_args(target: &str, key_path: Option<&str>, password: Option<&str>) -> Vec<String> {
    let (host, port) = split_host_port(target);
    let mut args = vec!["-ssh".to_string(), host.to_string()];
    if let Some(p) = port {
        args.push("-P".to_string());
        args.push(p.to_string());
    }
    if let Some(k) = key_path.filter(|s| !s.trim().is_empty()) {
        args.push("-i".to_string());
        args.push(k.to_string());
    } else if let Some(pw) = password.filter(|s| !s.is_empty()) {
        args.push("-pw".to_string());
        args.push(pw.to_string());
    }
    args
}

/// Запустить putty.exe: PATH → стандартные пути установки.
fn spawn_putty(args: &[String]) -> AppResult<()> {
    const CANDIDATES: [&str; 3] = [
        "putty.exe",
        r"C:\Program Files\PuTTY\putty.exe",
        r"C:\Program Files (x86)\PuTTY\putty.exe",
    ];
    for exe in CANDIDATES {
        if Command::new(exe).args(args).spawn().is_ok() {
            return Ok(());
        }
    }
    Err(AppError {
        kind: ErrorKind::NotFound,
        message: "PuTTY не найден. Установите PuTTY или добавьте putty.exe в PATH.".into(),
    })
}

/// Открыть кред в PuTTY. Секрет расшифровывается на бэке и во фронт не уходит.
#[tauri::command]
pub fn launch_putty(state: State<AppState>, id: i64) -> AppResult<()> {
    let conn = lock(&state)?;
    let (username, key_path): (Option<String>, Option<String>) = conn
        .query_row(
            "SELECT username, key_path FROM credentials WHERE id=?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?
        .ok_or_else(|| AppError { kind: ErrorKind::NotFound, message: "Кред не найден".into() })?;
    let target = username.unwrap_or_default();
    if target.trim().is_empty() {
        return Err(AppError {
            kind: ErrorKind::Validation,
            message: "У креда не указан хост (поле «Логин / хост»)".into(),
        });
    }
    let has_key = key_path.as_deref().map(|s| !s.trim().is_empty()).unwrap_or(false);
    let password: Option<String> = if has_key {
        None
    } else {
        let blob: Option<Vec<u8>> =
            conn.query_row("SELECT secret_encrypted FROM credentials WHERE id=?1", [id], |r| r.get(0))?;
        match blob {
            Some(b) if !b.is_empty() => {
                let plain = decrypt_secret(&conn, &state.master_key, &b)?;
                Some(String::from_utf8_lossy(&plain).into_owned())
            }
            _ => None,
        }
    };
    let args = build_putty_args(target.trim(), key_path.as_deref(), password.as_deref());
    spawn_putty(&args)
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
    fn split_host_port_parses_numeric_tail() {
        assert_eq!(split_host_port("root@h:2222"), ("root@h", Some("2222")));
        assert_eq!(split_host_port("host"), ("host", None));
        assert_eq!(split_host_port("host:abc"), ("host:abc", None));
    }

    #[test]
    fn split_host_port_handles_ipv6() {
        // голый IPv6 — порт не извлекаем
        assert_eq!(split_host_port("::1"), ("::1", None));
        assert_eq!(split_host_port("fe80::1"), ("fe80::1", None));
        // bracketed IPv6 с портом — извлекаем
        assert_eq!(split_host_port("[::1]:22"), ("[::1]", Some("22")));
    }

    #[test]
    fn build_putty_args_prefers_key_over_password() {
        let a = build_putty_args("root@1.2.3.4", Some("k.ppk"), Some("pw"));
        assert_eq!(a, vec!["-ssh", "root@1.2.3.4", "-i", "k.ppk"]);
        assert!(!a.iter().any(|x| x == "-pw"));
    }

    #[test]
    fn build_putty_args_uses_password_when_no_key() {
        let a = build_putty_args("host:2222", None, Some("secret"));
        assert_eq!(a, vec!["-ssh", "host", "-P", "2222", "-pw", "secret"]);
    }

    #[test]
    fn build_putty_args_bare_host() {
        assert_eq!(build_putty_args("host", None, None), vec!["-ssh", "host"]);
    }

    #[test]
    fn list_cred_ids_merges_local_and_global() {
        let conn = mem();
        conn.execute("INSERT INTO projects(name,status,sort_order) VALUES('P1','active',0)", []).unwrap();
        let p1 = conn.last_insert_rowid();
        conn.execute("INSERT INTO projects(name,status,sort_order) VALUES('P2','active',0)", []).unwrap();
        let p2 = conn.last_insert_rowid();
        conn.execute("INSERT INTO credentials(project_id,label,type,sort_order,is_global) VALUES(?1,'A','note',0,0)", [p1]).unwrap();
        let a = conn.last_insert_rowid();
        conn.execute("INSERT INTO credentials(project_id,label,type,sort_order,is_global) VALUES(?1,'G','note',0,1)", [p1]).unwrap();
        let g = conn.last_insert_rowid();

        let p1_ids = list_cred_ids(&conn, p1).unwrap();
        assert!(p1_ids.contains(&a), "P1 должен видеть локальный A");
        assert!(p1_ids.contains(&g), "P1 должен видеть глобальный G");

        let p2_ids = list_cred_ids(&conn, p2).unwrap();
        assert_eq!(p2_ids, vec![g], "P2 видит только глобальный G");
    }

    #[test]
    fn reorder_creds_routes_by_global_flag() {
        let conn = mem();
        conn.execute("INSERT INTO projects(name,status,sort_order) VALUES('P','active',0)", []).unwrap();
        let p = conn.last_insert_rowid();
        conn.execute("INSERT INTO credentials(project_id,label,type,sort_order,is_global) VALUES(?1,'L','note',0,0)", [p]).unwrap();
        let l = conn.last_insert_rowid();
        conn.execute("INSERT INTO credentials(project_id,label,type,sort_order,is_global) VALUES(?1,'G','note',0,1)", [p]).unwrap();
        let g = conn.last_insert_rowid();

        reorder_creds(&conn, p, &[g, l]).unwrap();

        // локальный l → credentials.sort_order = 1
        let l_order: i64 = conn.query_row("SELECT sort_order FROM credentials WHERE id=?1", [l], |r| r.get(0)).unwrap();
        assert_eq!(l_order, 1);
        // глобальный g → строка в cred_order = 0
        let g_pos: i64 = conn.query_row("SELECT sort_order FROM cred_order WHERE cred_id=?1 AND project_id=?2", params![g, p], |r| r.get(0)).unwrap();
        assert_eq!(g_pos, 0);
        // глобальный g НЕ трогает credentials.sort_order
        let g_cred: i64 = conn.query_row("SELECT sort_order FROM credentials WHERE id=?1", [g], |r| r.get(0)).unwrap();
        assert_eq!(g_cred, 0);
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
