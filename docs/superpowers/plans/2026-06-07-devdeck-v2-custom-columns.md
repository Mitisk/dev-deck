# DevDeck v2.0 — Срез «Настраиваемые колонки канбана» — план реализации

> **Для исполнителя:** ОБЯЗАТЕЛЬНЫЙ ПОД-НАВЫК: superpowers:subagent-driven-development. Шаги — чекбоксы.

**Цель:** Колонки канбана настраиваются на проект (добавить/переименовать/удалить/пометить «выполнено») вместо фиксированных To Do/In Progress/Done. Существующее поведение сохраняется (дефолтные 3 колонки).

**Архитектура:** Таблица `task_columns(project_id, key, name, is_done, sort_order)`. `tasks.status` = **ключ колонки** (схему задач НЕ меняем). Флаг `is_done` колонки заменяет хардкод `status=='done'` (для `completed_at` и повестки). Дефолтные колонки сидируются: миграцией (для существующих проектов), в `projects_create` (новые проекты), в `import_doc` (импорт). Фронт: динамический TasksTab по колонкам проекта + управление колонками.

**Стек:** миграция БД 0003. Без новых crate.

**Решения / границы:**
- **Только колонки.** Метки задач и фильтры — отдельным срезом после (отмечено).
- `tasks.status` хранит ключ колонки: дефолтные — `todo`/`doing`/`done` (совпадают с текущими значениями), кастомные — `c{id}`.
- Удаление колонки переносит её задачи в первую оставшуюся; нельзя удалить последнюю.
- Реордер колонок drag-ом — отложен (добавляются в конец; порядок по `sort_order`).
- Экспорт кастомных колонок — отложен: при импорте проект получает дефолтные колонки (custom не переносятся).

**Источники:** `TZ_DevDeck.md` (11 — v2.0, настраиваемые колонки). Текущие `commands/tasks.rs`, `commands/projects.rs` (`projects_create`), `commands/transfer.rs` (`import_doc`), `db/migrations.rs`, `TasksTab.svelte`.

---

## Контекст

- Rust: 72 команды; `tasks.rs` (status hardcoded 'todo'/'done' в create/move/update + agenda `status != 'done'`); `projects.rs` (`projects_create` — без колонок); `transfer.rs` (`import_doc` вставляет проект). Миграции: 0001, 0002 (files) → текущая версия 2. Таблица `tasks(status TEXT, ...)`.
- Фронт: `TasksTab.svelte` — фиксированные COLUMNS todo/doing/done. api `tasks.ts`.

---

## Структура файлов

```
src-tauri/
├─ migrations/0003_task_columns.sql  # NEW
└─ src/
   ├─ db/migrations.rs               # MOD: + (3, ...) + тесты на версию 3 / таблицу
   ├─ models.rs                      # MOD: + TaskColumn
   ├─ commands/columns.rs            # NEW: columns_* + seed_default_columns + is_done_column
   ├─ commands/tasks.rs              # MOD: completed_at через is_done_column; default status; agenda join
   ├─ commands/projects.rs           # MOD: projects_create сидирует дефолтные колонки
   ├─ commands/transfer.rs           # MOD: import_doc сидирует дефолтные колонки
   ├─ commands/mod.rs                # MOD: + pub mod columns;
   └─ lib.rs                         # MOD: регистрация columns_*

src/lib/
├─ types.ts                          # MOD: + TaskColumn
├─ api/columns.ts                    # NEW
└─ components/TasksTab.svelte         # MOD: динамические колонки + управление
```

---

## Task 1: Rust — миграция 0003 + колонки

**Files:** Create `migrations/0003_task_columns.sql`, `commands/columns.rs`; Modify `db/migrations.rs`, `models.rs`, `commands/mod.rs`, `lib.rs`.

