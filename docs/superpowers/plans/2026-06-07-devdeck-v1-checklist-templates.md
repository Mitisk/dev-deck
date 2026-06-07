# DevDeck v1.0 — Срез «Шаблоны чеклистов» — план реализации

> **Для исполнителя:** ОБЯЗАТЕЛЬНЫЙ ПОД-НАВЫК: superpowers:subagent-driven-development. Шаги — чекбоксы.

**Цель:** Сохранить чеклист как переиспользуемый шаблон (не привязан к проекту) и применять шаблон к любому проекту (создаёт новый чеклист с пунктами). Управление шаблонами — во вкладке «Чеклисты».

**Архитектура:** Команды `templates_*` поверх таблицы `checklist_templates` (`items_json` — JSON-массив строк). `template_apply` создаёт чеклист + пункты в указанном проекте. Фронт: в `ChecklistsTab` — кнопка «Сохранить как шаблон» на чеклисте и панель «Шаблоны» (применить/удалить).

**Стек:** как раньше (`serde_json` уже dep). Без миграций (`checklist_templates` в 0001).

**Решения / границы:** шаблон хранит только тексты пунктов (галочки сбрасываются при применении). Имя шаблона при сохранении = заголовок чеклиста (правка имени — позже).

**Источники:** `TZ_DevDeck.md` (5.6 — шаблоны; 10 — `templates_*`). Таблица `checklist_templates(id,name,items_json)` в 0001. `commands/checklists.rs` (образец).

---

## Контекст

- Rust: 61 команда; `models.rs`; `commands/checklists.rs`; `serde_json` dep. Таблица `checklist_templates(id INTEGER PK, name TEXT, items_json TEXT)`.
- Фронт: `ChecklistsTab.svelte` (грид чеклистов + «+ чеклист»). api `client.ts`, `Icon.svelte`. global.css: `.checklist`/`.cl-head`/`.cl-list-del`/`.add-checklist`/`.gbtn`.

---

## Структура файлов

```
src-tauri/src/
├─ models.rs                # MOD: + ChecklistTemplate
├─ commands/templates.rs    # NEW: templates_list/save/apply/delete + тест
├─ commands/mod.rs          # MOD: + pub mod templates;
└─ lib.rs                   # MOD: регистрация 4 команд

src/lib/
├─ types.ts                 # MOD: + ChecklistTemplate
├─ api/templates.ts         # NEW
└─ components/ChecklistsTab.svelte  # MOD: «сохранить как шаблон» + панель «Шаблоны»
```

---

## Task 1: Rust — шаблоны

**Files:** Modify `models.rs`, `commands/mod.rs`, `lib.rs`; Create `commands/templates.rs`.

- [ ] **Step 1: Модель** (в `models.rs`, после `ImportSummary`):
```rust
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChecklistTemplate {
    pub id: i64,
    pub name: String,
    pub items: Vec<String>,
}
```

- [ ] **Step 2: commands/templates.rs**

