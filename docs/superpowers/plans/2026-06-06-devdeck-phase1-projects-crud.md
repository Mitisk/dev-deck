# DevDeck Phase 1 — Срез «Проекты CRUD» — план реализации

> **Для исполнителя:** ОБЯЗАТЕЛЬНЫЙ ПОД-НАВЫК: используйте superpowers:subagent-driven-development для пошагового выполнения. Шаги отмечены чекбоксами (`- [ ]`).

**Цель:** Заменить моковые проекты реальными данными из SQLite. Полный CRUD проекта (создание/чтение/обновление/удаление/архив), закрепление (pin) и порядок (`sort_order`), с тегами. Сайдбар и дашборд работают на живых данных; пустое состояние при отсутствии проектов; создание через модалку; редактирование/удаление/архив через вкладку «Настройки».

**Архитектура:** Rust-команды по домену `projects` поверх таблиц `projects`/`tags`/`project_tags` (уже в миграции 0001). Структуры сериализуются в **camelCase** (`#[serde(rename_all = "camelCase")]`), Tauri конвертирует имена top-level аргументов camelCase↔snake_case. Фронт зовёт их только через `src/lib/api/projects.ts`; стор `projects` грузится из бэкенда. Тип `Project` на фронте сужается до полей записи проекта (вспомогательные git/tasks/creds появятся в своих срезах).

**Стек:** как в Phase 0 (Tauri 2 · rusqlite 0.32 · SvelteKit SPA / Svelte 5 руны · Vitest).

**Решения (от пользователя):** при пустой БД — пустое состояние с кнопкой «Создать» (без сид-данных). Drag-сортировку в сайдбаре сейчас НЕ делаем (команда `project_set_sort` закладывается, UI — позже).

**Источники:** `TZ_DevDeck.md` (раздел 4 — схема, раздел 5.1 — требования к проектам, раздел 10 — команды), `_prototype/index.html` (модалка нового проекта — `openNewProject`/markup мастера; вкладка настроек — `settingsHTML`/`wireSettings`), Phase 0 (`src/lib/...`, `src-tauri/src/...`).

---

## Контекст текущего кода (после Phase 0)

- БД: миграция `0001_init.sql` уже создала `projects`, `tags`, `project_tags` и др. `db::open` накатывает миграции; `AppState { db: Mutex<Connection> }`; команда `db_health`.
- Фронт: `src/lib/types.ts` (мок-тип `Project` со множеством полей: git/commands/links/tasks/checklists/creds/note), `src/lib/mock.ts` (`USER`, `MOCK_PROJECTS`), стор `projects` (= MOCK_PROJECTS), `activeProjectId: string|null`. Компоненты `Sidebar`, `Dashboard`, `ProjectView` (табы-плейсхолдеры), `Workspace`, `Toasts`, `Icon`. api-слой `client.ts` (`call<T>`), `health.ts`.
- `Icon.svelte` читает проп `name` (статически) — для CRUD будем передавать литералы; динамических имён в этом срезе нет.
- Кнопка «+ Новый проект» в сайдбаре пока без обработчика.

---

## Структура файлов (создаём/меняем в этом срезе)

```
src-tauri/src/
├─ models.rs            # NEW: Project, ProjectInput (serde camelCase)
├─ commands/
│  ├─ mod.rs            # MOD: + pub mod projects;
│  └─ projects.rs       # NEW: 8 команд + tag-хелперы + тесты
└─ lib.rs               # MOD: mod models; регистрация команд projects_*

src/lib/
├─ types.ts             # MOD: core Project (number id, camelCase), убрать aux-типы
├─ mock.ts              # MOD: оставить только USER, убрать MOCK_PROJECTS
├─ format.ts            # NEW: statusLabel() локализация статуса
├─ api/
│  └─ projects.ts       # NEW: типизированные обёртки команд
├─ stores/
│  └─ projects.ts       # MOD: грузить из бэкенда; activeProjectId: number|null; loadProjects()
└─ components/
   ├─ Sidebar.svelte        # MOD: живые данные, icon/description, статус, empty-state, открыть модалку
   ├─ Dashboard.svelte      # MOD: живые данные, убрать git.branch, empty-state
   ├─ ProjectView.svelte    # MOD: статус-локализация; вкладка «Настройки» = SettingsTab
   ├─ NewProjectModal.svelte# NEW: мастер создания (порт из прототипа), wired to create
   ├─ SettingsTab.svelte    # NEW: редактирование/pin/архив/удаление, wired to update/...
   └─ ConfirmDialog.svelte  # NEW: подтверждение удаления

src/routes/+page.svelte  # MOD: loadProjects() на старте; смонтировать NewProjectModal
```

---

## Task 1: Rust — модели Project / ProjectInput

**Files:**
- Create: `src-tauri/src/models.rs`
- Modify: `src-tauri/src/lib.rs` (добавить `mod models;`)

- [ ] **Step 1: Создать models.rs**

Create `src-tauri/src/models.rs`:
```rust
use serde::{Deserialize, Serialize};

/// Запись проекта (camelCase для фронта).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub status: String, // active | paused | done | archived
    pub color: Option<String>,
    pub icon: Option<String>, // emoji
    pub path: Option<String>,
    pub repo_path: Option<String>,
    pub pinned: bool,
    pub sort_order: i64,
    pub tags: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Вход на создание/обновление проекта.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInput {
    pub name: String,
    pub description: Option<String>,
    pub status: Option<String>,
    pub color: Option<String>,
    pub icon: Option<String>,
    pub path: Option<String>,
    pub repo_path: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}
```

- [ ] **Step 2: Подключить модуль в lib.rs**

В `src-tauri/src/lib.rs` в начало (рядом с `mod commands; mod db; mod error; mod state;`) добавить:
```rust
mod models;
```

- [ ] **Step 3: Сборка (не пройдёт до Task 2 — модели ещё не используются → dead_code).** Перейти к Task 2; собирать будем там.

