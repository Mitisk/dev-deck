# DevDeck v1.0 — Срез «Командный палет (Ctrl+K) + глобальный поиск» — план реализации

> **Для исполнителя:** ОБЯЗАТЕЛЬНЫЙ ПОД-НАВЫК: superpowers:subagent-driven-development. Шаги — чекбоксы.

**Цель:** По Ctrl+K открывается палет с полем поиска. Пустой запрос — список проектов для быстрого перехода. С вводом — глобальный поиск по проектам/задачам/заметкам/ссылкам/кредам/командам/файлам; выбор результата (мышью или ↑/↓/Enter) переходит к проекту и открывает нужную вкладку. Esc закрывает.

**Архитектура:** Команда `search_global(query)` ищет `LIKE %q%` по таблицам и возвращает `Vec<SearchHit>` (kind/projectId/projectName/id/title/subtitle). Секреты кредов в поиск не попадают (только `label`). Фронт: `CommandPalette.svelte` (модальный палет, debounce-поиск, клавиатурная навигация), Ctrl+K — глобальный хоткей в `+page.svelte`. Переход: `activeProjectId` + `activeTab` по типу хита.

**Стек:** как раньше. Без миграций.

**Решения / границы:**
- В этом срезе — **навигация и поиск**. Действия из палета (push проекта, копировать кред, запустить команду) — **отложены** (отмечено); сейчас выбор хита = переход к месту.
- Пустой запрос → проекты из стора (мгновенная навигация). Непустой → бэкенд-поиск.
- Ctrl+F (фокус поиска) — позже; сейчас только Ctrl+K.

**Источники:** `TZ_DevDeck.md` (5.9 — палет и глобальный поиск; 6.3 — Ctrl+K; 10 — `search_global`). `_prototype/index.html` (`.palette-scrim`/`.palette`/`.pal-input`/`.pal-list`/`.pal-item`/`.pal-group`/`.pal-foot` + `buildPaletteItems`/`renderPalette` — разметка/логика-референс). Сторы `projects`/`ui` (`activeTab`, `showNewProject`).

---

## Контекст

- Rust: 54 команды; `models.rs`, `commands/{...}.rs`, `error`. Таблицы: projects, tasks, notes, links, credentials, commands, files, checklists/checklist_items.
- Фронт: `src/lib/stores/ui.ts` (`activeTab: Tab`, `showNewProject`); `stores/projects.ts` (`projects`, `activeProjectId: number|null`). `+page.svelte` монтирует Sidebar/Workspace/NewProjectModal/Toasts. `Icon.svelte`. global.css содержит все классы палета (`.palette-scrim`/`.palette`/`.pal-input`/`.pal-list`/`.pal-group`/`.pal-item`(`.sel`, `.pi-ico`, `.pt`, `.pk`)/`.pal-empty`/`.pal-foot`, `.kbd`).

---

## Структура файлов

```
src-tauri/src/
├─ models.rs              # MOD: + SearchHit
├─ commands/search.rs     # NEW: search_global + тест
├─ commands/mod.rs        # MOD: + pub mod search;
└─ lib.rs                 # MOD: регистрация search_global

src/lib/
├─ types.ts               # MOD: + SearchHit
├─ api/search.ts          # NEW
├─ stores/ui.ts           # MOD: + showPalette
└─ components/
   ├─ CommandPalette.svelte# NEW
   └─ ...
src/routes/+page.svelte    # MOD: Ctrl+K хоткей + монтирование CommandPalette
```

---

## Task 1: Rust — глобальный поиск

**Files:** Modify `models.rs`, `commands/mod.rs`, `lib.rs`; Create `commands/search.rs`.

- [ ] **Step 1: Модель** (в `models.rs`, после `ProjectCommand`/`CommandInput`):
```rust
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub kind: String,        // project | task | note | link | cred | command | file
    pub project_id: i64,
    pub project_name: String,
    pub id: i64,             // id сущности (для project — id проекта)
    pub title: String,
    pub subtitle: String,
}
```

- [ ] **Step 2: commands/search.rs**

