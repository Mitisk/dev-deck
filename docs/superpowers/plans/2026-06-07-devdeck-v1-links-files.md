# DevDeck v1.0 — Срез «Ссылки + файлы-ярлыки» — план реализации

> **Для исполнителя:** ОБЯЗАТЕЛЬНЫЙ ПОД-НАВЫК: superpowers:subagent-driven-development. Шаги — чекбоксы.

**Цель:** Вкладка «Обзор» оживает: описание проекта + раздел «Ссылки» (именованные закладки, открываются в браузере) + раздел «Файлы» (ярлыки на файлы/папки, открываются ассоциированной программой/в проводнике). Добавление/удаление прямо в обзоре.

**Архитектура:** Команды `links_*` (таблица `links`) и `files_*` (новая таблица `files`, миграция 0002). Открытие ссылки — существующая `open_url`; открытие файла/папки — новая `open_shortcut(path)` через плагин opener (ShellExecute, без shell). Фронт: новый `OverviewTab.svelte` (описание + ссылки + файлы), заменяет плейсхолдер вкладки «Обзор».

**Стек:** как раньше. Новая миграция БД (0002).

**Решения / границы:**
- «Файлы» и «папки» — одна сущность `files` (путь может быть файлом или папкой); открываются через `open_shortcut` (opener сам выбирает обработчик: файл → программа, папка → проводник).
- Иконка ссылки — пока фикс (`globe`); пикер иконок — отложен. Файлы — фикс-иконка.
- Редактирование ссылок/файлов — инлайн в «Обзоре» (добавить-строка + удалить). Композитные «git-сводка / ближайшие задачи» в Обзоре — отдельные доработки позже.

**Источники:** `TZ_DevDeck.md` (5.8 — ссылки и файлы; 10 — `links_*`; раздел 4 — таблица `links`). `_prototype/index.html` (`.links`/`.link-row`/`overviewHTML` — разметка-референс). Текущий `ProjectView.svelte` (таб «overview» = `{:else}`-плейсхолдер), `actions.rs` (есть `expand_path`, `open_url`), `migrations.rs`.

---

## Контекст

- Rust: 40 команд; `migrations.rs` (`MIGRATIONS = [(1, 0001_init.sql)]`, тесты на версию схемы); `commands/actions.rs` (`expand_path` приватный, `open_url` через opener); `models.rs`; `error`. Таблица `links`: `id, project_id, label, url, icon, sort_order`. Таблицы `files` НЕТ — добавляем миграцией.
- Фронт: `ProjectView.svelte` — таб «overview» попадает в `{:else}`-плейсхолдер. api `client.ts`, `Icon.svelte`. global.css: `.links`/`.link-row`(`.lico`/`.lt`/`.lu`/`.ext`), `.section-title`, `.desc`, `.cmd-grid` (для команд позже), `.erow`/`.er-del` (редактируемые строки).

---

## Структура файлов

```
src-tauri/
├─ migrations/0002_files.sql   # NEW: таблица files + user_version=2
└─ src/
   ├─ db/migrations.rs         # MOD: + (2, 0002_files.sql); обновить тесты версии
   ├─ models.rs                # MOD: + Link, FileShortcut
   ├─ commands/links.rs        # NEW: links_*
   ├─ commands/files.rs        # NEW: files_*
   ├─ commands/actions.rs      # MOD: + open_shortcut
   ├─ commands/mod.rs          # MOD: + pub mod links; pub mod files;
   └─ lib.rs                   # MOD: регистрация команд

src/lib/
├─ types.ts                    # MOD: + Link, FileShortcut
├─ api/links.ts                # NEW
├─ api/files.ts                # NEW
├─ api/actions.ts              # MOD: + openShortcut
└─ components/
   ├─ OverviewTab.svelte       # NEW
   └─ ProjectView.svelte       # MOD: рендер OverviewTab на табе "overview"
```

---

## Task 1: Rust — миграция 0002 + ссылки/файлы + open_shortcut