---

## Task 2: Rust — команды projects + теги + тесты

**Files:**
- Create: `src-tauri/src/commands/projects.rs`
- Modify: `src-tauri/src/commands/mod.rs`, `src-tauri/src/lib.rs`
- Test: внутри `projects.rs` (`#[cfg(test)]`)

- [ ] **Step 1: Написать команды и хелперы**

Create `src-tauri/src/commands/projects.rs`:
```rust
use crate::error::{AppError, AppResult};
use crate::models::{Project, ProjectInput};
use crate::state::AppState;
use rusqlite::{params, Connection};
use tauri::State;

/// Загрузить теги проекта (отсортированные по имени).
fn load_tags(conn: &Connection, project_id: i64) -> AppResult<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT t.name FROM tags t
         JOIN project_tags pt ON pt.tag_id = t.id
         WHERE pt.project_id = ?1
         ORDER BY t.name",
    )?;
    let rows = stmt.query_map([project_id], |r| r.get::<_, String>(0))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

/// Перепривязать теги проекта: upsert в tags, пересоздать связи.
fn set_tags(conn: &Connection, project_id: i64, tags: &[String]) -> AppResult<()> {
    conn.execute("DELETE FROM project_tags WHERE project_id = ?1", [project_id])?;
    for raw in tags {
        let name = raw.trim();
        if name.is_empty() {
            continue;
        }
        conn.execute("INSERT OR IGNORE INTO tags(name) VALUES (?1)", [name])?;
        let tag_id: i64 = conn.query_row("SELECT id FROM tags WHERE name = ?1", [name], |r| r.get(0))?;
        conn.execute(
            "INSERT OR IGNORE INTO project_tags(project_id, tag_id) VALUES (?1, ?2)",
            params![project_id, tag_id],
        )?;
    }
    Ok(())
}

/// Прочитать один проект (без тегов) из строки.
fn row_to_project(conn: &Connection, id: i64) -> AppResult<Project> {
    let mut p = conn.query_row(
        "SELECT id, name, description, status, color, icon, path, repo_path,
                pinned, sort_order, created_at, updated_at
         FROM projects WHERE id = ?1",
        [id],
        |r| {
            Ok(Project {
                id: r.get(0)?,
                name: r.get(1)?,
                description: r.get(2)?,
                status: r.get(3)?,
                color: r.get(4)?,
                icon: r.get(5)?,
                path: r.get(6)?,
                repo_path: r.get(7)?,
                pinned: r.get::<_, i64>(8)? != 0,
                sort_order: r.get(9)?,
                tags: Vec::new(),
                created_at: r.get(10)?,
                updated_at: r.get(11)?,
            })
        },
    )?;
    p.tags = load_tags(conn, id)?;
    Ok(p)
}

fn lock<'a>(state: &'a State<AppState>) -> AppResult<std::sync::MutexGuard<'a, Connection>> {
    state.db.lock().map_err(|_| AppError::internal("db mutex poisoned"))
}

#[tauri::command]
pub fn projects_list(state: State<AppState>) -> AppResult<Vec<Project>> {
    let conn = lock(&state)?;
    let mut stmt = conn.prepare(
        "SELECT id FROM projects
         ORDER BY pinned DESC, sort_order ASC, name COLLATE NOCASE ASC",
    )?;
    let ids = stmt.query_map([], |r| r.get::<_, i64>(0))?;
    let mut out = Vec::new();
    for id in ids {
        out.push(row_to_project(&conn, id?)?);
    }
    Ok(out)
}

#[tauri::command]
pub fn projects_get(state: State<AppState>, id: i64) -> AppResult<Project> {
    let conn = lock(&state)?;
    row_to_project(&conn, id)
}

#[tauri::command]
pub fn projects_create(state: State<AppState>, input: ProjectInput) -> AppResult<Project> {
    let name = input.name.trim();
    if name.is_empty() {
        return Err(AppError { kind: crate::error::ErrorKind::Validation, message: "Имя проекта не может быть пустым".into() });
    }
    let conn = lock(&state)?;
    let next_sort: i64 = conn.query_row("SELECT COALESCE(MAX(sort_order), 0) + 1 FROM projects", [], |r| r.get(0))?;
    conn.execute(
        "INSERT INTO projects(name, description, status, color, icon, path, repo_path, pinned, sort_order)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, ?8)",
        params![
            name,
            input.description,
            input.status.as_deref().unwrap_or("active"),
            input.color,
            input.icon,
            input.path,
            input.repo_path,
            next_sort,
        ],
    )?;
    let id = conn.last_insert_rowid();
    set_tags(&conn, id, &input.tags)?;
    row_to_project(&conn, id)
}

#[tauri::command]
pub fn projects_update(state: State<AppState>, id: i64, input: ProjectInput) -> AppResult<Project> {
    let name = input.name.trim();
    if name.is_empty() {
        return Err(AppError { kind: crate::error::ErrorKind::Validation, message: "Имя проекта не может быть пустым".into() });
    }
    let conn = lock(&state)?;
    let n = conn.execute(
        "UPDATE projects SET name=?2, description=?3, status=?4, color=?5, icon=?6,
                path=?7, repo_path=?8, updated_at=datetime('now')
         WHERE id=?1",
        params![
            id, name, input.description,
            input.status.as_deref().unwrap_or("active"),
            input.color, input.icon, input.path, input.repo_path,
        ],
    )?;
    if n == 0 {
        return Err(AppError { kind: crate::error::ErrorKind::NotFound, message: "Проект не найден".into() });
    }
    set_tags(&conn, id, &input.tags)?;
    row_to_project(&conn, id)
}

#[tauri::command]
pub fn projects_delete(state: State<AppState>, id: i64) -> AppResult<()> {
    let conn = lock(&state)?;
    conn.execute("DELETE FROM projects WHERE id = ?1", [id])?;
    Ok(())
}

#[tauri::command]
pub fn projects_archive(state: State<AppState>, id: i64) -> AppResult<()> {
    let conn = lock(&state)?;
    conn.execute(
        "UPDATE projects SET status='archived', updated_at=datetime('now') WHERE id=?1",
        [id],
    )?;
    Ok(())
}

#[tauri::command]
pub fn project_set_pinned(state: State<AppState>, id: i64, pinned: bool) -> AppResult<()> {
    let conn = lock(&state)?;
    conn.execute(
        "UPDATE projects SET pinned=?2, updated_at=datetime('now') WHERE id=?1",
        params![id, pinned as i64],
    )?;
    Ok(())
}

#[tauri::command]
pub fn project_set_sort(state: State<AppState>, id: i64, sort_order: i64) -> AppResult<()> {
    let conn = lock(&state)?;
    conn.execute(
        "UPDATE projects SET sort_order=?2, updated_at=datetime('now') WHERE id=?1",
        params![id, sort_order],
    )?;
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

    // Хелперы вызываем напрямую (команды требуют State, его в юните не построить).
    fn insert(conn: &Connection, name: &str, tags: &[&str]) -> i64 {
        conn.execute(
            "INSERT INTO projects(name, status, sort_order) VALUES (?1, 'active', 0)",
            [name],
        ).unwrap();
        let id = conn.last_insert_rowid();
        let owned: Vec<String> = tags.iter().map(|s| s.to_string()).collect();
        set_tags(conn, id, &owned).unwrap();
        id
    }

    #[test]
    fn create_with_tags_and_read_back() {
        let conn = mem();
        let id = insert(&conn, "Aurora", &["rust", "axum"]);
        let p = row_to_project(&conn, id).unwrap();
        assert_eq!(p.name, "Aurora");
        assert_eq!(p.tags, vec!["axum".to_string(), "rust".to_string()]); // отсортированы по имени
        assert_eq!(p.status, "active");
        assert!(!p.pinned);
    }

    #[test]
    fn set_tags_replaces_and_dedups_global() {
        let conn = mem();
        let a = insert(&conn, "A", &["shared", "x"]);
        let b = insert(&conn, "B", &["shared", "y"]);
        // 'shared' в tags один раз (UNIQUE), но связан с обоими.
        let shared_count: i64 = conn
            .query_row("SELECT count(*) FROM tags WHERE name='shared'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(shared_count, 1);
        assert_eq!(row_to_project(&conn, a).unwrap().tags, vec!["shared".to_string(), "x".to_string()]);
        // Переустановка тегов A очищает старые связи.
        set_tags(&conn, a, &["only".to_string()]).unwrap();
        assert_eq!(row_to_project(&conn, a).unwrap().tags, vec!["only".to_string()]);
        assert_eq!(row_to_project(&conn, b).unwrap().tags, vec!["shared".to_string(), "y".to_string()]);
    }

    #[test]
    fn delete_cascades_project_tags() {
        let conn = mem();
        let id = insert(&conn, "Temp", &["t1", "t2"]);
        conn.execute("DELETE FROM projects WHERE id = ?1", [id]).unwrap();
        let links: i64 = conn
            .query_row("SELECT count(*) FROM project_tags WHERE project_id = ?1", [id], |r| r.get(0))
            .unwrap();
        assert_eq!(links, 0);
    }
}
```

