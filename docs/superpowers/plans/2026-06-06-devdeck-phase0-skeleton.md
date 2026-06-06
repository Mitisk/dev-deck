# DevDeck Phase 0 (Скелет) — план реализации

> **Для исполнителя:** ОБЯЗАТЕЛЬНЫЙ ПОД-НАВЫК: используйте superpowers:subagent-driven-development (рекомендуется) или superpowers:executing-plans для пошагового выполнения. Шаги отмечены чекбоксами (`- [ ]`).

**Цель:** Поднять Tauri 2 + Svelte 5 + TypeScript приложение DevDeck, которое открывается окном с перенесённой из прототипа дизайн-системой, рендерит оболочку (сайдбар + дашборд + карточку проекта) на моковых данных через Svelte-сторы, и имеет работающую инфраструктуру бэкенда (SQLite, раннер миграций, AppState, обработка ошибок) с одной проверочной командой.

**Архитектура:** Фронт (`src/`) общается с Rust-ядром (`src-tauri/`) только через слой `src/lib/api/`, оборачивающий `invoke()`. Состояние фронта — Svelte-сторы; состояние Rust — `tauri::State<AppState>` с `Mutex<Connection>`. Команды возвращают `Result<T, AppError>`; ошибки сериализуются и показываются тостом.

**Стек:** Tauri 2 · Rust (rusqlite 0.32 «bundled») · **SvelteKit SPA** (Svelte 5 + TypeScript, `adapter-static`, `ssr = false`, `prerender = true`) · Vite 6 · Vitest.

> **Svelte 5 runes (важно):** компоненты пишем на рунах — `let { x } = $props()`,
> `let v = $state(...)`, `const d = $derived(...)`, `$effect(() => {...})`, обработчики
> `onclick={...}` (НЕ legacy `export let` / `$:` / `on:click` / `afterUpdate`). Перерисовку
> lucide-иконок после изменения DOM делаем через `$effect`, читая внутри реактивную
> зависимость (список/состояние), чтобы эффект перезапускался.
>
> **SvelteKit-конвенции (важно для всех задач):** UI живёт в `src/routes/` (главный экран — `src/routes/+page.svelte`, общий layout — `src/routes/+layout.svelte`, конфиг рендеринга — `src/routes/+layout.ts`). HTML-оболочка — `src/app.html` (НЕ корневой `index.html`). Общие модули — в `src/lib/`, импортируются через алиас `$lib/...`. Сборка фронта идёт в `build/` (не `dist/`). Браузерные API (`document`, `localStorage`) в модулях гардить через `import { browser } from "$app/environment"`. Файла `src/main.ts`/`src/App.svelte` НЕТ — их роль играют `+layout.svelte`/`+page.svelte`.

**Источники:** `TZ_DevDeck.md` (раздел 4 — схема БД, раздел 10 — команды), `index.html` (прототип: CSS в `<style>` строки 14–737, мок-данные `PROJECTS` со строки 839, render-функции).

---

## Структура файлов (что создаём в Phase 0)

```
devdeck/
├─ CLAUDE.md                              # карта проекта, команды, конвенции
├─ package.json, vite.config.ts, tsconfig.json, svelte.config.js
├─ index.html                            # точка входа Vite (НЕ прототип)
├─ _prototype/index.html                 # сохранённый прототип-референс
├─ src/
│  ├─ app.html                           # HTML-оболочка: тема-класс, шрифты, lucide
│  ├─ routes/
│  │  ├─ +layout.ts                      # ssr=false, prerender=true (от скаффолда)
│  │  ├─ +layout.svelte                  # импорт global.css, обёртка
│  │  └─ +page.svelte                    # #app: Sidebar + Workspace + Toasts
│  ├─ lib/
│  │  ├─ styles/global.css               # дизайн-система из прототипа
│  │  ├─ icons.ts                        # обёртка инициализации lucide
│  │  ├─ types.ts                        # доменные TS-типы
│  │  ├─ mock.ts                         # мок-данные (портик из прототипа)
│  │  ├─ stores/
│  │  │  ├─ projects.ts                  # список проектов + активный
│  │  │  ├─ theme.ts                     # dark/light + persist (гард browser)
│  │  │  ├─ ui.ts                        # активная вкладка и т.п.
│  │  │  └─ toasts.ts                    # очередь тостов
│  │  ├─ api/
│  │  │  ├─ client.ts                    # invoke-обёртка + перехват ошибок
│  │  │  └─ health.ts                    # db_health()
│  │  └─ components/
│  │     ├─ Sidebar.svelte
│  │     ├─ Workspace.svelte             # роутер Dashboard ↔ Project
│  │     ├─ Dashboard.svelte
│  │     ├─ ProjectView.svelte           # шапка + табы (статично)
│  │     └─ Toasts.svelte
│  └─ tests/
│     └─ toasts.test.ts                  # Vitest на стор тостов
└─ src-tauri/
   ├─ Cargo.toml
   ├─ tauri.conf.json
   ├─ build.rs
   ├─ migrations/
   │  └─ 0001_init.sql                   # схема из раздела 4 ТЗ
   └─ src/
      ├─ main.rs
      ├─ lib.rs                          # сборка Tauri + регистрация команд
      ├─ error.rs                        # AppError
      ├─ state.rs                        # AppState
      ├─ db/
      │  ├─ mod.rs                       # открытие соединения
      │  └─ migrations.rs                # раннер + тест
      └─ commands/
         ├─ mod.rs
         └─ health.rs                    # db_health
```