**Files:** Create `migrations/0002_files.sql`, `commands/links.rs`, `commands/files.rs`; Modify `db/migrations.rs`, `models.rs`, `commands/actions.rs`, `commands/mod.rs`, `lib.rs`.

- [ ] **Step 1: Миграция 0002**

Create `src-tauri/migrations/0002_files.sql`:
```sql
-- Файлы/папки-ярлыки проекта (открываются ассоциированной программой/в проводнике).
CREATE TABLE files (
    id         INTEGER PRIMARY KEY,
    project_id INTEGER REFERENCES projects(id) ON DELETE CASCADE,
    label      TEXT NOT NULL,
    path       TEXT NOT NULL,
    sort_order INTEGER DEFAULT 0
);

PRAGMA user_version = 2;
```

- [ ] **Step 2: Подключить миграцию в раннер + обновить тесты**

В `src-tauri/src/db/migrations.rs`:
1. В `const MIGRATIONS` добавить вторую запись:
```rust
const MIGRATIONS: &[(i64, &str)] = &[
    (1, include_str!("../../migrations/0001_init.sql")),
    (2, include_str!("../../migrations/0002_files.sql")),
];
```
2. Обновить ожидаемую версию в тестах (теперь актуальная схема = 2):
   - в `applies_migrations_on_empty_db`: `assert_eq!(schema_version(&conn).unwrap(), 1);` → `2`.
   - в `run_is_idempotent`: финальный `assert_eq!(schema_version(&conn).unwrap(), 1);` → `2`.
   - Добавить проверку, что таблица `files` создана (рядом с проверкой `projects`):
```rust
        let files: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='files'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(files, 1);
```

- [ ] **Step 3: Модели** (в `models.rs`, после `Note`):
```rust
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Link {
    pub id: i64,
    pub project_id: i64,
    pub label: String,
    pub url: String,
    pub icon: Option<String>,
    pub sort_order: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileShortcut {
    pub id: i64,
    pub project_id: i64,
    pub label: String,
    pub path: String,
    pub sort_order: i64,
}
```

- [ ] **Step 4: commands/links.rs**

Create `src-tauri/src/commands/links.rs`:
```rust
use crate::error::{AppError, AppResult, ErrorKind};
use crate::models::Link;
use crate::state::AppState;
use rusqlite::{params, Connection};
use std::sync::MutexGuard;
use tauri::State;

fn lock<'a>(state: &'a State<AppState>) -> AppResult<MutexGuard<'a, Connection>> {
    state.db.lock().map_err(|_| AppError::internal("db mutex poisoned"))
}

fn row_to_link(conn: &Connection, id: i64) -> AppResult<Link> {
    Ok(conn.query_row(
        "SELECT id, project_id, label, url, icon, sort_order FROM links WHERE id = ?1",
        [id],
        |r| Ok(Link {
            id: r.get(0)?,
            project_id: r.get(1)?,
            label: r.get(2)?,
            url: r.get(3)?,
            icon: r.get(4)?,
            sort_order: r.get(5)?,
        }),
    )?)
}

#[tauri::command]
pub fn links_list(state: State<AppState>, project_id: i64) -> AppResult<Vec<Link>> {
    let conn = lock(&state)?;
    let mut stmt = conn.prepare(
        "SELECT id FROM links WHERE project_id = ?1 ORDER BY sort_order ASC, id ASC",
    )?;
    let ids = stmt.query_map([project_id], |r| r.get::<_, i64>(0))?;
    let mut out = Vec::new();
    for id in ids {
        out.push(row_to_link(&conn, id?)?);
    }
    Ok(out)
}

#[tauri::command]
pub fn links_create(state: State<AppState>, project_id: i64, label: String, url: String, icon: Option<String>) -> AppResult<Link> {
    if label.trim().is_empty() || url.trim().is_empty() {
        return Err(AppError { kind: ErrorKind::Validation, message: "Название и URL обязательны".into() });
    }
    let conn = lock(&state)?;
    let next: i64 = conn.query_row("SELECT COALESCE(MAX(sort_order),0)+1 FROM links WHERE project_id=?1", [project_id], |r| r.get(0))?;
    conn.execute(
        "INSERT INTO links(project_id, label, url, icon, sort_order) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![project_id, label.trim(), url.trim(), icon, next],
    )?;
    row_to_link(&conn, conn.last_insert_rowid())
}

#[tauri::command]
pub fn links_update(state: State<AppState>, id: i64, label: String, url: String, icon: Option<String>) -> AppResult<Link> {
    if label.trim().is_empty() || url.trim().is_empty() {
        return Err(AppError { kind: ErrorKind::Validation, message: "Название и URL обязательны".into() });
    }
    let conn = lock(&state)?;
    conn.execute(
        "UPDATE links SET label=?2, url=?3, icon=?4 WHERE id=?1",
        params![id, label.trim(), url.trim(), icon],
    )?;
    row_to_link(&conn, id)
}

#[tauri::command]
pub fn links_delete(state: State<AppState>, id: i64) -> AppResult<()> {
    let conn = lock(&state)?;
    conn.execute("DELETE FROM links WHERE id=?1", [id])?;
    Ok(())
}
```

