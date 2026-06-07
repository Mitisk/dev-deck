# DevDeck v1.0 — Срез «Заметки (Markdown)» — план реализации

> **Для исполнителя:** ОБЯЗАТЕЛЬНЫЙ ПОД-НАВЫК: superpowers:subagent-driven-development. Шаги — чекбоксы.

**Цель:** Вкладка «Заметки» — несколько Markdown-заметок на проект. Редактор с разделением «исходник | превью», рендер через `marked`. Автосохранение (debounce при вводе + сброс при переключении/закрытии). Создание/удаление заметок.

**Архитектура:** Команды `notes_*` лочат БД (логика — `&Connection`-функции, тестируемые). Фронт: `NotesTab.svelte` грузит `notes_list(projectId)`, селектор заметок + split-редактор; правки идут в локальный state, автосейв через debounce вызывает `notes_update`; превью — `{@html marked.parse(content)}`.

**Стек:** + npm `marked` (Markdown→HTML на фронте).

**Решения / границы:**
- Рендер превью через `{@html marked.parse(...)}` — контент пишет сам пользователь (local single-user). Санитизация (DOMPurify) понадобится, когда появится **импорт JSON** заметок (отмечено) — тогда чужой контент станет недоверенным.
- Автосейв: debounce 600 мс на ввод + явный сброс при смене заметки/размонтировании.

**Источники:** `TZ_DevDeck.md` (5.7 — заметки; 10 — `notes_*`). `_prototype/index.html` (`.notes`/`.note-pane`/`.note-src`/`.note-prev` + markdown-стили; `notesHTML`/`mdToHtml`). Схема `notes` в миграции 0001.

---

## Контекст

- Rust: 36 команд; `models.rs`, `error::{AppError,ErrorKind,AppResult}`, `state::AppState`, `commands/{...}.rs`. Таблица `notes`: `id, project_id, title, content_md, updated_at` (FK ON DELETE CASCADE).
- Фронт: `ProjectView.svelte` рендерит вкладки; `notes` — плейсхолдер. api `client.ts`, `Icon.svelte`. global.css: `.notes`(split grid), `.note-pane`(`.src`), `.pane-bar`, `.note-src`(textarea), `.note-prev` + стили `h1..h3/p/ul/li/code/pre/a/blockquote/hr`.

---

## Структура файлов

```
src-tauri/src/
├─ models.rs              # MOD: + Note
├─ commands/notes.rs      # NEW: notes_* + helpers + тест
├─ commands/mod.rs        # MOD: + pub mod notes;
└─ lib.rs                 # MOD: регистрация 4 команд

src/lib/
├─ types.ts               # MOD: + Note
├─ api/notes.ts           # NEW
└─ components/
   ├─ NotesTab.svelte     # NEW: селектор + split-редактор + автосейв
   └─ ProjectView.svelte  # MOD: рендер NotesTab на табе "notes"
package.json              # MOD: + marked
```

---

## Task 1: Rust — команды заметок

**Files:** Modify `models.rs`, `commands/mod.rs`, `lib.rs`; Create `commands/notes.rs`.

- [ ] **Step 1: Модель** (в `models.rs`, после `Credential`):
```rust
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    pub id: i64,
    pub project_id: i64,
    pub title: Option<String>,
    pub content_md: Option<String>,
    pub updated_at: String,
}
```

- [ ] **Step 2: commands/notes.rs**