Каждый файл — одна ответственность; компоненты дробим по экранам, бэкенд по доменам.

---

## Task 1: Скаффолдинг Tauri + Svelte + TS

**Files:**
- Create: весь каркас через `create-tauri-app`
- Modify: переименование существующего `index.html` (прототип)

- [ ] **Step 1: Сохранить прототип, чтобы он не конфликтовал со сборкой Vite**

Прототип `index.html` лежит в корне и будет затёрт скаффолдером. Переносим его в `_prototype/`.

Run (PowerShell, из `d:\DevDeck`):
```powershell
New-Item -ItemType Directory -Force _prototype
Move-Item -Force index.html _prototype\index.html
```
Expected: `_prototype/index.html` существует, корневой `index.html` исчез.

- [ ] **Step 2: Сгенерировать Tauri+Svelte+TS проект во временную папку**

Скаффолдер требует пустую целевую папку, поэтому генерируем рядом и потом сливаем.

Run:
```powershell
npm create tauri-app@latest devdeck-scaffold -- --template svelte-ts --manager npm --yes
```
Expected: создалась папка `devdeck-scaffold/` с `src/`, `src-tauri/`, `package.json`, `vite.config.ts`.

> Если флаги интерактивного промпта изменятся и команда переспросит — выбрать: TypeScript, шаблон **Svelte**, менеджер **npm**.

- [ ] **Step 3: Перенести содержимое скаффолда в корень проекта**

Run:
```powershell
Get-ChildItem -Force devdeck-scaffold | Where-Object { $_.Name -ne '.git' } | Move-Item -Destination . -Force
Remove-Item -Recurse -Force devdeck-scaffold
```
Expected: в `d:\DevDeck` появились `src/`, `src-tauri/`, `package.json`, `vite.config.ts`, новый `index.html` (от Vite, не прототип). `TZ_DevDeck.md`, `docs/`, `_prototype/` на месте.

- [ ] **Step 4: Установить зависимости**

Run:
```powershell
npm install
```
Expected: создаётся `node_modules/`, без ошибок.

- [ ] **Step 5: Проверить, что окно запускается**

Run:
```powershell
npm run tauri dev
```
Expected: компилируется Rust, открывается окно с дефолтным «Welcome to Tauri + Svelte». Закрыть окно (Ctrl+C в терминале).

- [ ] **Step 6: Коммит пока нельзя — git ещё не инициализирован**

Переходим к Task 2 (git init + первый коммит будет там).

---

## Task 2: Инициализация git, CLAUDE.md, первый коммит

**Files:**
- Create: `.gitignore` (если скаффолд не создал), `CLAUDE.md`

- [ ] **Step 1: Инициализировать репозиторий и завести ветку**

Run:
```powershell
git init
git checkout -b phase0-skeleton
```
Expected: `Initialized empty Git repository`, переключение на ветку `phase0-skeleton`.

- [ ] **Step 2: Убедиться, что `.gitignore` игнорирует мусор**

Прочитать существующий `.gitignore` (скаффолд обычно создаёт). Если в нём нет строк ниже — дописать их.

Содержимое, которое должно присутствовать:
```
node_modules
/dist
/src-tauri/target
```

- [ ] **Step 3: Создать CLAUDE.md**

Create `CLAUDE.md`:
```markdown
# DevDeck

Локальный «командный центр» разработчика под Windows. Tauri 2 (Rust) + Svelte 5 +
TypeScript + SQLite. Один пользователь, local-first, без облака и телеметрии.

ТЗ: `TZ_DevDeck.md`. Дизайн реализации: `docs/superpowers/specs/`. Планы:
`docs/superpowers/plans/`. Прототип-референс (вёрстка/мок-данные): `_prototype/index.html`.

## Команды

- `npm run tauri dev` — запуск приложения в дев-режиме.
- `npm run tauri build` — сборка `.msi`/`.exe`.
- `npm run dev` — только фронт (Vite) без Tauri-окна.
- `npm test` — Vitest (фронт).
- `cargo test --manifest-path src-tauri/Cargo.toml` — тесты Rust.

## Структура

- `src/` — фронтенд (Svelte). Компоненты — `src/lib/components/`, сторы —
  `src/lib/stores/`, типизированные вызовы бэкенда — `src/lib/api/`,
  дизайн-система — `src/lib/styles/global.css`.
- `src-tauri/src/` — Rust-ядро. Команды по доменам в `commands/`, доступ к БД в
  `db/`, миграции SQL в `src-tauri/migrations/`.

## Конвенции

- Компоненты НЕ вызывают `invoke()` напрямую — только через слой `src/lib/api/`.
- Rust-команды возвращают `Result<T, AppError>`; никаких `unwrap()`/`expect()`
  в командах. `AppError` сериализуется и показывается тостом на фронте.
- Состояние фронта — Svelte-сторы; состояние Rust — `tauri::State<AppState>`
  с `Mutex<Connection>` (rusqlite синхронный, пользователь один).
- Миграции БД — нумерованные `.sql` в `src-tauri/migrations/`, применяются
  раннером через `PRAGMA user_version`.
```

- [ ] **Step 4: Первый коммит**

Run:
```powershell
git add -A
git commit -m @'
chore: bootstrap Tauri 2 + Svelte 5 + TS skeleton

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```
Expected: создан коммит со всем каркасом, CLAUDE.md, спеком, планом и прототипом.

---

## Task 3: Перенос дизайн-системы в global.css

**Files:**
- Create: `src/lib/styles/global.css`
- Modify: `src/main.ts` (импорт стиля), удалить дефолтные стили скаффолда