- [ ] **Step 1: Миграция 0003** — Create `src-tauri/migrations/0003_task_columns.sql`:
```sql
CREATE TABLE task_columns (
    id         INTEGER PRIMARY KEY,
    project_id INTEGER REFERENCES projects(id) ON DELETE CASCADE,
    key        TEXT NOT NULL,
    name       TEXT NOT NULL,
    is_done    INTEGER DEFAULT 0,
    sort_order INTEGER DEFAULT 0,
    UNIQUE(project_id, key)
);

-- дефолтные колонки для уже существующих проектов
INSERT INTO task_columns(project_id, key, name, is_done, sort_order)
    SELECT id, 'todo', 'To Do', 0, 0 FROM projects;
INSERT INTO task_columns(project_id, key, name, is_done, sort_order)
    SELECT id, 'doing', 'In Progress', 0, 1 FROM projects;
INSERT INTO task_columns(project_id, key, name, is_done, sort_order)
    SELECT id, 'done', 'Done', 1, 2 FROM projects;

PRAGMA user_version = 3;
```

- [ ] **Step 2: Раннер + тесты** — в `src-tauri/src/db/migrations.rs`:
1. В `MIGRATIONS` добавить `(3, include_str!("../../migrations/0003_task_columns.sql"))`.
2. Обновить тесты версии: `applies_migrations_on_empty_db` и `run_is_idempotent` → ожидаемая версия `3` (была 2). Добавить проверку таблицы `task_columns`:
```rust
        let tc: i64 = conn.query_row("SELECT count(*) FROM sqlite_master WHERE type='table' AND name='task_columns'", [], |r| r.get(0)).unwrap();
        assert_eq!(tc, 1);
```

- [ ] **Step 3: Модель** (в `models.rs`, после `AgendaItem`):
```rust
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskColumn {
    pub id: i64,
    pub project_id: i64,
    pub key: String,
    pub name: String,
    pub is_done: bool,
    pub sort_order: i64,
}
```

- [ ] **Step 4: commands/columns.rs**