- [ ] **Step 2: Зарегистрировать модуль команд**

В `src-tauri/src/commands/mod.rs` добавить:
```rust
pub mod projects;
```

- [ ] **Step 3: Зарегистрировать команды в lib.rs**

В `src-tauri/src/lib.rs` в `tauri::generate_handler![...]` добавить команды (через запятую после `commands::health::db_health`):
```rust
        .invoke_handler(tauri::generate_handler![
            commands::health::db_health,
            commands::projects::projects_list,
            commands::projects::projects_get,
            commands::projects::projects_create,
            commands::projects::projects_update,
            commands::projects::projects_delete,
            commands::projects::projects_archive,
            commands::projects::project_set_pinned,
            commands::projects::project_set_sort,
        ])
```

- [ ] **Step 4: Тесты + сборка (NON-blocking; не запускать `npm run tauri dev`)**

Run:
```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib
cargo build --manifest-path src-tauri/Cargo.toml
```
Expected: тесты `create_with_tags_and_read_back`, `set_tags_replaces_and_dedups_global`, `delete_cascades_project_tags` + миграционные (`applies_migrations_on_empty_db`, `run_is_idempotent`) — все ok. Build успешен. Возможен dead_code warning на неиспользуемых вариантах `ErrorKind` — приемлемо.

- [ ] **Step 5: Commit**

```powershell
git add src-tauri/src/models.rs src-tauri/src/commands src-tauri/src/lib.rs
git commit -m @'
feat(backend): projects CRUD commands with tags

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

## Task 3: Фронт — типы, mock, локализация статуса

**Files:**
- Modify: `src/lib/types.ts`, `src/lib/mock.ts`
- Create: `src/lib/format.ts`

- [ ] **Step 1: Сузить types.ts до core Project**

Заменить ВЕСЬ `src/lib/types.ts` на:
```ts
// Статусы проекта (как в БД).
export type ProjectStatus = "active" | "paused" | "done" | "archived";

// Запись проекта (camelCase, совпадает с сериализацией Rust).
export type Project = {
  id: number;
  name: string;
  description: string | null;
  status: ProjectStatus;
  color: string | null;
  icon: string | null; // emoji
  path: string | null;
  repoPath: string | null;
  pinned: boolean;
  sortOrder: number;
  tags: string[];
  createdAt: string;
  updatedAt: string;
};

