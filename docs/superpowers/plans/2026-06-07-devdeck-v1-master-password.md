# DevDeck v1.0 — Срез «Режим мастер-пароля» — план реализации

> **Для исполнителя:** ОБЯЗАТЕЛЬНЫЙ ПОД-НАВЫК: superpowers:subagent-driven-development. Шаги — чекбоксы.

**Цель:** Альтернатива DPAPI — режим мастер-пароля: секреты шифруются AES-256-GCM ключом, выведенным из пароля через Argon2id. Переключаемо в настройках (включить/выключить — перешифровывает все секреты). Блокируются **только секреты**: приложение работает без пароля, но «показать/копировать» секрет требует разблокировки.

**Архитектура:**
- `crypto.rs`: DPAPI (есть) + AES-256-GCM (`aes_encrypt`/`aes_decrypt`) + Argon2id (`derive_key`) + случайная соль.
- `commands/security.rs`: настройки режима (`settings` table), **mode-aware** `encrypt_secret`/`decrypt_secret` (диспетчер DPAPI↔AES по текущему режиму), команды `crypto_status`/`master_enable`/`master_disable`/`master_unlock`/`master_lock`.
- `state::AppState` += `master_key: Mutex<Option<[u8;32]>>` (ключ в памяти после разблокировки).
- `creds.rs` и `transfer.rs` шифруют/дешифруют секреты ТОЛЬКО через `security::{encrypt_secret,decrypt_secret}` (а не напрямую DPAPI).
- При master-режиме и пустом ключе `decrypt_secret`/`encrypt_secret` → ошибка `Locked` (фронт показывает «разблокируйте»).

**Безопасность:**
- Argon2id(пароль, соль 16 байт) → 32-байтный ключ. AES-256-GCM, nonce 12 байт случайный на секрет (хранится перед ciphertext).
- Проверка пароля — verifier: зашифрованная константа; на unlock дешифруем и сравниваем.
- Соль/verifier/режим — в `settings` (hex). Пароль и ключ нигде на диск не пишутся. Забытый пароль = секреты невосстановимы (предупредить в UI).
- Перешифровка при включении/выключении — в транзакции.

**Стек:** + crates `aes-gcm`, `argon2`, `rand`, `hex`. Без миграций (`settings` в 0001).

**Решения (от пользователя):** полный переключаемый режим; блокируются только секреты.

**Источники:** `TZ_DevDeck.md` (7 — два режима; 10). Текущие `crypto.rs` (DPAPI), `commands/creds.rs`, `commands/transfer.rs`, `state.rs`, `error.rs`.

---

## Контекст

- Rust: 65 команд; `crypto::{encrypt,decrypt}` (DPAPI); `error::{AppError,ErrorKind(Db/Io/NotFound/Validation/Internal),AppResult}`; `state::AppState { db: Mutex<Connection> }`; `commands/creds.rs` (использует `crypto::encrypt/decrypt`), `commands/transfer.rs` (`build_export`/`import_doc` используют `crypto`). Таблица `settings(key TEXT PRIMARY KEY, value TEXT)`.
- Фронт: `CredsTab.svelte` (показать/копировать секрет через `creds_get_secret`), `AppSettings.svelte` (модалка «Данные»). `api/client.ts` (`call`; на reject — тост).

---

## Структура файлов

```
src-tauri/
├─ Cargo.toml             # MOD: + aes-gcm, argon2, rand, hex
└─ src/
   ├─ crypto.rs           # MOD: + derive_key/aes_encrypt/aes_decrypt/random_salt + тесты
   ├─ error.rs            # MOD: + ErrorKind::Locked
   ├─ state.rs            # MOD: + master_key
   ├─ models.rs           # MOD: + CryptoStatus
   ├─ commands/security.rs# NEW: режим + encrypt_secret/decrypt_secret + master_* + тест
   ├─ commands/creds.rs   # MOD: использовать security::{encrypt_secret,decrypt_secret}
   ├─ commands/transfer.rs# MOD: то же в build_export/import_doc
   ├─ commands/mod.rs     # MOD: + pub mod security;
   └─ lib.rs              # MOD: master_key в AppState; регистрация security-команд

src/lib/
├─ types.ts               # MOD: + CryptoStatus
├─ api/security.ts        # NEW
├─ components/AppSettings.svelte # MOD: секция «Безопасность»
└─ components/CredsTab.svelte    # MOD: обработка ошибки Locked → подсказка
```

