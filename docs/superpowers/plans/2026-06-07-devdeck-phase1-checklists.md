# DevDeck Phase 1 — Срез «Чеклисты» — план реализации

> **Для исполнителя:** ОБЯЗАТЕЛЬНЫЙ ПОД-НАВЫК: superpowers:subagent-driven-development. Шаги — чекбоксы.

**Цель:** Вкладка «Чеклисты» — несколько чеклистов на проект, каждый со списком пунктов (галочка + текст) и прогресс-баром (X из Y). Создание/переименование/удаление чеклиста; добавление/правка/переключение/удаление пунктов. Данные — в SQLite.

**Архитектура:** Логика — функции над `&Connection` (тестируемые), команды `checklists_*`/`checklist_items_*` лочат БД и вызывают их. `checklists_list` возвращает чеклисты вместе с их пунктами. Фронт: `ChecklistsTab.svelte` грузит `checklists_list(projectId)`, рисует карточки с прогрессом, после операций перечитывает.

**Решения / границы:**
- **Шаблоны чеклистов** — отложены (раздел 5.6/10 ТЗ, элемент v1.0). Только обычные чеклисты.
- Перетаскивание пунктов (reorder drag-ом) — отложено; порядок по `sort_order` (добавление в конец).
- Прогресс считается на фронте (done/total).

**Источники:** `TZ_DevDeck.md` (5.6 — чеклисты; 10 — `checklists_*`/`checklist_items_*`). `_prototype/index.html` (`.checklists`/`.checklist`/`.cl-head`/`.progress`/`.cl-item`/`.cbox`/`.cl-add` — разметка-референс; `checklistsHTML`/`wireChecklists`). Текущий `ProjectView.svelte` (таб «checklists» — плейсхолдер).

---

## Контекст

- Rust: 23 команды; `models.rs` (Project/ProjectInput/GitStatus/GitOpResult/Task/TaskInput); `commands/{health,projects,actions,git,tasks}.rs`. Таблицы `checklists`(`id,project_id,title,sort_order`) и `checklist_items`(`id,checklist_id,text,is_done default 0,sort_order default 0`) — в миграции 0001 (FK с ON DELETE CASCADE).
- Фронт: `ProjectView.svelte` рендерит SettingsTab/TasksTab по табу, иначе плейсхолдер. api `client.ts`, `Icon.svelte`.
- global.css: `.checklists`, `.checklist`, `.cl-head`(`.t`/`.pc`), `.progress`(`> i`), `.cl-items`, `.cl-item`(`.done`), `.cbox`, `.lab`, `.cl-add`, `.add-checklist`, удаления `.cl-del`/`.cl-list-del`.

---

## Структура файлов

```
src-tauri/src/
├─ models.rs               # MOD: + Checklist, ChecklistItem
├─ commands/checklists.rs  # NEW: checklists_* / checklist_items_* + helpers + тест
├─ commands/mod.rs         # MOD: + pub mod checklists;
└─ lib.rs                  # MOD: регистрация команд

src/lib/
├─ types.ts                # MOD: + Checklist, ChecklistItem
├─ api/checklists.ts       # NEW
└─ components/
   ├─ ChecklistsTab.svelte # NEW
   └─ ProjectView.svelte   # MOD: рендер ChecklistsTab на табе "checklists"
```

---

## Task 1: Rust — команды чеклистов

**Files:** Modify `models.rs`, `commands/mod.rs`, `lib.rs`; Create `commands/checklists.rs`.

- [ ] **Step 1: Модели** (в `models.rs`, после `TaskInput`):
```rust
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChecklistItem {
    pub id: i64,
    pub checklist_id: i64,
    pub text: String,
    pub is_done: bool,
    pub sort_order: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Checklist {
    pub id: i64,
    pub project_id: i64,
    pub title: String,
    pub sort_order: i64,
    pub items: Vec<ChecklistItem>,
}
```

- [ ] **Step 2: commands/checklists.rs**