- [ ] **Step 5: commands/files.rs**

Create `src-tauri/src/commands/files.rs`:
```rust
use crate::error::{AppError, AppResult, ErrorKind};
use crate::models::FileShortcut;
use crate::state::AppState;
use rusqlite::{params, Connection};
use std::sync::MutexGuard;
use tauri::State;

fn lock<'a>(state: &'a State<AppState>) -> AppResult<MutexGuard<'a, Connection>> {
    state.db.lock().map_err(|_| AppError::internal("db mutex poisoned"))
}

fn row_to_file(conn: &Connection, id: i64) -> AppResult<FileShortcut> {
    Ok(conn.query_row(
        "SELECT id, project_id, label, path, sort_order FROM files WHERE id = ?1",
        [id],
        |r| Ok(FileShortcut {
            id: r.get(0)?,
            project_id: r.get(1)?,
            label: r.get(2)?,
            path: r.get(3)?,
            sort_order: r.get(4)?,
        }),
    )?)
}

#[tauri::command]
pub fn files_list(state: State<AppState>, project_id: i64) -> AppResult<Vec<FileShortcut>> {
    let conn = lock(&state)?;
    let mut stmt = conn.prepare(
        "SELECT id FROM files WHERE project_id = ?1 ORDER BY sort_order ASC, id ASC",
    )?;
    let ids = stmt.query_map([project_id], |r| r.get::<_, i64>(0))?;
    let mut out = Vec::new();
    for id in ids {
        out.push(row_to_file(&conn, id?)?);
    }
    Ok(out)
}

#[tauri::command]
pub fn files_create(state: State<AppState>, project_id: i64, label: String, path: String) -> AppResult<FileShortcut> {
    if label.trim().is_empty() || path.trim().is_empty() {
        return Err(AppError { kind: ErrorKind::Validation, message: "Название и путь обязательны".into() });
    }
    let conn = lock(&state)?;
    let next: i64 = conn.query_row("SELECT COALESCE(MAX(sort_order),0)+1 FROM files WHERE project_id=?1", [project_id], |r| r.get(0))?;
    conn.execute(
        "INSERT INTO files(project_id, label, path, sort_order) VALUES (?1, ?2, ?3, ?4)",
        params![project_id, label.trim(), path.trim(), next],
    )?;
    row_to_file(&conn, conn.last_insert_rowid())
}

#[tauri::command]
pub fn files_update(state: State<AppState>, id: i64, label: String, path: String) -> AppResult<FileShortcut> {
    if label.trim().is_empty() || path.trim().is_empty() {
        return Err(AppError { kind: ErrorKind::Validation, message: "Название и путь обязательны".into() });
    }
    let conn = lock(&state)?;
    conn.execute("UPDATE files SET label=?2, path=?3 WHERE id=?1", params![id, label.trim(), path.trim()])?;
    row_to_file(&conn, id)
}

#[tauri::command]
pub fn files_delete(state: State<AppState>, id: i64) -> AppResult<()> {
    let conn = lock(&state)?;
    conn.execute("DELETE FROM files WHERE id=?1", [id])?;
    Ok(())
}
```