---

## Task 1: Rust — крипто-примитивы (AES-GCM + Argon2id)

**Files:** Modify `Cargo.toml`, `crypto.rs`, `error.rs`, `state.rs`.

- [ ] **Step 1: Зависимости** — в `src-tauri/Cargo.toml` `[dependencies]` добавить:
```toml
aes-gcm = "0.10"
argon2 = "0.5"
rand = "0.8"
hex = "0.4"
```

- [ ] **Step 2: ErrorKind::Locked** — в `src-tauri/src/error.rs` в enum `ErrorKind` добавить вариант `Locked`. (serde rename_all уже camelCase → сериализуется как `"locked"`.)

- [ ] **Step 3: master_key в AppState** — в `src-tauri/src/state.rs`:
```rust
use std::sync::Mutex;
// ... существующее ...
pub struct AppState {
    pub db: Mutex<rusqlite::Connection>,
    pub master_key: Mutex<Option<[u8; 32]>>,
}
```
(если `Connection` импортирован иначе — сохранить как было, добавить только поле `master_key`.)

- [ ] **Step 4: Крипто-примитивы в crypto.rs** — добавить (ВНИМАНИЕ: точные пути/типы `aes-gcm 0.10`/`argon2 0.5` могут отличаться — привести к фактическому API; критерий — round-trip тесты ниже зелёные):
```rust
use aes_gcm::aead::{Aead, AeadCore, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use argon2::Argon2;

/// 16 случайных байт соли.
pub fn random_salt() -> [u8; 16] {
    use aes_gcm::aead::rand_core::RngCore;
    let mut s = [0u8; 16];
    OsRng.fill_bytes(&mut s);
    s
}

/// Argon2id: пароль + соль → 32-байтный ключ.
pub fn derive_key(password: &str, salt: &[u8]) -> AppResult<[u8; 32]> {
    let mut key = [0u8; 32];
    Argon2::default()
        .hash_password_into(password.as_bytes(), salt, &mut key)
        .map_err(|e| AppError::internal(format!("argon2: {}", e)))?;
    Ok(key)
}

/// AES-256-GCM: вернуть nonce(12) || ciphertext.
pub fn aes_encrypt(key: &[u8; 32], plain: &[u8]) -> AppResult<Vec<u8>> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ct = cipher.encrypt(&nonce, plain).map_err(|_| AppError::internal("aes encrypt"))?;
    let mut out = nonce.to_vec();
    out.extend_from_slice(&ct);
    Ok(out)
}

/// Расшифровать nonce(12) || ciphertext.
pub fn aes_decrypt(key: &[u8; 32], blob: &[u8]) -> AppResult<Vec<u8>> {
    if blob.len() < 12 {
        return Err(AppError::internal("aes blob too short"));
    }
    let (n, ct) = blob.split_at(12);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    cipher
        .decrypt(Nonce::from_slice(n), ct)
        .map_err(|_| AppError::internal("aes decrypt (неверный ключ?)"))
}

#[cfg(test)]
mod aes_tests {
    use super::*;

    #[test]
    fn aes_roundtrip() {
        let key = derive_key("hunter2", b"saltsaltsaltsalt").unwrap();
        let enc = aes_encrypt(&key, b"secret-value").unwrap();
        assert_ne!(&enc[12..], b"secret-value");
        assert_eq!(aes_decrypt(&key, &enc).unwrap(), b"secret-value");
    }

    #[test]
    fn wrong_key_fails() {
        let salt = b"saltsaltsaltsalt";
        let k1 = derive_key("a", salt).unwrap();
        let k2 = derive_key("b", salt).unwrap();
        let enc = aes_encrypt(&k1, b"x").unwrap();
        assert!(aes_decrypt(&k2, &enc).is_err());
    }

    #[test]
    fn derive_is_deterministic() {
        let salt = super::random_salt();
        assert_eq!(derive_key("p", &salt).unwrap(), derive_key("p", &salt).unwrap());
    }
}
```
> Если `hash_password_into`/`generate_nonce`/`OsRng`/`rand_core::RngCore` отличаются в установленных версиях — подстроить (подсказывает `cargo build`). Возможно, потребуется `use rand::rngs::OsRng;` + `rand::RngCore` для соли/нонса вместо `aes_gcm::aead::OsRng` — взять то, что компилируется. Если крипто не собирается после разумной попытки — STOP, доложить BLOCKED.