export type User = { name: string; handle: string; initials: string };
```
(Вспомогательные типы GitInfo/Command/Link/Task/Checklist/Cred удалены — вернутся в своих срезах.)

- [ ] **Step 2: Очистить mock.ts (оставить только USER)**

Заменить ВЕСЬ `src/lib/mock.ts` на:
```ts
import type { User } from "./types";

// Личность пользователя приложения (не проект). Останется константой до среза настроек.
export const USER: User = { name: "Артём", handle: "@artyom", initials: "АК" };
```

- [ ] **Step 3: Хелпер локализации статуса**

Create `src/lib/format.ts`:
```ts
import type { ProjectStatus } from "./types";

const STATUS_LABELS: Record<ProjectStatus, string> = {
  active: "Активен",
  paused: "На паузе",
  done: "Завершён",
  archived: "В архиве",
};

export function statusLabel(s: ProjectStatus): string {
  return STATUS_LABELS[s] ?? s;
}

export const STATUS_OPTIONS: { value: ProjectStatus; label: string }[] = [
  { value: "active", label: "Активен" },
  { value: "paused", label: "На паузе" },
  { value: "done", label: "Завершён" },
  { value: "archived", label: "В архиве" },
];
```

- [ ] **Step 4: Проверка типов СЕЙЧАС упадёт** (стор/компоненты ещё на старом типе). Это ожидаемо — починим в Task 5–8. Build не запускаем здесь. Перейти к Task 4.

---

## Task 4: Фронт — api/projects.ts

**Files:**
- Create: `src/lib/api/projects.ts`

- [ ] **Step 1: Типизированные обёртки команд**

Create `src/lib/api/projects.ts`:
```ts
import { call } from "./client";
import type { Project, ProjectStatus } from "../types";

// Вход на создание/обновление (camelCase — сериализуется в ProjectInput на Rust).
export type ProjectInput = {
  name: string;
  description?: string | null;
  status?: ProjectStatus | null;
  color?: string | null;
  icon?: string | null;
  path?: string | null;
  repoPath?: string | null;
  tags: string[];
};

export const list = () => call<Project[]>("projects_list");
export const get = (id: number) => call<Project>("projects_get", { id });
export const create = (input: ProjectInput) => call<Project>("projects_create", { input });
export const update = (id: number, input: ProjectInput) => call<Project>("projects_update", { id, input });
export const remove = (id: number) => call<void>("projects_delete", { id });
export const archive = (id: number) => call<void>("projects_archive", { id });
export const setPinned = (id: number, pinned: boolean) => call<void>("project_set_pinned", { id, pinned });
export const setSort = (id: number, sortOrder: number) => call<void>("project_set_sort", { id, sortOrder });
```
> Tauri конвертирует top-level аргументы camelCase→snake_case (`sortOrder`→`sort_order`), а `#[serde(rename_all="camelCase")]` на `ProjectInput` принимает `repoPath` и т.п.

- [ ] **Step 2: Коммит вместе с Task 5 (стор зависит от api).** Перейти к Task 5.

---

## Task 5: Фронт — стор проектов из бэкенда

**Files:**
- Modify: `src/lib/stores/projects.ts`

- [ ] **Step 1: Переписать стор**

Заменить ВЕСЬ `src/lib/stores/projects.ts` на:
```ts
import { writable } from "svelte/store";
import type { Project } from "../types";
import * as projectsApi from "../api/projects";

// Грузится из БД (см. loadProjects). Пусто до первой загрузки.
export const projects = writable<Project[]>([]);

// id выбранного проекта; null = дашборд.
export const activeProjectId = writable<number | null>(null);

// Признак, что первая загрузка завершена (для отрисовки empty-state, а не «пусто во время загрузки»).
export const projectsLoaded = writable(false);

// Перечитать список из БД.
export async function loadProjects(): Promise<void> {
  const list = await projectsApi.list();
  projects.set(list);
  projectsLoaded.set(true);
}
```

- [ ] **Step 2: Проверка типов отложена до Task 6–8.** Перейти к Task 6.

---

## Task 6: Фронт — Sidebar и Dashboard на живых данных

**Files:**
- Modify: `src/lib/components/Sidebar.svelte`, `src/lib/components/Dashboard.svelte`

> Поля изменились: `emoji`→`icon`, `desc`→`description`, нет `git` (срез git позже), `id` теперь число, `status` — enum (локализуем `statusLabel`). Добавляем empty-state. Открытие модалки создания — через UI-стор (Task 7 добавит `showNewProject`).

- [ ] **Step 1: UI-стор для модалки создания**

В `src/lib/stores/ui.ts` добавить (в конец файла):
```ts
// Открыта ли модалка создания проекта.
export const showNewProject = writable(false);
```
И убедиться, что вверху есть `import { writable } from "svelte/store";` (был для activeTab — уже есть).

- [ ] **Step 2: Переписать Sidebar.svelte**

