# DevDeck v1.0 — Срез «Экспорт/импорт JSON» — план реализации

> **Для исполнителя:** ОБЯЗАТЕЛЬНЫЙ ПОД-НАВЫК: superpowers:subagent-driven-development. Шаги — чекбоксы.

**Цель:** Выгрузка всех данных в JSON (по умолчанию без секретов; опционально — с расшифровкой по подтверждению) и загрузка из JSON (мерж — добавляет проекты). Панель «Данные» (модалка настроек приложения) с кнопками: бэкап сейчас, экспорт в файл, импорт (вставкой JSON).

**Архитектура:** Команды `export_json(include_secrets)` (строка JSON), `export_to_file(include_secrets)` (пишет в `exports/`, возвращает путь), `import_json(json)` (парсит + вставляет, мерж). Сериализация/вставка — `&Connection`-функции (тестируемые round-trip). Файловые диалоги НЕ используем (экспорт — в папку, импорт — вставка текста), чтобы не тащить плагины.

**Безопасность:** По умолчанию секреты НЕ экспортируются. С `include_secrets=true` — секрет расшифровывается DPAPI и кладётся в JSON в открытом виде (для переноса между ПК) — только по явному подтверждению пользователя в UI. При импорте секрет (если есть) заново шифруется локальным DPAPI.

**Стек:** + `serde_json` (если ещё не прямой dep). Без миграций. Файлы пишутся Rust'ом в `%APPDATA%\com.devdeck.app\exports\`.

**Решения / границы:**
- Импорт = **мерж** (добавляет проекты с новыми id), существующие данные не трогает.
- Экспорт в файл — фиксированная папка `exports/`; выбор пути диалогом — позже.
- Импорт — вставка JSON в textarea; выбор файла диалогом — позже.

**Источники:** `TZ_DevDeck.md` (8 — экспорт/импорт; 10 — `export_json`, `import_json`). Модели в `models.rs`, `crypto` (encrypt/decrypt), `commands/backup.rs` (как образец file-команд).

---

## Контекст

- Rust: 58 команд; `models.rs` (Project/Task/Checklist/ChecklistItem/Credential/Note/Link/FileShortcut/ProjectCommand...); `crypto::{encrypt,decrypt}`; таблицы всех сущностей. `serde`/`serde_json` доступны (tauri тянет; при необходимости добавить `serde_json` в `[dependencies]`).
- Фронт: сайдбар-футер с кнопкой темы (`Sidebar.svelte`). Стор `ui` (`showNewProject`, `showPalette`). `projects` стор (`loadProjects`). `Icon.svelte`. global.css: модалка, `.field`, `.tin`, `.set-card`, `.toggle`.

---

## Структура файлов

```
src-tauri/
├─ Cargo.toml                # MOD: + serde_json (если нет)
└─ src/
   ├─ models.rs              # MOD: + ImportSummary (+ export-DTO структуры в transfer.rs)
   ├─ commands/transfer.rs   # NEW: export_json/export_to_file/import_json + round-trip тест
   ├─ commands/mod.rs        # MOD: + pub mod transfer;
   └─ lib.rs                 # MOD: регистрация 3 команд

src/lib/
├─ types.ts                  # MOD: + BackupInfo, ImportSummary
├─ api/backup.ts             # NEW (backup_now/backups_list)
├─ api/transfer.ts           # NEW (exportJson/exportToFile/importJson)
├─ stores/ui.ts              # MOD: + showSettings
└─ components/
   ├─ AppSettings.svelte     # NEW: модалка «Данные»
   ├─ Sidebar.svelte         # MOD: шестерёнка → showSettings
   └─ ... (+page.svelte монтирует AppSettings)
src/routes/+page.svelte       # MOD: смонтировать AppSettings
```

---

## Task 1: Rust — экспорт/импорт

**Files:** Modify `Cargo.toml`, `models.rs`, `commands/mod.rs`, `lib.rs`; Create `commands/transfer.rs`.

- [ ] **Step 1: serde_json в зависимостях**

Убедиться, что в `src-tauri/Cargo.toml` `[dependencies]` есть `serde_json`. Если нет — добавить:
```toml
serde_json = "1"
```

- [ ] **Step 2: Модель результата импорта** (в `models.rs`, после `BackupInfo`):
```rust
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSummary {
    pub projects: usize,
}
```

- [ ] **Step 3: commands/transfer.rs**

