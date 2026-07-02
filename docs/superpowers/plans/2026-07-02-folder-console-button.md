# Per-folder console (Git Bash) button — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Кнопка «консоль (Git Bash) в папке» справа у каждого ярлыка «Файлы и папки» на Обзоре, с per-ярлык чекбоксом (по умолчанию включён) в настройках.

**Architecture:** Новый флаг `show_terminal` на таблице `files`. Команда `open_git_bash(path)` резолвит директорию (папка либо родитель файла) и запускает `git-bash.exe --cd=<dir>` с фолбэком на системный терминал (вынесен из `open_terminal`). Команда `files_set_terminal` переключает флаг. Фронт: кнопка на Обзоре (gated) + чекбокс в настройках.

**Tech Stack:** Rust + rusqlite + `std::process::Command`; SvelteKit (Svelte 5 runes) + TypeScript.

## Global Constraints

- Rust-команды возвращают `AppResult<T>`; без `unwrap()`/`expect()` вне `#[cfg(test)]`.
- Миграции регистрируются в массиве `MIGRATIONS` в `src-tauri/src/db/migrations.rs` (include_str!).
- Компоненты вызывают бэкенд только через `src/lib/api/*`.
- `npm run check` = 0 ошибок (базовые ~40 warning допустимы); Rust-тесты `cargo test --manifest-path src-tauri/Cargo.toml --lib`.

Спека: `docs/superpowers/specs/2026-07-02-folder-console-button-design.md`.

---

## Task 1: Backend — миграция show_terminal

**Files:**
- Create: `src-tauri/migrations/0008_files_show_terminal.sql`
- Modify: `src-tauri/src/db/migrations.rs`

- [ ] **Step 1: Создать миграцию**

`src-tauri/migrations/0008_files_show_terminal.sql`:
```sql
ALTER TABLE files ADD COLUMN show_terminal INTEGER NOT NULL DEFAULT 1;

PRAGMA user_version = 8;
```

- [ ] **Step 2: Зарегистрировать + бампнуть версии тестов**

В `src-tauri/src/db/migrations.rs`, в `MIGRATIONS` после
`(7, include_str!("../../migrations/0007_global_creds.sql")),` добавить:
```rust
    (8, include_str!("../../migrations/0008_files_show_terminal.sql")),
```
В тестах `applies_migrations_on_empty_db` и `run_is_idempotent` заменить
`assert_eq!(schema_version(&conn).unwrap(), 7);` → `8` (обе строки).

- [ ] **Step 3: Прогнать тест миграций**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib migrations`
Expected: PASS (схема до v8).

- [ ] **Step 4: Commit**

```bash
git add src-tauri/migrations/0008_files_show_terminal.sql src-tauri/src/db/migrations.rs
git commit -m "feat(backend): migration 0008 — files.show_terminal"
```

---

## Task 2: Backend — show_terminal в модели, row_to_file, files_set_terminal

**Files:**
- Modify: `src-tauri/src/models.rs`
- Modify: `src-tauri/src/commands/files.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Produces: `FileShortcut { ..., show_terminal: bool }` (serde → `showTerminal`);
  `#[tauri::command] files_set_terminal(state, id: i64, show_terminal: bool) -> AppResult<FileShortcut>`.

- [ ] **Step 1: Поле в модели**

В `src-tauri/src/models.rs`, в `FileShortcut`, после `pub sort_order: i64,` добавить:
```rust
    pub show_terminal: bool,
```

- [ ] **Step 2: row_to_file**

В `src-tauri/src/commands/files.rs`, в `row_to_file`, заменить SELECT + конструктор.
SELECT:
```rust
        "SELECT id, project_id, label, path, sort_order, show_terminal FROM files WHERE id = ?1",
```
и в конструктор `FileShortcut`, после `sort_order: r.get(4)?,` добавить:
```rust
            show_terminal: r.get::<_, i64>(5)? != 0,
```

- [ ] **Step 3: Команда files_set_terminal**

В `src-tauri/src/commands/files.rs`, после `files_delete`, добавить:
```rust
#[tauri::command]
pub fn files_set_terminal(state: State<AppState>, id: i64, show_terminal: bool) -> AppResult<FileShortcut> {
    let conn = lock(&state)?;
    conn.execute(
        "UPDATE files SET show_terminal=?2 WHERE id=?1",
        params![id, if show_terminal { 1i64 } else { 0 }],
    )?;
    row_to_file(&conn, id)
}
```