Заменить ВЕСЬ `src/lib/components/Sidebar.svelte` на:
```svelte
<script lang="ts">
  import { projects, activeProjectId, projectsLoaded } from "$lib/stores/projects";
  import { showNewProject } from "$lib/stores/ui";
  import { theme, toggleTheme } from "$lib/stores/theme";
  import { USER } from "$lib/mock";
  import Icon from "./Icon.svelte";

  let query = $state("");

  // Архивные в сайдбаре не показываем.
  const visible = $derived($projects.filter((p) => p.status !== "archived"));
  const filtered = $derived(
    visible.filter(
      (p) =>
        !query ||
        p.name.toLowerCase().includes(query.toLowerCase()) ||
        p.tags.join(" ").toLowerCase().includes(query.toLowerCase()),
    ),
  );
  const pinned = $derived(filtered.filter((p) => p.pinned));
  const rest = $derived(filtered.filter((p) => !p.pinned));

  function select(id: number) {
    activeProjectId.set(id);
  }
</script>

<aside class="sidebar">
  <div class="sb-head">
    <span class="logo"><Icon name="layout-grid" class="" /></span>
    <span class="wordmark">Dev<span>Deck</span></span>
    <span class="ver">0.1</span>
  </div>

  <div class="sb-search" class:has-q={query}>
    <Icon name="search" class="ic" />
    <input placeholder="Поиск проектов…" bind:value={query} />
    <span class="kbd">Ctrl K</span>
  </div>

  <button class="sb-new" onclick={() => showNewProject.set(true)}>
    <Icon name="plus" class="ic ic-sm" /> Новый проект
  </button>

  <div class="sb-scroll">
    {#if pinned.length}
      <div class="sb-section"><span>Закреплённые</span><span class="count">{pinned.length}</span></div>
      {#each pinned as p (p.id)}
        <div class="proj" class:active={$activeProjectId === p.id}
             style="--p-color:{p.color ?? 'var(--accent)'}" role="button" tabindex="0"
             onclick={() => select(p.id)}>
          {#if p.icon}<span class="emoji">{p.icon}</span>{:else}<span class="dot"></span>{/if}
          <span class="nm">{p.name}</span>
        </div>
      {/each}
    {/if}

    <div class="sb-section"><span>Все проекты</span><span class="count">{rest.length}</span></div>
    {#each rest as p (p.id)}
      <div class="proj" class:active={$activeProjectId === p.id}
           style="--p-color:{p.color ?? 'var(--accent)'}" role="button" tabindex="0"
           onclick={() => select(p.id)}>
        {#if p.icon}<span class="emoji">{p.icon}</span>{:else}<span class="dot"></span>{/if}
        <span class="nm">{p.name}</span>
      </div>
    {/each}

    {#if $projectsLoaded && !visible.length}
      <div class="sb-empty">Пока нет проектов.<br />Создайте первый.</div>
    {:else if !filtered.length && query}
      <div class="sb-empty">Ничего не найдено</div>
    {/if}
  </div>

  <div class="sb-foot">
    <span class="avatar">{USER.initials}</span>
    <div class="who">{USER.name}<small>{USER.handle}</small></div>
    <button class="icon-btn" onclick={toggleTheme} title="Сменить тему">
      {#if $theme === "dark"}<Icon name="sun" class="ic" />{:else}<Icon name="moon" class="ic" />{/if}
    </button>
  </div>
</aside>
```

- [ ] **Step 3: Переписать Dashboard.svelte**

Заменить ВЕСЬ `src/lib/components/Dashboard.svelte` на:
```svelte
<script lang="ts">
  import { projects, activeProjectId, projectsLoaded } from "$lib/stores/projects";
  import { showNewProject } from "$lib/stores/ui";
  import { USER } from "$lib/mock";
  import { statusLabel } from "$lib/format";
  import Icon from "./Icon.svelte";

  const visible = $derived($projects.filter((p) => p.status !== "archived"));
  const pinned = $derived(visible.filter((p) => p.pinned));
  const cards = $derived(pinned.length ? pinned : visible); // если ничего не закреплено — показываем все
</script>

<div class="ws-inner dash">
  <div style="margin-bottom:22px">
    <div class="dash-hello">Привет, <span>{USER.name}</span></div>
    <div class="dash-sub">{visible.length} {visible.length === 1 ? "проект" : "проектов"}</div>
  </div>

  {#if $projectsLoaded && !visible.length}
    <div class="card" style="padding:40px;text-align:center;color:var(--muted)">
      <div style="font-size:15px;color:var(--text);font-weight:600;margin-bottom:6px">Здесь пока пусто</div>
      <div style="margin-bottom:16px">Создайте первый проект, чтобы начать.</div>
      <button class="btn-primary" onclick={() => showNewProject.set(true)}>
        <Icon name="plus" class="ic ic-sm" /> Новый проект
      </button>
    </div>
  {:else}
    <h3 class="section-title"><Icon name="star" class="ic-sm" /> {pinned.length ? "Закреплённые проекты" : "Проекты"}</h3>
    <div class="dash-cards">
      {#each cards as p (p.id)}
        <div class="card dcard" style="--p-color:{p.color ?? 'var(--accent)'}" role="button" tabindex="0"
             onclick={() => activeProjectId.set(p.id)}>
          <div class="top">
            <span class="be">{p.icon ?? "📁"}</span>
            <div style="min-width:0">
              <h3>{p.name}</h3>
              <div class="pmeta">{p.path ?? statusLabel(p.status)}</div>
            </div>
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>
```

- [ ] **Step 4: Проверка типов отложена до Task 8 (ProjectView ещё на старом типе).** Перейти к Task 7.

---

## Task 7: Фронт — модалка создания проекта

**Files:**
- Create: `src/lib/components/NewProjectModal.svelte`
- Modify: `src/routes/+page.svelte`

> Markup-референс: `_prototype/index.html` мастер «Новый проект» (`#modal-scrim` / `openNewProject` — эмодзи-пикер `NP_EMOJI`, цвет `NP_COLORS`, поля имя/путь/теги). Переносим в Svelte-компонент на рунах, wired to `api.create` + `loadProjects()`.

- [ ] **Step 1: Создать NewProjectModal.svelte**