Create `src-tauri/src/commands/columns.rs`:
```rust
use crate::error::{AppError, AppResult, ErrorKind};
use crate::models::TaskColumn;
use crate::state::AppState;
use rusqlite::{params, Connection, OptionalExtension};
use std::sync::MutexGuard;
use tauri::State;

fn lock<'a>(state: &'a State<AppState>) -> AppResult<MutexGuard<'a, Connection>> {
    state.db.lock().map_err(|_| AppError::internal("db mutex poisoned"))
}

/// Засеять дефолтные колонки (To Do/In Progress/Done) для проекта. Вызывается при
/// создании/импорте проекта. Идемпотентно по UNIQUE(project_id,key).
pub fn seed_default_columns(conn: &Connection, project_id: i64) -> AppResult<()> {
    let defaults = [("todo", "To Do", 0i64, 0i64), ("doing", "In Progress", 0, 1), ("done", "Done", 1, 2)];
    for (key, name, is_done, sort) in defaults {
        conn.execute(
            "INSERT OR IGNORE INTO task_columns(project_id,key,name,is_done,sort_order) VALUES(?1,?2,?3,?4,?5)",
            params![project_id, key, name, is_done, sort],
        )?;
    }
    Ok(())
}

/// Является ли колонка (по ключу) «выполненной».
pub fn is_done_column(conn: &Connection, project_id: i64, key: &str) -> bool {
    conn.query_row(
        "SELECT is_done FROM task_columns WHERE project_id=?1 AND key=?2",
        params![project_id, key],
        |r| r.get::<_, i64>(0),
    )
    .optional()
    .ok()
    .flatten()
    .map(|v| v != 0)
    .unwrap_or(false)
}

/// Ключ первой колонки проекта (для дефолтного статуса новой задачи).
pub fn first_column_key(conn: &Connection, project_id: i64) -> AppResult<String> {
    Ok(conn
        .query_row(
            "SELECT key FROM task_columns WHERE project_id=?1 ORDER BY sort_order, id LIMIT 1",
            [project_id],
            |r| r.get::<_, String>(0),
        )
        .optional()?
        .unwrap_or_else(|| "todo".to_string()))
}

fn row_to_column(conn: &Connection, id: i64) -> AppResult<TaskColumn> {
    Ok(conn.query_row(
        "SELECT id, project_id, key, name, is_done, sort_order FROM task_columns WHERE id=?1",
        [id],
        |r| Ok(TaskColumn {
            id: r.get(0)?,
            project_id: r.get(1)?,
            key: r.get(2)?,
            name: r.get(3)?,
            is_done: r.get::<_, i64>(4)? != 0,
            sort_order: r.get(5)?,
        }),
    )?)
}

#[tauri::command]
pub fn columns_list(state: State<AppState>, project_id: i64) -> AppResult<Vec<TaskColumn>> {
    let conn = lock(&state)?;
    let mut stmt = conn.prepare("SELECT id FROM task_columns WHERE project_id=?1 ORDER BY sort_order, id")?;
    let ids = stmt.query_map([project_id], |r| r.get::<_, i64>(0))?;
    let mut out = Vec::new();
    for id in ids { out.push(row_to_column(&conn, id?)?); }
    Ok(out)
}

#[tauri::command]
pub fn column_create(state: State<AppState>, project_id: i64, name: String, is_done: bool) -> AppResult<TaskColumn> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError { kind: ErrorKind::Validation, message: "Название колонки пусто".into() });
    }
    let conn = lock(&state)?;
    let next: i64 = conn.query_row("SELECT COALESCE(MAX(sort_order),0)+1 FROM task_columns WHERE project_id=?1", [project_id], |r| r.get(0))?;
    // временный ключ, затем фиксируем по id
    conn.execute("INSERT INTO task_columns(project_id,key,name,is_done,sort_order) VALUES(?1,'',?2,?3,?4)", params![project_id, name, is_done as i64, next])?;
    let id = conn.last_insert_rowid();
    let key = format!("c{}", id);
    conn.execute("UPDATE task_columns SET key=?2 WHERE id=?1", params![id, key])?;
    row_to_column(&conn, id)
}

#[tauri::command]
pub fn column_update(state: State<AppState>, id: i64, name: String, is_done: bool) -> AppResult<TaskColumn> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError { kind: ErrorKind::Validation, message: "Название колонки пусто".into() });
    }
    let conn = lock(&state)?;
    conn.execute("UPDATE task_columns SET name=?2, is_done=?3 WHERE id=?1", params![id, name, is_done as i64])?;
    // если стала done — проставить completed_at задачам в ней; иначе снять
    let (pid, key): (i64, String) = conn.query_row("SELECT project_id, key FROM task_columns WHERE id=?1", [id], |r| Ok((r.get(0)?, r.get(1)?)))?;
    if is_done {
        conn.execute("UPDATE tasks SET completed_at=COALESCE(completed_at, datetime('now')) WHERE project_id=?1 AND status=?2", params![pid, key])?;
    } else {
        conn.execute("UPDATE tasks SET completed_at=NULL WHERE project_id=?1 AND status=?2", params![pid, key])?;
    }
    row_to_column(&conn, id)
}

#[tauri::command]
pub fn column_delete(state: State<AppState>, id: i64) -> AppResult<()> {
    let conn = lock(&state)?;
    let (pid, key): (i64, String) = conn.query_row("SELECT project_id, key FROM task_columns WHERE id=?1", [id], |r| Ok((r.get(0)?, r.get(1)?)))?;
    let count: i64 = conn.query_row("SELECT count(*) FROM task_columns WHERE project_id=?1", [pid], |r| r.get(0))?;
    if count <= 1 {
        return Err(AppError { kind: ErrorKind::Validation, message: "Нельзя удалить последнюю колонку".into() });
    }
    // перенести задачи в первую оставшуюся колонку
    let target: String = conn.query_row(
        "SELECT key FROM task_columns WHERE project_id=?1 AND id!=?2 ORDER BY sort_order, id LIMIT 1",
        params![pid, id],
        |r| r.get(0),
    )?;
    conn.execute("UPDATE tasks SET status=?3 WHERE project_id=?1 AND status=?2", params![pid, key, target])?;
    conn.execute("DELETE FROM task_columns WHERE id=?1", [id])?;
    Ok(())
}
```