- [ ] **Step 1: Скопировать CSS из прототипа**

Открыть `_prototype/index.html`, скопировать всё содержимое между `<style>` (строка 15) и `</style>` (строка 737) — это полная дизайн-система (токены, темы `.dark`/`.light`, все классы компонентов). Вставить в новый файл `src/lib/styles/global.css` **без** тегов `<style>`.

> Это дословный перенос существующего CSS, поэтому он не дублируется в плане — источник в репозитории (`_prototype/index.html:15-737`).

- [ ] **Step 2: Подключить шрифты и lucide в src/app.html (SvelteKit-оболочка)**

В `src/app.html` внутри `<head>` (перед `%sveltekit.head%`) добавить (взять строки 10–12 и 821 из `_prototype/index.html`):
```html
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&family=JetBrains+Mono:wght@400;500;600&display=swap" rel="stylesheet">
<script src="https://unpkg.com/lucide@0.460.0/dist/umd/lucide.min.js"></script>
```
И поставить классу `<html>` тему по умолчанию: `<html lang="ru" class="dark">`.

- [ ] **Step 3: Импортировать global.css в +layout.svelte**

SvelteKit-проект импортирует глобальный CSS в общем layout. Создать (или заменить, если скаффолд создал) `src/routes/+layout.svelte`:
```svelte
<script lang="ts">
  import "$lib/styles/global.css";
  let { children } = $props();
</script>

{@render children()}
```
Удалить дефолтные демо-стили скаффолда из `src/routes/+page.svelte` (`<style>...</style>`) — они затрутся в Task 11.

- [ ] **Step 4: Проверить, что стили грузятся**

Run:
```powershell
npm run tauri dev
```
Expected: окно открывается с тёмным фоном `#0e0f12` (фон из темы), шрифт Inter. Контент пока дефолтный — это нормально. Закрыть окно.

- [ ] **Step 5: Коммит**

Run:
```powershell
git add -A
git commit -m @'
feat: port design system to global.css

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

## Task 4: Доменные типы и мок-данные

**Files:**
- Create: `src/lib/types.ts`, `src/lib/mock.ts`, `src/lib/icons.ts`

- [ ] **Step 1: Описать доменные типы**

Create `src/lib/types.ts`:
```ts
export type GitInfo = {
  branch: string;
  ahead: number;
  behind: number;
  dirty: number;
  lastHash: string;
  lastMsg: string;
  staged: number;
  untracked: number;
};

export type Command = { label: string; run: string; icon: string; tint: string };
export type Link = { title: string; url: string; icon: string };
export type Task = { t: string; pri: "high" | "med" | "low"; due: string };
export type Tasks = { todo: Task[]; doing: Task[]; done: Task[] };
export type ChecklistItem = { t: string; done: boolean };
export type Checklist = { title: string; items: ChecklistItem[] };
export type CredField = { k: string; v: string; secret: boolean };
export type Cred = { title: string; type: string; fields: CredField[] };

export type Project = {
  id: string;
  name: string;
  emoji: string;
  color: string;
  pinned: boolean;
  status: string;
  tags: string[];
  path: string;
  desc: string;
  git: GitInfo;
  commands: Command[];
  links: Link[];
  tasks: Tasks;
  checklists: Checklist[];
  creds: Cred[];
  note: string;
};

export type User = { name: string; handle: string; initials: string };
```

- [ ] **Step 2: Перенести мок-данные**

Create `src/lib/mock.ts`. Скопировать из `_prototype/index.html` значения `USER` (строка 831) и массив `PROJECTS` (со строки 839 до закрывающей `];`), обернув в типизированные экспорты:
```ts
import type { Project, User } from "./types";

export const USER: User = { name: "Артём", handle: "@artyom", initials: "АК" };

export const MOCK_PROJECTS: Project[] = [
  // ← вставить сюда содержимое массива PROJECTS из _prototype/index.html:839+
];
```

> Содержимое `PROJECTS` — дословный перенос из прототипа; не дублируется здесь.

- [ ] **Step 3: Обёртка для lucide-иконок**

Create `src/lib/icons.ts`:
```ts
// lucide подключён глобально через <script> в index.html (window.lucide).
declare global {
  interface Window {
    lucide?: { createIcons: (opts?: { nameAttr?: string }) => void };
  }
}

// Перерисовать <svg data-lucide="name"> в реальные иконки внутри узла.
export function paintIcons(): void {
  window.lucide?.createIcons();
}

export function ico(name: string, cls = "ic"): string {
  return `<svg class="${cls}" data-lucide="${name}"></svg>`;
}
```

- [ ] **Step 4: Проверка типов**

Run:
```powershell
npm run check
```
> Скаффолд Svelte-TS добавляет скрипт `check` (svelte-check). Если его нет — `npx svelte-check`.
Expected: 0 ошибок типов в `types.ts`/`mock.ts`.

- [ ] **Step 5: Коммит**

Run:
```powershell
git add -A
git commit -m @'
feat: add domain types and mock data ported from prototype

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

## Task 5: Svelte-сторы

**Files:**
- Create: `src/lib/stores/projects.ts`, `theme.ts`, `ui.ts`, `toasts.ts`

- [ ] **Step 1: Стор проектов**

Create `src/lib/stores/projects.ts`:
```ts
import { writable } from "svelte/store";
import type { Project } from "../types";
import { MOCK_PROJECTS } from "../mock";

// На Phase 0 источник — моки. В срезе «Проекты CRUD» заменится загрузкой из БД.
export const projects = writable<Project[]>(MOCK_PROJECTS);

// id выбранного проекта; null = дашборд.
export const activeProjectId = writable<string | null>(null);
```