- [ ] **Step 5: Тесты крипто**
```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib aes_tests
cargo test --manifest-path src-tauri/Cargo.toml --lib crypto
cargo build --manifest-path src-tauri/Cargo.toml --lib
```
Expected: `aes_roundtrip`/`wrong_key_fails`/`derive_is_deterministic` + DPAPI `encrypt_decrypt_roundtrip` ok; lib-сборка успешна.

(Коммит — в конце Task 3, чтобы AppState с master_key собрался вместе с использованием.)

---

## Task 2: Rust — mode-aware шифрование + рефактор creds/transfer

**Files:** Create `commands/security.rs` (часть 1); Modify `commands/creds.rs`, `commands/transfer.rs`, `commands/mod.rs`, `models.rs`, `lib.rs`.

- [ ] **Step 1: models.rs** — добавить (после `ChecklistTemplate`):
```rust
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CryptoStatus {
    pub mode: String,   // "dpapi" | "master"
    pub locked: bool,   // master && ключ не загружен
}
```

- [ ] **Step 2: commands/security.rs (диспетчер + настройки)**

Create `src-tauri/src/commands/security.rs`:
```rust
use crate::crypto;
use crate::error::{AppError, AppResult, ErrorKind};
use crate::models::CryptoStatus;
use crate::state::AppState;
use rusqlite::{params, Connection, OptionalExtension};
use std::sync::Mutex;
use tauri::State;

const VERIFY: &[u8] = b"devdeck-master-verify-v1";

pub fn get_setting(conn: &Connection, key: &str) -> AppResult<Option<String>> {
    Ok(conn.query_row("SELECT value FROM settings WHERE key=?1", [key], |r| r.get::<_, String>(0)).optional()?)
}
fn set_setting(conn: &Connection, key: &str, val: &str) -> AppResult<()> {
    conn.execute("INSERT INTO settings(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=?2", params![key, val])?;
    Ok(())
}
fn del_setting(conn: &Connection, key: &str) -> AppResult<()> {
    conn.execute("DELETE FROM settings WHERE key=?1", [key])?;
    Ok(())
}

pub fn current_mode(conn: &Connection) -> String {
    get_setting(conn, "crypto_mode").ok().flatten().unwrap_or_else(|| "dpapi".to_string())
}

fn locked() -> AppError {
    AppError { kind: ErrorKind::Locked, message: "Секреты заблокированы — введите мастер-пароль".into() }
}

/// Зашифровать секрет согласно текущему режиму.
pub fn encrypt_secret(conn: &Connection, mk: &Mutex<Option<[u8; 32]>>, plain: &[u8]) -> AppResult<Vec<u8>> {
    if current_mode(conn) == "master" {
        let guard = mk.lock().map_err(|_| AppError::internal("master key mutex poisoned"))?;
        let key = guard.as_ref().ok_or_else(locked)?;
        crypto::aes_encrypt(key, plain)
    } else {
        crypto::encrypt(plain)
    }
}

/// Расшифровать секрет согласно текущему режиму.
pub fn decrypt_secret(conn: &Connection, mk: &Mutex<Option<[u8; 32]>>, blob: &[u8]) -> AppResult<Vec<u8>> {
    if current_mode(conn) == "master" {
        let guard = mk.lock().map_err(|_| AppError::internal("master key mutex poisoned"))?;
        let key = guard.as_ref().ok_or_else(locked)?;
        crypto::aes_decrypt(key, blob)
    } else {
        crypto::decrypt(blob)
    }
}
```

- [ ] **Step 3: Рефактор creds.rs** — заменить прямые вызовы крипто на диспетчер:
  - В `creds_create`: было `Some(crypto::encrypt(s.as_bytes())?)` → стало `Some(crate::commands::security::encrypt_secret(&conn, &state.master_key, s.as_bytes())?)`. (Удалить `use crate::crypto;`, если станет неиспользуемым; либо оставить — проверить предупреждения.)
  - В `creds_get_secret`: `crypto::decrypt(&b)?` → `crate::commands::security::decrypt_secret(&conn, &state.master_key, &b)?`.
  - В `creds_update`: ветка установки секрета `Some(crypto::encrypt(s.as_bytes())?)` → `Some(crate::commands::security::encrypt_secret(&conn, &state.master_key, s.as_bytes())?)`.
  > `state` доступен в команде (`state: State<AppState>`); `&state.master_key` — новый Mutex.