- [ ] **Step 5: Регистрация** — `commands/mod.rs`: `pub mod columns;`; `lib.rs` `generate_handler![...]`: добавить
```rust
            commands::columns::columns_list,
            commands::columns::column_create,
            commands::columns::column_update,
            commands::columns::column_delete,
```

- [ ] **Step 6: lib-сборка + тесты миграций**
```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib migrations
cargo build --manifest-path src-tauri/Cargo.toml --lib
```
Expected: миграционные тесты (версия 3 + таблица) ok; lib-сборка успешна. (Тесты tasks временно могут падать — починим в Task 2.)

---

## Task 2: Rust — задачи через is_done_column + сиды в projects/transfer

**Files:** Modify `commands/tasks.rs`, `commands/projects.rs`, `commands/transfer.rs`.

- [ ] **Step 1: tasks.rs — completed_at через колонку, дефолтный статус, agenda**

В `src-tauri/src/commands/tasks.rs`:
1. Импорт: `use crate::commands::columns::{is_done_column, first_column_key};`
2. `create_task`: статус по умолчанию = первая колонка; `completed_at` по `is_done_column`:
```rust
fn create_task(conn: &Connection, project_id: i64, input: TaskInput) -> AppResult<Task> {
    let title = input.title.trim();
    if title.is_empty() {
        return Err(AppError { kind: ErrorKind::Validation, message: "Заголовок задачи пуст".into() });
    }
    let status = match input.status.as_deref() {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => first_column_key(conn, project_id)?,
    };
    let priority = input.priority.unwrap_or(0).clamp(0, 2);
    let next_sort: i64 = conn.query_row(
        "SELECT COALESCE(MAX(sort_order), 0) + 1 FROM tasks WHERE project_id = ?1 AND status = ?2",
        params![project_id, status],
        |r| r.get(0),
    )?;
    let done = is_done_column(conn, project_id, &status);
    conn.execute(
        "INSERT INTO tasks(project_id, title, description, status, priority, due_date, sort_order, completed_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, CASE WHEN ?8=1 THEN datetime('now') ELSE NULL END)",
        params![project_id, title, input.description, status, priority, input.due_date, next_sort, done as i64],
    )?;
    row_to_task(conn, conn.last_insert_rowid())
}
```
3. `update_task`: `completed_at` по колонке (нужен project_id задачи):
```rust
fn update_task(conn: &Connection, id: i64, input: TaskInput) -> AppResult<Task> {
    let title = input.title.trim();
    if title.is_empty() {
        return Err(AppError { kind: ErrorKind::Validation, message: "Заголовок задачи пуст".into() });
    }
    let status = input.status.as_deref().unwrap_or("todo").to_string();
    let priority = input.priority.unwrap_or(0).clamp(0, 2);
    let pid: i64 = conn.query_row("SELECT project_id FROM tasks WHERE id=?1", [id], |r| r.get(0))?;
    let done = is_done_column(conn, pid, &status);
    let n = conn.execute(
        "UPDATE tasks SET title=?2, description=?3, status=?4, priority=?5, due_date=?6,
                completed_at = CASE WHEN ?7=1 THEN COALESCE(completed_at, datetime('now')) ELSE NULL END
         WHERE id=?1",
        params![id, title, input.description, status, priority, input.due_date, done as i64],
    )?;
    if n == 0 {
        return Err(AppError { kind: ErrorKind::NotFound, message: "Задача не найдена".into() });
    }
    row_to_task(conn, id)
}
```
4. `move_task`: то же:
```rust
fn move_task(conn: &Connection, id: i64, status: &str, sort_order: i64) -> AppResult<()> {
    let pid: i64 = conn.query_row("SELECT project_id FROM tasks WHERE id=?1", [id], |r| r.get(0))?;
    let done = is_done_column(conn, pid, status);
    conn.execute(
        "UPDATE tasks SET status=?2, sort_order=?3,
                completed_at = CASE WHEN ?4=1 THEN COALESCE(completed_at, datetime('now')) ELSE NULL END
         WHERE id=?1",
        params![id, status, sort_order, done as i64],
    )?;
    Ok(())
}
```
5. `agenda` (Task dates среза): исключать done-колонки через join:
```rust
fn agenda(conn: &Connection, today: &str) -> AppResult<Vec<crate::models::AgendaItem>> {
    let mut stmt = conn.prepare(
        "SELECT t.project_id, p.name, p.color, t.id, t.title, t.due_date, t.priority
         FROM tasks t
         JOIN projects p ON p.id = t.project_id
         LEFT JOIN task_columns tc ON tc.project_id = t.project_id AND tc.key = t.status
         WHERE COALESCE(tc.is_done,0) = 0
           AND t.due_date IS NOT NULL AND t.due_date != '' AND t.due_date <= ?1
           AND p.status != 'archived'
         ORDER BY t.due_date ASC, t.priority DESC
         LIMIT 50",
    )?;
    // ... тело без изменений (query_map + сбор) ...
}
```
6. **Тест-хелпер `project()`** в `mod tests` — засеять дефолтные колонки, чтобы `is_done_column`/`agenda` работали:
```rust
    fn project(conn: &Connection) -> i64 {
        conn.execute("INSERT INTO projects(name, status, sort_order) VALUES ('P', 'active', 0)", []).unwrap();
        let id = conn.last_insert_rowid();
        crate::commands::columns::seed_default_columns(conn, id).unwrap();
        id
    }
```
(прежние тесты `create_list_move_delete`/`agenda_*` теперь зелёные: колонка `done` помечена is_done.)

