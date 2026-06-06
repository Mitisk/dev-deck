# DevDeck Phase 1 — Срез «Задачи (канбан)» — план реализации

> **Для исполнителя:** ОБЯЗАТЕЛЬНЫЙ ПОД-НАВЫК: superpowers:subagent-driven-development. Шаги — чекбоксы.

**Цель:** Вкладка «Задачи» — мини-канбан из трёх колонок (To Do / In Progress / Done). Карточка: заголовок, приоритет (цветная метка), срок. Быстрое добавление в каждой колонке, drag-and-drop между колонками (меняет статус), редактирование/удаление через диалог. Данные — в SQLite (`tasks`).

**Архитектура:** Логика — функции над `&Connection` (тестируемые без Tauri State), команды `tasks_*` лочат БД и вызывают их. `completed_at` ставится при переходе в `done`, очищается при выходе. Фронт: `TasksTab.svelte` грузит `tasks_list(projectId)`, группирует по статусу, DnD через нативные HTML5-события, после операций перечитывает список.

**Стек:** как раньше.

**Решения / границы:**
- DnD — **между колонками** (смена статуса, карточка добавляется в конец целевой колонки). Переупорядочивание **внутри** колонки drag-ом — отложено (команда `tasks_move` принимает `sortOrder`, задел есть; точное drop-позиционирование — позже).
- Срок (`dueDate`) — свободный текст (как в прототипе, напр. «5 июн»). Полноценный date-picker — позже.
- Приоритет — число 0/1/2 (0 низкий, 1 средний, 2 высокий), цвет/метка на фронте.
- «Свернуть выполненные» — отложено (колонка Done просто показывает карточки).

**Источники:** `TZ_DevDeck.md` (5.5 — задачи/канбан; 10 — `tasks_list/create/update/move/delete`). `_prototype/index.html` (`.kanban`/`.col`/`.tcard`/`.add-task`/`tasksHTML`/`wireTasks` — разметка и DnD-референс). Текущий `ProjectView.svelte` (таб «tasks» — плейсхолдер), `Project`.

---

## Контекст

- Rust: 18 команд; `models.rs` (Project/ProjectInput/GitStatus/GitOpResult); `error::{AppError,ErrorKind,AppResult}`; `commands/{health,projects,actions,git}.rs`. Таблица `tasks` уже в миграции 0001 (`id, project_id, title, description, status default 'todo', priority default 0, due_date, sort_order default 0, created_at, completed_at`).
- Фронт: `Project` (id:number). `ProjectView.svelte` рендерит SettingsTab на табе settings, иначе плейсхолдер; `activeTab` стор. api `client.ts`. `Icon.svelte`.
- global.css: `.kanban`, `.col`(`.drag-over`), `.col-head`(`.h`/`.n`/`.led`), `.col-body`, `.tcard`(`.dragging`), `.tt`, `.row`, `.tag-pri`, `.due`, `.add-task`; модалка `.modal-scrim`/`.modal`/`.field`/`.tin`/`.set-grid`/`.btn-*`.

---

## Структура файлов

```
src-tauri/src/
├─ models.rs            # MOD: + Task, TaskInput
├─ commands/tasks.rs    # NEW: list/create/update/move/delete + helpers + тест
├─ commands/mod.rs      # MOD: + pub mod tasks;
└─ lib.rs               # MOD: регистрация 5 команд

src/lib/
├─ types.ts             # MOD: + TaskStatus, Task
├─ api/tasks.ts         # NEW
└─ components/
   ├─ TasksTab.svelte   # NEW: канбан + DnD + quick-add + edit-диалог
   └─ ProjectView.svelte# MOD: рендер TasksTab на табе "tasks"
```

---

## Task 1: Rust — команды задач

**Files:** Modify `models.rs`, `commands/mod.rs`, `lib.rs`; Create `commands/tasks.rs`.

- [ ] **Step 1: Модели в models.rs** (после `GitOpResult`):
```rust
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: i64,
    pub project_id: i64,
    pub title: String,
    pub description: Option<String>,
    pub status: String, // todo | doing | done
    pub priority: i64,  // 0 | 1 | 2
    pub due_date: Option<String>,
    pub sort_order: i64,
    pub created_at: String,
    pub completed_at: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskInput {
    pub title: String,
    pub description: Option<String>,
    pub status: Option<String>,
    pub priority: Option<i64>,
    pub due_date: Option<String>,
}
```

- [ ] **Step 2: commands/tasks.rs**

