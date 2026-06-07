# DevDeck v2.0 — Срез «Метки задач + фильтры» — план реализации

> **Для исполнителя:** ОБЯЗАТЕЛЬНЫЙ ПОД-НАВЫК: superpowers:subagent-driven-development. Шаги — чекбоксы.

**Цель:** Цветные метки на проект, назначаемые задачам (M:N). В канбане — фильтр по меткам и отображение меток на карточках. Закрывает роадмап ТЗ.

**Архитектура:** Таблицы `labels(project_id,name,color,sort_order)` + `task_labels(task_id,label_id)` (миграция 0004). `Task` возвращает `label_ids`. Команды `labels_*` + `task_set_labels`. Фронт: в TasksTab — фильтр-бар (toggle-чипы меток), метки на карточках, выбор меток в edit-диалоге, менеджер меток.

**Стек:** миграция БД 0004. Без новых crate. Фильтрация — клиентская.

**Решения / границы:**
- Метки — на проект, цветные. Фильтр: задача показывается, если у неё есть хотя бы одна из выбранных меток (ИЛИ). Пустой фильтр — показывать всё.
- Экспорт/импорт меток — **отложен** (export/import пока без меток; отмечено).

**Источники:** `TZ_DevDeck.md` (11 — метки/фильтры v2.0; 5.5). Текущие `commands/tasks.rs` (Task без меток), `TasksTab.svelte` (динамические колонки), `db/migrations.rs`.

---

## Контекст

- Rust: 76 команд; миграции 0001–0003 (версия 3); `tasks.rs` (`Task`, `row_to_task`); `models.rs`. Таблица `tasks`.
- Фронт: `TasksTab.svelte` (динамический канбан по колонкам); `api/tasks.ts`, `api/columns.ts`. global.css: `.chip`, `.tag-pri`, модалка, `.toggle`, `.color-pick`.

---

## Структура файлов

```
src-tauri/
├─ migrations/0004_labels.sql   # NEW
└─ src/
   ├─ db/migrations.rs          # MOD: + (4, ...) + тесты версии 4 / таблицы
   ├─ models.rs                 # MOD: + Label; Task += label_ids
   ├─ commands/labels.rs        # NEW: labels_* + task_set_labels
   ├─ commands/tasks.rs         # MOD: row_to_task грузит label_ids
   ├─ commands/mod.rs           # MOD: + pub mod labels;
   └─ lib.rs                    # MOD: регистрация labels-команд

src/lib/
├─ types.ts                     # MOD: + Label; Task += labelIds
├─ api/labels.ts                # NEW
└─ components/TasksTab.svelte    # MOD: фильтр-бар + метки на карточках + выбор в диалоге + менеджер
```

---

## Task 1: Rust — метки

**Files:** Create `migrations/0004_labels.sql`, `commands/labels.rs`; Modify `db/migrations.rs`, `models.rs`, `commands/tasks.rs`, `commands/mod.rs`, `lib.rs`.

- [ ] **Step 1: Миграция 0004** — Create `src-tauri/migrations/0004_labels.sql`:
```sql
CREATE TABLE labels (
    id         INTEGER PRIMARY KEY,
    project_id INTEGER REFERENCES projects(id) ON DELETE CASCADE,
    name       TEXT NOT NULL,
    color      TEXT,
    sort_order INTEGER DEFAULT 0
);
CREATE TABLE task_labels (
    task_id  INTEGER REFERENCES tasks(id) ON DELETE CASCADE,
    label_id INTEGER REFERENCES labels(id) ON DELETE CASCADE,
    PRIMARY KEY (task_id, label_id)
);

PRAGMA user_version = 4;
```

- [ ] **Step 2: Раннер + тесты** — `db/migrations.rs`:
1. В `MIGRATIONS` добавить `(4, include_str!("../../migrations/0004_labels.sql"))`.
2. Обновить тесты версии: `applies_migrations_on_empty_db` и `run_is_idempotent` → версия `4`. Добавить проверку таблицы `labels`:
```rust
        let lbl: i64 = conn.query_row("SELECT count(*) FROM sqlite_master WHERE type='table' AND name='labels'", [], |r| r.get(0)).unwrap();
        assert_eq!(lbl, 1);
```

