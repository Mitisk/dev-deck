# Кнопка «консоль в папке» для файлов/папок проекта

Дата: 2026-07-02
Статус: согласовано, к реализации

## Проблема / цель

На вкладке «Обзор» в блоке «Файлы и папки» ([OverviewTab.svelte](../../../src/lib/components/OverviewTab.svelte))
каждый ярлык только открывается (`openShortcut`). Нужно добавить справа кнопку,
открывающую **Git Bash** в директории этого ярлыка (как «Git Bash Here»), с
фолбэком на системный терминал. Видимость кнопки настраивается **для каждого
ярлыка отдельно** — в настройках проекта чекбокс (по умолчанию включён); если
выключить, кнопка для этого ярлыка не показывается.

## 1. Data model — миграция `0008_files_show_terminal.sql`

```sql
ALTER TABLE files ADD COLUMN show_terminal INTEGER NOT NULL DEFAULT 1;

PRAGMA user_version = 8;
```

- `db/migrations.rs`: зарегистрировать `(8, include_str!("../../migrations/0008_files_show_terminal.sql"))`;
  в тестах `applies_migrations_on_empty_db` и `run_is_idempotent` обновить версию `7` → `8`.
- `models.rs`: `FileShortcut` += `pub show_terminal: bool`.
- `types.ts`: `FileShortcut` += `showTerminal: boolean`.
- `files.rs` `row_to_file`: SELECT добавить `show_terminal` (индекс 5, после
  `sort_order`=4); конструктор `show_terminal: r.get::<_, i64>(5)? != 0`.
  `files_create`/`files_update` НЕ трогаем (флаг ставится дефолтом столбца при
  вставке; правка label/path его не меняет).

## 2. Backend — команды

### `open_git_bash(path)` (в actions.rs)

Резолвит директорию и запускает Git Bash в ней.

Чистый хелпер (тестируемый без ФС):
```rust
/// Директория для консоли: сама папка, либо родитель файла.
fn dir_of(p: &str, is_dir: bool) -> Option<String> {
    if is_dir {
        Some(p.to_string())
    } else {
        std::path::Path::new(p)
            .parent()
            .map(|x| x.to_string_lossy().into_owned())
            .filter(|s| !s.is_empty())
    }
}
```

Кандидаты git-bash:
```rust
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
```

Общий фолбэк-терминал (вынести из `open_terminal`, чтобы переиспользовать):
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
```
`open_terminal` переписать так, чтобы использовать `spawn_terminal_at` (после
`require_dir`), — поведение не меняется.

Команда:
```rust
#[tauri::command]
pub fn open_git_bash(path: String) -> AppResult<()> {
    let p = require_dir_or_file(&path)?;
    let is_dir = Path::new(&p).is_dir();
    let dir = dir_of(&p, is_dir)
        .ok_or_else(|| AppError { kind: ErrorKind::NotFound, message: "Не удалось определить папку".into() })?;
    for exe in git_bash_candidates() {
        if Command::new(&exe).arg(format!("--cd={}", dir)).spawn().is_ok() {
            return Ok(());
        }
    }
    spawn_terminal_at(&dir) // фолбэк, если Git Bash не найден
}
```
argv без shell; путь — отдельный аргумент. Регистрация в `generate_handler!`.

### `files_set_terminal(id, show_terminal)` (в files.rs)

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
Регистрация в `generate_handler!`.

## 3. Frontend — Overview

В блоке «Файлы и папки» ([OverviewTab.svelte](../../../src/lib/components/OverviewTab.svelte),
строка ~175), справа в `.link-row` (перед `.ext`-стрелкой) добавить кнопку,
показываемую только при `f.showTerminal`:
```svelte
{#if f.showTerminal}
  <button class="lrow-act" title="Открыть консоль (Git Bash) здесь"
          onclick={(e) => { e.stopPropagation(); actions.openGitBash(f.path); }}>
    <Icon name="square-terminal" class="ic-sm" />
  </button>
{/if}
```
`stopPropagation` — чтобы не срабатывал переход по ярлыку (клик строки =
`openShortcut`). Класс кнопки — переиспользовать существующий стиль иконки-кнопки
в строке (как `.play` в OverviewTab) либо добавить компактный `.lrow-act`.

## 4. Frontend — Settings

В блоке «Файлы и папки» ([SettingsTab.svelte](../../../src/lib/components/SettingsTab.svelte),
строка ~239) в каждой `.link-row` перед кнопкой удаления добавить чекбокс:
```svelte
<label class="file-term" title="Показывать кнопку консоли на «Обзоре»">
  <input type="checkbox" checked={f.showTerminal} onchange={() => toggleTerminal(f)} />
  консоль
</label>
```
Обработчик:
```ts
async function toggleTerminal(f: FileShortcut) {
  await filesApi.setTerminal(f.id, !f.showTerminal);
  await loadExtras();
}
```

## 5. API

- `files.ts`: `setTerminal = (id, showTerminal) => call<FileShortcut>("files_set_terminal", { id, showTerminal })`.
- `actions.ts`: `openGitBash = (path) => call<void>("open_git_bash", { path })`.

## 6. Тестирование

- **Rust**:
  - миграция доходит до v8 (обе version-assertion правки) — существующий тест.
  - `dir_of`: `dir_of("C:\\a\\b", true)` → `Some("C:\\a\\b")`; `dir_of("C:\\a\\f.txt", false)`
    → `Some("C:\\a")`; `dir_of("f.txt", false)` → `None`.
  - Сам запуск Git Bash/терминала — вручную.
- **Фронт**: рендер-тестов компонента нет — кнопка и чекбокс проверяются вручную.

## 7. Не-цели

- Экспорт/импорт флага `show_terminal` (`transfer.rs`) — вне scope; при импорте
  колонка примет дефолт (1). Приемлемо.

## Затрагиваемые файлы

Backend: `src-tauri/migrations/0008_files_show_terminal.sql` (новый),
`src-tauri/src/db/migrations.rs`, `src-tauri/src/models.rs`,
`src-tauri/src/commands/files.rs` (row_to_file + files_set_terminal),
`src-tauri/src/commands/actions.rs` (dir_of/git_bash_candidates/spawn_terminal_at/open_git_bash),
`src-tauri/src/lib.rs`.
Frontend: `src/lib/types.ts`, `src/lib/api/files.ts`, `src/lib/api/actions.ts`,
`src/lib/components/OverviewTab.svelte`, `src/lib/components/SettingsTab.svelte`,
`src/lib/styles/global.css` (стиль кнопки/чекбокса, если нужен).