- [ ] **Step 2: Стор темы с persist**

Create `src/lib/stores/theme.ts` (гардим браузерные API через SvelteKit `browser`,
чтобы модуль не падал при prerender):
```ts
import { writable } from "svelte/store";
import { browser } from "$app/environment";

type Theme = "dark" | "light";
const KEY = "devdeck-theme";

function initial(): Theme {
  if (!browser) return "dark";
  return localStorage.getItem(KEY) === "light" ? "light" : "dark";
}

export const theme = writable<Theme>(initial());

// Применяем класс к <html> и сохраняем при каждом изменении (только в браузере).
theme.subscribe((t) => {
  if (!browser) return;
  document.documentElement.classList.toggle("dark", t === "dark");
  document.documentElement.classList.toggle("light", t === "light");
  localStorage.setItem(KEY, t);
});

export function toggleTheme(): void {
  theme.update((t) => (t === "dark" ? "light" : "dark"));
}
```

- [ ] **Step 3: Стор UI-состояния**

Create `src/lib/stores/ui.ts`:
```ts
import { writable } from "svelte/store";

export type Tab = "overview" | "tasks" | "checklists" | "creds" | "notes" | "settings";

// Активная вкладка карточки проекта.
export const activeTab = writable<Tab>("overview");
```

- [ ] **Step 4: Стор тостов**

Create `src/lib/stores/toasts.ts`:
```ts
import { writable } from "svelte/store";

export type ToastKind = "ok" | "info" | "error";
export type Toast = { id: number; text: string; sub: string; kind: ToastKind };

let seq = 1;
export const toasts = writable<Toast[]>([]);

export function pushToast(text: string, sub = "", kind: ToastKind = "ok"): number {
  const id = seq++;
  toasts.update((list) => [...list, { id, text, sub, kind }]);
  setTimeout(() => dismissToast(id), 3200);
  return id;
}

export function dismissToast(id: number): void {
  toasts.update((list) => list.filter((t) => t.id !== id));
}
```

- [ ] **Step 5: Проверка типов и коммит**

Run:
```powershell
npm run check
```
Expected: 0 ошибок.

Run:
```powershell
git add -A
git commit -m @'
feat: add svelte stores (projects, theme, ui, toasts)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

## Task 6: Слой api (invoke-обёртка)

**Files:**
- Create: `src/lib/api/client.ts`, `src/lib/api/health.ts`

- [ ] **Step 1: Обёртка invoke с перехватом ошибок**

Create `src/lib/api/client.ts`:
```ts
import { invoke } from "@tauri-apps/api/core";
import { pushToast } from "../stores/toasts";

// Все вызовы бэкенда идут через эту обёртку.
// Бэкенд возвращает Result<T, AppError>; при ошибке Tauri отклоняет промис
// объектом AppError ({ kind, message }) — ловим и показываем тостом.
export async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, args);
  } catch (err) {
    const e = err as { kind?: string; message?: string };
    pushToast("Ошибка", e?.message ?? String(err), "error");
    throw err;
  }
}
```

- [ ] **Step 2: Команда проверки БД**

Create `src/lib/api/health.ts`:
```ts
import { call } from "./client";

// Возвращает версию схемы (PRAGMA user_version). Проверяет, что БД открыта
// и миграции применены.
export function dbHealth(): Promise<number> {
  return call<number>("db_health");
}
```

- [ ] **Step 3: Коммит**

Run:
```powershell
git add -A
git commit -m @'
feat: add typed api layer over invoke()

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

## Task 7: Rust — AppError

**Files:**
- Create: `src-tauri/src/error.rs`

- [ ] **Step 1: Описать тип ошибки**

Create `src-tauri/src/error.rs`:
```rust
use serde::Serialize;

/// Единый тип ошибки команд. Сериализуется во фронт как { kind, message }.
#[derive(Debug, Serialize)]
pub struct AppError {
    pub kind: ErrorKind,
    pub message: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    Db,
    Io,
    NotFound,
    Validation,
    Internal,
}

impl AppError {
    pub fn db(msg: impl Into<String>) -> Self {
        Self { kind: ErrorKind::Db, message: msg.into() }
    }
    pub fn internal(msg: impl Into<String>) -> Self {
        Self { kind: ErrorKind::Internal, message: msg.into() }
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(e: rusqlite::Error) -> Self {
        AppError::db(e.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;
```

- [ ] **Step 2: Сборка ещё не пройдёт (модуль не подключён) — это нормально, подключим в Task 9.**

---

## Task 8: Rust — БД и раннер миграций (с тестом)

**Files:**
- Create: `src-tauri/migrations/0001_init.sql`, `src-tauri/src/db/mod.rs`, `src-tauri/src/db/migrations.rs`
- Modify: `src-tauri/Cargo.toml` (зависимость rusqlite)

- [ ] **Step 1: Добавить rusqlite в зависимости**

В `src-tauri/Cargo.toml` в секцию `[dependencies]` добавить:
```toml
rusqlite = { version = "0.32", features = ["bundled"] }
```
> `bundled` собирает SQLite из исходников — не нужна системная библиотека на Windows.

- [ ] **Step 2: SQL-схема (раздел 4 ТЗ)**