- [ ] **Step 3: Модели** — в `models.rs`:
1. Добавить `Label` (после `TaskColumn`):
```rust
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Label {
    pub id: i64,
    pub project_id: i64,
    pub name: String,
    pub color: Option<String>,
    pub sort_order: i64,
}
```
2. В `Task` добавить поле (после `completed_at`):
```rust
    pub label_ids: Vec<i64>,
```

- [ ] **Step 4: row_to_task грузит метки** — в `commands/tasks.rs` функция `row_to_task`: после получения задачи догрузить `label_ids`. Текущая `row_to_task` использует `conn.query_row(...)`. Переписать так, чтобы вернуть Task с метками:
```rust
fn row_to_task(conn: &Connection, id: i64) -> AppResult<Task> {
    let mut t = conn.query_row(
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
                label_ids: Vec::new(),
            })
        },
    )?;
    let mut stmt = conn.prepare("SELECT label_id FROM task_labels WHERE task_id=?1 ORDER BY label_id")?;
    t.label_ids = stmt.query_map([id], |r| r.get::<_, i64>(0))?.collect::<Result<_, _>>()?;
    Ok(t)
}
```

- [ ] **Step 5: commands/labels.rs**

Create `src-tauri/src/commands/labels.rs`:
```rust
use crate::error::{AppError, AppResult, ErrorKind};
use crate::models::Label;
use crate::state::AppState;
use rusqlite::{params, Connection};
use std::sync::MutexGuard;
use tauri::State;

fn lock<'a>(state: &'a State<AppState>) -> AppResult<MutexGuard<'a, Connection>> {
    state.db.lock().map_err(|_| AppError::internal("db mutex poisoned"))
}

fn row_to_label(conn: &Connection, id: i64) -> AppResult<Label> {
    Ok(conn.query_row(
        "SELECT id, project_id, name, color, sort_order FROM labels WHERE id=?1",
        [id],
        |r| Ok(Label { id: r.get(0)?, project_id: r.get(1)?, name: r.get(2)?, color: r.get(3)?, sort_order: r.get(4)? }),
    )?)
}

#[tauri::command]
pub fn labels_list(state: State<AppState>, project_id: i64) -> AppResult<Vec<Label>> {
    let conn = lock(&state)?;
    let mut stmt = conn.prepare("SELECT id FROM labels WHERE project_id=?1 ORDER BY sort_order, id")?;
    let ids = stmt.query_map([project_id], |r| r.get::<_, i64>(0))?;
    let mut out = Vec::new();
    for id in ids { out.push(row_to_label(&conn, id?)?); }
    Ok(out)
}

#[tauri::command]
pub fn label_create(state: State<AppState>, project_id: i64, name: String, color: Option<String>) -> AppResult<Label> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError { kind: ErrorKind::Validation, message: "Название метки пусто".into() });
    }
    let conn = lock(&state)?;
    let next: i64 = conn.query_row("SELECT COALESCE(MAX(sort_order),0)+1 FROM labels WHERE project_id=?1", [project_id], |r| r.get(0))?;
    conn.execute("INSERT INTO labels(project_id,name,color,sort_order) VALUES(?1,?2,?3,?4)", params![project_id, name, color, next])?;
    row_to_label(&conn, conn.last_insert_rowid())
}

#[tauri::command]
pub fn label_update(state: State<AppState>, id: i64, name: String, color: Option<String>) -> AppResult<Label> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError { kind: ErrorKind::Validation, message: "Название метки пусто".into() });
    }
    let conn = lock(&state)?;
    conn.execute("UPDATE labels SET name=?2, color=?3 WHERE id=?1", params![id, name, color])?;
    row_to_label(&conn, id)
}

#[tauri::command]
pub fn label_delete(state: State<AppState>, id: i64) -> AppResult<()> {
    let conn = lock(&state)?;
    conn.execute("DELETE FROM labels WHERE id=?1", [id])?;
    Ok(())
}

/// Заменить набор меток задачи.
#[tauri::command]
pub fn task_set_labels(state: State<AppState>, task_id: i64, label_ids: Vec<i64>) -> AppResult<()> {
    let conn = lock(&state)?;
    conn.execute("DELETE FROM task_labels WHERE task_id=?1", [task_id])?;
    for lid in label_ids {
        conn.execute("INSERT OR IGNORE INTO task_labels(task_id,label_id) VALUES(?1,?2)", params![task_id, lid])?;
    }
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
    fn labels_crud_and_task_assignment() {
        let conn = mem();
        conn.execute("INSERT INTO projects(name,status,sort_order) VALUES('P','active',0)", []).unwrap();
        let pid = conn.last_insert_rowid();
        conn.execute("INSERT INTO labels(project_id,name,color,sort_order) VALUES(?1,'bug','#f00',0)", [pid]).unwrap();
        let l1 = conn.last_insert_rowid();
        conn.execute("INSERT INTO labels(project_id,name,color,sort_order) VALUES(?1,'urgent','#0f0',1)", [pid]).unwrap();
        let l2 = conn.last_insert_rowid();
        conn.execute("INSERT INTO tasks(project_id,title,status,sort_order) VALUES(?1,'T','todo',0)", [pid]).unwrap();
        let tid = conn.last_insert_rowid();

        // назначить две метки
        conn.execute("INSERT INTO task_labels(task_id,label_id) VALUES(?1,?2)", params![tid, l1]).unwrap();
        conn.execute("INSERT INTO task_labels(task_id,label_id) VALUES(?1,?2)", params![tid, l2]).unwrap();
        let cnt: i64 = conn.query_row("SELECT count(*) FROM task_labels WHERE task_id=?1", [tid], |r| r.get(0)).unwrap();
        assert_eq!(cnt, 2);

        // удаление метки каскадит связь
        conn.execute("DELETE FROM labels WHERE id=?1", [l1]).unwrap();
        let cnt2: i64 = conn.query_row("SELECT count(*) FROM task_labels WHERE task_id=?1", [tid], |r| r.get(0)).unwrap();
        assert_eq!(cnt2, 1);
    }
}
```

