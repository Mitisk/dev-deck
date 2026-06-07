use crate::crypto;
use crate::error::{AppError, AppResult, ErrorKind};
use crate::models::CryptoStatus;
use crate::state::AppState;
use rusqlite::{params, Connection, OptionalExtension};
use std::sync::Mutex;
use tauri::State;

const VERIFY: &[u8] = b"devdeck-master-verify-v1";

pub fn get_setting(conn: &Connection, key: &str) -> AppResult<Option<String>> {
    Ok(conn
        .query_row("SELECT value FROM settings WHERE key=?1", [key], |r| {
            r.get::<_, String>(0)
        })
        .optional()?)
}

fn set_setting(conn: &Connection, key: &str, val: &str) -> AppResult<()> {
    conn.execute(
        "INSERT INTO settings(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=?2",
        params![key, val],
    )?;
    Ok(())
}

fn del_setting(conn: &Connection, key: &str) -> AppResult<()> {
    conn.execute("DELETE FROM settings WHERE key=?1", [key])?;
    Ok(())
}

pub fn current_mode(conn: &Connection) -> String {
    get_setting(conn, "crypto_mode")
        .ok()
        .flatten()
        .unwrap_or_else(|| "dpapi".to_string())
}

fn locked() -> AppError {
    AppError {
        kind: ErrorKind::Locked,
        message: "Секреты заблокированы — введите мастер-пароль".into(),
    }
}

/// Зашифровать секрет согласно текущему режиму.
pub fn encrypt_secret(
    conn: &Connection,
    mk: &Mutex<Option<[u8; 32]>>,
    plain: &[u8],
) -> AppResult<Vec<u8>> {
    if current_mode(conn) == "master" {
        let guard = mk
            .lock()
            .map_err(|_| AppError::internal("master key mutex poisoned"))?;
        let key = guard.as_ref().ok_or_else(locked)?;
        crypto::aes_encrypt(key, plain)
    } else {
        crypto::encrypt(plain)
    }
}

/// Расшифровать секрет согласно текущему режиму.
pub fn decrypt_secret(
    conn: &Connection,
    mk: &Mutex<Option<[u8; 32]>>,
    blob: &[u8],
) -> AppResult<Vec<u8>> {
    if current_mode(conn) == "master" {
        let guard = mk
            .lock()
            .map_err(|_| AppError::internal("master key mutex poisoned"))?;
        let key = guard.as_ref().ok_or_else(locked)?;
        crypto::aes_decrypt(key, blob)
    } else {
        crypto::decrypt(blob)
    }
}

// ---------- команды master_* + crypto_status ----------

fn re_encrypt_all<F>(conn: &Connection, mut transform: F) -> AppResult<()>
where
    F: FnMut(&[u8]) -> AppResult<Vec<u8>>,
{
    let rows: Vec<(i64, Vec<u8>)> = {
        let mut stmt = conn.prepare(
            "SELECT id, secret_encrypted FROM credentials WHERE secret_encrypted IS NOT NULL AND length(secret_encrypted)>0",
        )?;
        let collected = stmt
            .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Vec<u8>>(1)?)))?
            .collect::<Result<_, _>>()?;
        collected
    };
    for (id, blob) in rows {
        let newblob = transform(&blob)?;
        conn.execute(
            "UPDATE credentials SET secret_encrypted=?2 WHERE id=?1",
            params![id, newblob],
        )?;
    }
    Ok(())
}

#[tauri::command]
pub fn crypto_status(state: State<AppState>) -> AppResult<CryptoStatus> {
    let conn = state
        .db
        .lock()
        .map_err(|_| AppError::internal("db mutex poisoned"))?;
    let mode = current_mode(&conn);
    let locked = if mode == "master" {
        state
            .master_key
            .lock()
            .map_err(|_| AppError::internal("mk poisoned"))?
            .is_none()
    } else {
        false
    };
    Ok(CryptoStatus { mode, locked })
}

/// Включить мастер-пароль: перешифровать все секреты DPAPI→AES, сохранить соль+verifier, разблокировать.
#[tauri::command]
pub fn master_enable(state: State<AppState>, password: String) -> AppResult<()> {
    if password.trim().len() < 4 {
        return Err(AppError {
            kind: ErrorKind::Validation,
            message: "Пароль слишком короткий (мин. 4)".into(),
        });
    }
    let conn = state
        .db
        .lock()
        .map_err(|_| AppError::internal("db mutex poisoned"))?;
    if current_mode(&conn) == "master" {
        return Err(AppError {
            kind: ErrorKind::Validation,
            message: "Мастер-пароль уже включён".into(),
        });
    }
    let salt = crypto::random_salt();
    let key = crypto::derive_key(&password, &salt)?;
    let tx = conn.unchecked_transaction()?;
    // DPAPI → AES
    re_encrypt_all(&conn, |blob| {
        let plain = crypto::decrypt(blob)?;
        crypto::aes_encrypt(&key, &plain)
    })?;
    set_setting(&conn, "crypto_mode", "master")?;
    set_setting(&conn, "kdf_salt", &hex::encode(salt))?;
    set_setting(&conn, "verifier", &hex::encode(crypto::aes_encrypt(&key, VERIFY)?))?;
    tx.commit()?;
    *state
        .master_key
        .lock()
        .map_err(|_| AppError::internal("mk poisoned"))? = Some(key);
    Ok(())
}