Create `src-tauri/migrations/0001_init.sql`. Перенести дословно весь SQL из `TZ_DevDeck.md` раздел 4 (строки 76–191): таблицы `projects`, `tags`, `project_tags`, `credentials`, `tasks`, `checklists`, `checklist_items`, `checklist_templates`, `notes`, `links`, `commands`, `settings`. В конец файла добавить:
```sql
PRAGMA user_version = 1;
```

- [ ] **Step 3: Раннер миграций + тест**

Create `src-tauri/src/db/migrations.rs`:
```rust
use crate::error::AppResult;
use rusqlite::Connection;

/// Встроенные миграции: (версия, SQL). Применяются по возрастанию, если
/// текущий user_version меньше.
const MIGRATIONS: &[(i64, &str)] = &[
    (1, include_str!("../../migrations/0001_init.sql")),
];

/// Применяет все миграции с номером выше текущего user_version.
pub fn run(conn: &Connection) -> AppResult<()> {
    let current: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    for (version, sql) in MIGRATIONS {
        if *version > current {
            conn.execute_batch(sql)?;
        }
    }
    Ok(())
}

/// Текущая версия схемы.
pub fn schema_version(conn: &Connection) -> AppResult<i64> {
    Ok(conn.query_row("PRAGMA user_version", [], |r| r.get(0))?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applies_migrations_on_empty_db() {
        let conn = Connection::open_in_memory().unwrap();
        assert_eq!(schema_version(&conn).unwrap(), 0);
        run(&conn).unwrap();
        assert_eq!(schema_version(&conn).unwrap(), 1);
        // Таблица projects должна существовать.
        let count: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='projects'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn run_is_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        run(&conn).unwrap();
        run(&conn).unwrap(); // второй прогон не должен падать
        assert_eq!(schema_version(&conn).unwrap(), 1);
    }
}
```

- [ ] **Step 4: Открытие соединения**

Create `src-tauri/src/db/mod.rs`:
```rust
pub mod migrations;

use crate::error::AppResult;
use rusqlite::Connection;
use std::path::Path;

/// Открывает (создавая при необходимости) БД по пути и накатывает миграции.
pub fn open(path: &Path) -> AppResult<Connection> {
    let conn = Connection::open(path)?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    migrations::run(&conn)?;
    Ok(conn)
}
```

- [ ] **Step 5: Прогнать тест раннера (изолированно от Tauri)**

Поскольку модули ещё не подключены в `lib.rs`, тест пока не соберётся отдельно. Подключение и запуск тестов — в конце Task 9, Step 5. Перейти к Task 9.

---

## Task 9: Rust — AppState, команда health, сборка

**Files:**
- Create: `src-tauri/src/state.rs`, `src-tauri/src/commands/mod.rs`, `src-tauri/src/commands/health.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: AppState**

Create `src-tauri/src/state.rs`:
```rust
use rusqlite::Connection;
use std::sync::Mutex;

/// Глобальное состояние приложения. rusqlite синхронный, пользователь один —
/// Mutex<Connection> достаточно, пул не нужен.
pub struct AppState {
    pub db: Mutex<Connection>,
}
```

- [ ] **Step 2: Команда db_health**

Create `src-tauri/src/commands/health.rs`:
```rust
use crate::db::migrations::schema_version;
use crate::error::AppResult;
use crate::state::AppState;
use tauri::State;

/// Возвращает версию схемы БД — проверка, что соединение живо и миграции прошли.
#[tauri::command]
pub fn db_health(state: State<AppState>) -> AppResult<i64> {
    let conn = state
        .db
        .lock()
        .map_err(|_| crate::error::AppError::internal("db mutex poisoned"))?;
    schema_version(&conn)
}
```

Create `src-tauri/src/commands/mod.rs`:
```rust
pub mod health;
```

- [ ] **Step 3: Собрать Tauri-приложение в lib.rs**

Открыть `src-tauri/src/lib.rs` (скаффолд создаёт его с функцией `run`). Заменить содержимое на:
```rust
mod commands;
mod db;
mod error;
mod state;

use state::AppState;
use std::sync::Mutex;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // Каталог данных: %APPDATA%\DevDeck\
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            std::fs::create_dir_all(dir.join("backups"))?;
            let conn = db::open(&dir.join("devdeck.db"))
                .map_err(|e| format!("db init failed: {}", e.message))?;
            app.manage(AppState { db: Mutex::new(conn) });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![commands::health::db_health])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```
> `tauri_plugin_opener` — плагин, который скаффолд Tauri 2 добавляет по умолчанию; строку `.plugin(...)` оставить как в исходном `lib.rs`. Если её там не было — убрать.

- [ ] **Step 4: Прогнать тесты Rust**

Run:
```powershell
cargo test --manifest-path src-tauri/Cargo.toml
```
Expected: `test db::migrations::tests::applies_migrations_on_empty_db ... ok` и `run_is_idempotent ... ok`, сборка без ошибок.

- [ ] **Step 5: Запустить приложение и проверить health из консоли**

Run:
```powershell
npm run tauri dev
```
В окне открыть DevTools (правый клик → Inspect, или F12) и в консоли выполнить:
```js
await window.__TAURI__.core.invoke("db_health")
```
Expected: возвращается `1`. Файл `%APPDATA%\DevDeck\devdeck.db` создан. Закрыть окно.

- [ ] **Step 6: Коммит**

Run:
```powershell
git add -A
git commit -m @'
feat: sqlite backend with migrations, AppState, db_health command

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

## Task 10: Компонент Toasts

**Files:**
- Create: `src/lib/components/Toasts.svelte`

- [ ] **Step 1: Компонент тостов**