Create `src-tauri/src/commands/search.rs`:
```rust
use crate::error::{AppError, AppResult};
use crate::models::SearchHit;
use crate::state::AppState;
use rusqlite::Connection;
use std::sync::MutexGuard;
use tauri::State;

fn lock<'a>(state: &'a State<AppState>) -> AppResult<MutexGuard<'a, Connection>> {
    state.db.lock().map_err(|_| AppError::internal("db mutex poisoned"))
}

/// Выполнить один поисковый запрос и добавить хиты. `sql` должен выбирать
/// (project_id, project_name, id, title) и принимать один параметр-паттерн.
fn collect(
    conn: &Connection,
    sql: &str,
    pat: &str,
    kind: &str,
    subtitle: &str,
    out: &mut Vec<SearchHit>,
) -> AppResult<()> {
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([pat], |r| {
        Ok(SearchHit {
            kind: kind.to_string(),
            project_id: r.get(0)?,
            project_name: r.get(1)?,
            id: r.get(2)?,
            title: r.get(3)?,
            subtitle: subtitle.to_string(),
        })
    })?;
    for r in rows {
        out.push(r?);
    }
    Ok(())
}

fn search(conn: &Connection, query: &str) -> AppResult<Vec<SearchHit>> {
    let q = query.trim();
    if q.is_empty() {
        return Ok(Vec::new());
    }
    let pat = format!("%{}%", q);
    let mut hits = Vec::new();

    collect(conn,
        "SELECT id, name, id, name FROM projects WHERE name LIKE ?1 ORDER BY name LIMIT 15",
        &pat, "project", "Проект", &mut hits)?;
    collect(conn,
        "SELECT t.project_id, p.name, t.id, t.title FROM tasks t JOIN projects p ON p.id=t.project_id
         WHERE t.title LIKE ?1 OR IFNULL(t.description,'') LIKE ?1 LIMIT 15",
        &pat, "task", "Задача", &mut hits)?;
    collect(conn,
        "SELECT n.project_id, p.name, n.id, IFNULL(NULLIF(n.title,''),'Заметка') FROM notes n JOIN projects p ON p.id=n.project_id
         WHERE IFNULL(n.title,'') LIKE ?1 OR IFNULL(n.content_md,'') LIKE ?1 LIMIT 15",
        &pat, "note", "Заметка", &mut hits)?;
    collect(conn,
        "SELECT l.project_id, p.name, l.id, l.label FROM links l JOIN projects p ON p.id=l.project_id
         WHERE l.label LIKE ?1 OR l.url LIKE ?1 LIMIT 15",
        &pat, "link", "Ссылка", &mut hits)?;
    collect(conn,
        "SELECT c.project_id, p.name, c.id, c.label FROM credentials c JOIN projects p ON p.id=c.project_id
         WHERE c.label LIKE ?1 OR IFNULL(c.username,'') LIKE ?1 LIMIT 15",
        &pat, "cred", "Кред", &mut hits)?;
    collect(conn,
        "SELECT cm.project_id, p.name, cm.id, cm.label FROM commands cm JOIN projects p ON p.id=cm.project_id
         WHERE cm.label LIKE ?1 OR cm.command LIKE ?1 LIMIT 15",
        &pat, "command", "Команда", &mut hits)?;
    collect(conn,
        "SELECT f.project_id, p.name, f.id, f.label FROM files f JOIN projects p ON p.id=f.project_id
         WHERE f.label LIKE ?1 OR f.path LIKE ?1 LIMIT 15",
        &pat, "file", "Файл", &mut hits)?;

    hits.truncate(50);
    Ok(hits)
}

#[tauri::command]
pub fn search_global(state: State<AppState>, query: String) -> AppResult<Vec<SearchHit>> {
    let conn = lock(&state)?;
    search(&conn, &query)
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
    fn finds_across_tables() {
        let conn = mem();
        conn.execute("INSERT INTO projects(name,status,sort_order) VALUES('Aurora','active',0)", []).unwrap();
        let pid = conn.last_insert_rowid();
        conn.execute("INSERT INTO tasks(project_id,title,status,sort_order) VALUES(?1,'Настроить aurora retries','todo',0)", [pid]).unwrap();
        conn.execute("INSERT INTO notes(project_id,title,content_md) VALUES(?1,'Архитектура','текст про aurora')", [pid]).unwrap();

        let hits = search(&conn, "aurora").unwrap();
        // проект по имени, задача по title, заметка по content
        assert!(hits.iter().any(|h| h.kind == "project"));
        assert!(hits.iter().any(|h| h.kind == "task"));
        assert!(hits.iter().any(|h| h.kind == "note"));
        for h in &hits {
            assert_eq!(h.project_id, pid);
        }
    }

    #[test]
    fn empty_query_returns_nothing() {
        let conn = mem();
        assert!(search(&conn, "   ").unwrap().is_empty());
    }
}
```

- [ ] **Step 3: Регистрация**
- `commands/mod.rs`: `pub mod search;`
- `lib.rs` `generate_handler![...]`: добавить `commands::search::search_global,`

