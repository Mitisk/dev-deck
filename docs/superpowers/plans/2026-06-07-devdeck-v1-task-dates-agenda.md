# DevDeck — Срез «Даты задач + повестка дашборда» — план реализации

> **Для исполнителя:** ОБЯЗАТЕЛЬНЫЙ ПОД-НАВЫК: superpowers:subagent-driven-development. Шаги — чекбоксы.

**Цель:** Срок задачи — структурированная дата (date-picker, ISO `YYYY-MM-DD`) вместо свободного текста. На дашборде — секция «Задачи на сегодня и просроченные» по всем проектам. Карточки задач подсвечивают просроченные.

**Архитектура:** `tasks.due_date` хранит ISO-дату (колонка TEXT — миграция не нужна). Команда `tasks_agenda(today)` возвращает незавершённые задачи со сроком ≤ today (по всем проектам). Фронт: в редакторе задачи `<input type="date">`; на дашборде — секция повестки; карточки форматируют дату и метят просроченные.

**Стек:** как раньше. Без миграций.

**Решения / границы:**
- `today` приходит с фронта (локальная дата `YYYY-MM-DD`), чтобы не зависеть от UTC в SQLite.
- Старые свободнотекстовые значения срока (если были) перестанут попадать в повестку и в date-picker отобразятся пустыми — пользователь перевыберет дату (реальные данные свежие).
- Просрочка/сегодня = `due_date <= today` (просроченные и сегодняшние вместе).

**Источники:** `TZ_DevDeck.md` (5.5 — срок; 5.10 — задачи на сегодня/просроченные). Текущие `TasksTab.svelte` (срок — текст), `Dashboard.svelte`, `commands/tasks.rs`, `api/dashboard.ts`.

---

## Контекст

- Rust: 70 команд; `commands/tasks.rs` (Task/TaskInput, due_date Option<String>); `models.rs`. Таблица `tasks(due_date TEXT)`.
- Фронт: `TasksTab.svelte` — edit-диалог с `<input ... bind:value={eDue}>` (текст), карточка показывает `{t.dueDate}` сырым. `Dashboard.svelte` — секции pinned/recent/attention; `api/dashboard.ts` (`attention`). `stores/projects` (`activeProjectId`).

---

## Структура файлов

```
src-tauri/src/
├─ models.rs            # MOD: + AgendaItem
├─ commands/tasks.rs    # MOD: + tasks_agenda + тест
└─ lib.rs               # MOD: регистрация tasks_agenda

src/lib/
├─ types.ts             # MOD: + AgendaItem
├─ api/dashboard.ts     # MOD: + agenda(today)
└─ components/
   ├─ TasksTab.svelte   # MOD: срок = date-picker; формат даты + просрочка на карточке
   └─ Dashboard.svelte  # MOD: секция «Задачи на сегодня/просроченные»
```

---

## Task 1: Rust — tasks_agenda

**Files:** Modify `models.rs`, `commands/tasks.rs`, `lib.rs`.

- [ ] **Step 1: Модель** (в `models.rs`, после `ChecklistTemplate`/последней):
```rust
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgendaItem {
    pub project_id: i64,
    pub project_name: String,
    pub project_color: Option<String>,
    pub task_id: i64,
    pub title: String,
    pub due_date: String,
    pub priority: i64,
}
```

- [ ] **Step 2: Команда + хелпер в tasks.rs** (добавить рядом с прочими; модель импортировать в существующий `use crate::models::{...}`):
```rust
fn agenda(conn: &Connection, today: &str) -> AppResult<Vec<crate::models::AgendaItem>> {
    let mut stmt = conn.prepare(
        "SELECT t.project_id, p.name, p.color, t.id, t.title, t.due_date, t.priority
         FROM tasks t JOIN projects p ON p.id = t.project_id
         WHERE t.status != 'done'
           AND t.due_date IS NOT NULL AND t.due_date != '' AND t.due_date <= ?1
           AND p.status != 'archived'
         ORDER BY t.due_date ASC, t.priority DESC
         LIMIT 50",
    )?;
    let rows = stmt.query_map([today], |r| {
        Ok(crate::models::AgendaItem {
            project_id: r.get(0)?,
            project_name: r.get(1)?,
            project_color: r.get(2)?,
            task_id: r.get(3)?,
            title: r.get(4)?,
            due_date: r.get(5)?,
            priority: r.get(6)?,
        })
    })?;
    let mut out = Vec::new();
    for r in rows { out.push(r?); }
    Ok(out)
}

#[tauri::command]
pub fn tasks_agenda(state: State<AppState>, today: String) -> AppResult<Vec<crate::models::AgendaItem>> {
    let conn = lock(&state)?;
    agenda(&conn, &today)
}
```