- [ ] **Step 4: Рефактор transfer.rs** — `build_export`/`import_doc` принимают `mk: &Mutex<Option<[u8;32]>>` и используют `security::{decrypt_secret,encrypt_secret}` вместо `crypto::{decrypt,encrypt}`:
  - Сигнатуры: `fn build_export(conn, mk, include_secrets)`, `fn import_doc(conn, mk, doc)`.
  - В `export_json`/`export_to_file`/`import_json` передавать `&state.master_key`.
  - В тестах `transfer` — передавать `&Mutex::new(None)` (в тестах режим dpapi по умолчанию, ключ не нужен): обновить вызовы `build_export(&src, &Mutex::new(None), true)` и `import_doc(&dst, &Mutex::new(None), &parsed)` (добавить `use std::sync::Mutex;` в тестовый модуль).

- [ ] **Step 5: Регистрация модуля** — `commands/mod.rs`: `pub mod security;`

- [ ] **Step 6: AppState с master_key в lib.rs** — где создаётся AppState:
```rust
            app.manage(AppState { db: Mutex::new(conn), master_key: Mutex::new(None) });
```

- [ ] **Step 7: lib-сборка**
```powershell
cargo build --manifest-path src-tauri/Cargo.toml --lib
cargo test --manifest-path src-tauri/Cargo.toml --lib
```
Expected: компилируется; тесты (включая обновлённые transfer) зелёные.

---

## Task 3: Rust — команды master_* + crypto_status

**Files:** Modify `commands/security.rs`, `lib.rs`.

- [ ] **Step 1: Команды в security.rs** — добавить:
```rust
fn re_encrypt_all<F>(conn: &Connection, mut transform: F) -> AppResult<()>
where F: FnMut(&[u8]) -> AppResult<Vec<u8>> {
    let rows: Vec<(i64, Vec<u8>)> = {
        let mut stmt = conn.prepare("SELECT id, secret_encrypted FROM credentials WHERE secret_encrypted IS NOT NULL AND length(secret_encrypted)>0")?;
        stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Vec<u8>>(1)?)))?.collect::<Result<_, _>>()?
    };
    for (id, blob) in rows {
        let newblob = transform(&blob)?;
        conn.execute("UPDATE credentials SET secret_encrypted=?2 WHERE id=?1", params![id, newblob])?;
    }
    Ok(())
}

#[tauri::command]
pub fn crypto_status(state: State<AppState>) -> AppResult<CryptoStatus> {
    let conn = state.db.lock().map_err(|_| AppError::internal("db mutex poisoned"))?;
    let mode = current_mode(&conn);
    let locked = if mode == "master" {
        state.master_key.lock().map_err(|_| AppError::internal("mk poisoned"))?.is_none()
    } else { false };
    Ok(CryptoStatus { mode, locked })
}

/// Включить мастер-пароль: перешифровать все секреты DPAPI→AES, сохранить соль+verifier, разблокировать.
#[tauri::command]
pub fn master_enable(state: State<AppState>, password: String) -> AppResult<()> {
    if password.trim().len() < 4 {
        return Err(AppError { kind: ErrorKind::Validation, message: "Пароль слишком короткий (мин. 4)".into() });
    }
    let conn = state.db.lock().map_err(|_| AppError::internal("db mutex poisoned"))?;
    if current_mode(&conn) == "master" {
        return Err(AppError { kind: ErrorKind::Validation, message: "Мастер-пароль уже включён".into() });
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
    *state.master_key.lock().map_err(|_| AppError::internal("mk poisoned"))? = Some(key);
    Ok(())
}

/// Выключить мастер-пароль: перешифровать AES→DPAPI, очистить настройки, заблокировать ключ.
#[tauri::command]
pub fn master_disable(state: State<AppState>, password: String) -> AppResult<()> {
    let conn = state.db.lock().map_err(|_| AppError::internal("db mutex poisoned"))?;
    if current_mode(&conn) != "master" {
        return Err(AppError { kind: ErrorKind::Validation, message: "Мастер-пароль не включён".into() });
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
    *state.master_key.lock().map_err(|_| AppError::internal("mk poisoned"))? = None;
    Ok(())
}

fn verify_password(conn: &Connection, password: &str) -> AppResult<[u8; 32]> {
    let salt_hex = get_setting(conn, "kdf_salt")?.ok_or_else(|| AppError::internal("нет соли"))?;
    let ver_hex = get_setting(conn, "verifier")?.ok_or_else(|| AppError::internal("нет verifier"))?;
    let salt = hex::decode(salt_hex).map_err(|_| AppError::internal("плохая соль"))?;
    let verifier = hex::decode(ver_hex).map_err(|_| AppError::internal("плохой verifier"))?;
    let key = crypto::derive_key(password, &salt)?;
    match crypto::aes_decrypt(&key, &verifier) {
        Ok(v) if v == VERIFY => Ok(key),
        _ => Err(AppError { kind: ErrorKind::Validation, message: "Неверный мастер-пароль".into() }),
    }
}

/// Разблокировать: проверить пароль, положить ключ в память.
#[tauri::command]
pub fn master_unlock(state: State<AppState>, password: String) -> AppResult<()> {
    let conn = state.db.lock().map_err(|_| AppError::internal("db mutex poisoned"))?;
    let key = verify_password(&conn, &password)?;
    *state.master_key.lock().map_err(|_| AppError::internal("mk poisoned"))? = Some(key);
    Ok(())
}

/// Заблокировать (забыть ключ).
#[tauri::command]
pub fn master_lock(state: State<AppState>) -> AppResult<()> {
    *state.master_key.lock().map_err(|_| AppError::internal("mk poisoned"))? = None;
    Ok(())
}
```
> `conn.unchecked_transaction()` в rusqlite даёт транзакцию при `&Connection` (не `&mut`). Если недоступно — обернуть в `BEGIN`/`COMMIT` через `execute_batch` или убрать транзакцию (перешифровка идемпотентна по полю). Привести к компилирующемуся варианту.