Create `src-tauri/src/commands/checklists.rs`:
```rust
use crate::error::{AppError, AppResult, ErrorKind};
use crate::models::{Checklist, ChecklistItem};
use crate::state::AppState;
use rusqlite::{params, Connection};
use std::sync::MutexGuard;
use tauri::State;

fn lock<'a>(state: &'a State<AppState>) -> AppResult<MutexGuard<'a, Connection>> {
    state.db.lock().map_err(|_| AppError::internal("db mutex poisoned"))
}

fn load_items(conn: &Connection, checklist_id: i64) -> AppResult<Vec<ChecklistItem>> {
    let mut stmt = conn.prepare(
        "SELECT id, checklist_id, text, is_done, sort_order FROM checklist_items
         WHERE checklist_id = ?1 ORDER BY sort_order ASC, id ASC",
    )?;
    let rows = stmt.query_map([checklist_id], |r| {
        Ok(ChecklistItem {
            id: r.get(0)?,
            checklist_id: r.get(1)?,
            text: r.get(2)?,
            is_done: r.get::<_, i64>(3)? != 0,
            sort_order: r.get(4)?,
        })
    })?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

fn list_checklists(conn: &Connection, project_id: i64) -> AppResult<Vec<Checklist>> {
    let mut stmt = conn.prepare(
        "SELECT id, project_id, title, sort_order FROM checklists
         WHERE project_id = ?1 ORDER BY sort_order ASC, id ASC",
    )?;
    let rows = stmt
        .query_map([project_id], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, String>(2)?, r.get::<_, i64>(3)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let mut out = Vec::new();
    for (id, pid, title, sort_order) in rows {
        out.push(Checklist { id, project_id: pid, title, sort_order, items: load_items(conn, id)? });
    }
    Ok(out)
}

fn create_checklist(conn: &Connection, project_id: i64, title: &str) -> AppResult<Checklist> {
    let title = title.trim();
    if title.is_empty() {
        return Err(AppError { kind: ErrorKind::Validation, message: "Название чеклиста пусто".into() });
    }
    let next: i64 = conn.query_row(
        "SELECT COALESCE(MAX(sort_order),0)+1 FROM checklists WHERE project_id=?1",
        [project_id],
        |r| r.get(0),
    )?;
    conn.execute(
        "INSERT INTO checklists(project_id, title, sort_order) VALUES (?1, ?2, ?3)",
        params![project_id, title, next],
    )?;
    let id = conn.last_insert_rowid();
    Ok(Checklist { id, project_id, title: title.to_string(), sort_order: next, items: Vec::new() })
}

fn update_checklist(conn: &Connection, id: i64, title: &str) -> AppResult<()> {
    let title = title.trim();
    if title.is_empty() {
        return Err(AppError { kind: ErrorKind::Validation, message: "Название чеклиста пусто".into() });
    }
    conn.execute("UPDATE checklists SET title=?2 WHERE id=?1", params![id, title])?;
    Ok(())
}

fn delete_checklist(conn: &Connection, id: i64) -> AppResult<()> {
    conn.execute("DELETE FROM checklists WHERE id=?1", [id])?;
    Ok(())
}

fn add_item(conn: &Connection, checklist_id: i64, text: &str) -> AppResult<ChecklistItem> {
    let text = text.trim();
    if text.is_empty() {
        return Err(AppError { kind: ErrorKind::Validation, message: "Пункт пуст".into() });
    }
    let next: i64 = conn.query_row(
        "SELECT COALESCE(MAX(sort_order),0)+1 FROM checklist_items WHERE checklist_id=?1",
        [checklist_id],
        |r| r.get(0),
    )?;
    conn.execute(
        "INSERT INTO checklist_items(checklist_id, text, is_done, sort_order) VALUES (?1, ?2, 0, ?3)",
        params![checklist_id, text, next],
    )?;
    Ok(ChecklistItem { id: conn.last_insert_rowid(), checklist_id, text: text.to_string(), is_done: false, sort_order: next })
}

fn update_item(conn: &Connection, id: i64, text: &str) -> AppResult<()> {
    let text = text.trim();
    if text.is_empty() {
        return Err(AppError { kind: ErrorKind::Validation, message: "Пункт пуст".into() });
    }
    conn.execute("UPDATE checklist_items SET text=?2 WHERE id=?1", params![id, text])?;
    Ok(())
}

fn toggle_item(conn: &Connection, id: i64, is_done: bool) -> AppResult<()> {
    conn.execute("UPDATE checklist_items SET is_done=?2 WHERE id=?1", params![id, is_done as i64])?;
    Ok(())
}

fn delete_item(conn: &Connection, id: i64) -> AppResult<()> {
    conn.execute("DELETE FROM checklist_items WHERE id=?1", [id])?;
    Ok(())
}

#[tauri::command]
pub fn checklists_list(state: State<AppState>, project_id: i64) -> AppResult<Vec<Checklist>> {
    let conn = lock(&state)?;
    list_checklists(&conn, project_id)
}

#[tauri::command]
pub fn checklists_create(state: State<AppState>, project_id: i64, title: String) -> AppResult<Checklist> {
    let conn = lock(&state)?;
    create_checklist(&conn, project_id, &title)
}

#[tauri::command]
pub fn checklists_update(state: State<AppState>, id: i64, title: String) -> AppResult<()> {
    let conn = lock(&state)?;
    update_checklist(&conn, id, &title)
}

#[tauri::command]
pub fn checklists_delete(state: State<AppState>, id: i64) -> AppResult<()> {
    let conn = lock(&state)?;
    delete_checklist(&conn, id)
}

#[tauri::command]
pub fn checklist_items_add(state: State<AppState>, checklist_id: i64, text: String) -> AppResult<ChecklistItem> {
    let conn = lock(&state)?;
    add_item(&conn, checklist_id, &text)
}

#[tauri::command]
pub fn checklist_items_update(state: State<AppState>, id: i64, text: String) -> AppResult<()> {
    let conn = lock(&state)?;
    update_item(&conn, id, &text)
}

#[tauri::command]
pub fn checklist_items_toggle(state: State<AppState>, id: i64, is_done: bool) -> AppResult<()> {
    let conn = lock(&state)?;
    toggle_item(&conn, id, is_done)
}

#[tauri::command]
pub fn checklist_items_delete(state: State<AppState>, id: i64) -> AppResult<()> {
    let conn = lock(&state)?;
    delete_item(&conn, id)
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
        conn.execute("INSERT INTO projects(name, status, sort_order) VALUES ('P','active',0)", []).unwrap();
        conn.last_insert_rowid()
    }

    #[test]
    fn checklist_with_items_roundtrip() {
        let conn = mem();
        let pid = project(&conn);
        let cl = create_checklist(&conn, pid, "Релиз").unwrap();
        add_item(&conn, cl.id, "Тег").unwrap();
        let it2 = add_item(&conn, cl.id, "Changelog").unwrap();
        toggle_item(&conn, it2.id, true).unwrap();

        let lists = list_checklists(&conn, pid).unwrap();
        assert_eq!(lists.len(), 1);
        assert_eq!(lists[0].title, "Релиз");
        assert_eq!(lists[0].items.len(), 2);
        let done = lists[0].items.iter().filter(|i| i.is_done).count();
        assert_eq!(done, 1);
    }

    #[test]
    fn delete_checklist_cascades_items() {
        let conn = mem();
        let pid = project(&conn);
        let cl = create_checklist(&conn, pid, "X").unwrap();
        add_item(&conn, cl.id, "a").unwrap();
        delete_checklist(&conn, cl.id).unwrap();
        let cnt: i64 = conn
            .query_row("SELECT count(*) FROM checklist_items WHERE checklist_id=?1", [cl.id], |r| r.get(0))
            .unwrap();
        assert_eq!(cnt, 0);
    }
}
```