/// Выключить мастер-пароль: перешифровать AES→DPAPI, очистить настройки, заблокировать ключ.
#[tauri::command]
pub fn master_disable(state: State<AppState>, password: String) -> AppResult<()> {
    let conn = state
        .db
        .lock()
        .map_err(|_| AppError::internal("db mutex poisoned"))?;
    if current_mode(&conn) != "master" {
        return Err(AppError {
            kind: ErrorKind::Validation,
            message: "Мастер-пароль не включён".into(),
        });
    }
    let key = verify_password(&conn, &password)?;
    let tx = conn.unchecked_transaction()?;
    re_encrypt_all(&conn, |blob| {
        let plain = crypto::aes_decrypt(&key, blob)?;
        crypto::encrypt(&plain)
    })?;
    set_setting(&conn, "crypto_mode", "dpapi")?;
    del_setting(&conn, "kdf_salt")?;
    del_setting(&conn, "verifier")?;
    tx.commit()?;
    *state
        .master_key
        .lock()
        .map_err(|_| AppError::internal("mk poisoned"))? = None;
    Ok(())
}

fn verify_password(conn: &Connection, password: &str) -> AppResult<[u8; 32]> {
    let salt_hex = get_setting(conn, "kdf_salt")?.ok_or_else(|| AppError::internal("нет соли"))?;
    let ver_hex =
        get_setting(conn, "verifier")?.ok_or_else(|| AppError::internal("нет verifier"))?;
    let salt = hex::decode(salt_hex).map_err(|_| AppError::internal("плохая соль"))?;
    let verifier = hex::decode(ver_hex).map_err(|_| AppError::internal("плохой verifier"))?;
    let key = crypto::derive_key(password, &salt)?;
    match crypto::aes_decrypt(&key, &verifier) {
        Ok(v) if v == VERIFY => Ok(key),
        _ => Err(AppError {
            kind: ErrorKind::Validation,
            message: "Неверный мастер-пароль".into(),
        }),
    }
}

/// Разблокировать: проверить пароль, положить ключ в память.
#[tauri::command]
pub fn master_unlock(state: State<AppState>, password: String) -> AppResult<()> {
    let conn = state
        .db
        .lock()
        .map_err(|_| AppError::internal("db mutex poisoned"))?;
    let key = verify_password(&conn, &password)?;
    *state
        .master_key
        .lock()
        .map_err(|_| AppError::internal("mk poisoned"))? = Some(key);
    Ok(())
}

/// Заблокировать (забыть ключ).
#[tauri::command]
pub fn master_lock(state: State<AppState>) -> AppResult<()> {
    *state
        .master_key
        .lock()
        .map_err(|_| AppError::internal("mk poisoned"))? = None;
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
    fn enable_reencrypts_and_unlock_decrypts() {
        let conn = mem();
        conn.execute(
            "INSERT INTO projects(name,status,sort_order) VALUES('P','active',0)",
            [],
        )
        .unwrap();
        let pid = conn.last_insert_rowid();
        // секрет в режиме DPAPI
        let dpapi_blob = crypto::encrypt(b"my-secret").unwrap();
        conn.execute(
            "INSERT INTO credentials(project_id,label,type,secret_encrypted,sort_order) VALUES(?1,'C','token',?2,0)",
            params![pid, dpapi_blob],
        )
        .unwrap();

        // включаем master вручную (как master_enable, без State)
        let salt = crypto::random_salt();
        let key = crypto::derive_key("pw1234", &salt).unwrap();
        re_encrypt_all(&conn, |blob| {
            let p = crypto::decrypt(blob)?;
            crypto::aes_encrypt(&key, &p)
        })
        .unwrap();
        set_setting(&conn, "crypto_mode", "master").unwrap();
        set_setting(&conn, "kdf_salt", &hex::encode(salt)).unwrap();
        set_setting(
            &conn,
            "verifier",
            &hex::encode(crypto::aes_encrypt(&key, VERIFY).unwrap()),
        )
        .unwrap();

        // verify_password верный/неверный
        assert!(verify_password(&conn, "pw1234").is_ok());
        assert!(verify_password(&conn, "wrong").is_err());

        // decrypt_secret в master-режиме с ключом → исходный секрет
        let mk = Mutex::new(Some(key));
        let stored: Vec<u8> = conn
            .query_row("SELECT secret_encrypted FROM credentials", [], |r| r.get(0))
            .unwrap();
        let plain = decrypt_secret(&conn, &mk, &stored).unwrap();
        assert_eq!(plain, b"my-secret");

        // без ключа (locked) → ошибка
        let locked_mk = Mutex::new(None);
        assert!(decrypt_secret(&conn, &locked_mk, &stored).is_err());
    }
}