Create `src-tauri/src/commands/tasks.rs`:
```rust
use crate::error::{AppError, AppResult, ErrorKind};
use crate::models::{Task, TaskInput};
use crate::state::AppState;
use rusqlite::{params, Connection};
use std::sync::MutexGuard;
use tauri::State;

fn lock<'a>(state: &'a State<AppState>) -> AppResult<MutexGuard<'a, Connection>> {
    state.db.lock().map_err(|_| AppError::internal("db mutex poisoned"))
}

fn row_to_task(conn: &Connection, id: i64) -> AppResult<Task> {
    Ok(conn.query_row(
        "SELECT id, project_id, title, description, status, priority, due_date, sort_order, created_at, completed_at
         FROM tasks WHERE id = ?1",
        [id],
        |r| {
            Ok(Task {
                id: r.get(0)?,
                project_id: r.get(1)?,
                title: r.get(2)?,
                description: r.get(3)?,
                status: r.get(4)?,
                priority: r.get(5)?,
                due_date: r.get(6)?,
                sort_order: r.get(7)?,
                created_at: r.get(8)?,
                completed_at: r.get(9)?,
            })
        },
    )?)
}

fn list_tasks(conn: &Connection, project_id: i64) -> AppResult<Vec<Task>> {
    let mut stmt = conn.prepare(
        "SELECT id FROM tasks WHERE project_id = ?1 ORDER BY sort_order ASC, id ASC",
    )?;
    let ids = stmt.query_map([project_id], |r| r.get::<_, i64>(0))?;
    let mut out = Vec::new();
    for id in ids {
        out.push(row_to_task(conn, id?)?);
    }
    Ok(out)
}

fn create_task(conn: &Connection, project_id: i64, input: TaskInput) -> AppResult<Task> {
    let title = input.title.trim();
    if title.is_empty() {
        return Err(AppError { kind: ErrorKind::Validation, message: "Заголовок задачи пуст".into() });
    }
    let status = input.status.as_deref().unwrap_or("todo");
    let priority = input.priority.unwrap_or(0).clamp(0, 2);
    let next_sort: i64 = conn.query_row(
        "SELECT COALESCE(MAX(sort_order), 0) + 1 FROM tasks WHERE project_id = ?1 AND status = ?2",
        params![project_id, status],
        |r| r.get(0),
    )?;
    conn.execute(
        "INSERT INTO tasks(project_id, title, description, status, priority, due_date, sort_order, completed_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, CASE WHEN ?4='done' THEN datetime('now') ELSE NULL END)",
        params![project_id, title, input.description, status, priority, input.due_date, next_sort],
    )?;
    row_to_task(conn, conn.last_insert_rowid())
}

fn update_task(conn: &Connection, id: i64, input: TaskInput) -> AppResult<Task> {
    let title = input.title.trim();
    if title.is_empty() {
        return Err(AppError { kind: ErrorKind::Validation, message: "Заголовок задачи пуст".into() });
    }
    let status = input.status.as_deref().unwrap_or("todo");
    let priority = input.priority.unwrap_or(0).clamp(0, 2);
    let n = conn.execute(
        "UPDATE tasks SET title=?2, description=?3, status=?4, priority=?5, due_date=?6,
                completed_at = CASE WHEN ?4='done' THEN COALESCE(completed_at, datetime('now')) ELSE NULL END
         WHERE id=?1",
        params![id, title, input.description, status, priority, input.due_date],
    )?;
    if n == 0 {
        return Err(AppError { kind: ErrorKind::NotFound, message: "Задача не найдена".into() });
    }
    row_to_task(conn, id)
}

fn move_task(conn: &Connection, id: i64, status: &str, sort_order: i64) -> AppResult<()> {
    conn.execute(
        "UPDATE tasks SET status=?2, sort_order=?3,
                completed_at = CASE WHEN ?2='done' THEN COALESCE(completed_at, datetime('now')) ELSE NULL END
         WHERE id=?1",
        params![id, status, sort_order],
    )?;
    Ok(())
}

fn delete_task(conn: &Connection, id: i64) -> AppResult<()> {
    conn.execute("DELETE FROM tasks WHERE id = ?1", [id])?;
    Ok(())
}

#[tauri::command]
pub fn tasks_list(state: State<AppState>, project_id: i64) -> AppResult<Vec<Task>> {
    list_tasks(&lock(&state)?, project_id)
}

#[tauri::command]
pub fn tasks_create(state: State<AppState>, project_id: i64, input: TaskInput) -> AppResult<Task> {
    create_task(&lock(&state)?, project_id, input)
}

#[tauri::command]
pub fn tasks_update(state: State<AppState>, id: i64, input: TaskInput) -> AppResult<Task> {
    update_task(&lock(&state)?, id, input)
}

#[tauri::command]
pub fn tasks_move(state: State<AppState>, id: i64, status: String, sort_order: i64) -> AppResult<()> {
    move_task(&lock(&state)?, id, &status, sort_order)
}

#[tauri::command]
pub fn tasks_delete(state: State<AppState>, id: i64) -> AppResult<()> {
    delete_task(&lock(&state)?, id)
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

    fn project(conn: &Connection) -> i64 {
        conn.execute("INSERT INTO projects(name, status, sort_order) VALUES ('P', 'active', 0)", []).unwrap();
        conn.last_insert_rowid()
    }

    fn input(title: &str, status: &str) -> TaskInput {
        TaskInput { title: title.into(), description: None, status: Some(status.into()), priority: Some(2), due_date: None }
    }

    #[test]
    fn create_list_move_delete() {
        let conn = mem();
        let pid = project(&conn);
        let a = create_task(&conn, pid, input("A", "todo")).unwrap();
        create_task(&conn, pid, input("B", "doing")).unwrap();
        assert_eq!(list_tasks(&conn, pid).unwrap().len(), 2);
        assert_eq!(a.priority, 2);
        assert!(a.completed_at.is_none());

        move_task(&conn, a.id, "done", 7).unwrap();
        let done = list_tasks(&conn, pid).unwrap().into_iter().find(|t| t.id == a.id).unwrap();
        assert_eq!(done.status, "done");
        assert_eq!(done.sort_order, 7);
        assert!(done.completed_at.is_some());

        delete_task(&conn, a.id).unwrap();
        assert_eq!(list_tasks(&conn, pid).unwrap().len(), 1);
    }

    #[test]
    fn deleted_with_project_cascade() {
        let conn = mem();
        let pid = project(&conn);
        create_task(&conn, pid, input("X", "todo")).unwrap();
        conn.execute("DELETE FROM projects WHERE id = ?1", [pid]).unwrap();
        assert_eq!(list_tasks(&conn, pid).unwrap().len(), 0);
    }
}
```