- [ ] **Step 6: Регистрация** — `commands/mod.rs`: `pub mod labels;`; `lib.rs` `generate_handler![...]`: добавить
```rust
            commands::labels::labels_list,
            commands::labels::label_create,
            commands::labels::label_update,
            commands::labels::label_delete,
            commands::labels::task_set_labels,
```

- [ ] **Step 7: Тесты + lib-сборка**
```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib
cargo build --manifest-path src-tauri/Cargo.toml --lib
```
Expected: новый `labels_crud_and_task_assignment` + миграционные (версия 4) + прежние → 32 ok; lib-сборка успешна.

- [ ] **Step 8: Commit**
```powershell
git add src-tauri/migrations/0004_labels.sql src-tauri/src/db/migrations.rs src-tauri/src/models.rs src-tauri/src/commands/labels.rs src-tauri/src/commands/tasks.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs
git commit -m @'
feat(backend): task labels (labels + task_labels, migration 0004)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

## Task 2: Фронт — метки + фильтр в TasksTab

**Files:** Modify `types.ts`, `components/TasksTab.svelte`; Create `api/labels.ts`.

- [ ] **Step 1: Типы** — в `types.ts`:
1. Добавить `Label`:
```ts
export type Label = { id: number; projectId: number; name: string; color: string | null; sortOrder: number };
```
2. В тип `Task` добавить поле `labelIds: number[];`.

- [ ] **Step 2: api/labels.ts**
```ts
import { call } from "./client";
import type { Label } from "../types";

export const list = (projectId: number) => call<Label[]>("labels_list", { projectId });
export const create = (projectId: number, name: string, color: string | null) => call<Label>("label_create", { projectId, name, color });
export const update = (id: number, name: string, color: string | null) => call<Label>("label_update", { id, name, color });
export const remove = (id: number) => call<void>("label_delete", { id });
export const setTaskLabels = (taskId: number, labelIds: number[]) => call<void>("task_set_labels", { taskId, labelIds });
```

- [ ] **Step 3: TasksTab.svelte — метки и фильтр**

В `src/lib/components/TasksTab.svelte` (текущая динамическая версия) добавить:

1. Импорты в `<script>`:
```ts
  import * as labelsApi from "$lib/api/labels";
  import type { Label } from "$lib/types";

  const LABEL_COLORS = ["#7c7dff","#c77dff","#3fb863","#e0a83a","#f0616d","#5b9cff","#19c3c0","#ff8b5b"];
  let labels = $state<Label[]>([]);
  let activeLabels = $state<number[]>([]);
  // менеджер меток
  let showLabels = $state(false);
  let newLabelName = $state("");
  let newLabelColor = $state(LABEL_COLORS[0]);
  // выбор меток в edit-диалоге
  let eLabels = $state<number[]>([]);