- [ ] **Step 3: Регистрация**
- `commands/mod.rs`: `pub mod checklists;`
- `lib.rs` `generate_handler![...]`: добавить
```rust
            commands::checklists::checklists_list,
            commands::checklists::checklists_create,
            commands::checklists::checklists_update,
            commands::checklists::checklists_delete,
            commands::checklists::checklist_items_add,
            commands::checklists::checklist_items_update,
            commands::checklists::checklist_items_toggle,
            commands::checklists::checklist_items_delete,
```

- [ ] **Step 4: Тесты + сборка**
```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib
cargo build --manifest-path src-tauri/Cargo.toml
```
Expected: новые `checklist_with_items_roundtrip`, `delete_checklist_cascades_items` + прежние (12) → 14 ok; build успешен.

- [ ] **Step 5: Commit**
```powershell
git add src-tauri/src/models.rs src-tauri/src/commands/checklists.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs
git commit -m @'
feat(backend): checklists and items CRUD

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

## Task 2: Фронт — ChecklistsTab

**Files:** Modify `types.ts`, `ProjectView.svelte`; Create `api/checklists.ts`, `components/ChecklistsTab.svelte`.

- [ ] **Step 1: Типы** (в конец `types.ts`):
```ts
export type ChecklistItem = {
  id: number;
  checklistId: number;
  text: string;
  isDone: boolean;
  sortOrder: number;
};
export type Checklist = {
  id: number;
  projectId: number;
  title: string;
  sortOrder: number;
  items: ChecklistItem[];
};
```

- [ ] **Step 2: api/checklists.ts**
```ts
import { call } from "./client";
import type { Checklist, ChecklistItem } from "../types";