Create `src/lib/components/Toasts.svelte`:
```svelte
<script lang="ts">
  import { toasts, dismissToast } from "../stores/toasts";
  import { ico } from "../icons";
  import { paintIcons } from "../icons";
  import { afterUpdate } from "svelte";

  afterUpdate(() => paintIcons());
</script>

<div id="toasts">
  {#each $toasts as t (t.id)}
    <div class="toast {t.kind}" on:click={() => dismissToast(t.id)} role="status">
      <span class="ti">
        {#if t.kind === "ok"}{@html ico("check")}{:else}{@html ico("info")}{/if}
      </span>
      <div class="tx">
        {t.text}
        {#if t.sub}<small>{t.sub}</small>{/if}
      </div>
    </div>
  {/each}
</div>
```
> Классы `#toasts`, `.toast`, `.ti`, `.tx` уже описаны в `global.css`.

- [ ] **Step 2: Коммит (проверка вместе с App в Task 11)**

---

## Task 11: Оболочка — Sidebar, Workspace, Dashboard, ProjectView, App

**Files:**
- Create: `src/lib/components/Sidebar.svelte`, `Workspace.svelte`, `Dashboard.svelte`, `ProjectView.svelte`
- Modify: `src/App.svelte`

> Разметку этих компонентов переносим из соответствующих render-функций прототипа
> (`_prototype/index.html`), адаптируя под Svelte: `sidebarListHTML`/`renderSidebar`
> → `Sidebar.svelte`; `renderDashboard` → `Dashboard.svelte`; `headerHTML`/`tabsHTML`
> → `ProjectView.svelte`. На Phase 0 интерактив минимален: выбор проекта, переключение
> вкладок, переключение темы. Кнопки действий (git/команды/креды) показывают тост-заглушку
> через `pushToast` — реальные `invoke` появятся в срезах Phase 1.

- [ ] **Step 1: Sidebar**

Create `src/lib/components/Sidebar.svelte`:
```svelte
<script lang="ts">
  import { projects, activeProjectId } from "../stores/projects";
  import { paintIcons } from "../icons";
  import { afterUpdate } from "svelte";

  let query = "";

  $: filtered = $projects.filter(
    (p) =>
      !query ||
      p.name.toLowerCase().includes(query.toLowerCase()) ||
      p.tags.join(" ").toLowerCase().includes(query.toLowerCase()),
  );
  $: pinned = filtered.filter((p) => p.pinned);
  $: rest = filtered.filter((p) => !p.pinned);

  afterUpdate(() => paintIcons());

  function select(id: string) {
    activeProjectId.set(id);
  }
</script>

<aside class="sidebar">
  <div class="sb-head">
    <span class="logo"><svg data-lucide="layout-grid"></svg></span>
    <span class="wordmark">Dev<span>Deck</span></span>
    <span class="ver">0.1</span>
  </div>

  <div class="sb-search" class:has-q={query}>
    <svg class="ic" data-lucide="search"></svg>
    <input placeholder="Поиск проектов…" bind:value={query} />
    <span class="kbd">Ctrl K</span>
  </div>

  <button class="sb-new"><svg class="ic ic-sm" data-lucide="plus"></svg> Новый проект</button>

  <div class="sb-scroll">
    {#if pinned.length}
      <div class="sb-section"><span>Закреплённые</span><span class="count">{pinned.length}</span></div>
      {#each pinned as p (p.id)}
        <div class="proj" class:active={$activeProjectId === p.id}
             style="--p-color:{p.color}" on:click={() => select(p.id)} role="button" tabindex="0">
          <span class="emoji">{p.emoji}</span>
          <span class="nm">{p.name}</span>
        </div>
      {/each}
    {/if}

    <div class="sb-section"><span>Все проекты</span><span class="count">{rest.length}</span></div>
    {#each rest as p (p.id)}
      <div class="proj" class:active={$activeProjectId === p.id}
           style="--p-color:{p.color}" on:click={() => select(p.id)} role="button" tabindex="0">
        <span class="dot"></span>
        <span class="nm">{p.name}</span>
      </div>
    {/each}

    {#if !filtered.length}
      <div class="sb-empty">Ничего не найдено</div>
    {/if}
  </div>
</aside>
```

- [ ] **Step 2: Dashboard**

Create `src/lib/components/Dashboard.svelte`:
```svelte
<script lang="ts">
  import { projects, activeProjectId } from "../stores/projects";
  import { USER } from "../mock";
  import { paintIcons } from "../icons";
  import { afterUpdate } from "svelte";

  $: pinned = $projects.filter((p) => p.pinned);
  afterUpdate(() => paintIcons());
</script>

<div class="ws-inner dash">
  <div style="margin-bottom:22px">
    <div class="dash-hello">Привет, <span>{USER.name}</span></div>
    <div class="dash-sub">{$projects.length} проектов</div>
  </div>

  <h3 class="section-title"><svg class="ic-sm" data-lucide="star"></svg> Закреплённые проекты</h3>
  <div class="dash-cards">
    {#each pinned as p (p.id)}
      <div class="card dcard" style="--p-color:{p.color}"
           on:click={() => activeProjectId.set(p.id)} role="button" tabindex="0">
        <div class="top">
          <span class="be">{p.emoji}</span>
          <div style="min-width:0">
            <h3>{p.name}</h3>
            <div class="pmeta">{p.git.branch}</div>
          </div>
        </div>
      </div>
    {/each}
  </div>
</div>
```

- [ ] **Step 3: ProjectView (шапка + табы, статично)**