- [ ] **Step 2: projects.rs — сидировать колонки при создании**

В `projects_create` (после получения `id` нового проекта и `set_tags`, до `row_to_project`) добавить:
```rust
    crate::commands::columns::seed_default_columns(&conn, id)?;
```

- [ ] **Step 3: transfer.rs — сидировать колонки при импорте**

В `import_doc`, после вставки проекта и получения `pid` (до вставки задач), добавить:
```rust
        crate::commands::columns::seed_default_columns(conn, pid)?;
```
> Импортированные задачи имеют `status` из экспорта (todo/doing/done) — совпадают с засеянными ключами. Кастомные колонки при импорте не воссоздаются (отложено).

- [ ] **Step 4: Тесты + lib-сборка**
```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib
cargo build --manifest-path src-tauri/Cargo.toml --lib
```
Expected: все прежние (включая tasks/agenda/transfer/projects) зелёные на новой схеме → 31 ok; lib-сборка успешна.

- [ ] **Step 5: Commit (бэкенд)**
```powershell
git add src-tauri/migrations/0003_task_columns.sql src-tauri/src/db/migrations.rs src-tauri/src/models.rs src-tauri/src/commands/columns.rs src-tauri/src/commands/tasks.rs src-tauri/src/commands/projects.rs src-tauri/src/commands/transfer.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs
git commit -m @'
feat(backend): customizable kanban columns (task_columns, migration 0003)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

## Task 3: Фронт — динамический TasksTab

**Files:** Modify `types.ts`, `components/TasksTab.svelte`; Create `api/columns.ts`.

- [ ] **Step 1: Тип** (в конец `types.ts`):
```ts
export type TaskColumn = { id: number; projectId: number; key: string; name: string; isDone: boolean; sortOrder: number };
```

- [ ] **Step 2: api/columns.ts**
```ts
import { call } from "./client";
import type { TaskColumn } from "../types";