Create `src-tauri/src/commands/transfer.rs`:
```rust
use crate::crypto;
use crate::error::{AppError, AppResult, ErrorKind};
use crate::models::ImportSummary;
use crate::state::AppState;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
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

fn build_export(conn: &Connection, include_secrets: bool) -> AppResult<ExportDoc> {
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
                    Some(b) if !b.is_empty() => Some(String::from_utf8_lossy(&crypto::decrypt(&b)?).into_owned()),
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

fn import_doc(conn: &Connection, doc: &ExportDoc) -> AppResult<usize> {
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
                Some(s) if !s.is_empty() => Some(crypto::encrypt(s.as_bytes())?),
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
    let doc = build_export(&conn, include_secrets)?;
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
    let n = import_doc(&conn, &doc)?;
    Ok(ImportSummary { projects: n })
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
    fn export_import_roundtrip_with_secret() {
        let src = mem();
        src.execute("INSERT INTO projects(name,status,sort_order) VALUES('Aurora','active',0)", []).unwrap();
        let pid = src.last_insert_rowid();
        src.execute("INSERT INTO tasks(project_id,title,status,sort_order) VALUES(?1,'T1','todo',0)", [pid]).unwrap();
        src.execute("INSERT INTO notes(project_id,title,content_md) VALUES(?1,'N','body')", [pid]).unwrap();
        let blob = crypto::encrypt(b"shh").unwrap();
        src.execute("INSERT INTO credentials(project_id,label,type,secret_encrypted,sort_order) VALUES(?1,'Stripe','api_key',?2,0)", params![pid, blob]).unwrap();

        let doc = build_export(&src, true).unwrap();
        assert_eq!(doc.projects.len(), 1);
        assert_eq!(doc.projects[0].creds[0].secret.as_deref(), Some("shh"));

        let json = serde_json::to_string(&doc).unwrap();
        let parsed: ExportDoc = serde_json::from_str(&json).unwrap();

        let dst = mem();
        let n = import_doc(&dst, &parsed).unwrap();
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
        let doc = build_export(&src, false).unwrap();
        assert!(doc.projects[0].creds[0].secret.is_none());
    }
}
```

- [ ] **Step 4: Регистрация**
- `commands/mod.rs`: `pub mod transfer;`
- `lib.rs` `generate_handler![...]`: добавить
```rust
            commands::transfer::export_json,
            commands::transfer::export_to_file,
            commands::transfer::import_json,
```

- [ ] **Step 5: Тесты + lib-сборка**
```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib
cargo build --manifest-path src-tauri/Cargo.toml --lib
```
Expected: новые `export_import_roundtrip_with_secret`, `export_without_secrets_omits_them` + прежние (21) → 23 ok; lib-сборка успешна.

- [ ] **Step 6: Commit**
```powershell
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/models.rs src-tauri/src/commands/transfer.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs
git commit -m @'
feat(backend): JSON export/import (optional decrypted secrets, re-encrypt on import)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

## Task 2: Фронт — панель «Данные» (AppSettings)

**Files:** Modify `types.ts`, `stores/ui.ts`, `Sidebar.svelte`, `routes/+page.svelte`; Create `api/backup.ts`, `api/transfer.ts`, `components/AppSettings.svelte`.

- [ ] **Step 1: Типы** (в конец `types.ts`):
```ts
export type BackupInfo = { name: string; sizeBytes: number; createdEpoch: number };
export type ImportSummary = { projects: number };
```

- [ ] **Step 2: api**

Create `src/lib/api/backup.ts`:
```ts
import { call } from "./client";
import type { BackupInfo } from "../types";
export const backupNow = () => call<string>("backup_now");
export const backupsList = () => call<BackupInfo[]>("backups_list");
```

Create `src/lib/api/transfer.ts`:
```ts
import { call } from "./client";
import type { ImportSummary } from "../types";
export const exportJson = (includeSecrets: boolean) => call<string>("export_json", { includeSecrets });
export const exportToFile = (includeSecrets: boolean) => call<string>("export_to_file", { includeSecrets });
export const importJson = (json: string) => call<ImportSummary>("import_json", { json });
```

- [ ] **Step 3: ui-стор** — в `stores/ui.ts` добавить `export const showSettings = writable(false);`

- [ ] **Step 4: AppSettings.svelte**

Create `src/lib/components/AppSettings.svelte`:
```svelte
<script lang="ts">
  import { showSettings } from "$lib/stores/ui";
  import { loadProjects } from "$lib/stores/projects";
  import { pushToast } from "$lib/stores/toasts";
  import * as backup from "$lib/api/backup";
  import * as transfer from "$lib/api/transfer";
  import type { BackupInfo } from "$lib/types";
  import Icon from "./Icon.svelte";

  let backups = $state<BackupInfo[]>([]);
  let includeSecrets = $state(false);
  let importText = $state("");
  let busy = $state(false);

  $effect(() => {
    if ($showSettings) refreshBackups();
  });
  async function refreshBackups() {
    try { backups = await backup.backupsList(); } catch { /* */ }
  }

  async function doBackup() {
    busy = true;
    try { const p = await backup.backupNow(); pushToast("Бэкап создан", p, "ok"); await refreshBackups(); }
    finally { busy = false; }
  }
  async function doExport() {
    busy = true;
    try { const p = await transfer.exportToFile(includeSecrets); pushToast("Экспортировано", p, "ok"); }
    finally { busy = false; }
  }
  async function doImport() {
    if (!importText.trim()) return;
    busy = true;
    try {
      const r = await transfer.importJson(importText);
      pushToast("Импортировано", `${r.projects} проект(ов)`, "ok");
      importText = "";
      await loadProjects();
    } finally { busy = false; }
  }
</script>