Create `src/lib/components/ProjectView.svelte`:
```svelte
<script lang="ts">
  import type { Project } from "../types";
  import { activeTab, type Tab } from "../stores/ui";
  import { pushToast } from "../stores/toasts";
  import { paintIcons } from "../icons";
  import { afterUpdate } from "svelte";

  export let project: Project;

  const tabs: { key: Tab; label: string }[] = [
    { key: "overview", label: "Обзор" },
    { key: "tasks", label: "Задачи" },
    { key: "checklists", label: "Чеклисты" },
    { key: "creds", label: "Креды" },
    { key: "notes", label: "Заметки" },
    { key: "settings", label: "Настройки" },
  ];

  afterUpdate(() => paintIcons());
</script>

<div class="ws-inner" style="--p-color:{project.color}">
  <div class="proj-head">
    <span class="big-emoji">{project.emoji}</span>
    <div>
      <h1>{project.name}</h1>
      <div class="sub">
        <span class="pill">{project.status}</span>
        <span class="path mono">{project.path}</span>
      </div>
    </div>
    <div class="head-actions">
      <button class="act" on:click={() => pushToast("Открываю папку", project.path, "info")}>
        <svg class="ic-sm" data-lucide="folder-open"></svg> Папка
      </button>
    </div>
  </div>

  <div class="tabs">
    {#each tabs as t}
      <button class="tab" class:active={$activeTab === t.key} on:click={() => activeTab.set(t.key)}>
        {t.label}
      </button>
    {/each}
  </div>

  <div class="tab-body">
    <p class="desc">{project.desc}</p>
    <p class="desc" style="color:var(--muted);margin-top:14px">
      Вкладка «{tabs.find((x) => x.key === $activeTab)?.label}» — содержимое появится в срезах Phase 1.
    </p>
  </div>
</div>
```

- [ ] **Step 4: Workspace (роутер)**

Create `src/lib/components/Workspace.svelte`:
```svelte
<script lang="ts">
  import { projects, activeProjectId } from "../stores/projects";
  import Dashboard from "./Dashboard.svelte";
  import ProjectView from "./ProjectView.svelte";

  $: active = $projects.find((p) => p.id === $activeProjectId) ?? null;
</script>

<main class="workspace">
  {#if active}
    <ProjectView project={active} />
  {:else}
    <Dashboard />
  {/if}
</main>
```

- [ ] **Step 5: src/routes/+page.svelte (главный экран)**

Заменить содержимое `src/routes/+page.svelte` (удалив демо-разметку скаффолда) на:
```svelte
<script lang="ts">
  import "$lib/stores/theme"; // активирует подписку темы
  import Sidebar from "$lib/components/Sidebar.svelte";
  import Workspace from "$lib/components/Workspace.svelte";
  import Toasts from "$lib/components/Toasts.svelte";
  import { onMount } from "svelte";
  import { paintIcons } from "$lib/icons";

  onMount(() => paintIcons());
</script>

<div id="app">
  <Sidebar />
  <Workspace />
</div>
<Toasts />
```

- [ ] **Step 6: Запустить и проверить вручную**

Run:
```powershell
npm run tauri dev
```
Expected:
- Открывается окно DevDeck в тёмной теме.
- Слева сайдбар с закреплёнными и остальными проектами; поиск фильтрует список.
- По центру дашборд с карточками закреплённых проектов.
- Клик по проекту в сайдбаре или по карточке → открывается карточка проекта с шапкой и табами.
- Переключение вкладок работает; кнопка «Папка» показывает тост.

Закрыть окно.

- [ ] **Step 7: Коммит**