export const list = (projectId: number) => call<Checklist[]>("checklists_list", { projectId });
export const create = (projectId: number, title: string) => call<Checklist>("checklists_create", { projectId, title });
export const update = (id: number, title: string) => call<void>("checklists_update", { id, title });
export const remove = (id: number) => call<void>("checklists_delete", { id });
export const addItem = (checklistId: number, text: string) =>
  call<ChecklistItem>("checklist_items_add", { checklistId, text });
export const updateItem = (id: number, text: string) => call<void>("checklist_items_update", { id, text });
export const toggleItem = (id: number, isDone: boolean) => call<void>("checklist_items_toggle", { id, isDone });
export const removeItem = (id: number) => call<void>("checklist_items_delete", { id });
```

- [ ] **Step 3: components/ChecklistsTab.svelte**

Create `src/lib/components/ChecklistsTab.svelte`:
```svelte
<script lang="ts">
  import type { Project, Checklist } from "$lib/types";
  import * as cl from "$lib/api/checklists";
  import Icon from "./Icon.svelte";

  let { project }: { project: Project } = $props();

  let lists = $state<Checklist[]>([]);
  let newItem = $state<Record<number, string>>({});

  let reqId = 0;
  async function load() {
    const my = ++reqId;
    try {
      const r = await cl.list(project.id);
      if (my === reqId) lists = r;
    } catch {
      /* тост из api/client.ts */
    }
  }
  $effect(() => {
    project.id;
    load();
  });

  function pct(c: Checklist): number {
    return c.items.length ? Math.round((c.items.filter((i) => i.isDone).length / c.items.length) * 100) : 0;
  }
  function doneCount(c: Checklist): number {
    return c.items.filter((i) => i.isDone).length;
  }

  async function addChecklist() {
    await cl.create(project.id, "Новый чеклист");
    await load();
  }
  async function renameChecklist(id: number, title: string) {
    if (!title.trim()) return;
    await cl.update(id, title.trim());
    // без перезагрузки: заголовок уже в локальном состоянии
  }
  async function deleteChecklist(id: number) {
    await cl.remove(id);
    await load();
  }
  async function toggle(itemId: number, isDone: boolean) {
    await cl.toggleItem(itemId, isDone);
    await load();
  }
  async function editItem(itemId: number, text: string) {
    if (!text.trim()) return;
    await cl.updateItem(itemId, text.trim());
  }
  async function deleteItem(itemId: number) {
    await cl.removeItem(itemId);
    await load();
  }
  async function addItem(checklistId: number) {
    const text = (newItem[checklistId] ?? "").trim();
    if (!text) return;
    newItem[checklistId] = "";
    await cl.addItem(checklistId, text);
    await load();
  }
</script>