- [ ] **Step 3: Тест** (в `mod tests` в tasks.rs):
```rust
    #[test]
    fn agenda_returns_overdue_and_today_not_done() {
        let conn = mem();
        let pid = project(&conn);
        // просрочена, сегодня, в будущем, выполнена-просрочена
        conn.execute("INSERT INTO tasks(project_id,title,status,due_date,sort_order) VALUES(?1,'overdue','todo','2026-06-01',0)", [pid]).unwrap();
        conn.execute("INSERT INTO tasks(project_id,title,status,due_date,sort_order) VALUES(?1,'today','doing','2026-06-07',1)", [pid]).unwrap();
        conn.execute("INSERT INTO tasks(project_id,title,status,due_date,sort_order) VALUES(?1,'future','todo','2026-12-31',2)", [pid]).unwrap();
        conn.execute("INSERT INTO tasks(project_id,title,status,due_date,sort_order) VALUES(?1,'done','done','2026-06-01',3)", [pid]).unwrap();
        conn.execute("INSERT INTO tasks(project_id,title,status,due_date,sort_order) VALUES(?1,'nodate','todo','',4)", [pid]).unwrap();

        let items = agenda(&conn, "2026-06-07").unwrap();
        let titles: Vec<&str> = items.iter().map(|i| i.title.as_str()).collect();
        assert_eq!(titles, vec!["overdue", "today"]); // future/done/nodate исключены, сортировка по дате
    }
```

- [ ] **Step 4: Регистрация** — `lib.rs` `generate_handler![...]`: добавить `commands::tasks::tasks_agenda,`.

- [ ] **Step 5: Тесты + lib-сборка**
```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib
cargo build --manifest-path src-tauri/Cargo.toml --lib
```
Expected: новый `agenda_returns_overdue_and_today_not_done` + прежние (28) → 29 ok; lib-сборка успешна.

- [ ] **Step 6: Commit**
```powershell
git add src-tauri/src/models.rs src-tauri/src/commands/tasks.rs src-tauri/src/lib.rs
git commit -m @'
feat(backend): tasks_agenda (overdue/today tasks across projects)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

## Task 2: Фронт — date-picker + повестка дашборда

**Files:** Modify `types.ts`, `api/dashboard.ts`, `TasksTab.svelte`, `Dashboard.svelte`.

- [ ] **Step 1: Тип** (в конец `types.ts`):
```ts
export type AgendaItem = {
  projectId: number;
  projectName: string;
  projectColor: string | null;
  taskId: number;
  title: string;
  dueDate: string;
  priority: number;
};
```

- [ ] **Step 2: api/dashboard.ts** — добавить:
```ts
import type { AttentionItem, AgendaItem } from "../types"; // ← заменить существующий import AttentionItem на этот

export const agenda = (today: string) => call<AgendaItem[]>("tasks_agenda", { today });
```
(существующий `attention` оставить; объединить import.)

- [ ] **Step 3: TasksTab.svelte — date-picker + формат даты**

В `src/lib/components/TasksTab.svelte`:
1. В edit-диалоге заменить поле срока на date-picker:
```svelte
        <div class="field">
          <label for="et-due">Срок</label>
          <input id="et-due" class="tin" type="date" bind:value={eDue} />
        </div>
```
2. Добавить хелперы в `<script>`:
```ts
  function todayStr(): string {
    const d = new Date();
    return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
  }
  function fmtDue(due: string | null): string {
    if (!due) return "";
    const d = new Date(due + "T00:00:00");
    if (isNaN(d.getTime())) return due; // не ISO — показать как есть
    return d.toLocaleDateString("ru-RU", { day: "numeric", month: "short" });
  }
  function isOverdue(due: string | null): boolean {
    return !!due && /^\d{4}-\d{2}-\d{2}$/.test(due) && due < todayStr();
  }