Create `src-tauri/src/commands/notes.rs`:
```rust
use crate::error::{AppError, AppResult};
use crate::models::Note;
use crate::state::AppState;
use rusqlite::{params, Connection};
use std::sync::MutexGuard;
use tauri::State;

fn lock<'a>(state: &'a State<AppState>) -> AppResult<MutexGuard<'a, Connection>> {
    state.db.lock().map_err(|_| AppError::internal("db mutex poisoned"))
}

fn row_to_note(conn: &Connection, id: i64) -> AppResult<Note> {
    Ok(conn.query_row(
        "SELECT id, project_id, title, content_md, updated_at FROM notes WHERE id = ?1",
        [id],
        |r| {
            Ok(Note {
                id: r.get(0)?,
                project_id: r.get(1)?,
                title: r.get(2)?,
                content_md: r.get(3)?,
                updated_at: r.get(4)?,
            })
        },
    )?)
}

fn list_notes(conn: &Connection, project_id: i64) -> AppResult<Vec<Note>> {
    let mut stmt = conn.prepare(
        "SELECT id FROM notes WHERE project_id = ?1 ORDER BY updated_at DESC, id DESC",
    )?;
    let ids = stmt.query_map([project_id], |r| r.get::<_, i64>(0))?;
    let mut out = Vec::new();
    for id in ids {
        out.push(row_to_note(conn, id?)?);
    }
    Ok(out)
}

fn create_note(conn: &Connection, project_id: i64) -> AppResult<Note> {
    conn.execute(
        "INSERT INTO notes(project_id, title, content_md) VALUES (?1, 'Без названия', '')",
        [project_id],
    )?;
    row_to_note(conn, conn.last_insert_rowid())
}

fn update_note(conn: &Connection, id: i64, title: &str, content_md: &str) -> AppResult<()> {
    conn.execute(
        "UPDATE notes SET title=?2, content_md=?3, updated_at=datetime('now') WHERE id=?1",
        params![id, title, content_md],
    )?;
    Ok(())
}

fn delete_note(conn: &Connection, id: i64) -> AppResult<()> {
    conn.execute("DELETE FROM notes WHERE id=?1", [id])?;
    Ok(())
}

#[tauri::command]
pub fn notes_list(state: State<AppState>, project_id: i64) -> AppResult<Vec<Note>> {
    let conn = lock(&state)?;
    list_notes(&conn, project_id)
}

#[tauri::command]
pub fn notes_create(state: State<AppState>, project_id: i64) -> AppResult<Note> {
    let conn = lock(&state)?;
    create_note(&conn, project_id)
}

#[tauri::command]
pub fn notes_update(state: State<AppState>, id: i64, title: String, content_md: String) -> AppResult<()> {
    let conn = lock(&state)?;
    update_note(&conn, id, &title, &content_md)
}

#[tauri::command]
pub fn notes_delete(state: State<AppState>, id: i64) -> AppResult<()> {
    let conn = lock(&state)?;
    delete_note(&conn, id)
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
    fn create_update_list_delete() {
        let conn = mem();
        conn.execute("INSERT INTO projects(name,status,sort_order) VALUES('P','active',0)", []).unwrap();
        let pid = conn.last_insert_rowid();

        let n = create_note(&conn, pid).unwrap();
        assert_eq!(n.title.as_deref(), Some("Без названия"));

        update_note(&conn, n.id, "Архитектура", "# Заголовок\n\nтекст").unwrap();
        let list = list_notes(&conn, pid).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].title.as_deref(), Some("Архитектура"));
        assert_eq!(list[0].content_md.as_deref(), Some("# Заголовок\n\nтекст"));

        delete_note(&conn, n.id).unwrap();
        assert_eq!(list_notes(&conn, pid).unwrap().len(), 0);
    }
}
```

- [ ] **Step 3: Регистрация**
- `commands/mod.rs`: `pub mod notes;`
- `lib.rs` `generate_handler![...]`: добавить
```rust
            commands::notes::notes_list,
            commands::notes::notes_create,
            commands::notes::notes_update,
            commands::notes::notes_delete,
```

- [ ] **Step 4: Тесты + сборка**
```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib
cargo build --manifest-path src-tauri/Cargo.toml
```
Expected: новый `create_update_list_delete` + прежние (16) → 17 ok; build успешен.

- [ ] **Step 5: Commit**
```powershell
git add src-tauri/src/models.rs src-tauri/src/commands/notes.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs
git commit -m @'
feat(backend): notes CRUD

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

## Task 2: Фронт — NotesTab

**Files:** Modify `package.json` (через npm), `types.ts`, `ProjectView.svelte`; Create `api/notes.ts`, `components/NotesTab.svelte`.

- [ ] **Step 1: Зависимость marked**
```powershell
npm install marked
```
Expected: `marked` в `dependencies`.

- [ ] **Step 2: Тип** (в конец `types.ts`):
```ts
export type Note = {
  id: number;
  projectId: number;
  title: string | null;
  contentMd: string | null;
  updatedAt: string;
};
```

- [ ] **Step 3: api/notes.ts**
```ts
import { call } from "./client";
import type { Note } from "../types";

export const list = (projectId: number) => call<Note[]>("notes_list", { projectId });
export const create = (projectId: number) => call<Note>("notes_create", { projectId });
export const update = (id: number, title: string, contentMd: string) =>
  call<void>("notes_update", { id, title, contentMd });
export const remove = (id: number) => call<void>("notes_delete", { id });
```

- [ ] **Step 4: components/NotesTab.svelte**

Create `src/lib/components/NotesTab.svelte`:
```svelte
<script lang="ts">
  import type { Project, Note } from "$lib/types";
  import * as notesApi from "$lib/api/notes";
  import { marked } from "marked";
  import Icon from "./Icon.svelte";

  let { project }: { project: Project } = $props();

  let notes = $state<Note[]>([]);
  let activeId = $state<number | null>(null);
  let title = $state("");
  let content = $state("");
  let saveTimer: ReturnType<typeof setTimeout> | null = null;

  const active = $derived(notes.find((n) => n.id === activeId) ?? null);
  const preview = $derived(content ? marked.parse(content) : "");

  let reqId = 0;
  async function load(selectId?: number) {
    const my = ++reqId;
    try {
      const r = await notesApi.list(project.id);
      if (my !== reqId) return;
      notes = r;
      const pick = selectId ?? (r.some((n) => n.id === activeId) ? activeId : r[0]?.id ?? null);
      selectNote(pick, false);
    } catch {
      /* тост из api/client.ts */
    }
  }

  function selectNote(id: number | null, flushFirst = true) {
    if (flushFirst) flush();
    activeId = id;
    const n = notes.find((x) => x.id === id);
    title = n?.title ?? "";
    content = n?.contentMd ?? "";
  }

  // Перезагрузка при смене проекта.
  $effect(() => {
    project.id;
    load();
    return () => flush();
  });

  function scheduleSave() {
    if (saveTimer) clearTimeout(saveTimer);
    saveTimer = setTimeout(flush, 600);
  }

  function flush() {
    if (saveTimer) {
      clearTimeout(saveTimer);
      saveTimer = null;
    }
    const id = activeId;
    if (id == null) return;
    const n = notes.find((x) => x.id === id);
    if (!n) return;
    if ((n.title ?? "") === title && (n.contentMd ?? "") === content) return; // нет изменений
    n.title = title;
    n.contentMd = content;
    void notesApi.update(id, title, content);
  }

  async function newNote() {
    flush();
    const n = await notesApi.create(project.id);
    await load(n.id);
  }

  async function deleteActive() {
    const id = activeId;
    if (id == null) return;
    if (saveTimer) { clearTimeout(saveTimer); saveTimer = null; }
    activeId = null;
    await notesApi.remove(id);
    await load();
  }