- [ ] **Step 3: Регистрация**
- `commands/mod.rs`: `pub mod tasks;`
- `lib.rs` `generate_handler![...]`: добавить
```rust
            commands::tasks::tasks_list,
            commands::tasks::tasks_create,
            commands::tasks::tasks_update,
            commands::tasks::tasks_move,
            commands::tasks::tasks_delete,
```

- [ ] **Step 4: Тесты + сборка**
```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib
cargo build --manifest-path src-tauri/Cargo.toml
```
Expected: новые `create_list_move_delete`, `deleted_with_project_cascade` + прежние (10) → 12 ok; build успешен.

- [ ] **Step 5: Commit**
```powershell
git add src-tauri/src/models.rs src-tauri/src/commands/tasks.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs
git commit -m @'
feat(backend): tasks CRUD + move commands (kanban)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

## Task 2: Фронт — TasksTab (канбан)

**Files:** Modify `types.ts`, `ProjectView.svelte`; Create `api/tasks.ts`, `components/TasksTab.svelte`.

- [ ] **Step 1: Типы** (в конец `src/lib/types.ts`):
```ts
export type TaskStatus = "todo" | "doing" | "done";
export type Task = {
  id: number;
  projectId: number;
  title: string;
  description: string | null;
  status: TaskStatus;
  priority: number; // 0 | 1 | 2
  dueDate: string | null;
  sortOrder: number;
  createdAt: string;
  completedAt: string | null;
};
```

- [ ] **Step 2: api/tasks.ts**
```ts
import { call } from "./client";
import type { Task, TaskStatus } from "../types";

export type TaskInput = {
  title: string;
  description?: string | null;
  status?: TaskStatus | null;
  priority?: number | null;
  dueDate?: string | null;
};

export const list = (projectId: number) => call<Task[]>("tasks_list", { projectId });
export const create = (projectId: number, input: TaskInput) => call<Task>("tasks_create", { projectId, input });
export const update = (id: number, input: TaskInput) => call<Task>("tasks_update", { id, input });
export const move = (id: number, status: TaskStatus, sortOrder: number) =>
  call<void>("tasks_move", { id, status, sortOrder });
