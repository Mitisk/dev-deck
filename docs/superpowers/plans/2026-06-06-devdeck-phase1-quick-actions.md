# DevDeck Phase 1 — Срез «Быстрые действия» — план реализации

> **Для исполнителя:** ОБЯЗАТЕЛЬНЫЙ ПОД-НАВЫК: superpowers:subagent-driven-development. Шаги — чекбоксы (`- [ ]`).

**Цель:** Кнопки в шапке проекта реально что-то делают: открыть папку проекта в Проводнике, открыть в редакторе (`code`), открыть терминал в папке (Windows Terminal с фолбэком на `cmd`). Плюс команда «открыть URL в браузере» (понадобится срезу ссылок). Действуют на `path` проекта; раскрытие `~` в домашнюю папку.

**Архитектура:** Rust-команды `open_path`/`open_in_editor`/`open_terminal`/`open_url` (раздел 10 ТЗ) запускают внешние программы через `std::process::Command` (spawn без ожидания). Фронт зовёт их через `src/lib/api/actions.ts`; кнопки шапки `ProjectView.svelte` привязаны к ним. Ошибки запуска → `AppError` → тост.

**Стек:** как раньше (Tauri 2 · Rust · SvelteKit/Svelte 5 руны).

**Решения:**
- Редактор пока **захардкожен `code`** (раздел 5.4 ТЗ хочет настраиваемый — отложено до среза настроек приложения; в плане отмечено).
- Раскрытие пути: ведущий `~` → `%USERPROFILE%`. Остальной путь — как есть (Windows-пути работают напрямую).
- На Windows `code` — это `code.cmd`, поэтому запуск через `cmd /C`. Терминал: `wt -d <path>`, при неудаче — `cmd` с рабочей папкой.

**Источники:** `TZ_DevDeck.md` (раздел 5.4 — быстрые действия, раздел 10 — команды). Текущий `ProjectView.svelte` имеет одну кнопку «Папка» со stub-тостом — заменим на реальные.

---

## Контекст (после среза «Проекты CRUD»)

- Rust: команды `db_health` + `projects_*` зарегистрированы в `generate_handler!`. `AppState { db: Mutex<Connection> }`, `error::{AppError, ErrorKind, AppResult}`. Модули `commands/{mod,health,projects}.rs`.
- Фронт: `ProjectView.svelte` — шапка проекта; сейчас одна кнопка `.act` «Папка», вызывает `pushToast(...)`-заглушку. `Project` имеет `path: string | null`. api-слой: `client.ts` (`call<T>`).
- Иконки: `Icon.svelte` (`<Icon name=... />`), пакет lucide.

---

## Структура файлов

```
src-tauri/src/commands/
├─ mod.rs            # MOD: + pub mod actions;
└─ actions.rs        # NEW: open_path/open_in_editor/open_terminal/open_url + expand_path + тест
src-tauri/src/lib.rs # MOD: регистрация 4 команд

src/lib/api/
└─ actions.ts        # NEW: 4 обёртки
src/lib/components/
└─ ProjectView.svelte# MOD: кнопки Папка/Редактор/Терминал → реальные команды
```

---

## Task 1: Rust — команды быстрых действий

**Files:**
- Create: `src-tauri/src/commands/actions.rs`
- Modify: `src-tauri/src/commands/mod.rs`, `src-tauri/src/lib.rs`

- [ ] **Step 1: Создать actions.rs**