- [ ] **Step 6: open_shortcut в actions.rs**

В `src-tauri/src/commands/actions.rs` добавить команду (использует существующий `expand_path` и плагин opener — `use tauri_plugin_opener::OpenerExt;` уже есть в файле):
```rust
/// Открыть файл/папку-ярлык ассоциированной программой (файл) или в проводнике (папка)
/// через системный обработчик opener — без shell.
#[tauri::command]
pub fn open_shortcut(app: tauri::AppHandle, path: String) -> AppResult<()> {
    let p = require_dir_or_file(&path)?;
    app.opener()
        .open_path(p, None::<&str>)
        .map_err(|e| AppError { kind: ErrorKind::Io, message: format!("Не удалось открыть: {}", e) })
}
```
И рядом — хелпер (путь может быть файлом ИЛИ папкой; `require_dir` из actions.rs проверяет только папку, поэтому отдельный):
```rust
fn require_dir_or_file(path: &str) -> AppResult<String> {
    let p = expand_path(path);
    if p.is_empty() {
        return Err(AppError { kind: ErrorKind::Validation, message: "Путь не задан".into() });
    }
    if !std::path::Path::new(&p).exists() {
        return Err(AppError { kind: ErrorKind::NotFound, message: format!("Не найдено: {}", p) });
    }
    Ok(p)
}
```
> `OpenerExt`/`AppError`/`ErrorKind`/`expand_path` уже импортированы/определены в actions.rs. `std::path::Path` — добавить `use std::path::Path;` если ещё не импортирован (в actions.rs он есть после фикса безопасности — проверить; если нет, использовать полный путь `std::path::Path` как выше).

- [ ] **Step 7: Регистрация**
- `commands/mod.rs`: `pub mod links;` и `pub mod files;`
- `lib.rs` `generate_handler![...]`: добавить
```rust
            commands::links::links_list,
            commands::links::links_create,
            commands::links::links_update,
            commands::links::links_delete,
            commands::files::files_list,
            commands::files::files_create,
            commands::files::files_update,
            commands::files::files_delete,
            commands::actions::open_shortcut,
```

- [ ] **Step 8: Тесты + сборка**
```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib
cargo build --manifest-path src-tauri/Cargo.toml
```
Expected: миграционные тесты (обновлённые на версию 2 + проверка таблицы `files`) и прежние — все ok (17 тестов); build успешен. ВАЖНО: если в `%APPDATA%\com.devdeck.app\devdeck.db` уже существует БД версии 1 — раннер до-накатит 0002 при следующем запуске (миграция применяется по `user_version`).

- [ ] **Step 9: Commit**
```powershell
git add src-tauri/migrations/0002_files.sql src-tauri/src/db/migrations.rs src-tauri/src/models.rs src-tauri/src/commands/links.rs src-tauri/src/commands/files.rs src-tauri/src/commands/actions.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs
git commit -m @'
feat(backend): links and file-shortcuts CRUD + open_shortcut (migration 0002)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

## Task 2: Фронт — OverviewTab (ссылки + файлы)

**Files:** Modify `types.ts`, `api/actions.ts`, `ProjectView.svelte`; Create `api/links.ts`, `api/files.ts`, `components/OverviewTab.svelte`.

- [ ] **Step 1: Типы** (в конец `types.ts`):
```ts
export type Link = { id: number; projectId: number; label: string; url: string; icon: string | null; sortOrder: number };
export type FileShortcut = { id: number; projectId: number; label: string; path: string; sortOrder: number };
```

- [ ] **Step 2: api**

Create `src/lib/api/links.ts`:
```ts
import { call } from "./client";
import type { Link } from "../types";

export const list = (projectId: number) => call<Link[]>("links_list", { projectId });
export const create = (projectId: number, label: string, url: string, icon: string | null = "globe") =>
  call<Link>("links_create", { projectId, label, url, icon });
export const update = (id: number, label: string, url: string, icon: string | null) =>
  call<Link>("links_update", { id, label, url, icon });