- [ ] **Step 4: Тесты + сборка**
```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib
cargo build --manifest-path src-tauri/Cargo.toml
```
Expected: новые `finds_across_tables`, `empty_query_returns_nothing` + прежние (18) → 20 ok; build успешен.

- [ ] **Step 5: Commit**
```powershell
git add src-tauri/src/models.rs src-tauri/src/commands/search.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs
git commit -m @'
feat(backend): global search across projects/tasks/notes/links/creds/commands/files

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

## Task 2: Фронт — палет

**Files:** Modify `types.ts`, `stores/ui.ts`, `routes/+page.svelte`; Create `api/search.ts`, `components/CommandPalette.svelte`.

- [ ] **Step 1: Тип** (в конец `types.ts`):
```ts
export type SearchHit = {
  kind: string;
  projectId: number;
  projectName: string;
  id: number;
  title: string;
  subtitle: string;
};
```

- [ ] **Step 2: api/search.ts**
```ts
import { call } from "./client";
import type { SearchHit } from "../types";

export const global = (query: string) => call<SearchHit[]>("search_global", { query });
```

- [ ] **Step 3: ui-стор** — в `src/lib/stores/ui.ts` добавить:
```ts
export const showPalette = writable(false);
```
(`writable` уже импортирован.)

- [ ] **Step 4: components/CommandPalette.svelte**

Create `src/lib/components/CommandPalette.svelte`:
```svelte
<script lang="ts">
  import type { SearchHit, Tab } from "$lib/types";
  import { projects, activeProjectId } from "$lib/stores/projects";
  import { activeTab } from "$lib/stores/ui";
  import { showPalette } from "$lib/stores/ui";
  import * as search from "$lib/api/search";
  import Icon from "./Icon.svelte";

  let query = $state("");
  let hits = $state<SearchHit[]>([]);
  let sel = $state(0);
  let inputEl = $state<HTMLInputElement | null>(null);
  let timer: ReturnType<typeof setTimeout> | null = null;
  let runId = 0;

  // Пустой запрос → проекты из стора; иначе — бэкенд-поиск.
  const projectHits = $derived(
    $projects
      .filter((p) => p.status !== "archived")
      .map<SearchHit>((p) => ({ kind: "project", projectId: p.id, projectName: p.name, id: p.id, title: p.name, subtitle: "Проект" })),
  );
  const items = $derived(query.trim() ? hits : projectHits);

  // Поиск с debounce при вводе.
  $effect(() => {
    const q = query;
    if (timer) clearTimeout(timer);
    if (!q.trim()) {
      hits = [];
      sel = 0;
      return;
    }
    timer = setTimeout(async () => {
      const my = ++runId;
      try {
        const r = await search.global(q);
        if (my === runId) {
          hits = r;
          sel = 0;
        }
      } catch {
        /* тост из api/client.ts */
      }
    }, 180);
  });

  // При открытии — очистить и сфокусировать.
  $effect(() => {
    if ($showPalette) {
      query = "";
      hits = [];
      sel = 0;
      setTimeout(() => inputEl?.focus(), 10);
    }
  });

  const ICONS: Record<string, string> = {
    project: "box", task: "square-check-big", note: "file-text",
    link: "link", cred: "key-round", command: "terminal", file: "file",
  };
  function tabFor(kind: string): Tab {
    if (kind === "task") return "tasks";
    if (kind === "note") return "notes";
    if (kind === "cred") return "creds";
    return "overview"; // link | command | file | project
  }

  function activate(h: SearchHit | undefined) {
    if (!h) return;
    showPalette.set(false);
    activeProjectId.set(h.projectId);
    if (h.kind !== "project") activeTab.set(tabFor(h.kind));
    else activeTab.set("overview");
  }

  function onKey(e: KeyboardEvent) {
    if (!$showPalette) return;
    if (e.key === "Escape") { e.preventDefault(); showPalette.set(false); }
    else if (e.key === "ArrowDown") { e.preventDefault(); sel = Math.min(sel + 1, items.length - 1); }
    else if (e.key === "ArrowUp") { e.preventDefault(); sel = Math.max(sel - 1, 0); }
    else if (e.key === "Enter") { e.preventDefault(); activate(items[sel]); }
  }
</script>

<svelte:window onkeydown={onKey} />