export const list = (projectId: number) => call<TaskColumn[]>("columns_list", { projectId });
export const create = (projectId: number, name: string, isDone: boolean) => call<TaskColumn>("column_create", { projectId, name, isDone });
export const update = (id: number, name: string, isDone: boolean) => call<TaskColumn>("column_update", { id, name, isDone });
export const remove = (id: number) => call<void>("column_delete", { id });
```

- [ ] **Step 3: TasksTab.svelte — динамические колонки**

Заменить ВЕСЬ `src/lib/components/TasksTab.svelte` на (канбан по колонкам проекта + быстрое добавление + DnD + edit-диалог задачи + управление колонками):
```svelte
<script lang="ts">
  import type { Project, Task, TaskColumn } from "$lib/types";
  import * as tasksApi from "$lib/api/tasks";
  import * as columnsApi from "$lib/api/columns";
  import { pushToast } from "$lib/stores/toasts";
  import Icon from "./Icon.svelte";

  let { project }: { project: Project } = $props();

  const PRIORITY = [
    { label: "Низкий", color: "#5b9cff" },
    { label: "Средний", color: "#e0a83a" },
    { label: "Высокий", color: "#f0616d" },
  ];

  let columns = $state<TaskColumn[]>([]);
  let tasks = $state<Task[]>([]);
  let adding = $state<Record<string, string>>({});
  let dragId = $state<number | null>(null);
  let dragging = $state(false);
  let overCol = $state<string | null>(null);

  // edit task dialog
  let editing = $state<Task | null>(null);
  let eTitle = $state("");
  let eDesc = $state("");
  let ePriority = $state(0);
  let eDue = $state("");
  let eStatus = $state("");

  // column dialog
  let colEditing = $state<TaskColumn | null>(null);
  let colNew = $state(false);
  let cName = $state("");
  let cDone = $state(false);

  let reqId = 0;
  async function load() {
    const my = ++reqId;
    try {
      const [cols, ts] = await Promise.all([columnsApi.list(project.id), tasksApi.list(project.id)]);
      if (my === reqId) { columns = cols; tasks = ts; }
    } catch { /* тост из api/client.ts */ }
  }
  $effect(() => { project.id; load(); });

  function colTasks(key: string): Task[] {
    return tasks.filter((t) => t.status === key).sort((a, b) => a.sortOrder - b.sortOrder);
  }

  function todayStr(): string {
    const d = new Date();
    return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
  }
  function fmtDue(due: string | null): string {
    if (!due) return "";
    const d = new Date(due + "T00:00:00");
    return isNaN(d.getTime()) ? due : d.toLocaleDateString("ru-RU", { day: "numeric", month: "short" });
  }
  function isOverdue(due: string | null): boolean {
    return !!due && /^\d{4}-\d{2}-\d{2}$/.test(due) && due < todayStr();
  }

  async function quickAdd(key: string) {
    const title = (adding[key] ?? "").trim();
    if (!title) return;
    adding[key] = "";
    await tasksApi.create(project.id, { title, status: key });
    await load();
  }
  async function onDrop(key: string) {
    overCol = null;
    const id = dragId;
    if (id == null) return;
    const t = tasks.find((x) => x.id === id);
    if (!t || t.status === key) return;
    const nextSort = Math.max(0, ...colTasks(key).map((x) => x.sortOrder)) + 1;
    await tasksApi.move(id, key, nextSort);
    await load();
  }

  function openEdit(t: Task) {
    if (dragging) return;
    editing = t; eTitle = t.title; eDesc = t.description ?? ""; ePriority = t.priority; eDue = t.dueDate ?? ""; eStatus = t.status;
  }
  async function saveEdit() {
    if (!editing) return;
    if (!eTitle.trim()) return;
    await tasksApi.update(editing.id, { title: eTitle.trim(), description: eDesc.trim() || null, priority: ePriority, dueDate: eDue.trim() || null, status: eStatus });
    editing = null; await load();
  }
  async function deleteTask() {
    if (!editing) return;
    const id = editing.id; editing = null; await tasksApi.remove(id); await load();
  }

  // columns management
  function openNewCol() { colNew = true; colEditing = { id: 0, projectId: project.id, key: "", name: "", isDone: false, sortOrder: 0 }; cName = ""; cDone = false; }
  function openEditCol(c: TaskColumn) { colNew = false; colEditing = c; cName = c.name; cDone = c.isDone; }
  async function saveCol() {
    if (!colEditing || !cName.trim()) return;
    if (colNew) await columnsApi.create(project.id, cName.trim(), cDone);
    else await columnsApi.update(colEditing.id, cName.trim(), cDone);
    colEditing = null; await load();
  }
  async function deleteCol() {
    if (!colEditing) return;
    const id = colEditing.id; colEditing = null;
    await columnsApi.remove(id); await load();
  }