export const remove = (id: number) => call<void>("links_delete", { id });
```

Create `src/lib/api/files.ts`:
```ts
import { call } from "./client";
import type { FileShortcut } from "../types";

export const list = (projectId: number) => call<FileShortcut[]>("files_list", { projectId });
export const create = (projectId: number, label: string, path: string) =>
  call<FileShortcut>("files_create", { projectId, label, path });
export const update = (id: number, label: string, path: string) =>
  call<FileShortcut>("files_update", { id, label, path });
export const remove = (id: number) => call<void>("files_delete", { id });
```

В `src/lib/api/actions.ts` добавить (после `openUrl`):
```ts
export const openShortcut = (path: string) => call<void>("open_shortcut", { path });
```

- [ ] **Step 3: components/OverviewTab.svelte**

Create `src/lib/components/OverviewTab.svelte`:
```svelte
<script lang="ts">
  import type { Project, Link, FileShortcut } from "$lib/types";
  import * as linksApi from "$lib/api/links";
  import * as filesApi from "$lib/api/files";
  import * as actions from "$lib/api/actions";
  import Icon from "./Icon.svelte";

  let { project }: { project: Project } = $props();

  let links = $state<Link[]>([]);
  let files = $state<FileShortcut[]>([]);
  let newLink = $state({ label: "", url: "" });
  let newFile = $state({ label: "", path: "" });

  let reqId = 0;
  async function load() {
    const my = ++reqId;
    try {
      const [l, f] = await Promise.all([linksApi.list(project.id), filesApi.list(project.id)]);
      if (my === reqId) {
        links = l;
        files = f;
      }
    } catch {
      /* тост из api/client.ts */
    }
  }
  $effect(() => {
    project.id;
    load();
  });

  async function addLink() {
    if (!newLink.label.trim() || !newLink.url.trim()) return;
    await linksApi.create(project.id, newLink.label.trim(), newLink.url.trim());
    newLink = { label: "", url: "" };
    await load();
  }
  async function delLink(id: number) {
    await linksApi.remove(id);
    await load();
  }
  async function addFile() {
    if (!newFile.label.trim() || !newFile.path.trim()) return;
    await filesApi.create(project.id, newFile.label.trim(), newFile.path.trim());
    newFile = { label: "", path: "" };
    await load();
  }
  async function delFile(id: number) {
    await filesApi.remove(id);
    await load();
  }
</script>