- [ ] **Step 2: Тест полного цикла** — добавить в `security.rs` `#[cfg(test)] mod tests`:
```rust
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
        conn.execute("INSERT INTO projects(name,status,sort_order) VALUES('P','active',0)", []).unwrap();
        let pid = conn.last_insert_rowid();
        // секрет в режиме DPAPI
        let dpapi_blob = crypto::encrypt(b"my-secret").unwrap();
        conn.execute("INSERT INTO credentials(project_id,label,type,secret_encrypted,sort_order) VALUES(?1,'C','token',?2,0)", params![pid, dpapi_blob]).unwrap();

        // включаем master вручную (как master_enable, без State)
        let salt = crypto::random_salt();
        let key = crypto::derive_key("pw1234", &salt).unwrap();
        re_encrypt_all(&conn, |blob| { let p = crypto::decrypt(blob)?; crypto::aes_encrypt(&key, &p) }).unwrap();
        set_setting(&conn, "crypto_mode", "master").unwrap();
        set_setting(&conn, "kdf_salt", &hex::encode(salt)).unwrap();
        set_setting(&conn, "verifier", &hex::encode(crypto::aes_encrypt(&key, VERIFY).unwrap())).unwrap();

        // verify_password верный/неверный
        assert!(verify_password(&conn, "pw1234").is_ok());
        assert!(verify_password(&conn, "wrong").is_err());

        // decrypt_secret в master-режиме с ключом → исходный секрет
        let mk = Mutex::new(Some(key));
        let stored: Vec<u8> = conn.query_row("SELECT secret_encrypted FROM credentials", [], |r| r.get(0)).unwrap();
        let plain = decrypt_secret(&conn, &mk, &stored).unwrap();
        assert_eq!(plain, b"my-secret");

        // без ключа (locked) → ошибка
        let locked_mk = Mutex::new(None);
        assert!(decrypt_secret(&conn, &locked_mk, &stored).is_err());
    }
}
```

- [ ] **Step 3: Регистрация команд** — `lib.rs` `generate_handler![...]`: добавить
```rust
            commands::security::crypto_status,
            commands::security::master_enable,
            commands::security::master_disable,
            commands::security::master_unlock,
            commands::security::master_lock,
```