Create `src-tauri/src/commands/templates.rs`:
```rust
use crate::error::{AppError, AppResult, ErrorKind};
use crate::models::{Checklist, ChecklistTemplate};
use crate::state::AppState;
use rusqlite::{params, Connection};
use std::sync::MutexGuard;
use tauri::State;

fn lock<'a>(state: &'a State<AppState>) -> AppResult<MutexGuard<'a, Connection>> {
    state.db.lock().map_err(|_| AppError::internal("db mutex poisoned"))
}

fn parse_items(json: &str) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(json).unwrap_or_default()
}

fn row_to_template(conn: &Connection, id: i64) -> AppResult<ChecklistTemplate> {
    let (name, items_json): (String, String) =
        conn.query_row("SELECT name, items_json FROM checklist_templates WHERE id=?1", [id], |r| Ok((r.get(0)?, r.get(1)?)))?;
    Ok(ChecklistTemplate { id, name, items: parse_items(&items_json) })
}

#[tauri::command]
pub fn templates_list(state: State<AppState>) -> AppResult<Vec<ChecklistTemplate>> {
    let conn = lock(&state)?;
    let mut stmt = conn.prepare("SELECT id FROM checklist_templates ORDER BY name COLLATE NOCASE")?;
    let ids = stmt.query_map([], |r| r.get::<_, i64>(0))?;
    let mut out = Vec::new();
    for id in ids {
        out.push(row_to_template(&conn, id?)?);
    }
    Ok(out)
}

/// Сохранить существующий чеклист как шаблон (тексты пунктов).
#[tauri::command]
pub fn template_save(state: State<AppState>, checklist_id: i64, name: String) -> AppResult<ChecklistTemplate> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError { kind: ErrorKind::Validation, message: "Имя шаблона пусто".into() });
    }
    let conn = lock(&state)?;
    let mut stmt = conn.prepare("SELECT text FROM checklist_items WHERE checklist_id=?1 ORDER BY sort_order, id")?;
    let texts: Vec<String> = stmt.query_map([checklist_id], |r| r.get::<_, String>(0))?.collect::<Result<_, _>>()?;
    let items_json = serde_json::to_string(&texts).map_err(|e| AppError { kind: ErrorKind::Internal, message: format!("JSON: {}", e) })?;
    conn.execute("INSERT INTO checklist_templates(name, items_json) VALUES(?1, ?2)", params![name, items_json])?;
    row_to_template(&conn, conn.last_insert_rowid())
}

/// Применить шаблон к проекту: создать новый чеклист с пунктами (галочки сброшены).
#[tauri::command]
pub fn template_apply(state: State<AppState>, template_id: i64, project_id: i64) -> AppResult<Checklist> {
    let conn = lock(&state)?;
    let (name, items_json): (String, String) =
        conn.query_row("SELECT name, items_json FROM checklist_templates WHERE id=?1", [template_id], |r| Ok((r.get(0)?, r.get(1)?)))?;
    let items = parse_items(&items_json);

    let next: i64 = conn.query_row("SELECT COALESCE(MAX(sort_order),0)+1 FROM checklists WHERE project_id=?1", [project_id], |r| r.get(0))?;
    conn.execute("INSERT INTO checklists(project_id, title, sort_order) VALUES(?1, ?2, ?3)", params![project_id, name, next])?;
    let cid = conn.last_insert_rowid();
    for (i, text) in items.iter().enumerate() {
        conn.execute("INSERT INTO checklist_items(checklist_id, text, is_done, sort_order) VALUES(?1, ?2, 0, ?3)", params![cid, text, i as i64])?;
    }
    // вернуть созданный чеклист
    let items_out = {
        let mut s = conn.prepare("SELECT id, checklist_id, text, is_done, sort_order FROM checklist_items WHERE checklist_id=?1 ORDER BY sort_order, id")?;
        s.query_map([cid], |r| Ok(crate::models::ChecklistItem {
            id: r.get(0)?, checklist_id: r.get(1)?, text: r.get(2)?, is_done: r.get::<_, i64>(3)? != 0, sort_order: r.get(4)?,
        }))?.collect::<Result<Vec<_>, _>>()?
    };
    Ok(Checklist { id: cid, project_id, title: name, sort_order: next, items: items_out })
}

#[tauri::command]
pub fn template_delete(state: State<AppState>, id: i64) -> AppResult<()> {
    let conn = lock(&state)?;
    conn.execute("DELETE FROM checklist_templates WHERE id=?1", [id])?;
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
    fn save_and_apply_template() {
        let conn = mem();
        conn.execute("INSERT INTO projects(name,status,sort_order) VALUES('A','active',0)", []).unwrap();
        let pa = conn.last_insert_rowid();
        conn.execute("INSERT INTO checklists(project_id,title,sort_order) VALUES(?1,'Релиз',0)", [pa]).unwrap();
        let cl = conn.last_insert_rowid();
        conn.execute("INSERT INTO checklist_items(checklist_id,text,is_done,sort_order) VALUES(?1,'Тег',1,0)", [cl]).unwrap();
        conn.execute("INSERT INTO checklist_items(checklist_id,text,is_done,sort_order) VALUES(?1,'Changelog',0,1)", [cl]).unwrap();

        // сохранить как шаблон (тексты)
        let texts: Vec<String> = {
            let mut s = conn.prepare("SELECT text FROM checklist_items WHERE checklist_id=?1 ORDER BY sort_order").unwrap();
            s.query_map([cl], |r| r.get::<_, String>(0)).unwrap().collect::<Result<_, _>>().unwrap()
        };
        let items_json = serde_json::to_string(&texts).unwrap();
        conn.execute("INSERT INTO checklist_templates(name,items_json) VALUES('Чеклист релиза',?1)", [&items_json]).unwrap();
        let tid = conn.last_insert_rowid();

        // применить к другому проекту через прямой вызов логики (apply делает то же)
        conn.execute("INSERT INTO projects(name,status,sort_order) VALUES('B','active',0)", []).unwrap();
        let pb = conn.last_insert_rowid();
        let (name, ijson): (String, String) = conn.query_row("SELECT name,items_json FROM checklist_templates WHERE id=?1", [tid], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
        let items = parse_items(&ijson);
        assert_eq!(items, vec!["Тег".to_string(), "Changelog".to_string()]);
        assert_eq!(name, "Чеклист релиза");

        conn.execute("INSERT INTO checklists(project_id,title,sort_order) VALUES(?1,?2,0)", params![pb, name]).unwrap();
        let ncl = conn.last_insert_rowid();
        for (i, t) in items.iter().enumerate() {
            conn.execute("INSERT INTO checklist_items(checklist_id,text,is_done,sort_order) VALUES(?1,?2,0,?3)", params![ncl, t, i as i64]).unwrap();
        }
        let cnt: i64 = conn.query_row("SELECT count(*) FROM checklist_items WHERE checklist_id=?1 AND is_done=0", [ncl], |r| r.get(0)).unwrap();
        assert_eq!(cnt, 2); // галочки сброшены
    }
}
```

