# DevDeck v1.0 — Срез «Авто-бэкап БД» — план реализации

> **Для исполнителя:** ОБЯЗАТЕЛЬНЫЙ ПОД-НАВЫК: superpowers:subagent-driven-development. Шаги — чекбоксы.

**Цель:** Безопасность данных: при старте приложения создаётся консистентная копия `devdeck.db` в папке `backups/` (не чаще ~раза в сутки), хранятся последние N копий. Плюс команды `backup_now`/`backups_list` (UI — в срезе экспорта/импорта).

**Архитектура:** Модуль `backup`: `make_backup(conn, dir)` через `VACUUM INTO` (консистентный снимок без копирования полуоткрытого файла), `prune(dir, keep)`, `maybe_auto_backup(conn, dir)` (пропуск, если свежий бэкап моложе ~20 ч). Авто-бэкап вызывается в `setup()` на открытом соединении. Команды `backup_now`/`backups_list` — для будущего UI.

**Стек:** как раньше. Без миграций, без фронта.

**Решения / границы:**
- Снимок через `VACUUM INTO` (а не `fs::copy`) — гарантирует целостность БД.
- Имя файла: `devdeck-backup-<epoch>-<seq>.db` (seq — атомарный счётчик для уникальности). Логика «свежести» — по mtime файлов.
- Хранить последние **N=10**.
- UI (кнопки «Бэкап сейчас», список) — в срезе экспорта/импорта.

**Источники:** `TZ_DevDeck.md` (8 — хранение/бэкап; 10 — `backup_now`). Phase 0 уже создаёт папку `backups` в `setup`. `state::AppState` (`Mutex<Connection>`), `db::open`.

---

## Контекст

- Rust: `lib.rs` `setup` создаёт каталог данных (`app.path().app_data_dir()`), папку `backups`, открывает БД (`db::open(&dir.join("devdeck.db"))`), кладёт `AppState`, строит трей. 56 команд.
- `error::{AppError, AppResult, ErrorKind}`, `state::AppState`.

---

## Структура файлов

```
src-tauri/src/
├─ backup.rs            # NEW: make_backup / prune / maybe_auto_backup + тест
├─ models.rs            # MOD: + BackupInfo
├─ commands/backup.rs   # NEW: backup_now / backups_list
├─ commands/mod.rs      # MOD: + pub mod backup;
└─ lib.rs               # MOD: mod backup; авто-бэкап в setup; регистрация команд
```

---

## Task 1: Rust — модуль бэкапа + авто-бэкап + команды

**Files:** Create `src-tauri/src/backup.rs`, `src-tauri/src/commands/backup.rs`; Modify `models.rs`, `commands/mod.rs`, `lib.rs`.

- [ ] **Step 1: backup.rs**

Create `src-tauri/src/backup.rs`:
```rust
use crate::error::{AppError, AppResult, ErrorKind};
use rusqlite::Connection;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const KEEP: usize = 10;
const PREFIX: &str = "devdeck-backup-";
static SEQ: AtomicU64 = AtomicU64::new(0);

fn epoch_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Список файлов бэкапов в папке (полные пути).
fn list_files(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with(PREFIX) && n.ends_with(".db"))
                .unwrap_or(false)
        })
        .collect();
    // новые сверху (по mtime)
    v.sort_by_key(|p| std::cmp::Reverse(mtime(p)));
    v
}

fn mtime(p: &Path) -> SystemTime {
    std::fs::metadata(p).and_then(|m| m.modified()).unwrap_or(UNIX_EPOCH)
}

/// Удалить старые бэкапы сверх `keep` (оставить самые свежие).
fn prune(dir: &Path, keep: usize) {
    let files = list_files(dir);
    for old in files.into_iter().skip(keep) {
        let _ = std::fs::remove_file(old);
    }
}

/// Сделать консистентный снимок БД через VACUUM INTO. Возвращает путь копии.
pub fn make_backup(conn: &Connection, dir: &Path) -> AppResult<PathBuf> {
    std::fs::create_dir_all(dir).map_err(|e| AppError { kind: ErrorKind::Io, message: format!("Папка бэкапов: {}", e) })?;
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    let dest = dir.join(format!("{}{}-{}.db", PREFIX, epoch_secs(), seq));
    // VACUUM INTO принимает строковый литерал — путь контролируется приложением,
    // экранируем одинарные кавычки на всякий случай.
    let path_str = dest.to_string_lossy().replace('\'', "''");
    conn.execute_batch(&format!("VACUUM INTO '{}';", path_str))?;
    prune(dir, KEEP);
    Ok(dest)
}

/// Авто-бэкап при старте: пропустить, если самый свежий бэкап моложе ~20 часов.
pub fn maybe_auto_backup(conn: &Connection, dir: &Path) {
    let recent = list_files(dir)
        .first()
        .map(|p| mtime(p))
        .and_then(|t| SystemTime::now().duration_since(t).ok())
        .map(|age| age < Duration::from_secs(20 * 3600))
        .unwrap_or(false);
    if !recent {
        let _ = make_backup(conn, dir); // тихо: ошибка бэкапа не должна валить старт
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    #[test]
    fn make_backup_creates_file_and_prunes() {
        let conn = Connection::open_in_memory().unwrap();
        db::migrations::run(&conn).unwrap();
        let dir = std::env::temp_dir().join(format!("devdeck_bk_{}_{}", std::process::id(), epoch_secs()));
        let _ = std::fs::remove_dir_all(&dir);

        // создаём больше KEEP бэкапов
        for _ in 0..(KEEP + 3) {
            make_backup(&conn, &dir).unwrap();
        }
        let files = list_files(&dir);
        assert_eq!(files.len(), KEEP, "должно остаться ровно KEEP бэкапов");
        // каждый бэкап — валидная SQLite-БД с таблицей projects
        let any = &files[0];
        let bk = Connection::open(any).unwrap();
        let cnt: i64 = bk
            .query_row("SELECT count(*) FROM sqlite_master WHERE type='table' AND name='projects'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(cnt, 1);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
```