- [ ] **Step 4: Зарегистрировать команду**

В `src-tauri/src/lib.rs`, после `commands::files::files_delete,` добавить:
```rust
            commands::files::files_set_terminal,
```

- [ ] **Step 5: Компиляция и тесты**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib`
Expected: всё компилируется и проходит.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/models.rs src-tauri/src/commands/files.rs src-tauri/src/lib.rs
git commit -m "feat(backend): show_terminal in FileShortcut + files_set_terminal"
```

---

## Task 3: Backend — open_git_bash + dir_of (TDD)

**Files:**
- Modify: `src-tauri/src/commands/actions.rs` (хелперы, команда, тест)
- Modify: `src-tauri/src/lib.rs` (регистрация)

**Interfaces:**
- Produces: `#[tauri::command] open_git_bash(path: String) -> AppResult<()>`;
  pure `dir_of(p: &str, is_dir: bool) -> Option<String>`.

- [ ] **Step 1: Написать падающий тест**

В `src-tauri/src/commands/actions.rs`, в `#[cfg(test)] mod tests` (после
`expands_leading_tilde`), добавить:
```rust
    #[test]
    fn dir_of_returns_folder_or_parent() {
        // папка → сама
        assert_eq!(super::dir_of(r"C:\a\b", true), Some(r"C:\a\b".to_string()));
        // файл → родительская папка
        assert_eq!(super::dir_of(r"C:\a\f.txt", false), Some(r"C:\a".to_string()));
        // файл без директории → None
        assert_eq!(super::dir_of("f.txt", false), None);
    }
```

- [ ] **Step 2: Запустить — не компилируется**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib dir_of_returns`
Expected: ошибка компиляции — `dir_of` не найдена.

- [ ] **Step 3: Реализовать хелперы + команду + рефактор open_terminal**

В `src-tauri/src/commands/actions.rs`. Сначала заменить тело `open_terminal`,
вынеся общий запуск терминала в `spawn_terminal_at`. Текущее:
```rust
#[tauri::command]
pub fn open_terminal(path: String) -> AppResult<()> {
    let p = require_dir(&path)?;
    let mut wt = Command::new("wt.exe");
    wt.args(["-d", &p]);
    if wt.spawn().is_ok() {
        return Ok(());
    }
    let mut fallback = Command::new("cmd");
    fallback.arg("/K").current_dir(&p).creation_flags(CREATE_NEW_CONSOLE);
    spawn(fallback, "терминал")
}
```
заменить на:
```rust
/// Открыть системный терминал в папке: Windows Terminal `wt -d`, затем cmd.
fn spawn_terminal_at(dir: &str) -> AppResult<()> {
    let mut wt = Command::new("wt.exe");
    wt.args(["-d", dir]);
    if wt.spawn().is_ok() {
        return Ok(());
    }
    let mut fallback = Command::new("cmd");
    fallback.arg("/K").current_dir(dir).creation_flags(CREATE_NEW_CONSOLE);
    spawn(fallback, "терминал")
}

#[tauri::command]
pub fn open_terminal(path: String) -> AppResult<()> {
    let p = require_dir(&path)?;
    spawn_terminal_at(&p)
}

/// Директория для консоли: сама папка, либо родитель файла.
fn dir_of(p: &str, is_dir: bool) -> Option<String> {
    if is_dir {
        Some(p.to_string())
    } else {
        Path::new(p)
            .parent()
            .map(|x| x.to_string_lossy().into_owned())
            .filter(|s| !s.is_empty())
    }
}

/// Кандидаты git-bash.exe: PATH → стандартные пути установки.
fn git_bash_candidates() -> Vec<String> {
    let mut v = vec![
        "git-bash.exe".to_string(),
        r"C:\Program Files\Git\git-bash.exe".to_string(),
        r"C:\Program Files (x86)\Git\git-bash.exe".to_string(),
    ];
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        v.push(format!(r"{}\Programs\Git\git-bash.exe", local.trim_end_matches('\\')));
    }
    v
}