- [ ] **Step 3: Регистрация**
- `commands/mod.rs`: `pub mod templates;`
- `lib.rs` `generate_handler![...]`: добавить
```rust
            commands::templates::templates_list,
            commands::templates::template_save,
            commands::templates::template_apply,
            commands::templates::template_delete,
```

- [ ] **Step 4: Тесты + lib-сборка**
```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib
cargo build --manifest-path src-tauri/Cargo.toml --lib
```
Expected: новый `save_and_apply_template` + прежние (23) → 24 ok; lib-сборка успешна.

- [ ] **Step 5: Commit**
```powershell
git add src-tauri/src/models.rs src-tauri/src/commands/templates.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs
git commit -m @'
feat(backend): checklist templates (save/apply/list/delete)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

## Task 2: Фронт — шаблоны в ChecklistsTab

**Files:** Modify `types.ts`, `components/ChecklistsTab.svelte`; Create `api/templates.ts`.

- [ ] **Step 1: Тип** (в конец `types.ts`):
```ts
export type ChecklistTemplate = { id: number; name: string; items: string[] };
```

- [ ] **Step 2: api/templates.ts**
```ts
import { call } from "./client";
import type { ChecklistTemplate, Checklist } from "../types";

export const list = () => call<ChecklistTemplate[]>("templates_list");
export const save = (checklistId: number, name: string) => call<ChecklistTemplate>("template_save", { checklistId, name });
export const apply = (templateId: number, projectId: number) => call<Checklist>("template_apply", { templateId, projectId });
export const remove = (id: number) => call<void>("template_delete", { id });
```

- [ ] **Step 3: Обновить ChecklistsTab.svelte**

В `src/lib/components/ChecklistsTab.svelte`:

1. В `<script>` добавить импорты и состояние шаблонов:
```ts
  import * as templatesApi from "$lib/api/templates";
  import type { ChecklistTemplate } from "$lib/types";
  import { pushToast } from "$lib/stores/toasts";

  let templates = $state<ChecklistTemplate[]>([]);
  let showTemplates = $state(false);

  async function loadTemplates() {
    try { templates = await templatesApi.list(); } catch { /* */ }
  }
  // догрузить шаблоны вместе со списком (вызвать в существующем load или отдельным $effect)
```
И добавить отдельный эффект загрузки шаблонов (один раз/при открытии вкладки):
```ts
  $effect(() => { loadTemplates(); });