Create `src-tauri/src/commands/actions.rs`:
```rust
use crate::error::{AppError, AppResult, ErrorKind};
use std::process::Command;

/// Раскрыть ведущий `~` в %USERPROFILE%. Остальной путь не трогаем.
fn expand_path(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed == "~" {
        return std::env::var("USERPROFILE").unwrap_or_else(|_| trimmed.to_string());
    }
    if let Some(rest) = trimmed.strip_prefix("~/").or_else(|| trimmed.strip_prefix("~\\")) {
        if let Ok(home) = std::env::var("USERPROFILE") {
            return format!("{}\\{}", home.trim_end_matches('\\'), rest.replace('/', "\\"));
        }
    }
    trimmed.to_string()
}

fn require_path(path: &str) -> AppResult<String> {
    let p = expand_path(path);
    if p.is_empty() {
        return Err(AppError { kind: ErrorKind::Validation, message: "Путь к проекту не задан".into() });
    }
    Ok(p)
}

fn spawn(mut cmd: Command, what: &str) -> AppResult<()> {
    cmd.spawn()
        .map(|_| ())
        .map_err(|e| AppError { kind: ErrorKind::Io, message: format!("Не удалось запустить {}: {}", what, e) })
}

/// Открыть папку в Проводнике. explorer.exe возвращает ненулевой код даже при успехе —
/// поэтому только spawn, без проверки статуса.
#[tauri::command]
pub fn open_path(path: String) -> AppResult<()> {
    let p = require_path(&path)?;
    let mut cmd = Command::new("explorer.exe");
    cmd.arg(&p);
    spawn(cmd, "Проводник")
}

/// Открыть папку в редакторе. По умолчанию VS Code (`code`). На Windows `code` —
/// это code.cmd, поэтому через `cmd /C`. (Настраиваемый редактор — позже, в срезе настроек.)
#[tauri::command]
pub fn open_in_editor(path: String) -> AppResult<()> {
    let p = require_path(&path)?;
    let mut cmd = Command::new("cmd");
    cmd.args(["/C", "code", &p]);
    spawn(cmd, "редактор (code)")
}

/// Открыть терминал в папке: Windows Terminal `wt -d <path>`; при неудаче — cmd в этой папке.
#[tauri::command]
pub fn open_terminal(path: String) -> AppResult<()> {
    let p = require_path(&path)?;
    let mut wt = Command::new("wt.exe");
    wt.args(["-d", &p]);
    if wt.spawn().is_ok() {
        return Ok(());
    }
    // Фолбэк: новое окно cmd с рабочей папкой.
    let mut fallback = Command::new("cmd");
    fallback.args(["/C", "start", "cmd", "/K", "cd", "/d", &p]);
    spawn(fallback, "терминал")
}

/// Открыть URL в браузере по умолчанию.
#[tauri::command]
pub fn open_url(url: String) -> AppResult<()> {
    let u = url.trim();
    if u.is_empty() {
        return Err(AppError { kind: ErrorKind::Validation, message: "URL не задан".into() });
    }
    // Если нет схемы — добавим https:// (localhost:3000, github.com/... и т.п.).
    let full = if u.contains("://") { u.to_string() } else { format!("https://{}", u) };
    let mut cmd = Command::new("cmd");
    cmd.args(["/C", "start", "", &full]);
    spawn(cmd, "браузер")
}

#[cfg(test)]
mod tests {
    use super::expand_path;

    #[test]
    fn expands_leading_tilde() {
        std::env::set_var("USERPROFILE", "C:\\Users\\test");
        assert_eq!(expand_path("~"), "C:\\Users\\test");
        assert_eq!(expand_path("~/dev/proj"), "C:\\Users\\test\\dev\\proj");
        assert_eq!(expand_path("~\\dev\\proj"), "C:\\Users\\test\\dev\\proj");
    }

    #[test]
    fn leaves_absolute_paths_untouched() {
        assert_eq!(expand_path("D:\\code\\app"), "D:\\code\\app");
        assert_eq!(expand_path("  C:\\x  "), "C:\\x"); // только trim
    }
}
```

- [ ] **Step 2: Зарегистрировать модуль**

В `src-tauri/src/commands/mod.rs` добавить:
```rust
pub mod actions;
```

- [ ] **Step 3: Зарегистрировать команды в lib.rs**

В `tauri::generate_handler![...]` добавить (после команд projects):
```rust
            commands::actions::open_path,
            commands::actions::open_in_editor,
            commands::actions::open_terminal,
            commands::actions::open_url,
```

- [ ] **Step 4: Тесты + сборка (NON-blocking; не запускать `npm run tauri dev`)**

Run:
```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib
cargo build --manifest-path src-tauri/Cargo.toml
```
Expected: новые тесты `expands_leading_tilde`, `leaves_absolute_paths_untouched` + прежние (5) — все ok; build успешен.

- [ ] **Step 5: Commit**