/// Открыть Git Bash в папке ярлыка (файл → родительская папка). Если Git Bash
/// не найден — фолбэк на системный терминал.
#[tauri::command]
pub fn open_git_bash(path: String) -> AppResult<()> {
    let p = require_dir_or_file(&path)?;
    let is_dir = Path::new(&p).is_dir();
    let dir = dir_of(&p, is_dir).ok_or_else(|| AppError {
        kind: ErrorKind::NotFound,
        message: "Не удалось определить папку".into(),
    })?;
    for exe in git_bash_candidates() {
        if Command::new(&exe).arg(format!("--cd={}", dir)).spawn().is_ok() {
            return Ok(());
        }
    }
    spawn_terminal_at(&dir)
}
```

- [ ] **Step 4: Зарегистрировать команду**

В `src-tauri/src/lib.rs`, после `commands::actions::open_terminal,` добавить:
```rust
            commands::actions::open_git_bash,
```

- [ ] **Step 5: Запустить тесты**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib`
Expected: все PASS, включая `dir_of_returns_folder_or_parent`.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/commands/actions.rs src-tauri/src/lib.rs
git commit -m "feat(backend): open_git_bash command (Git Bash Here with fallback)"
```

---

## Task 4: Frontend — типы и API

**Files:**
- Modify: `src/lib/types.ts`
- Modify: `src/lib/api/files.ts`
- Modify: `src/lib/api/actions.ts`

**Interfaces:**
- Produces: `FileShortcut.showTerminal: boolean`;
  `files.setTerminal(id: number, showTerminal: boolean): Promise<FileShortcut>`;
  `actions.openGitBash(path: string): Promise<void>`.

- [ ] **Step 1: Тип**

В `src/lib/types.ts`, найти
```ts
export type FileShortcut = { id: number; projectId: number; label: string; path: string; sortOrder: number };
```
и заменить на:
```ts
export type FileShortcut = { id: number; projectId: number; label: string; path: string; sortOrder: number; showTerminal: boolean };
```

- [ ] **Step 2: files API**

В `src/lib/api/files.ts`, после строки `export const remove = ...` добавить:
```ts
export const setTerminal = (id: number, showTerminal: boolean) =>
  call<FileShortcut>("files_set_terminal", { id, showTerminal });
```

- [ ] **Step 3: actions API**

В `src/lib/api/actions.ts`, после `openShortcut` добавить:
```ts
export const openGitBash = (path: string) => call<void>("open_git_bash", { path });
```

- [ ] **Step 4: Проверить типы**

Run: `npm run check`
Expected: 0 ошибок.

- [ ] **Step 5: Commit**

```bash
git add src/lib/types.ts src/lib/api/files.ts src/lib/api/actions.ts
git commit -m "feat(frontend): showTerminal type + setTerminal/openGitBash api"
```

---

## Task 5: Frontend — кнопка консоли на Обзоре

**Files:**
- Modify: `src/lib/components/OverviewTab.svelte`
- Modify: `src/lib/styles/global.css`

**Interfaces:**
- Consumes: `actions.openGitBash(path)`, `FileShortcut.showTerminal`.

- [ ] **Step 1: Кнопка в строке ярлыка**

В `src/lib/components/OverviewTab.svelte`, в блоке «Файлы и папки», заменить строку
ярлыка. Текущее:
```svelte
          <div class="link-row" role="button" tabindex="0" style="cursor:pointer" onclick={() => actions.openShortcut(f.path)}>
            <span class="lico"><Icon name="file" class="ic-sm" /></span>
            <div style="flex:1;min-width:0"><div class="lt">{f.label}</div><div class="lu">{f.path}</div></div>
            <span class="ext"><Icon name="arrow-up-right" class="ic-sm" /></span>
          </div>