```
3. На карточке задачи заменить блок срока на форматированный + подсветку просрочки:
```svelte
              {#if t.dueDate}<span class="due" style={isOverdue(t.dueDate) ? "color:var(--danger)" : ""}><Icon name="calendar" class="ic-sm" /> {fmtDue(t.dueDate)}</span>{/if}
```

- [ ] **Step 4: Dashboard.svelte — секция повестки**

В `src/lib/components/Dashboard.svelte`:
1. Импорт типа и состояние:
```ts
  import type { AttentionItem, AgendaItem } from "$lib/types"; // ← объединить с существующим import
  let agenda = $state<AgendaItem[]>([]);
  function todayStr(): string {
    const d = new Date();
    return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
  }
  function fmtDue(due: string): string {
    const d = new Date(due + "T00:00:00");
    return isNaN(d.getTime()) ? due : d.toLocaleDateString("ru-RU", { day: "numeric", month: "short" });
  }
  function overdue(due: string): boolean { return /^\d{4}-\d{2}-\d{2}$/.test(due) && due < todayStr(); }
```
2. В функции `loadAttention` (или рядом, в том же `$effect`) догрузить повестку:
```ts
  async function loadAgenda() {
    try { agenda = await dash.agenda(todayStr()); } catch { /* */ }
  }
```
И вызвать `loadAgenda()` там же, где `loadAttention()` (в `$effect`).
3. Разметку секции добавить ПЕРЕД блоком «Требуют внимания» (внутри `{:else}` ветки, после pinned/recent):
```svelte
    {#if agenda.length}
      <div style="margin-top:26px">
        <h3 class="section-title"><Icon name="calendar-clock" class="ic-sm" /> Задачи на сегодня и просроченные</h3>
        <div class="card att-card">
          {#each agenda as a (a.taskId)}
            <div class="att-row" role="button" tabindex="0" onclick={() => open(a.projectId)}>
              <span class="att-be" style="--p-color:{a.projectColor ?? 'var(--accent)'}"><Icon name="square-check-big" class="ic-sm" /></span>
              <div class="att-main">
                <div class="att-line">
                  <span class="att-name">{a.title}</span>
                  <span class="att-branch">{a.projectName}</span>
                </div>
              </div>
              <span class="att-tag {overdue(a.dueDate) ? 'dirty' : 'ahead'}">{fmtDue(a.dueDate)}</span>
            </div>
          {/each}
        </div>
      </div>
    {/if}
```
> Иконки `calendar-clock`/`square-check-big`/`calendar` — из lucide. `open(id)` и `$effect` уже есть в Dashboard. Классы `.att-*` — в global.css.

- [ ] **Step 5: Проверка**
```powershell
npm run check
npm test
npm run build
```
Expected: 0 ошибок типов; Vitest 3; build успешен.

- [ ] **Step 6: Commit**
```powershell
git add src/lib/types.ts src/lib/api/dashboard.ts src/lib/components/TasksTab.svelte src/lib/components/Dashboard.svelte
git commit -m @'
feat(frontend): task date-picker and dashboard agenda (overdue/today)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

## Task 3: Проверка среза

- [ ] **Step 1: Автопроверка**
```powershell
npm run check ; npm test ; cargo test --manifest-path src-tauri/Cargo.toml --lib
```
Expected: типы 0; Vitest 3; Rust 29.

- [ ] **Step 2: Ручной GUI-смоук (пользователь; перезапустить `npm run tauri dev`)**
- [ ] Задача → редактировать → срок выбирается календарём (date-picker).
- [ ] Карточка показывает дату коротко; просроченная дата — красным.
- [ ] Поставить задаче срок «сегодня» или в прошлом → на дашборде секция «Задачи на сегодня и просроченные» показывает её (с именем проекта).
- [ ] Задача со сроком в будущем / выполненная / без срока → в повестку НЕ попадает.
- [ ] Клик по строке повестки → открывается проект.

---

## Итог среза

Сроки задач — настоящие даты с календарём; дашборд показывает просроченные и сегодняшние задачи по всем проектам. Закрывает последний пробел дашборда из v1.0. Дальше — v2.0 (слежение за папкой через `notify`, настраиваемый канбан) или другие доработки.
```