Create `src/lib/components/NewProjectModal.svelte`:
```svelte
<script lang="ts">
  import { showNewProject } from "$lib/stores/ui";
  import { loadProjects, activeProjectId } from "$lib/stores/projects";
  import { pushToast } from "$lib/stores/toasts";
  import * as projectsApi from "$lib/api/projects";
  import Icon from "./Icon.svelte";

  const EMOJI = ["🚀", "🎨", "📊", "🤖", "🛒", "📱", "⚙️", "🧪", "🔌", "📦", "🌐", "🔥"];
  const COLORS = ["#7c7dff", "#c77dff", "#3fb863", "#e0a83a", "#f0616d", "#5b9cff", "#19c3c0", "#ff8b5b"];

  let name = $state("");
  let path = $state("");
  let tagsRaw = $state("");
  let emoji = $state(EMOJI[0]);
  let color = $state(COLORS[0]);
  let saving = $state(false);

  const canCreate = $derived(name.trim().length > 0 && !saving);

  function close() {
    showNewProject.set(false);
    name = ""; path = ""; tagsRaw = ""; emoji = EMOJI[0]; color = COLORS[0]; saving = false;
  }

  async function create() {
    if (!canCreate) return;
    saving = true;
    try {
      const tags = tagsRaw.split(",").map((t) => t.trim()).filter(Boolean).slice(0, 8);
      const p = await projectsApi.create({
        name: name.trim(),
        path: path.trim() || null,
        repoPath: path.trim() || null,
        icon: emoji,
        color,
        tags,
        status: "active",
      });
      await loadProjects();
      activeProjectId.set(p.id);
      pushToast("Проект создан", p.name, "ok");
      close();
    } catch {
      // тост об ошибке уже показан в api/client.ts
      saving = false;
    }
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape") close();
    if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) create();
  }
</script>

{#if $showNewProject}
  <div class="modal-scrim open" onmousedown={(e) => { if (e.currentTarget === e.target) close(); }}
       onkeydown={onKey} role="dialog" tabindex="-1" aria-label="Новый проект">
    <div class="modal">
      <div class="modal-head">
        <span class="mh-ico"><Icon name="folder-plus" class="ic" /></span>
        <span class="t">Новый проект</span>
        <button class="icon-btn x" onclick={close}><Icon name="x" class="ic" /></button>
      </div>
      <div class="modal-body">
        <div class="field">
          <label>Иконка</label>
          <div class="picker">
            {#each EMOJI as e}
              <button class="emoji-pick" class:sel={emoji === e} onclick={() => (emoji = e)}>{e}</button>
            {/each}
          </div>
        </div>
        <div class="field">
          <label for="np-name">Название</label>
          <input id="np-name" class="tin" bind:value={name} placeholder="Например, Aurora API" autocomplete="off" />
        </div>
        <div class="field">
          <label for="np-path">Путь к репозиторию</label>
          <input id="np-path" class="tin mono" bind:value={path} placeholder="~/dev/my-project" autocomplete="off" />
        </div>
        <div class="field">
          <label for="np-tags">Теги (через запятую)</label>
          <input id="np-tags" class="tin mono" bind:value={tagsRaw} placeholder="rust, postgres, docker" autocomplete="off" />
        </div>
        <div class="field">
          <label>Цвет проекта</label>
          <div class="picker">
            {#each COLORS as c}
              <button class="color-pick" class:sel={color === c} style="--c:{c};background:{c}" onclick={() => (color = c)} aria-label="цвет"></button>
            {/each}
          </div>
        </div>
      </div>
      <div class="modal-foot">
        <span class="spacer"></span>
        <button class="btn-ghost" onclick={close}>Отмена</button>
        <button class="btn-primary" disabled={!canCreate} onclick={create}>
          <Icon name="check" class="ic ic-sm" /> Создать проект
        </button>
      </div>
    </div>
  </div>
{/if}
```
> Классы `.modal-scrim`, `.modal`, `.field`, `.tin`, `.emoji-pick`, `.color-pick`, `.btn-primary` и т.д. уже есть в global.css.

- [ ] **Step 2: Смонтировать модалку и грузить проекты на старте**

Заменить ВЕСЬ `src/routes/+page.svelte` на:
```svelte
<script lang="ts">
  import "$lib/stores/theme"; // активирует подписку темы
  import Sidebar from "$lib/components/Sidebar.svelte";
  import Workspace from "$lib/components/Workspace.svelte";
  import Toasts from "$lib/components/Toasts.svelte";
  import NewProjectModal from "$lib/components/NewProjectModal.svelte";
  import { loadProjects } from "$lib/stores/projects";
  import { onMount } from "svelte";

  onMount(() => {
    loadProjects();
  });
</script>

<div id="app">
  <Sidebar />
  <Workspace />
</div>
<NewProjectModal />
<Toasts />
```

- [ ] **Step 3: Коммит откладываем до конца (после Task 8 проверим всё вместе).** Перейти к Task 8.

---

## Task 8: Фронт — ProjectView + вкладка «Настройки» (update/pin/archive/delete)

**Files:**
- Create: `src/lib/components/SettingsTab.svelte`, `src/lib/components/ConfirmDialog.svelte`
- Modify: `src/lib/components/ProjectView.svelte`

> Markup-референс настроек: `_prototype/index.html` `settingsHTML` (поля «Основное», статус-select, цвет/эмодзи-пикеры, danger zone). Переносим в Svelte на рунах, wired to `api.update`/`setPinned`/`archive`/`remove`.

- [ ] **Step 1: ConfirmDialog.svelte (подтверждение удаления)**

Create `src/lib/components/ConfirmDialog.svelte`:
```svelte
<script lang="ts">
  import Icon from "./Icon.svelte";

  let {
    open = false,
    title = "Подтвердите",
    message = "",
    confirmLabel = "Удалить",
    onConfirm,
    onCancel,
  }: {
    open?: boolean;
    title?: string;
    message?: string;
    confirmLabel?: string;
    onConfirm: () => void;
    onCancel: () => void;
  } = $props();
</script>

{#if open}
  <div class="modal-scrim open" role="dialog" aria-label={title}
       onmousedown={(e) => { if (e.currentTarget === e.target) onCancel(); }}
       onkeydown={(e) => { if (e.key === "Escape") onCancel(); }} tabindex="-1">
    <div class="modal" style="max-width:440px">
      <div class="modal-head">
        <span class="mh-ico" style="background:color-mix(in oklab,var(--danger) 14%,transparent);color:var(--danger)">
          <Icon name="triangle-alert" class="ic" />
        </span>
        <span class="t">{title}</span>
      </div>
      <div class="modal-body"><p class="desc">{message}</p></div>
      <div class="modal-foot">
        <span class="spacer"></span>
        <button class="btn-ghost" onclick={onCancel}>Отмена</button>
        <button class="btn-danger solid" onclick={onConfirm}>
          <Icon name="trash-2" class="ic-sm" /> {confirmLabel}
        </button>
      </div>
    </div>
  </div>
{/if}
```