- [ ] **Step 2: Модель** (в `models.rs`, после `AttentionItem`):
```rust
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupInfo {
    pub name: String,
    pub size_bytes: u64,
    pub created_epoch: u64,
}
```

- [ ] **Step 3: commands/backup.rs**

Create `src-tauri/src/commands/backup.rs`:
```rust
use crate::backup;
use crate::error::{AppError, AppResult, ErrorKind};
use crate::models::BackupInfo;
use crate::state::AppState;
use std::time::UNIX_EPOCH;
use tauri::{Manager, State};

fn backups_dir(app: &tauri::AppHandle) -> AppResult<std::path::PathBuf> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError { kind: ErrorKind::Io, message: format!("Каталог данных: {}", e) })?
        .join("backups");
    Ok(dir)
}

#[tauri::command]
pub fn backup_now(state: State<AppState>, app: tauri::AppHandle) -> AppResult<String> {
    let dir = backups_dir(&app)?;
    let conn = state.db.lock().map_err(|_| AppError::internal("db mutex poisoned"))?;
    let path = backup::make_backup(&conn, &dir)?;
    Ok(path.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn backups_list(app: tauri::AppHandle) -> AppResult<Vec<BackupInfo>> {
    let dir = backups_dir(&app)?;
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for e in rd.flatten() {
            let p = e.path();
            let name = match p.file_name().and_then(|n| n.to_str()) {
                Some(n) if n.starts_with("devdeck-backup-") && n.ends_with(".db") => n.to_string(),
                _ => continue,
            };
            let meta = std::fs::metadata(&p).ok();
            let size_bytes = meta.as_ref().map(|m| m.len()).unwrap_or(0);
            let created_epoch = meta
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            out.push(BackupInfo { name, size_bytes, created_epoch });
        }
    }
    out.sort_by(|a, b| b.created_epoch.cmp(&a.created_epoch));
    Ok(out)
}
```

- [ ] **Step 4: Подключить модуль и авто-бэкап в lib.rs**

В `src-tauri/src/lib.rs`:
1. Добавить `mod backup;` (рядом с `mod crypto; mod db; ...`).
2. В `setup`, ПОСЛЕ открытия соединения и СОЗДАНИЯ папки `backups`, но до/после `app.manage(AppState{...})` — вызвать авто-бэкап на соединении ДО передачи его в AppState. Поскольку соединение перемещается в `AppState`, сделать бэкап до `manage`. Пример: если сейчас
```rust
            let conn = db::open(&dir.join("devdeck.db")).map_err(...)?;
            app.manage(AppState { db: Mutex::new(conn) });
```
заменить на
```rust
            let conn = db::open(&dir.join("devdeck.db")).map_err(|e| format!("db init failed: {}", e.message))?;
            backup::maybe_auto_backup(&conn, &dir.join("backups"));
            app.manage(AppState { db: Mutex::new(conn) });
```
(папка `backups` уже создаётся выше через `create_dir_all`).

- [ ] **Step 5: Регистрация**
- `commands/mod.rs`: `pub mod backup;`
- `lib.rs` `generate_handler![...]`: добавить
```rust
            commands::backup::backup_now,
            commands::backup::backups_list,
```

- [ ] **Step 6: Тесты + сборка**
```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib
cargo build --manifest-path src-tauri/Cargo.toml --lib
```
Expected: новый `make_backup_creates_file_and_prunes` + прежние (20) → 21 ok; lib-сборка успешна. (Полная сборка `.exe` может быть заблокирована, если запущено приложение — это ок, проверяем `--lib`.)

- [ ] **Step 7: Commit**
```powershell
git add src-tauri/src/backup.rs src-tauri/src/models.rs src-tauri/src/commands/backup.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs
git commit -m @'
feat(backend): auto-backup on startup + backup_now/backups_list (VACUUM INTO)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

## Task 2: Проверка среза

- [ ] **Step 1: Автопроверка**
```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib ; npm run check
```
Expected: Rust 21; типы 0 ошибок (фронт не менялся).

- [ ] **Step 2: Ручной GUI-смоук (пользователь; перезапустить `npm run tauri dev`)**
- [ ] После первого запуска в `%APPDATA%\com.devdeck.app\backups\` появился файл `devdeck-backup-<...>.db`.
- [ ] Повторный запуск в тот же день — новый бэкап НЕ создаётся (свежий моложе 20 ч).
- [ ] (Опц., из DevTools) `await window.__TAURI__.core.invoke("backup_now")` → возвращает путь, в папке появился ещё файл; `backups_list` → массив с именами/размерами/датами.
- [ ] После >10 бэкапов в папке остаётся 10 самых свежих.

---

## Итог среза

При старте создаётся консистентный снимок БД (VACUUM INTO) не чаще раза в сутки, хранятся 10 последних; команды `backup_now`/`backups_list` готовы для UI. Дальше — экспорт/импорт JSON (+ панель «Данные» с кнопками бэкапа), затем шаблоны чеклистов и режим мастер-пароля.
```