- [ ] **Step 4: Тесты + lib-сборка**
```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib
cargo build --manifest-path src-tauri/Cargo.toml --lib
```
Expected: новые крипто- и security-тесты + прежние → ~28 ok; lib-сборка успешна.

- [ ] **Step 5: Commit (весь бэкенд master-пароля)**
```powershell
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/crypto.rs src-tauri/src/error.rs src-tauri/src/state.rs src-tauri/src/models.rs src-tauri/src/commands/security.rs src-tauri/src/commands/creds.rs src-tauri/src/commands/transfer.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs
git commit -m @'
feat(backend): master-password mode (Argon2id + AES-256-GCM), mode-aware secret crypto

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

## Task 4: Фронт — секция «Безопасность» + обработка блокировки

**Files:** Modify `types.ts`, `components/AppSettings.svelte`, `components/CredsTab.svelte`; Create `api/security.ts`.

- [ ] **Step 1: Тип** (в конец `types.ts`):
```ts
export type CryptoStatus = { mode: "dpapi" | "master"; locked: boolean };
```

- [ ] **Step 2: api/security.ts**
```ts
import { call } from "./client";
import type { CryptoStatus } from "../types";

export const status = () => call<CryptoStatus>("crypto_status");
export const enable = (password: string) => call<void>("master_enable", { password });
export const disable = (password: string) => call<void>("master_disable", { password });
export const unlock = (password: string) => call<void>("master_unlock", { password });
export const lock = () => call<void>("master_lock");
```

- [ ] **Step 3: Секция «Безопасность» в AppSettings.svelte** — добавить в `<script>`:
```ts
  import * as security from "$lib/api/security";
  import type { CryptoStatus } from "$lib/types";

  let crypto = $state<CryptoStatus>({ mode: "dpapi", locked: false });
  let pwd = $state("");
  let pwd2 = $state("");
  async function refreshCrypto() { try { crypto = await security.status(); } catch { /* */ } }
```
Дополнить существующий `$effect(() => { if ($showSettings) refreshBackups(); })` вызовом `refreshCrypto()`.
Функции:
```ts
  async function enableMaster() {
    if (pwd.length < 4) { pushToast("Пароль короткий", "минимум 4 символа", "error"); return; }
    if (pwd !== pwd2) { pushToast("Пароли не совпадают", "", "error"); return; }
    busy = true;
    try { await security.enable(pwd); pwd = ""; pwd2 = ""; pushToast("Мастер-пароль включён", "секреты перешифрованы", "ok"); await refreshCrypto(); }
    finally { busy = false; }
  }
  async function unlockMaster() {
    busy = true;
    try { await security.unlock(pwd); pwd = ""; pushToast("Разблокировано", "", "ok"); await refreshCrypto(); }
    finally { busy = false; }
  }
  async function disableMaster() {
    busy = true;
    try { await security.disable(pwd); pwd = ""; pushToast("Мастер-пароль выключен", "секреты на DPAPI", "ok"); await refreshCrypto(); }
    finally { busy = false; }
  }
  async function lockMaster() { await security.lock(); await refreshCrypto(); pushToast("Заблокировано", "", "info"); }