Run:
```powershell
git add -A
git commit -m @'
feat: app shell (sidebar, dashboard, project view, toasts) on mock data

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

## Task 12: Переключатель темы

**Files:**
- Modify: `src/lib/components/Sidebar.svelte` (кнопка в подвале)

- [ ] **Step 1: Добавить подвал сайдбара с кнопкой темы**

В `src/lib/components/Sidebar.svelte` импортировать тему:
```ts
import { theme, toggleTheme } from "../stores/theme";
import { USER } from "../mock";
```
И перед закрывающим `</aside>` добавить подвал:
```svelte
  <div class="sb-foot">
    <span class="avatar">{USER.initials}</span>
    <div class="who">{USER.name}<small>{USER.handle}</small></div>
    <button class="icon-btn" on:click={toggleTheme} title="Сменить тему">
      {#if $theme === "dark"}<svg class="ic" data-lucide="sun"></svg>
      {:else}<svg class="ic" data-lucide="moon"></svg>{/if}
    </button>
  </div>
```

- [ ] **Step 2: Проверить переключение темы**

Run:
```powershell
npm run tauri dev
```
Expected: клик по кнопке в подвале сайдбара переключает тёмную/светлую тему; выбор сохраняется после перезапуска окна. Закрыть окно.

- [ ] **Step 3: Коммит**

Run:
```powershell
git add -A
git commit -m @'
feat: theme toggle in sidebar footer with persistence

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

## Task 13: Vitest на стор тостов

**Files:**
- Create: `src/tests/toasts.test.ts`
- Modify: `package.json` (скрипт test, devDependency vitest), `vite.config.ts` (опц. test-блок)

- [ ] **Step 1: Установить Vitest**

Run:
```powershell
npm install -D vitest
```
Expected: `vitest` в `devDependencies`.

- [ ] **Step 2: Добавить скрипт test**

В `package.json` в `"scripts"` добавить:
```json
"test": "vitest run"
```

- [ ] **Step 3: Тест стора тостов**

Create `src/tests/toasts.test.ts`:
```ts
import { describe, it, expect, vi, beforeEach } from "vitest";
import { get } from "svelte/store";
import { toasts, pushToast, dismissToast } from "../lib/stores/toasts";

describe("toasts store", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    toasts.set([]);
  });

  it("adds a toast and returns its id", () => {
    const id = pushToast("Готово", "детали", "ok");
    const list = get(toasts);
    expect(list).toHaveLength(1);
    expect(list[0]).toMatchObject({ id, text: "Готово", sub: "детали", kind: "ok" });
  });

  it("dismisses a toast by id", () => {
    const id = pushToast("X");
    dismissToast(id);
    expect(get(toasts)).toHaveLength(0);
  });

  it("auto-dismisses after timeout", () => {
    pushToast("X");
    expect(get(toasts)).toHaveLength(1);
    vi.advanceTimersByTime(3300);
    expect(get(toasts)).toHaveLength(0);
  });
});
```

- [ ] **Step 4: Прогнать тесты**

Run:
```powershell
npm test
```
Expected: 3 passed.

- [ ] **Step 5: Коммит**

Run:
```powershell
git add -A
git commit -m @'
test: add vitest and toasts store tests

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

## Task 14: Финальная проверка Phase 0

**Files:** —

- [ ] **Step 1: Полная проверка сборки и тестов**

Run:
```powershell
npm run check
npm test
cargo test --manifest-path src-tauri/Cargo.toml
```
Expected: типы 0 ошибок; Vitest 3 passed; Rust-тесты 2 passed.

- [ ] **Step 2: Ручной чек-лист в окне**

Run:
```powershell
npm run tauri dev
```
Проверить:
- [ ] Окно открывается без ошибок в консоли DevTools.
- [ ] Дизайн-система применена (тёмная тема, шрифты Inter/JetBrains Mono, иконки lucide отрисованы).
- [ ] Сайдбар: список проектов, поиск фильтрует, закреплённые сверху.
- [ ] Дашборд: карточки закреплённых проектов кликабельны.
- [ ] Карточка проекта: шапка, табы переключаются.
- [ ] Тема переключается и сохраняется между перезапусками.
- [ ] `await window.__TAURI__.core.invoke("db_health")` возвращает `1`.
- [ ] Файл `%APPDATA%\DevDeck\devdeck.db` и папка `backups` созданы.

Закрыть окно.

- [ ] **Step 3: Итоговый коммит (если были правки по чек-листу)**

Run:
```powershell
git add -A
git commit -m @'
chore: phase 0 skeleton complete

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

## Task 15: (до релиза, не блокирует запуск) Локальный бандлинг lucide и шрифтов

**Зачем:** CDN-теги lucide и Google Fonts (a) нарушают NFR «полная работа офлайн»
(раздел 9 ТЗ) — без интернета иконки не отрисуются, шрифты откатятся на системные;
(b) без `integrity`/SRI создают риск компрометации CDN. Бандлинг через npm убирает оба.

**Files:**
- Modify: `index.html` (убрать CDN-теги lucide и шрифтов)
- Modify: `src/lib/icons.ts` (использовать npm-пакет вместо `window.lucide`)
- Modify: `src/lib/styles/global.css` (подключить локальные шрифты)

- [ ] **Step 1: Установить lucide и шрифты**

Run:
```powershell
npm install lucide @fontsource/inter @fontsource/jetbrains-mono
```

- [ ] **Step 2: Переключить icons.ts на npm-пакет**

Заменить `src/lib/icons.ts`:
```ts
import { createIcons, icons } from "lucide";

export function paintIcons(): void {
  createIcons({ icons });
}

export function ico(name: string, cls = "ic"): string {
  return `<svg class="${cls}" data-lucide="${name}"></svg>`;
}
```
> `createIcons({ icons })` берёт все иконки из пакета; при желании позже сузить до
> используемого набора для меньшего бандла.

- [ ] **Step 3: Подключить шрифты в global.css**

В начало `src/lib/styles/global.css` добавить:
```css
@import "@fontsource/inter/400.css";
@import "@fontsource/inter/500.css";
@import "@fontsource/inter/600.css";
@import "@fontsource/inter/700.css";
@import "@fontsource/jetbrains-mono/400.css";
@import "@fontsource/jetbrains-mono/500.css";
@import "@fontsource/jetbrains-mono/600.css";
```

- [ ] **Step 4: Убрать CDN-теги из index.html**

Удалить из `index.html` строки `<link>` Google Fonts, `<link rel=preconnect>` и
`<script src=...lucide...>` — теперь всё из npm.

- [ ] **Step 5: Проверить офлайн**

Run `npm run tauri dev`, затем отключить сеть. Expected: иконки и шрифты отрисованы,
приложение полностью работает офлайн.

- [ ] **Step 6: Коммит**

Run:
```powershell
git add -A
git commit -m @'
build: bundle lucide and fonts locally for offline support

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

## Итог Phase 0

Запускающееся окно DevDeck с перенесённой дизайн-системой, оболочкой на Svelte-сторах
(сайдбар / дашборд / карточка проекта на моках) и рабочей инфраструктурой бэкенда
(SQLite + миграции через `user_version` + `AppState` + `AppError` + команда `db_health`),
покрытой Rust- и Vitest-тестами. Готовая база для первого вертикального среза Phase 1
(«Проекты CRUD»), который заменит моки реальными данными из БД.
```