</script>

<div style="display:flex;align-items:center;gap:8px;margin-bottom:12px;flex-wrap:wrap">
  {#each notes as n (n.id)}
    <button class="tab" class:active={activeId === n.id} onclick={() => selectNote(n.id)}>
      {n.title || "Без названия"}
    </button>
  {/each}
  <button class="gbtn" onclick={newNote}><Icon name="plus" class="ic-sm" /> Заметка</button>
</div>

{#if active}
  <div style="display:flex;align-items:center;gap:8px;margin-bottom:10px">
    <input
      class="tin"
      style="flex:1;font-weight:600"
      placeholder="Заголовок заметки"
      bind:value={title}
      oninput={scheduleSave}
      onblur={flush}
    />
    <button class="btn-danger" onclick={deleteActive}><Icon name="trash-2" class="ic-sm" /> Удалить</button>
  </div>
  <div class="card notes">
    <div class="note-pane src">
      <div class="pane-bar">Исходник</div>
      <textarea class="note-src" bind:value={content} oninput={scheduleSave} onblur={flush}></textarea>
    </div>
    <div class="note-pane">
      <div class="pane-bar">Превью</div>
      <div class="note-prev">{@html preview}</div>
    </div>
  </div>
{:else}
  <button class="add-cred" onclick={newNote}><Icon name="plus" class="ic-sm" /> Создать первую заметку</button>
{/if}
```
> `marked.parse` синхронный (marked v12+) — возвращает строку. Превью через `{@html}` (контент свой; санитизация — при будущем импорте). Классы `.notes`/`.note-pane`/`.note-src`/`.note-prev`/`.pane-bar` — в global.css.

- [ ] **Step 5: Рендер в ProjectView.svelte**
Импорт `import NotesTab from "./NotesTab.svelte";` и ветка в `tab-body`:
```svelte
    {:else if $activeTab === "notes"}
      <NotesTab {project} />
```

- [ ] **Step 6: Проверка**
```powershell
npm run check
npm test
npm run build
```
Expected: 0 ошибок типов (если `marked.parse` тип `string | Promise<string>` мешает `{@html}` — обернуть: `const preview = $derived(content ? (marked.parse(content) as string) : "")`); Vitest 3 (при редком транзиенте «no tests» — перезапустить); build успешен.

- [ ] **Step 7: Commit**
```powershell
git add package.json package-lock.json src/lib/types.ts src/lib/api/notes.ts src/lib/components/NotesTab.svelte src/lib/components/ProjectView.svelte
git commit -m @'
feat(frontend): markdown notes tab (split editor, autosave)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

## Task 3: Проверка среза

- [ ] **Step 1: Автопроверка**
```powershell
npm run check ; npm test ; cargo test --manifest-path src-tauri/Cargo.toml --lib
```
Expected: типы 0 ошибок; Vitest 3; Rust 17.

- [ ] **Step 2: Ручной GUI-смоук (пользователь; `npm run tauri dev`)**
- [ ] Вкладка «Заметки» → «Заметка» создаёт новую; заголовок «Без названия».
- [ ] Ввод Markdown в «Исходник» → превью обновляется (заголовки, списки, код, ссылки).
- [ ] Правка заголовка/текста → подождать ~1 с → переключиться на другую заметку и обратно → изменения сохранены (автосейв).
- [ ] Перезапуск приложения → заметки и их содержимое на месте.
- [ ] Несколько заметок → переключение между ними табами.
- [ ] «Удалить» → заметка исчезла.

---

## Итог среза

Вкладка «Заметки» — несколько Markdown-заметок на проект со split-редактором, живым превью (`marked`) и автосохранением. Закрывает последнюю «живую» вкладку проекта (Обзор — композитная, наполнится ссылками/командами/git-сводкой в своих срезах). Дальше по v1.0 — ссылки/файлы-ярлыки, кастомные команды-кнопки, командный палет. Отложено: санитизация Markdown при импорте JSON.
```