```
И блок разметки (внутри `.modal-body`, например первым разделом):
```svelte
        <div class="field">
          <label>Безопасность секретов</label>
          <div style="color:var(--muted);font-size:12px;margin-bottom:8px">
            Режим: <b style="color:var(--text)">{crypto.mode === "master" ? "Мастер-пароль" : "DPAPI (Windows)"}</b>
            {#if crypto.mode === "master"} · {crypto.locked ? "🔒 заблокировано" : "🔓 разблокировано"}{/if}
          </div>

          {#if crypto.mode === "dpapi"}
            <div style="display:flex;flex-direction:column;gap:6px">
              <input class="tin" type="password" placeholder="Новый мастер-пароль" bind:value={pwd} />
              <input class="tin" type="password" placeholder="Повторите пароль" bind:value={pwd2} />
              <div><button class="btn-ghost" disabled={busy} onclick={enableMaster}><Icon name="lock" class="ic-sm" /> Включить мастер-пароль</button></div>
              <small style="color:var(--muted-2)">Секреты будут шифроваться AES-256-GCM ключом из пароля (Argon2id). Забытый пароль = секреты не восстановить.</small>
            </div>
          {:else if crypto.locked}
            <div style="display:flex;gap:6px;align-items:center">
              <input class="tin" type="password" placeholder="Мастер-пароль" bind:value={pwd} onkeydown={(e) => { if (e.key==='Enter') unlockMaster(); }} />
              <button class="btn-primary" disabled={busy} onclick={unlockMaster}><Icon name="lock-open" class="ic-sm" /> Разблокировать</button>
            </div>
          {:else}
            <div style="display:flex;flex-direction:column;gap:6px">
              <button class="btn-ghost" onclick={lockMaster}><Icon name="lock" class="ic-sm" /> Заблокировать сейчас</button>
              <div style="display:flex;gap:6px;align-items:center">
                <input class="tin" type="password" placeholder="Текущий пароль" bind:value={pwd} />
                <button class="btn-danger" disabled={busy} onclick={disableMaster}>Выключить мастер-пароль</button>
              </div>
            </div>
          {/if}
        </div>
```

- [ ] **Step 4: CredsTab — обработка Locked** — в `src/lib/components/CredsTab.svelte` `toggleReveal`/`copyTheSecret` оборачивают `creds.getSecret(id)`; при ошибке с `kind==='locked'` показать подсказку. Поскольку `call` уже бросает и показывает тост, добавить специальную обработку: обернуть вызовы getSecret в try/catch и при `(err as {kind?:string}).kind === 'locked'` показать тост «Разблокируйте мастер-паролем в Настройках» (вместо общего). Минимально — обернуть:
```ts
  async function getSecretSafe(id: number): Promise<string | null> {
    try { return await creds.getSecret(id); }
    catch (e) {
      if ((e as { kind?: string }).kind === "locked") pushToast("Заблокировано", "Разблокируйте мастер-паролем в Настройках", "error");
      return null;
    }
  }
```
> Внимание: `call` в `client.ts` показывает общий тост на reject. Чтобы избежать двойного тоста, можно в `client.ts` НЕ тостить для `kind==='locked'` (пропускать), а компонент покажет свой. Реализовать: в `api/client.ts` в catch — `if ((e as any)?.kind !== 'locked') pushToast(...)`. Тогда locked-тост показывает только компонент. Импортировать `pushToast` в CredsTab уже есть (через clipboard? нет — добавить `import { pushToast } from "$lib/stores/toasts";`). Заменить `toggleReveal`/`copyTheSecret` на использование `getSecretSafe` (если вернул null — ничего не делать).

- [ ] **Step 5: Проверка**
```powershell
npm run check
npm test
npm run build
```
Expected: 0 ошибок типов; Vitest 3; build успешен.

- [ ] **Step 6: Commit**
```powershell
git add src/lib/types.ts src/lib/api/security.ts src/lib/api/client.ts src/lib/components/AppSettings.svelte src/lib/components/CredsTab.svelte
git commit -m @'
feat(frontend): master-password UI (enable/disable/unlock) and locked-secret handling

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

## Task 5: Проверка среза

- [ ] **Step 1: Автопроверка**
```powershell
npm run check ; npm test ; cargo test --manifest-path src-tauri/Cargo.toml --lib
```
Expected: типы 0; Vitest 3; Rust ~28.

- [ ] **Step 2: Ручной GUI-смоук (пользователь; перезапустить `npm run tauri dev`)**
- [ ] Настройки → «Безопасность»: режим DPAPI. Создать кред с секретом, проверить показать/копировать (работает).
- [ ] Включить мастер-пароль (пароль + повтор) → тост «перешифрованы». Показать секрет — работает (разблокировано после включения).
- [ ] «Заблокировать сейчас» → попытка показать секрет → подсказка «разблокируйте». Разблокировать паролем → секрет снова доступен.
- [ ] **Перезапуск приложения** → режим master, заблокировано; показать секрет → подсказка; разблокировать → работает; секрет тот же (перешифровка корректна).
- [ ] Неверный пароль при разблокировке → «Неверный мастер-пароль».
- [ ] Выключить мастер-пароль (текущий пароль) → режим DPAPI; секрет снова доступен без пароля.

---

## Итог среза

Полноценный режим мастер-пароля: AES-256-GCM + Argon2id, переключаемый в настройках с перешифровкой секретов, разблокировка при доступе к секрету (приложение работает и заблокированным). Завершает v1.0. Отложено: смена пароля без выключения, таймаут авто-блокировки, индикатор замка в шапке кредов.
```