{#if $showPalette}
  <div class="palette-scrim open" role="presentation" onmousedown={(e) => { if (e.currentTarget === e.target) showPalette.set(false); }}>
    <div class="palette" role="dialog" aria-label="Командный палет">
      <div class="pal-input">
        <Icon name="search" class="ic ic-lg" />
        <input bind:this={inputEl} bind:value={query} placeholder="Поиск проектов и содержимого…" autocomplete="off" />
        <span class="kbd">Esc</span>
      </div>
      <div class="pal-list">
        {#if items.length}
          {#each items as h, i (h.kind + "-" + h.id + "-" + i)}
            <div class="pal-item" class:sel={i === sel}
                 role="button" tabindex="-1"
                 onmousemove={() => (sel = i)} onclick={() => activate(h)}>
              <span class="pi-ico"><Icon name={ICONS[h.kind] ?? "box"} class="ic-sm" /></span>
              <span class="pt">{h.title}<small>{h.subtitle}{h.kind !== "project" ? ` · ${h.projectName}` : ""}</small></span>
              {#if i === sel}<span class="pk"><span class="kbd">↵</span></span>{/if}
            </div>
          {/each}
        {:else}
          <div class="pal-empty">{query.trim() ? "Ничего не найдено" : "Нет проектов"}</div>
        {/if}
      </div>
      <div class="pal-foot">
        <span class="k"><span class="kbd">↑</span><span class="kbd">↓</span> навигация</span>
        <span class="k"><span class="kbd">↵</span> выбрать</span>
        <span class="k"><span class="kbd">Esc</span> закрыть</span>
      </div>
    </div>
  </div>
{/if}
```
> Иконки `search`/`box`/`square-check-big`/`file-text`/`link`/`key-round`/`terminal`/`file` — из lucide. Все `.palette-*`/`.pal-*`/`.kbd` классы — в global.css. `Tab` — тип из `stores/ui` экспортируется? Он объявлен в `ui.ts` (`export type Tab`). Импортируем из `$lib/stores/ui`, а не из types — поправить импорт: `import { activeTab, showPalette, type Tab } from "$lib/stores/ui";` и убрать `Tab` из импорта types.

- [ ] **Step 5: Ctrl+K + монтирование в +page.svelte**

В `src/routes/+page.svelte`:
1. Импорты:
```ts
  import CommandPalette from "$lib/components/CommandPalette.svelte";
  import { showPalette } from "$lib/stores/ui";
```
2. Глобальный хоткей — добавить `<svelte:window onkeydown={...} />` (или дополнить существующий, если есть):
```svelte
<svelte:window onkeydown={(e) => {
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
    e.preventDefault();
    showPalette.update((v) => !v);
  }
}} />
```
3. Смонтировать компонент рядом с `<NewProjectModal />`:
```svelte
<CommandPalette />
```

> Если в `+page.svelte` уже есть `<svelte:window>` — объединить обработчики в один.

- [ ] **Step 6: Проверка**
```powershell
npm run check
npm test
npm run build
```
Expected: 0 ошибок типов; Vitest 3 (при редком транзиенте — перезапустить); build успешен.

- [ ] **Step 7: Commit**
```powershell
git add src/lib/types.ts src/lib/api/search.ts src/lib/stores/ui.ts src/lib/components/CommandPalette.svelte src/routes/+page.svelte
git commit -m @'
feat(frontend): command palette (Ctrl+K) with global search

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

## Task 3: Проверка среза

- [ ] **Step 1: Автопроверка**
```powershell
npm run check ; npm test ; cargo test --manifest-path src-tauri/Cargo.toml --lib
```
Expected: типы 0 ошибок; Vitest 3; Rust 20.

- [ ] **Step 2: Ручной GUI-смоук (пользователь; `npm run tauri dev`)**
- [ ] Ctrl+K → палет открывается, фокус в поле; пустой запрос показывает список проектов.
- [ ] ↑/↓ выделяют, Enter переходит к проекту (вкладка «Обзор»).
- [ ] Ввести слово из задачи/заметки/ссылки → в результатах видны хиты с типом и именем проекта; Enter → переход к проекту и нужной вкладке (задача→Задачи, заметка→Заметки, кред→Креды).
- [ ] Клик мышью по результату — то же.
- [ ] Esc / клик по фону — закрывает.
- [ ] Ctrl+K повторно — переключает (открыть/закрыть).

---

## Итог среза

Командный палет по Ctrl+K с глобальным поиском по всему содержимому (проекты/задачи/заметки/ссылки/креды/команды/файлы) и быстрым переходом к месту. Секреты кредов в поиск не попадают. Отложено: действия из палета (push/копировать/запустить), Ctrl+F, подсветка совпадений. Дальше по v1.0 — дашборд/темы/трей, бэкап/экспорт.
```