{#if $showSettings}
  <div class="modal-scrim open" role="dialog" tabindex="-1" aria-label="Настройки"
       onmousedown={(e) => { if (e.currentTarget === e.target) showSettings.set(false); }}
       onkeydown={(e) => { if (e.key === 'Escape') showSettings.set(false); }}>
    <div class="modal" style="max-width:560px">
      <div class="modal-head">
        <span class="mh-ico"><Icon name="settings" class="ic" /></span>
        <span class="t">Настройки · Данные</span>
        <button class="icon-btn x" onclick={() => showSettings.set(false)}><Icon name="x" class="ic" /></button>
      </div>
      <div class="modal-body">
        <div class="field">
          <label>Бэкап</label>
          <div style="display:flex;align-items:center;gap:10px">
            <button class="btn-ghost" disabled={busy} onclick={doBackup}><Icon name="database-backup" class="ic-sm" /> Сделать бэкап</button>
            <span style="color:var(--muted);font-size:12px">{backups.length} копий в backups/</span>
          </div>
        </div>

        <div class="field">
          <label>Экспорт</label>
          <div class="toggle-row" style="margin-bottom:8px">
            <button class="toggle" class:on={includeSecrets} onclick={() => (includeSecrets = !includeSecrets)} aria-pressed={includeSecrets} aria-label="секреты"></button>
            <div class="tl">Включить секреты<small>Расшифровать и положить в файл в открытом виде — только для переноса на доверенный ПК.</small></div>
          </div>
          <button class="btn-ghost" disabled={busy} onclick={doExport}><Icon name="download" class="ic-sm" /> Экспорт в файл (exports/)</button>
        </div>

        <div class="field">
          <label for="imp">Импорт (вставьте JSON)</label>
          <textarea id="imp" class="tin mono" style="min-height:120px" placeholder={'{ "version": 1, "projects": [...] }'} bind:value={importText}></textarea>
          <div style="margin-top:8px"><button class="btn-primary" disabled={busy || !importText.trim()} onclick={doImport}><Icon name="upload" class="ic-sm" /> Импортировать</button></div>
        </div>
      </div>
      <div class="modal-foot">
        <span class="spacer"></span>
        <button class="btn-ghost" onclick={() => showSettings.set(false)}>Закрыть</button>
      </div>
    </div>
  </div>
{/if}
```
> Иконки `settings`/`x`/`database-backup`/`download`/`upload` — из lucide. Классы модалки/`.toggle`/`.field`/`.tin` — в global.css.

- [ ] **Step 5: Шестерёнка в Sidebar.svelte**

В `src/lib/components/Sidebar.svelte`: импортировать `import { showSettings } from "$lib/stores/ui";` и в `.sb-foot` добавить кнопку рядом с кнопкой темы:
```svelte
    <button class="icon-btn" onclick={() => showSettings.set(true)} title="Настройки"><Icon name="settings" class="ic" /></button>
```

- [ ] **Step 6: Смонтировать AppSettings в +page.svelte**
Импорт `import AppSettings from "$lib/components/AppSettings.svelte";` и рядом с `<CommandPalette />` добавить `<AppSettings />`.

- [ ] **Step 7: Проверка**
```powershell
npm run check
npm test
npm run build
```
Expected: 0 ошибок типов; Vitest 3; build успешен.

- [ ] **Step 8: Commit**
```powershell
git add src/lib/types.ts src/lib/api/backup.ts src/lib/api/transfer.ts src/lib/stores/ui.ts src/lib/components/AppSettings.svelte src/lib/components/Sidebar.svelte src/routes/+page.svelte
git commit -m @'
feat(frontend): app settings panel with backup, JSON export and import

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

## Task 3: Проверка среза

- [ ] **Step 1: Автопроверка**
```powershell
npm run check ; npm test ; cargo test --manifest-path src-tauri/Cargo.toml --lib
```
Expected: типы 0 ошибок; Vitest 3; Rust 23.

- [ ] **Step 2: Ручной GUI-смоук (пользователь; перезапустить `npm run tauri dev`)**
- [ ] Шестерёнка в подвале сайдбара → модалка «Настройки · Данные».
- [ ] «Сделать бэкап» → тост с путём; счётчик копий вырос.
- [ ] «Экспорт в файл» (без секретов) → тост с путём; файл `devdeck-export-*.json` в `%APPDATA%\com.devdeck.app\exports\`, секреты НЕ в нём.
- [ ] Включить тумблер «секреты» → экспорт → в JSON секреты в открытом виде (проверить файл).
- [ ] Импорт: вставить экспортированный JSON → «Импортировать» → тост «N проектов»; проекты появились в сайдбаре (мерж, дубликаты добавились).
- [ ] Импорт с секретами → у кредов секрет работает (показать/копировать).

---

## Итог среза

Полный экспорт данных в JSON (опционально с расшифрованными секретами по подтверждению) и импорт-мерж; панель «Данные» с бэкапом/экспортом/импортом. Секреты при импорте заново шифруются локальным DPAPI. Дальше — шаблоны чеклистов, затем режим мастер-пароля. Отложено: файловые диалоги (выбор пути/файла), импорт-замена (сейчас только мерж).
```