- [ ] **Step 2: SettingsTab.svelte**

Create `src/lib/components/SettingsTab.svelte`:
```svelte
<script lang="ts">
  import type { Project } from "$lib/types";
  import { STATUS_OPTIONS } from "$lib/format";
  import { loadProjects, activeProjectId } from "$lib/stores/projects";
  import { pushToast } from "$lib/stores/toasts";
  import * as projectsApi from "$lib/api/projects";
  import Icon from "./Icon.svelte";
  import ConfirmDialog from "./ConfirmDialog.svelte";

  let { project }: { project: Project } = $props();

  const EMOJI = ["🚀", "🎨", "📊", "🤖", "🛒", "📱", "⚙️", "🧪", "🔌", "📦", "🌐", "🔥"];
  const COLORS = ["#7c7dff", "#c77dff", "#3fb863", "#e0a83a", "#f0616d", "#5b9cff", "#19c3c0", "#ff8b5b"];

  // Локальные редактируемые копии (инициализируются от проекта; $derived от project.id — пересоздать при смене проекта).
  let name = $state(project.name);
  let description = $state(project.description ?? "");
  let status = $state(project.status);
  let path = $state(project.path ?? "");
  let repoPath = $state(project.repoPath ?? "");
  let tagsRaw = $state(project.tags.join(", "));
  let icon = $state(project.icon ?? EMOJI[0]);
  let color = $state(project.color ?? COLORS[0]);
  let confirmDelete = $state(false);

  // Если переключили проект — перезаполнить форму.
  let lastId = $state(project.id);
  $effect(() => {
    if (project.id !== lastId) {
      lastId = project.id;
      name = project.name; description = project.description ?? ""; status = project.status;
      path = project.path ?? ""; repoPath = project.repoPath ?? "";
      tagsRaw = project.tags.join(", "); icon = project.icon ?? EMOJI[0]; color = project.color ?? COLORS[0];
    }
  });

  async function save() {
    if (!name.trim()) { pushToast("Нужно имя", "Название проекта не может быть пустым", "error"); return; }
    await projectsApi.update(project.id, {
      name: name.trim(),
      description: description.trim() || null,
      status,
      path: path.trim() || null,
      repoPath: repoPath.trim() || null,
      tags: tagsRaw.split(",").map((t) => t.trim()).filter(Boolean).slice(0, 8),
      icon,
      color,
    });
    await loadProjects();
    pushToast("Сохранено", name.trim(), "ok");
  }

  async function togglePin() {
    await projectsApi.setPinned(project.id, !project.pinned);
    await loadProjects();
  }

  async function doArchive() {
    await projectsApi.archive(project.id);
    await loadProjects();
    activeProjectId.set(null);
    pushToast("Проект в архиве", project.name, "info");
  }

  async function doDelete() {
    confirmDelete = false;
    await projectsApi.remove(project.id);
    await loadProjects();
    activeProjectId.set(null);
    pushToast("Проект удалён", project.name, "ok");
  }
</script>

<div class="settings">
  <div class="card set-card">
    <h3 class="section-title">Основное</h3>
    <div class="set-grid">
      <div class="field">
        <label for="st-name">Название</label>
        <input id="st-name" class="tin" bind:value={name} />
      </div>
      <div class="field">
        <label for="st-status">Статус</label>
        <select id="st-status" class="tin" bind:value={status}>
          {#each STATUS_OPTIONS as o}<option value={o.value}>{o.label}</option>{/each}
        </select>
      </div>
      <div class="field span-2">
        <label for="st-desc">Описание</label>
        <textarea id="st-desc" class="tin" bind:value={description}></textarea>
      </div>
      <div class="field">
        <label for="st-path">Папка проекта</label>
        <input id="st-path" class="tin mono" bind:value={path} placeholder="~/dev/project" />
      </div>
      <div class="field">
        <label for="st-repo">Git-репозиторий</label>
        <input id="st-repo" class="tin mono" bind:value={repoPath} placeholder="~/dev/project" />
      </div>
      <div class="field span-2">
        <label for="st-tags">Теги (через запятую)</label>
        <input id="st-tags" class="tin mono" bind:value={tagsRaw} placeholder="rust, docker" />
      </div>
      <div class="field">
        <label>Иконка</label>
        <div class="set-pick-emoji">
          {#each EMOJI as e}<button class="emoji-pick" class:sel={icon === e} onclick={() => (icon = e)}>{e}</button>{/each}
        </div>
      </div>
      <div class="field">
        <label>Цвет</label>
        <div class="set-pick-color">
          {#each COLORS as c}<button class="color-pick" class:sel={color === c} style="--c:{c};background:{c}" onclick={() => (color = c)} aria-label="цвет"></button>{/each}
        </div>
      </div>
    </div>
    <div style="display:flex;gap:10px;margin-top:16px;align-items:center">
      <button class="btn-primary" onclick={save}><Icon name="check" class="ic ic-sm" /> Сохранить</button>
      <button class="btn-ghost" onclick={togglePin}>
        <Icon name={project.pinned ? "pin-off" : "pin"} class="ic-sm" /> {project.pinned ? "Открепить" : "Закрепить"}
      </button>
    </div>
  </div>

  <div class="card set-card danger-zone" style="margin-top:22px">
    <h3 class="section-title">Опасная зона</h3>
    <div class="dz-row">
      <div class="dz-txt">Архивировать проект<small>Скроет из списка, данные сохранятся.</small></div>
      <span class="spacer"></span>
      <button class="btn-ghost" onclick={doArchive}><Icon name="archive" class="ic-sm" /> В архив</button>
    </div>
    <div class="dz-row" style="margin-top:12px">
      <div class="dz-txt">Удалить проект<small>Безвозвратно удалит проект и все его данные.</small></div>
      <span class="spacer"></span>
      <button class="btn-danger" onclick={() => (confirmDelete = true)}><Icon name="trash-2" class="ic-sm" /> Удалить</button>
    </div>
  </div>
</div>

<ConfirmDialog
  open={confirmDelete}
  title="Удалить проект?"
  message={`«${project.name}» и все связанные данные будут удалены безвозвратно.`}
  confirmLabel="Удалить навсегда"
  onConfirm={doDelete}
  onCancel={() => (confirmDelete = false)} />
```