<div class="stack">
  {#if project.description}
    <div>
      <h3 class="section-title">Описание</h3>
      <p class="desc">{project.description}</p>
    </div>
  {/if}

  <div>
    <h3 class="section-title"><Icon name="link" class="ic-sm" /> Ссылки</h3>
    <div class="card links">
      {#each links as l (l.id)}
        <div class="link-row">
          <span class="lico" role="button" tabindex="0" onclick={() => actions.openUrl(l.url)}><Icon name={l.icon ?? "globe"} class="ic-sm" /></span>
          <div style="flex:1;min-width:0;cursor:pointer" role="button" tabindex="0" onclick={() => actions.openUrl(l.url)}>
            <div class="lt">{l.label}</div>
            <div class="lu">{l.url}</div>
          </div>
          <button class="mini" title="Открыть" onclick={() => actions.openUrl(l.url)}><Icon name="external-link" class="ic-sm" /></button>
          <button class="mini" title="Удалить" onclick={() => delLink(l.id)}><Icon name="x" class="ic-sm" /></button>
        </div>
      {/each}
      <div class="erow" style="margin:8px 4px 4px">
        <input class="tin" placeholder="Название" bind:value={newLink.label} />
        <input class="tin url" placeholder="localhost:3000 / github.com/…" bind:value={newLink.url}
               onkeydown={(e) => { if (e.key === 'Enter') addLink(); }} />
        <button class="er-del" style="color:var(--accent)" title="Добавить" onclick={addLink}><Icon name="plus" class="ic-sm" /></button>
      </div>
    </div>
  </div>

  <div>
    <h3 class="section-title"><Icon name="folder" class="ic-sm" /> Файлы и папки</h3>
    <div class="card links">
      {#each files as f (f.id)}
        <div class="link-row">
          <span class="lico" role="button" tabindex="0" onclick={() => actions.openShortcut(f.path)}><Icon name="file" class="ic-sm" /></span>
          <div style="flex:1;min-width:0;cursor:pointer" role="button" tabindex="0" onclick={() => actions.openShortcut(f.path)}>
            <div class="lt">{f.label}</div>
            <div class="lu">{f.path}</div>
          </div>
          <button class="mini" title="Открыть" onclick={() => actions.openShortcut(f.path)}><Icon name="external-link" class="ic-sm" /></button>
          <button class="mini" title="Удалить" onclick={() => delFile(f.id)}><Icon name="x" class="ic-sm" /></button>
        </div>
      {/each}
      <div class="erow" style="margin:8px 4px 4px">
        <input class="tin" placeholder="Название" bind:value={newFile.label} />
        <input class="tin url" placeholder="~/dev/proj/.env" bind:value={newFile.path}
               onkeydown={(e) => { if (e.key === 'Enter') addFile(); }} />
        <button class="er-del" style="color:var(--accent)" title="Добавить" onclick={addFile}><Icon name="plus" class="ic-sm" /></button>
      </div>
    </div>
  </div>
</div>
```
> Иконки `link`/`globe`/`folder`/`file`/`external-link`/`x`/`plus` — из lucide. Классы `.stack`/`.section-title`/`.desc`/`.links`/`.link-row`(`.lico`/`.lt`/`.lu`)/`.erow`/`.er-del`/`.mini`/`.tin` — в global.css.

- [ ] **Step 4: Рендер в ProjectView.svelte**
Импорт `import OverviewTab from "./OverviewTab.svelte";` и явная ветка «overview» в `tab-body` (перед финальным `{:else}`):
```svelte
    {:else if $activeTab === "overview"}
      <OverviewTab {project} />
```
(Финальный `{:else}`-плейсхолдер можно оставить как фолбэк.)

- [ ] **Step 5: Проверка**
```powershell
npm run check
npm test
npm run build
```
Expected: 0 ошибок типов; Vitest 3 (при редком транзиенте — перезапустить); build успешен.

- [ ] **Step 6: Commit**
```powershell
git add src/lib/types.ts src/lib/api/links.ts src/lib/api/files.ts src/lib/api/actions.ts src/lib/components/OverviewTab.svelte src/lib/components/ProjectView.svelte
git commit -m @'
feat(frontend): overview tab with links and file shortcuts

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

## Task 3: Проверка среза

- [ ] **Step 1: Автопроверка**
```powershell
npm run check ; npm test ; cargo test --manifest-path src-tauri/Cargo.toml --lib
```
Expected: типы 0 ошибок; Vitest 3; Rust 17 (миграционные тесты обновлены под версию 2).

- [ ] **Step 2: Ручной GUI-смоук (пользователь; `npm run tauri dev`)**
- [ ] Открыть проект → вкладка «Обзор»: описание + разделы «Ссылки» и «Файлы и папки».
- [ ] Добавить ссылку (название + `localhost:3000` или `github.com/...`) → появилась; клик → открывается в браузере.
- [ ] Добавить файл/папку-ярлык (название + реальный путь, напр. `~/dev` или конкретный файл) → клик → открывается ассоциированной программой / в проводнике.
- [ ] Удалить ссылку/файл (×).
- [ ] Перезапуск приложения → ссылки/файлы на месте (БД до-мигрировала до версии 2 при первом запуске).

---

## Итог среза

Вкладка «Обзор» наполнена: описание, ссылки (открытие в браузере), файлы/папки-ярлыки (открытие ассоциированной программой/в проводнике), с инлайн-добавлением/удалением. Добавлена миграция БД 0002 (`files`). Дальше — **кастомные команды-кнопки** (тоже в «Обзор»): `npm run dev` одной кнопкой. Отложено: пикер иконок ссылок, git-сводка/ближайшие задачи в обзоре.
```