```

2. В `load()` догрузить метки (в `Promise.all`):
```ts
    const [cols, ts, lbs] = await Promise.all([columnsApi.list(project.id), tasksApi.list(project.id), labelsApi.list(project.id)]);
    if (my === reqId) { columns = cols; tasks = ts; labels = lbs; }
```

3. Хелперы и функции:
```ts
  function labelById(id: number): Label | undefined { return labels.find((l) => l.id === id); }
  function taskLabels(t: Task): Label[] { return t.labelIds.map(labelById).filter((l): l is Label => !!l); }
  function toggleFilter(id: number) {
    activeLabels = activeLabels.includes(id) ? activeLabels.filter((x) => x !== id) : [...activeLabels, id];
  }
  // задачи колонки с учётом фильтра
  function visibleColTasks(key: string): Task[] {
    const list = colTasks(key);
    if (!activeLabels.length) return list;
    return list.filter((t) => t.labelIds.some((id) => activeLabels.includes(id)));
  }

  async function addLabel() {
    if (!newLabelName.trim()) return;
    await labelsApi.create(project.id, newLabelName.trim(), newLabelColor);
    newLabelName = "";
    await load();
  }
  async function deleteLabel(id: number) {
    activeLabels = activeLabels.filter((x) => x !== id);
    await labelsApi.remove(id);
    await load();
  }
  function toggleEditLabel(id: number) {
    eLabels = eLabels.includes(id) ? eLabels.filter((x) => x !== id) : [...eLabels, id];
  }
```

4. В `openEdit(t)` — инициализировать выбранные метки:
```ts
    eLabels = [...t.labelIds];
```
(добавить строку рядом с `eStatus = t.status;`)

5. В `saveEdit()` — после `tasksApi.update(...)` (до `editing = null`) сохранить метки:
```ts
    await labelsApi.setTaskLabels(editing.id, eLabels);