```

2. Функции работы с шаблонами:
```ts
  async function saveAsTemplate(c: { id: number; title: string }) {
    await templatesApi.save(c.id, c.title || "Шаблон");
    await loadTemplates();
    pushToast("Шаблон сохранён", c.title, "ok");
  }
  async function applyTemplate(t: ChecklistTemplate) {
    await templatesApi.apply(t.id, project.id);
    showTemplates = false;
    await load(); // существующая функция перезагрузки чеклистов
    pushToast("Шаблон применён", t.name, "ok");
  }
  async function deleteTemplate(id: number) {
    await templatesApi.remove(id);
    await loadTemplates();
  }
```

3. Разметка: ПЕРЕД `<div class="checklists">` добавить шапку с кнопкой «Шаблоны» и панелью:
```svelte
<div style="display:flex;align-items:center;gap:8px;margin-bottom:12px;position:relative">
  <span style="flex:1"></span>
  <button class="gbtn" onclick={() => (showTemplates = !showTemplates)}>
    <Icon name="layout-template" class="ic-sm" /> Шаблоны{#if templates.length} ({templates.length}){/if}
  </button>
  {#if showTemplates}
    <div class="card" style="position:absolute;right:0;top:36px;z-index:50;min-width:260px;padding:8px">
      {#if templates.length}
        {#each templates as t (t.id)}
          <div class="link-row" style="padding:6px 8px">
            <div style="flex:1;cursor:pointer" role="button" tabindex="0" onclick={() => applyTemplate(t)}>
              <div class="lt">{t.name}</div>
              <div class="lu">{t.items.length} пунктов</div>
            </div>
            <button class="mini" title="Применить" onclick={() => applyTemplate(t)}><Icon name="plus" class="ic-sm" /></button>
            <button class="mini" title="Удалить" onclick={() => deleteTemplate(t.id)}><Icon name="trash-2" class="ic-sm" /></button>
          </div>
        {/each}
      {:else}
        <div class="erow-empty" style="padding:8px">Нет шаблонов. Сохраните чеклист как шаблон.</div>
      {/if}
    </div>
  {/if}
</div>
```

4. В шапке каждого чеклиста (`.cl-head`) добавить кнопку «сохранить как шаблон» рядом с кнопкой удаления `.cl-list-del`:
```svelte
        <button class="cl-list-del" title="Сохранить как шаблон" onclick={() => saveAsTemplate(c)}>
          <Icon name="bookmark" class="ic-sm" />
        </button>
```
(вставить ПЕРЕД существующей кнопкой удаления чеклиста).

> Иконки `layout-template`/`bookmark`/`plus`/`trash-2` — из lucide. Если `layout-template` нет — использовать `copy`. Функция `load` и переменная `project` уже есть в компоненте.

- [ ] **Step 4: Проверка**
```powershell
npm run check
npm test
npm run build
```
Expected: 0 ошибок типов; Vitest 3; build успешен.

- [ ] **Step 5: Commit**
```powershell
git add src/lib/types.ts src/lib/api/templates.ts src/lib/components/ChecklistsTab.svelte
git commit -m @'
feat(frontend): checklist templates (save as / apply / manage)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

## Task 3: Проверка среза

- [ ] **Step 1: Автопроверка**
```powershell
npm run check ; npm test ; cargo test --manifest-path src-tauri/Cargo.toml --lib
```
Expected: типы 0 ошибок; Vitest 3; Rust 24.

- [ ] **Step 2: Ручной GUI-смоук (пользователь; перезапустить `npm run tauri dev`)**
- [ ] Вкладка «Чеклисты»: создать чеклист с пунктами → кнопка «сохранить как шаблон» (закладка) на чеклисте → тост.
- [ ] Кнопка «Шаблоны» вверху → панель со списком шаблонов (с числом пунктов).
- [ ] Открыть ДРУГОЙ проект → «Чеклисты» → «Шаблоны» → применить → создался чеклист с пунктами (галочки сброшены).
- [ ] Удалить шаблон из панели.
- [ ] Перезапуск → шаблоны на месте.

---

## Итог среза

Чеклисты можно сохранять как переиспользуемые шаблоны и применять к любому проекту (галочки сбрасываются). Остаётся последний пункт v1.0 — **режим мастер-пароля** (AES-256-GCM + Argon2id как альтернатива DPAPI). Отложено: правка имени шаблона, шаблоны в общих настройках.
```