- [ ] **Step 3: ProjectView.svelte — статус-локализация + рендер SettingsTab**

Заменить ВЕСЬ `src/lib/components/ProjectView.svelte` на:
```svelte
<script lang="ts">
  import type { Project } from "$lib/types";
  import { activeTab, type Tab } from "$lib/stores/ui";
  import { pushToast } from "$lib/stores/toasts";
  import { statusLabel } from "$lib/format";
  import Icon from "./Icon.svelte";
  import SettingsTab from "./SettingsTab.svelte";

  let { project }: { project: Project } = $props();

  const tabs: { key: Tab; label: string }[] = [
    { key: "overview", label: "Обзор" },
    { key: "tasks", label: "Задачи" },
    { key: "checklists", label: "Чеклисты" },
    { key: "creds", label: "Креды" },
    { key: "notes", label: "Заметки" },
    { key: "settings", label: "Настройки" },
  ];

  const activeLabel = $derived(tabs.find((x) => x.key === $activeTab)?.label ?? "");
</script>

<div class="ws-inner" style="--p-color:{project.color ?? 'var(--accent)'}">
  <div class="proj-head">
    <span class="big-emoji">{project.icon ?? "📁"}</span>
    <div>
      <h1>{project.name}</h1>
      <div class="sub">
        <span class="pill">{statusLabel(project.status)}</span>
        {#if project.path}<span class="path mono">{project.path}</span>{/if}
        {#if project.tags.length}
          <span class="chips">{#each project.tags as t}<span class="chip">{t}</span>{/each}</span>
        {/if}
      </div>
    </div>
    <div class="head-actions">
      <button class="act" onclick={() => pushToast("Открываю папку", project.path ?? "путь не задан", "info")}>
        <Icon name="folder-open" class="ic-sm" /> Папка
      </button>
    </div>
  </div>

  <div class="tabs">
    {#each tabs as t}
      <button class="tab" class:active={$activeTab === t.key} onclick={() => activeTab.set(t.key)}>{t.label}</button>
    {/each}
  </div>

  <div class="tab-body">
    {#if $activeTab === "settings"}
      <SettingsTab {project} />
    {:else}
      {#if project.description}<p class="desc">{project.description}</p>{/if}
      <p class="desc" style="color:var(--muted);margin-top:14px">
        Вкладка «{activeLabel}» — содержимое появится в следующих срезах Phase 1.
      </p>
    {/if}
  </div>
</div>
```

> Workspace.svelte уже находит активный проект по `$activeProjectId` — менять не нужно (сравнение `p.id === $activeProjectId` теперь число-число).

- [ ] **Step 4: Полная проверка (NON-blocking; не запускать `npm run tauri dev`)**

Run:
```powershell
npm run check
npm test
npm run build
```
Expected: `npm run check` 0 ОШИБОК (a11y-warnings на кликабельных div и `node`-type warning — приемлемы; на `Icon.svelte` возможен `state_referenced_locally` — приемлем). Vitest 3 passed. build успешен.

- [ ] **Step 5: Commit (фронт целиком)**

```powershell
git add src/lib src/routes/+page.svelte
git commit -m @'
feat(frontend): live projects CRUD — sidebar/dashboard, new-project modal, settings tab

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

## Task 9: Проверка среза (GUI — шаг пользователя)

**Files:** —

- [ ] **Step 1: Автопроверка (контроллер)**
```powershell
npm run check ; npm test ; cargo test --manifest-path src-tauri/Cargo.toml --lib
```
Expected: типы 0 ошибок; Vitest 3; Rust-тесты (миграции + 3 projects) — все ok.

- [ ] **Step 2: Ручной GUI-смоук (пользователь; `npm run tauri dev`)**
- [ ] Пустая БД → сайдбар и дашборд показывают empty-state с кнопкой «Новый проект».
- [ ] Создание: кнопка «+ Новый проект» / кнопка в дашборде открывает модалку; выбор эмодзи/цвета, ввод имени/пути/тегов; «Создать» → проект появляется в сайдбаре и открывается; тост «Проект создан».
- [ ] Перезапуск приложения → проект на месте (данные в `%APPDATA%\com.devdeck.app\devdeck.db`).
- [ ] Открыть проект → вкладка «Настройки»: правка имени/статуса/описания/пути/тегов/иконки/цвета → «Сохранить» → шапка и сайдбар обновились; тост «Сохранено».
- [ ] «Закрепить» → проект переезжает в раздел «Закреплённые».
- [ ] «В архив» → проект исчезает из сайдбара, возврат на дашборд.
- [ ] «Удалить» → подтверждение → проект удалён.
- [ ] Поиск в сайдбаре фильтрует по имени и тегам.

---

## Итог среза

Проекты живут в SQLite: создание (модалка) → список (сайдбар/дашборд) → редактирование/закрепление/архив/удаление (вкладка «Настройки»), с тегами и локализованным статусом, пустым состоянием и подтверждением удаления. Команды `projects_*` покрыты Rust-тестами. Готовая основа для следующих срезов Phase 1 (быстрые действия, git, креды, задачи), которые наполнят остальные вкладки и git-сводку.
```