```

6. Заменить вызовы `colTasks(c.key)` в РАЗМЕТКЕ доски на `visibleColTasks(c.key)` (и в счётчике колонки, и в `{#each}`), чтобы фильтр действовал. (Счётчик `.n` оставить общим `colTasks(c.key).length` ИЛИ показать отфильтрованное — на выбор; рекомендую отфильтрованное `visibleColTasks(c.key).length`.)

7. Разметка фильтр-бара — заменить существующую шапку (`<div ...><span style="flex:1"></span><button ...>Колонка</button></div>`) на:
```svelte
<div style="display:flex;align-items:center;gap:8px;margin-bottom:10px;flex-wrap:wrap">
  {#each labels as l (l.id)}
    <button class="chip" style="cursor:pointer;border-color:{l.color ?? 'var(--border-2)'};{activeLabels.includes(l.id) ? `background:color-mix(in oklab, ${l.color ?? 'var(--accent)'} 22%, transparent);color:var(--text)` : ''}"
            onclick={() => toggleFilter(l.id)}>{l.name}</button>
  {/each}
  {#if activeLabels.length}<button class="chip" onclick={() => (activeLabels = [])}>сбросить</button>{/if}
  <span style="flex:1"></span>
  <button class="gbtn" onclick={() => (showLabels = true)}><Icon name="tag" class="ic-sm" /> Метки</button>
  <button class="gbtn" onclick={openNewCol}><Icon name="plus" class="ic-sm" /> Колонка</button>
</div>
```

8. На карточке задачи (в `.row`, после метки приоритета) — чипы меток:
```svelte
              {#each taskLabels(t) as l}<span class="chip" style="border-color:{l.color ?? 'var(--border-2)'};color:{l.color ?? 'var(--muted)'}">{l.name}</span>{/each}
```

9. В edit-диалоге задачи (в `.modal-body`, после поля «Срок») — выбор меток:
```svelte
        <div class="field">
          <label>Метки</label>
          <div class="chips">
            {#each labels as l}
              <button class="chip" style="cursor:pointer;border-color:{l.color ?? 'var(--border-2)'};{eLabels.includes(l.id) ? `background:color-mix(in oklab, ${l.color ?? 'var(--accent)'} 22%, transparent);color:var(--text)` : ''}"
                      onclick={() => toggleEditLabel(l.id)}>{l.name}</button>
            {/each}
            {#if !labels.length}<span style="color:var(--muted-2);font-size:12px">Меток нет — создайте через «Метки».</span>{/if}
          </div>
        </div>
```

10. Менеджер меток — модалка (добавить в конец компонента):
```svelte
{#if showLabels}
  <div class="modal-scrim open" role="dialog" tabindex="-1" aria-label="Метки"
       onmousedown={(e) => { if (e.currentTarget === e.target) (showLabels = false); }}
       onkeydown={(e) => { if (e.key === 'Escape') (showLabels = false); }}>
    <div class="modal" style="max-width:440px">
      <div class="modal-head"><span class="t">Метки проекта</span>
        <button class="icon-btn x" onclick={() => (showLabels = false)}><Icon name="x" class="ic" /></button></div>
      <div class="modal-body">
        {#each labels as l (l.id)}
          <div class="link-row" style="padding:6px 4px">
            <span class="chip" style="border-color:{l.color ?? 'var(--border-2)'};color:{l.color ?? 'var(--muted)'}">{l.name}</span>
            <span style="flex:1"></span>
            <button class="mini" title="Удалить" onclick={() => deleteLabel(l.id)}><Icon name="trash-2" class="ic-sm" /></button>
          </div>
        {/each}
        <div class="erow" style="margin-top:8px">
          <input class="tin" placeholder="Новая метка" bind:value={newLabelName} onkeydown={(e) => { if (e.key === 'Enter') addLabel(); }} />
          <div class="set-pick-color">
            {#each LABEL_COLORS as c}<button class="color-pick" class:sel={newLabelColor === c} style="--c:{c};background:{c}" onclick={() => (newLabelColor = c)} aria-label="цвет"></button>{/each}
          </div>
          <button class="er-del" style="color:var(--accent)" title="Добавить" onclick={addLabel}><Icon name="plus" class="ic-sm" /></button>
        </div>
      </div>
      <div class="modal-foot"><span class="spacer"></span><button class="btn-ghost" onclick={() => (showLabels = false)}>Закрыть</button></div>
    </div>
  </div>
{/if}
```
> Иконки `tag`/`plus`/`x`/`trash-2` — из lucide. Классы `.chip`/`.chips`/`.color-pick`/`.erow` — в global.css.

- [ ] **Step 4: Проверка**
```powershell
npm run check
npm test
npm run build
```
Expected: 0 ошибок типов; Vitest 3; build успешен.

- [ ] **Step 5: Commit**
```powershell
git add src/lib/types.ts src/lib/api/labels.ts src/lib/components/TasksTab.svelte
git commit -m @'
feat(frontend): task labels with kanban filter and label manager

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

## Task 3: Проверка среза

- [ ] **Step 1: Автопроверка**
```powershell
npm run check ; npm test ; cargo test --manifest-path src-tauri/Cargo.toml --lib
```
Expected: типы 0; Vitest 3; Rust 32 (миграции v4).

- [ ] **Step 2: Ручной GUI-смоук (пользователь; перезапустить `npm run tauri dev`)**
- [ ] Вкладка «Задачи» → «Метки» → создать метки (с цветом), удалить.
- [ ] Открыть задачу → в диалоге выбрать метки → «Сохранить» → на карточке появились цветные чипы.
- [ ] В фильтр-баре кликнуть метку → на доске остаются только задачи с этой меткой; «сбросить» → все.
- [ ] Удалить метку → она пропадает с карточек и из фильтра.
- [ ] Перезапуск → метки и назначения на месте.

---

## Итог среза

Метки задач (на проект, цветные) с назначением в диалоге, чипами на карточках и фильтром по доске; миграция 0004. **Закрывает роадмап ТЗ (MVP → v1.0 → v2.0).** Отложено: экспорт/импорт меток, фильтр по приоритету, реордер меток.
```