```powershell
git add src-tauri/src/commands/actions.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs
git commit -m @'
feat(backend): quick-action commands (open folder/editor/terminal/url)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

## Task 2: Фронт — api + кнопки шапки

**Files:**
- Create: `src/lib/api/actions.ts`
- Modify: `src/lib/components/ProjectView.svelte`

- [ ] **Step 1: api/actions.ts**

Create `src/lib/api/actions.ts`:
```ts
import { call } from "./client";

export const openPath = (path: string) => call<void>("open_path", { path });
export const openInEditor = (path: string) => call<void>("open_in_editor", { path });
export const openTerminal = (path: string) => call<void>("open_terminal", { path });
export const openUrl = (url: string) => call<void>("open_url", { url });
```

- [ ] **Step 2: Привязать кнопки шапки в ProjectView.svelte**

В `src/lib/components/ProjectView.svelte`:

1. Добавить импорты в `<script>` (рядом с прочими):
```ts
  import * as actions from "$lib/api/actions";
```
2. Добавить хелпер после объявления `let { project }`:
```ts
  function run(action: () => Promise<void>) {
    // Ошибки показывает api/client.ts тостом; здесь просто запускаем.
    void action();
  }
```
3. Заменить блок `<div class="head-actions">...</div>` (сейчас одна кнопка «Папка» с `pushToast`) на:
```svelte
    <div class="head-actions">
      <button class="act sq" title="Открыть папку"
              disabled={!project.path}
              onclick={() => project.path && run(() => actions.openPath(project.path!))}>
        <Icon name="folder-open" class="ic" />
      </button>
      <button class="act sq" title="Открыть в редакторе"
              disabled={!project.path}
              onclick={() => project.path && run(() => actions.openInEditor(project.path!))}>
        <Icon name="code-xml" class="ic" />
      </button>
      <button class="act sq" title="Открыть терминал здесь"
              disabled={!project.path}
              onclick={() => project.path && run(() => actions.openTerminal(project.path!))}>
        <Icon name="square-terminal" class="ic" />
      </button>
    </div>
```
> Если у проекта нет пути — кнопки задизейблены. Класс `.act.sq` (квадратная кнопка) уже есть в global.css. Иконки `folder-open`/`code-xml`/`square-terminal` — из lucide.
4. Если после правки `pushToast` больше нигде в файле не используется — убрать его импорт, чтобы не было предупреждения о неиспользуемом импорте. (Если используется — оставить.)

- [ ] **Step 3: Проверка (NON-blocking; не запускать `npm run tauri dev`)**

Run:
```powershell
npm run check
npm test
npm run build
```
Expected: `npm run check` 0 ОШИБОК (a11y/`node`/`state_referenced_locally` warnings приемлемы); Vitest 3 passed; build успешен.

- [ ] **Step 4: Commit**

```powershell
git add src/lib/api/actions.ts src/lib/components/ProjectView.svelte
git commit -m @'
feat(frontend): wire project header quick actions (folder/editor/terminal)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

## Task 3: Проверка среза

- [ ] **Step 1: Автопроверка (контроллер)**
```powershell
npm run check ; npm test ; cargo test --manifest-path src-tauri/Cargo.toml --lib
```
Expected: типы 0 ошибок; Vitest 3; Rust 7 (5 прежних + 2 expand_path).

- [ ] **Step 2: Ручной GUI-смоук (пользователь; `npm run tauri dev`)**
- [ ] Создать/открыть проект, задать реальный путь к существующей папке (через вкладку «Настройки»).
- [ ] Кнопка «папка» в шапке → открывается Проводник на этой папке.
- [ ] Кнопка «редактор» → открывается VS Code на папке (если `code` в PATH).
- [ ] Кнопка «терминал» → открывается Windows Terminal (или cmd) в этой папке.
- [ ] У проекта без пути кнопки задизейблены.
- [ ] (Опц.) Неверный путь / нет `code` → тост с ошибкой, приложение не падает.

---

## Итог среза

Шапка проекта запускает реальные действия: Проводник, редактор, терминал — на папке проекта, с раскрытием `~`. Команда `open_url` готова для среза ссылок. Редактор пока `code` (настраиваемый — в срезе настроек приложения). Дальше по Phase 1 — **git-сводка** (чтение через git2 + pull/push/fetch).
```