<div class="checklists">
  {#each lists as c (c.id)}
    <div class="card checklist">
      <div class="cl-head">
        <input
          class="t tin"
          style="border:0;background:none;padding:0;font-size:14px;font-weight:600"
          value={c.title}
          onchange={(e) => renameChecklist(c.id, (e.currentTarget as HTMLInputElement).value)}
        />
        <span class="pc">{doneCount(c)} / {c.items.length}</span>
        <button class="cl-list-del" title="Удалить чеклист" onclick={() => deleteChecklist(c.id)}>
          <Icon name="trash-2" class="ic-sm" />
        </button>
      </div>
      <div class="progress"><i style="width:{pct(c)}%"></i></div>
      <div class="cl-items">
        {#each c.items as it (it.id)}
          <div class="cl-item" class:done={it.isDone}>
            <button class="cbox" aria-label="переключить" onclick={() => toggle(it.id, !it.isDone)}>
              {#if it.isDone}<Icon name="check" class="ic-sm" />{/if}
            </button>
            <input
              class="lab tin"
              style="border:0;background:none;padding:0;flex:1"
              value={it.text}
              onchange={(e) => editItem(it.id, (e.currentTarget as HTMLInputElement).value)}
            />
            <button class="cl-del" title="Удалить пункт" onclick={() => deleteItem(it.id)}>
              <Icon name="x" class="ic-sm" />
            </button>
          </div>
        {/each}
      </div>
      <div class="cl-add">
        <input
          class="tin"
          placeholder="+ пункт"
          bind:value={newItem[c.id]}
          onkeydown={(e) => { if (e.key === "Enter") addItem(c.id); }}
        />
      </div>
    </div>
  {/each}

  <button class="add-checklist" onclick={addChecklist}>
    <Icon name="plus" class="ic-sm" /> Новый чеклист
  </button>
</div>
```
> Иконки `trash-2`/`check`/`x`/`plus` — из lucide. Классы `.checklists`/`.checklist`/`.cl-head`/`.progress`/`.cl-item`/`.cbox`/`.cl-add`/`.add-checklist` — в global.css. Переименование/правка пункта — по `onchange` (потеря фокуса). Reorder drag-ом отложен.

- [ ] **Step 4: Рендер в ProjectView.svelte**
Импорт `import ChecklistsTab from "./ChecklistsTab.svelte";` и ветка в `tab-body` (рядом с tasks):
```svelte
    {:else if $activeTab === "checklists"}
      <ChecklistsTab {project} />
```

- [ ] **Step 5: Проверка**
```powershell
npm run check
npm test
npm run build
```
Expected: 0 ошибок типов; Vitest 3; build успешен.

- [ ] **Step 6: Commit**
```powershell
git add src/lib/types.ts src/lib/api/checklists.ts src/lib/components/ChecklistsTab.svelte src/lib/components/ProjectView.svelte
git commit -m @'
feat(frontend): checklists tab (progress, items, add/edit/delete)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

## Task 3: Проверка среза

- [ ] **Step 1: Автопроверка**
```powershell
npm run check ; npm test ; cargo test --manifest-path src-tauri/Cargo.toml --lib
```
Expected: типы 0 ошибок; Vitest 3; Rust 14 (12 прежних + 2 checklists).

- [ ] **Step 2: Ручной GUI-смоук (пользователь; `npm run tauri dev`)**
- [ ] Вкладка «Чеклисты» → «Новый чеклист» → появилась карточка «Новый чеклист».
- [ ] Переименовать заголовок (клик, правка, потеря фокуса) → сохранилось.
- [ ] «+ пункт» + Enter → пункт добавился; прогресс «0 / N».
- [ ] Кликнуть галочку → пункт зачёркнут, прогресс-бар и счётчик «X / Y» обновились.
- [ ] Правка текста пункта (потеря фокуса) → сохранилась.
- [ ] Удалить пункт (×) / удалить чеклист (корзина) → исчезли.
- [ ] Перезапуск приложения → чеклисты и состояние галочек на месте.

---

## Итог среза

Вкладка «Чеклисты» работает на SQLite: несколько чеклистов с прогресс-баром, пункты с галочками/правкой/удалением, создание/переименование/удаление чеклистов. Завершает наполнение вкладок проекта в Phase 1 (кроме заметок/ссылок — отдельные срезы). Отложено: шаблоны чеклистов, reorder drag-ом.
```