export const remove = (id: number) => call<void>("tasks_delete", { id });
```

- [ ] **Step 3: components/TasksTab.svelte**

Create `src/lib/components/TasksTab.svelte`:
```svelte
<script lang="ts">
  import type { Project, Task, TaskStatus } from "$lib/types";
  import * as tasksApi from "$lib/api/tasks";
  import Icon from "./Icon.svelte";

  let { project }: { project: Project } = $props();

  const COLUMNS: { key: TaskStatus; label: string; led: string }[] = [
    { key: "todo", label: "To Do", led: "#5b9cff" },
    { key: "doing", label: "In Progress", led: "#e0a83a" },
    { key: "done", label: "Done", led: "#3fb863" },
  ];
  const PRIORITY = [
    { label: "Низкий", color: "#5b9cff" },
    { label: "Средний", color: "#e0a83a" },
    { label: "Высокий", color: "#f0616d" },
  ];

  let tasks = $state<Task[]>([]);
  let adding = $state<Record<TaskStatus, string>>({ todo: "", doing: "", done: "" });
  let dragId = $state<number | null>(null);
  let dragging = $state(false);
  let overCol = $state<TaskStatus | null>(null);

  // edit dialog
  let editing = $state<Task | null>(null);
  let eTitle = $state("");
  let eDesc = $state("");
  let ePriority = $state(0);
  let eDue = $state("");
  let eStatus = $state<TaskStatus>("todo");

  let reqId = 0;
  async function load() {
    const my = ++reqId;
    try {
      const list = await tasksApi.list(project.id);
      if (my === reqId) tasks = list;
    } catch {
      /* тост из api/client.ts */
    }
  }
  $effect(() => {
    project.id;
    load();
  });

  function col(status: TaskStatus): Task[] {
    return tasks.filter((t) => t.status === status).sort((a, b) => a.sortOrder - b.sortOrder);
  }

  async function quickAdd(status: TaskStatus) {
    const title = (adding[status] ?? "").trim();
    if (!title) return;
    adding[status] = "";
    await tasksApi.create(project.id, { title, status });
    await load();
  }

  async function onDrop(status: TaskStatus) {
    overCol = null;
    const id = dragId;
    if (id == null) return;
    const t = tasks.find((x) => x.id === id);
    if (!t || t.status === status) return;
    const nextSort = Math.max(0, ...col(status).map((x) => x.sortOrder)) + 1;
    await tasksApi.move(id, status, nextSort);
    await load();
  }

  function openEdit(t: Task) {
    if (dragging) return;
    editing = t;
    eTitle = t.title;
    eDesc = t.description ?? "";
    ePriority = t.priority;
    eDue = t.dueDate ?? "";
    eStatus = t.status;
  }

  async function saveEdit() {
    if (!editing) return;
    const title = eTitle.trim();
    if (!title) return;
    await tasksApi.update(editing.id, {
      title,
      description: eDesc.trim() || null,
      priority: ePriority,
      dueDate: eDue.trim() || null,
      status: eStatus,
    });
    editing = null;
    await load();
  }

  async function deleteTask() {
    if (!editing) return;
    const id = editing.id;
    editing = null;
    await tasksApi.remove(id);
    await load();
  }
</script>