</script>

<div style="display:flex;align-items:center;margin-bottom:10px">
  <span style="flex:1"></span>
  <button class="gbtn" onclick={openNewCol}><Icon name="plus" class="ic-sm" /> Колонка</button>
</div>

<div class="kanban" style="grid-template-columns:repeat({Math.max(columns.length, 1)}, 1fr)">
  {#each columns as c (c.id)}
    <div class="col" class:drag-over={overCol === c.key} role="list"
         ondragover={(e) => { e.preventDefault(); overCol = c.key; }}
         ondragleave={() => { if (overCol === c.key) overCol = null; }}
         ondrop={() => onDrop(c.key)}>
      <div class="col-head">
        <span class="led" style="background:{c.isDone ? '#3fb863' : 'var(--accent)'}"></span>
        <span class="h">{c.name}</span>
        <span class="n">{colTasks(c.key).length}</span>
        <button class="mini" title="Настроить колонку" style="margin-left:auto" onclick={() => openEditCol(c)}><Icon name="ellipsis" class="ic-sm" /></button>
      </div>
      <div class="col-body">
        {#each colTasks(c.key) as t (t.id)}
          <div class="tcard" class:dragging={dragId === t.id} draggable={true} role="button" tabindex="0"
               ondragstart={() => { dragId = t.id; dragging = true; }}
               ondragend={() => { dragging = false; setTimeout(() => (dragId = null), 0); }}
               onclick={() => openEdit(t)}>
            <div class="tt">{t.title}</div>
            <div class="row">
              <span class="tag-pri" style="color:{PRIORITY[t.priority].color};background:color-mix(in oklab, {PRIORITY[t.priority].color} 14%, transparent)">{PRIORITY[t.priority].label}</span>
              {#if t.dueDate}<span class="due" style={isOverdue(t.dueDate) ? "color:var(--danger)" : ""}><Icon name="calendar" class="ic-sm" /> {fmtDue(t.dueDate)}</span>{/if}
            </div>
          </div>
        {/each}
      </div>
      <input class="add-task" placeholder="+ задача" bind:value={adding[c.key]} onkeydown={(e) => { if (e.key === 'Enter') quickAdd(c.key); }} />
    </div>
  {/each}
</div>

{#if editing}
  <div class="modal-scrim open" role="dialog" tabindex="-1" aria-label="Задача"
       onmousedown={(e) => { if (e.currentTarget === e.target) (editing = null); }}
       onkeydown={(e) => { if (e.key === 'Escape') (editing = null); }}>
    <div class="modal">
      <div class="modal-head"><span class="t">Задача</span>
        <button class="icon-btn x" onclick={() => (editing = null)}><Icon name="x" class="ic" /></button></div>
      <div class="modal-body">
        <div class="field"><label for="et-title">Заголовок</label><input id="et-title" class="tin" bind:value={eTitle} /></div>
        <div class="field"><label for="et-desc">Описание</label><textarea id="et-desc" class="tin" bind:value={eDesc}></textarea></div>
        <div class="set-grid">
          <div class="field"><label for="et-pri">Приоритет</label>
            <select id="et-pri" class="tin" bind:value={ePriority}>
              <option value={0}>Низкий</option><option value={1}>Средний</option><option value={2}>Высокий</option>
            </select></div>
          <div class="field"><label for="et-status">Колонка</label>
            <select id="et-status" class="tin" bind:value={eStatus}>
              {#each columns as c}<option value={c.key}>{c.name}</option>{/each}
            </select></div>
        </div>
        <div class="field"><label for="et-due">Срок</label><input id="et-due" class="tin" type="date" bind:value={eDue} /></div>
      </div>
      <div class="modal-foot">
        <button class="btn-danger" onclick={deleteTask}><Icon name="trash-2" class="ic-sm" /> Удалить</button>
        <span class="spacer"></span>
        <button class="btn-ghost" onclick={() => (editing = null)}>Отмена</button>
        <button class="btn-primary" onclick={saveEdit}><Icon name="check" class="ic ic-sm" /> Сохранить</button>
      </div>
    </div>
  </div>
{/if}

{#if colEditing}
  <div class="modal-scrim open" role="dialog" tabindex="-1" aria-label="Колонка"
       onmousedown={(e) => { if (e.currentTarget === e.target) (colEditing = null); }}
       onkeydown={(e) => { if (e.key === 'Escape') (colEditing = null); }}>
    <div class="modal" style="max-width:440px">
      <div class="modal-head"><span class="t">{colNew ? "Новая колонка" : "Колонка"}</span>
        <button class="icon-btn x" onclick={() => (colEditing = null)}><Icon name="x" class="ic" /></button></div>
      <div class="modal-body">
        <div class="field"><label for="c-name">Название</label><input id="c-name" class="tin" bind:value={cName} placeholder="Review" /></div>
        <div class="toggle-row">
          <button class="toggle" class:on={cDone} onclick={() => (cDone = !cDone)} aria-pressed={cDone} aria-label="done"></button>
          <div class="tl">Колонка «выполнено»<small>Задачи здесь считаются завершёнными (проставляется дата выполнения).</small></div>
        </div>
      </div>
      <div class="modal-foot">
        {#if !colNew}<button class="btn-danger" onclick={deleteCol}><Icon name="trash-2" class="ic-sm" /> Удалить</button>{/if}
        <span class="spacer"></span>
        <button class="btn-ghost" onclick={() => (colEditing = null)}>Отмена</button>
        <button class="btn-primary" onclick={saveCol}><Icon name="check" class="ic ic-sm" /> Сохранить</button>
      </div>
    </div>
  </div>
{/if}
```
> Иконки `plus`/`ellipsis`/`calendar`/`x`/`trash-2`/`check` — из lucide. Классы `.kanban`/`.col`/`.tcard`/`.toggle` и т.д. — в global.css. `.kanban` получает inline `grid-template-columns` под число колонок.

- [ ] **Step 4: Проверка**
```powershell
npm run check
npm test
npm run build
```
Expected: 0 ошибок типов; Vitest 3; build успешен.

- [ ] **Step 5: Commit**
```powershell
git add src/lib/types.ts src/lib/api/columns.ts src/lib/components/TasksTab.svelte
git commit -m @'
feat(frontend): customizable kanban columns (add/edit/delete, dynamic board)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

## Task 4: Проверка среза

- [ ] **Step 1: Автопроверка**
```powershell
npm run check ; npm test ; cargo test --manifest-path src-tauri/Cargo.toml --lib
```
Expected: типы 0; Vitest 3; Rust 31 (миграции на v3).

- [ ] **Step 2: Ручной GUI-смоук (пользователь; перезапустить `npm run tauri dev`)**
- [ ] Вкладка «Задачи»: три дефолтные колонки (To Do / In Progress / Done), существующие задачи на местах.
- [ ] «+ Колонка» → название «Review» → создалась справа; задачу можно перетащить в неё.
- [ ] «…» на колонке → переименовать; тумблер «выполнено» → задачи в ней получают дату выполнения (и исключаются из повестки дашборда).
- [ ] Удалить колонку с задачами → задачи переехали в первую колонку; последнюю колонку удалить нельзя.
- [ ] Новый проект → у него тоже дефолтные 3 колонки.
- [ ] Перезапуск → колонки и задачи на месте.

---

## Итог среза

Канбан настраивается: свои колонки на проект (добавить/переименовать/удалить, флаг «выполнено»), задачи переносятся между ними; миграция 0003 + сиды дефолтов при создании/импорте. Закрывает основную часть роадмапа v2.0. Отложено (следующий срез): **метки задач + фильтры**, реордер колонок drag-ом, экспорт кастомных колонок.
```