```
на:
```svelte
          <div class="link-row" role="button" tabindex="0" style="cursor:pointer" onclick={() => actions.openShortcut(f.path)}>
            <span class="lico"><Icon name="file" class="ic-sm" /></span>
            <div style="flex:1;min-width:0"><div class="lt">{f.label}</div><div class="lu">{f.path}</div></div>
            {#if f.showTerminal}
              <button class="lrow-act" title="Открыть консоль (Git Bash) здесь" onclick={(e) => { e.stopPropagation(); actions.openGitBash(f.path); }}>
                <Icon name="square-terminal" class="ic-sm" />
              </button>
            {/if}
            <span class="ext"><Icon name="arrow-up-right" class="ic-sm" /></span>
          </div>
```

- [ ] **Step 2: Стиль кнопки**

В `src/lib/styles/global.css` добавить (рядом с прочими стилями строк-ссылок, напр.
после правил `.link-row`):
```css
.lrow-act {
  background: transparent; border: 0; color: var(--muted); cursor: pointer;
  display: inline-flex; align-items: center; padding: 4px; border-radius: 6px; flex: none;
}
.lrow-act:hover { color: var(--accent); background: var(--hover); }
```

- [ ] **Step 3: Проверить типы**

Run: `npm run check`
Expected: 0 ОШИБОК (это жёсткое требование). Кнопка вложена в `.link-row` с
`role="button"` — Svelte может выдать один a11y-WARNING про вложенный интерактив;
это допустимо (у этой строки уже есть похожие warning в базовых ~40). Блокирует
только ОШИБКА — если появится, сообщить.

- [ ] **Step 4: Commit**

```bash
git add src/lib/components/OverviewTab.svelte src/lib/styles/global.css
git commit -m "feat(frontend): console (Git Bash) button on file shortcuts in Overview"
```

---

## Task 6: Frontend — чекбокс в настройках

**Files:**
- Modify: `src/lib/components/SettingsTab.svelte`
- Modify: `src/lib/styles/global.css`

**Interfaces:**
- Consumes: `filesApi.setTerminal(id, showTerminal)`, `FileShortcut.showTerminal`.

- [ ] **Step 1: Обработчик**

В `src/lib/components/SettingsTab.svelte`, рядом с `delFile` (строка ~55) добавить:
```ts
  async function toggleTerminal(f: FileShortcut) { await filesApi.setTerminal(f.id, !f.showTerminal); await loadExtras(); }
```
(`FileShortcut` уже импортирован — используется в `let files = $state<FileShortcut[]>([])`.)

- [ ] **Step 2: Чекбокс в строке**

В блоке «Файлы и папки», заменить строку ярлыка. Текущее:
```svelte
        <div class="link-row">
          <span class="lico"><Icon name="file" class="ic-sm" /></span>
          <div style="flex:1;min-width:0"><div class="lt">{f.label}</div><div class="lu">{f.path}</div></div>
          <button class="mini" title="Удалить" onclick={() => delFile(f.id)}><Icon name="x" class="ic-sm" /></button>
        </div>
```
на:
```svelte
        <div class="link-row">
          <span class="lico"><Icon name="file" class="ic-sm" /></span>
          <div style="flex:1;min-width:0"><div class="lt">{f.label}</div><div class="lu">{f.path}</div></div>
          <label class="file-term" title="Показывать кнопку консоли на «Обзоре»">
            <input type="checkbox" checked={f.showTerminal} onchange={() => toggleTerminal(f)} /> консоль
          </label>
          <button class="mini" title="Удалить" onclick={() => delFile(f.id)}><Icon name="x" class="ic-sm" /></button>
        </div>
```

- [ ] **Step 3: Стиль чекбокса**

В `src/lib/styles/global.css` добавить:
```css
.file-term { display: inline-flex; align-items: center; gap: 5px; font-size: 11px; color: var(--muted); flex: none; cursor: pointer; white-space: nowrap; }
.file-term input { cursor: pointer; accent-color: var(--accent); }
```

- [ ] **Step 4: Проверить типы**

Run: `npm run check`
Expected: 0 ошибок.

- [ ] **Step 5: Commit**

```bash
git add src/lib/components/SettingsTab.svelte src/lib/styles/global.css
git commit -m "feat(frontend): per-shortcut console toggle in project settings"
```

---

## Task 7: Полная верификация

- [ ] **Step 1: Rust-тесты**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib`
Expected: все PASS (миграция v8, `dir_of`).

- [ ] **Step 2: Фронт-тесты + типы**

Run: `npm test` и `npm run check`
Expected: тесты PASS; 0 ошибок типов.

- [ ] **Step 3: Ручная проверка**

Run: `npm run tauri dev`
- На «Обзоре» у ярлыка «Файлы и папки» справа — иконка консоли; клик открывает
  Git Bash в этой папке (для файла — в его родительской папке); клик по иконке НЕ
  открывает сам ярлык.
- В «Настройках» у ярлыка есть чекбокс «консоль»; снять галочку → на «Обзоре»
  кнопка у этого ярлыка исчезает; вернуть → появляется.
- Если Git Bash не установлен — открывается Windows Terminal/cmd в той же папке.

- [ ] **Step 4: Финальный commit (если были правки)**

```bash
git add -A
git commit -m "fix: folder console button polish after manual verification"
```

(Если правок нет — пропустить.)