<div class="kanban">
  {#each COLUMNS as c (c.key)}
    <div
      class="col"
      class:drag-over={overCol === c.key}
      role="list"
      ondragover={(e) => { e.preventDefault(); overCol = c.key; }}
      ondragleave={() => { if (overCol === c.key) overCol = null; }}
      ondrop={() => onDrop(c.key)}
    >
      <div class="col-head">
        <span class="led" style="background:{c.led}"></span>
        <span class="h">{c.label}</span>
        <span class="n">{col(c.key).length}</span>
      </div>
      <div class="col-body">
        {#each col(c.key) as t (t.id)}
          <div
            class="tcard"
            class:dragging={dragId === t.id}
            draggable={true}
            role="button"
            tabindex="0"
            ondragstart={() => { dragId = t.id; dragging = true; }}
            ondragend={() => { dragging = false; setTimeout(() => (dragId = null), 0); }}
            onclick={() => openEdit(t)}
          >
            <div class="tt">{t.title}</div>
            <div class="row">
              <span class="tag-pri" style="color:{PRIORITY[t.priority].color};background:color-mix(in oklab, {PRIORITY[t.priority].color} 14%, transparent)">
                {PRIORITY[t.priority].label}
              </span>
              {#if t.dueDate}<span class="due"><Icon name="calendar" class="ic-sm" /> {t.dueDate}</span>{/if}
            </div>
          </div>
        {/each}
      </div>
      <input
        class="add-task"
        placeholder="+ задача"
        bind:value={adding[c.key]}
        onkeydown={(e) => { if (e.key === "Enter") quickAdd(c.key); }}
      />
    </div>
  {/each}
</div>

{#if editing}
  <div
    class="modal-scrim open"
    role="dialog"
    tabindex="-1"
    aria-label="Задача"
    onmousedown={(e) => { if (e.currentTarget === e.target) (editing = null); }}
    onkeydown={(e) => { if (e.key === "Escape") (editing = null); }}
  >
    <div class="modal">
      <div class="modal-head">
        <span class="t">Задача</span>
        <button class="icon-btn x" onclick={() => (editing = null)}><Icon name="x" class="ic" /></button>
      </div>
      <div class="modal-body">
        <div class="field">
          <label for="et-title">Заголовок</label>
          <input id="et-title" class="tin" bind:value={eTitle} />
        </div>
        <div class="field">
          <label for="et-desc">Описание</label>
          <textarea id="et-desc" class="tin" bind:value={eDesc}></textarea>
        </div>
        <div class="set-grid">
          <div class="field">
            <label for="et-pri">Приоритет</label>
            <select id="et-pri" class="tin" bind:value={ePriority}>
              <option value={0}>Низкий</option>
              <option value={1}>Средний</option>
              <option value={2}>Высокий</option>
            </select>
          </div>
          <div class="field">
            <label for="et-status">Статус</label>
            <select id="et-status" class="tin" bind:value={eStatus}>
              <option value="todo">To Do</option>
              <option value="doing">In Progress</option>
              <option value="done">Done</option>
            </select>
          </div>
        </div>
        <div class="field">
          <label for="et-due">Срок</label>
          <input id="et-due" class="tin" bind:value={eDue} placeholder="напр. 5 июн" />
        </div>
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
```
> `bind:value={ePriority}` с `<option value={0}>` биндит число. `bind:value={adding[c.key]}` — биндинг к свойству $state-объекта (Svelte 5 поддерживает). Иконки `calendar`/`x`/`trash-2`/`check` — из lucide. Переупорядочивание внутри колонки drag-ом не реализуем (отложено).

- [ ] **Step 4: Рендер TasksTab в ProjectView.svelte**

В `src/lib/components/ProjectView.svelte`:
1. Импорт: `import TasksTab from "./TasksTab.svelte";`
2. В блоке `<div class="tab-body">` добавить ветку перед `{:else}`-плейсхолдером. Сейчас там:
```svelte
    {#if $activeTab === "settings"}
      <SettingsTab {project} />
    {:else}
      ...placeholder...
    {/if}
```
Заменить на:
```svelte
    {#if $activeTab === "settings"}
      <SettingsTab {project} />
    {:else if $activeTab === "tasks"}
      <TasksTab {project} />
    {:else}
      {#if project.description}<p class="desc">{project.description}</p>{/if}
      <p class="desc" style="color:var(--muted);margin-top:14px">
        Вкладка «{activeLabel}» — содержимое появится в следующих срезах Phase 1.
      </p>
    {/if}
```

- [ ] **Step 5: Проверка**
```powershell
npm run check
npm test
npm run build
```
Expected: `npm run check` 0 ОШИБОК (a11y/`node`/`state_referenced_locally` warnings приемлемы); Vitest 3 passed; build успешен.

- [ ] **Step 6: Commit**
```powershell
git add src/lib/types.ts src/lib/api/tasks.ts src/lib/components/TasksTab.svelte src/lib/components/ProjectView.svelte
git commit -m @'
feat(frontend): tasks kanban tab (columns, quick-add, drag, edit)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

## Task 3: Проверка среза

- [ ] **Step 1: Автопроверка**
```powershell
npm run check ; npm test ; cargo test --manifest-path src-tauri/Cargo.toml --lib
```
Expected: типы 0 ошибок; Vitest 3; Rust 12 (10 прежних + 2 tasks).

- [ ] **Step 2: Ручной GUI-смоук (пользователь; `npm run tauri dev`)**
- [ ] Открыть проект → вкладка «Задачи» → три колонки.
- [ ] В «To Do» ввести текст в «+ задача» + Enter → карточка появилась; счётчик колонки вырос.
- [ ] Перетащить карточку в «In Progress»/«Done» → статус сменился, карточка в новой колонке (после перезапуска сохраняется).
- [ ] Клик по карточке → диалог: правка заголовка/описания/приоритета/статуса/срока → «Сохранить» → карточка обновилась; цвет метки приоритета изменился.
- [ ] «Удалить» в диалоге → задача исчезла.
- [ ] Перезапуск приложения → задачи на месте (в БД).

---

## Итог среза

Вкладка «Задачи» — рабочий канбан на SQLite: быстрое добавление, перетаскивание между колонками (со сменой статуса и `completed_at`), редактирование и удаление через диалог. Следующий срез — **Чеклисты**. Отложено: переупорядочивание внутри колонки drag-ом, сворачивание выполненных, date-picker для срока.
```
