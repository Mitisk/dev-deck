# Git changes — пилюля с раскрывающимся попапом файлов

Дата: 2026-06-10
Статус: согласовано, к реализации

## Проблема

В шапке проекта git-бар показывает `{st.dirty} изм.` (например, «6 изм.»
[GitBar.svelte:82](../../../src/lib/components/GitBar.svelte#L82)) — голое число
без понимания, *что именно* изменилось: какие файлы, насколько, что уже в индексе.
Цель — заменить непрозрачный счётчик на читаемую, действенную сводку.

## Решение (обзор)

Компактная **пилюля** с разбивкой изменений по категориям (цветами), которая по
клику раскрывает **попап** со списком изменённых файлов и diffstat `+X −Y`.
Строка файла кликабельна — открывает файл в редакторе (VS Code).

## 1. Backend (Rust)

### Команда `git_changes`

Новая команда в [git.rs](../../../src-tauri/src/commands/git.rs) рядом с `git_status`.
Переиспользует открытый `git2::Repository` (без внешних процессов).

```rust
// models.rs
struct GitChanges {
    insertions: usize,   // суммарно +строк (staged + unstaged)
    deletions: usize,    // суммарно −строк
    files: Vec<GitFile>, // сортировка: staged → modified → untracked → deleted
}
struct GitFile {
    path: String,        // относительный путь от корня репозитория
    code: String,        // "M" | "A" | "D" | "R" | "?" | "T" (рабочее дерево приоритетнее индекса)
    staged: bool,        // присутствует в индексе
}
```

- Сигнатура: `git_changes(repo_path: String) -> AppResult<Option<GitChanges>>`.
  `None` — путь пуст или не git-репозиторий (как у `git_status`).
- `insertions`/`deletions`: из `repo.diff_tree_to_workdir_with_index(<HEAD tree>, ...)`
  → `.stats()` → `insertions()` / `deletions()`. Это «всё, что не закоммичено»,
  одним числом. Если HEAD нет (пустой репозиторий) — diff против пустого дерева.
- `files`: из того же `repo.statuses()` с `include_untracked(true)`, что и
  `status_of`. Маппинг `Status`-флагов в букву (приоритет рабочего дерева):
  `WT_NEW`→`?`, `WT_DELETED`/`INDEX_DELETED`→`D`, `WT_RENAMED`/`INDEX_RENAMED`→`R`,
  `WT_TYPECHANGE`/`INDEX_TYPECHANGE`→`T`, `INDEX_NEW`→`A`, иначе→`M`.
  `staged = s.intersects(INDEX_*)` (те же флаги, что в `status_of`).
  Игнорируем `IGNORED` (как `status_of`).
- Путь: `e.path()` (libgit2 отдаёт относительный POSIX-путь от корня репозитория).

### Команда `open_file_in_editor`

Новая команда в [actions.rs](../../../src-tauri/src/commands/actions.rs). Существующий
`open_in_editor` валидирует путь как **папку** (`require_dir`), поэтому открыть
одиночный файл им нельзя.

- Сигнатура: `open_file_in_editor(path: String) -> AppResult<()>`.
- Валидация: переиспользуем существующий `require_dir_or_file` (проверяет, что путь
  не пуст и существует) — открыть нужно конкретный файл, но допускать и папку
  безопасно (VS Code открывает оба).
- Запуск: `code.cmd <file>` напрямую; фолбэк `cmd /C code <file>` — тем же
  паттерном, что `open_in_editor` (без shell-инъекции, путь — отдельный argv).

### Регистрация

Обе команды добавить в `tauri::generate_handler![...]` в
[lib.rs](../../../src-tauri/src/lib.rs) (рядом с `commands::git::git_status` и др.).

## 2. Frontend

### API и типы

- [types.ts](../../../src/lib/types.ts): `GitChanges`, `GitFile` (зеркало Rust,
  camelCase через serde-rename, как у `GitStatus`).
- [git.ts](../../../src/lib/api/git.ts): `changes = (repoPath) => call<GitChanges | null>("git_changes", { repoPath })`.
- [actions.ts](../../../src/lib/api/actions.ts): `openFileInEditor = (path) => call<void>("open_file_in_editor", { path })`.

### Компонент `GitChangesPill.svelte`

Новый файл (выносим из `GitBar.svelte` — он уже плотный, а попап со своим
состоянием/обработчиками — самостоятельный юнит).

Props: `repoPath: string`, `dirty: number`, `staged: number`, `untracked: number`.

Производные категории (из props, мгновенно, без запроса):
- `modified = dirty − staged − untracked` (изменённые, не в индексе, не новые);
  при отрицательном из-за пересечений — клампим в 0.
- `untracked`, `staged` — напрямую.

**Пилюля** (заменяет `{st.dirty} изм.` в [GitBar.svelte:82](../../../src/lib/components/GitBar.svelte#L82)):
показывает только ненулевые категории компактно: `● {modified}` (amber),
`＋ {untracked}` (blue), `✓ {staged}` (green), затем caret `▾`. Цвета — из
существующих CSS-переменных (`--git-dirty` и родственные).

**Попап** (раскрывается по клику на пилюлю):
- Детали грузятся **лениво** — `git.changes(repoPath)` при первом открытии.
  Кэш сбрасывается при изменении `repoPath` или `dirty` (после commit/pull
  счётчики меняются → перезагрузка).
- Шапка: `+{insertions}` (зелёный) `−{deletions}` (красный).
- Список файлов: буква `code` в цветном квадратике, путь моноширинный
  (каталог `dir/` приглушён, имя файла ярче), бейдж «индекс» для `staged`.
- **Строка кликабельна** → `actions.openFileInEditor(join(repoPath, file.path))`.
  Соединение пути — простая конкатенация через разделитель; backend раскрывает/
  валидирует.
- Закрытие: повторный клик по пилюле, `Escape`, клик вне попапа. Слушатели
  `window`/`document` гардить через `import { browser } from "$app/environment"`.

### Данные

Пилюля всегда отражает счётчики из уже загруженного `GitStatus` (мгновенно).
Diffstat и список файлов — по клику. `GitBar` передаёт `st.dirty/staged/untracked`
в проп; повторная загрузка деталей завязана на смену `dirty`.

## 3. Ошибки и edge-cases

- `git_changes` бросает → попап показывает «Не удалось прочитать изменения»
  (тост ошибки уже идёт из `client.ts`); пилюля остаётся видимой.
- Много файлов → попап с `max-height` и вертикальным скроллом. Показываем **все**
  файлы (без молчаливого обрезания).
- `dirty === 0` → пилюля не рендерится; ветка «✓ чисто» в `GitBar` как сейчас.
- Открытие файла, которого уже нет (удалён, `code D`) → backend вернёт ошибку
  валидации `require_file` → тост; не падаем.

## 4. Тестирование

- **Rust** ([git.rs](../../../src-tauri/src/commands/git.rs), `#[cfg(test)]`):
  `git_changes` во временном репо (по образцу существующих тестов) —
  модифицированный + новый (untracked) + staged файл. Проверяем: набор букв
  `code`, флаг `staged`, `insertions > 0`.
- **Фронт** (Vitest, появляется в Task 13): рендер `GitChangesPill` — ненулевые
  категории показаны по props; клик по строке файла дёргает `openFileInEditor`
  с ожидаемым путём.

## Затрагиваемые файлы

Backend: `src-tauri/src/commands/git.rs`, `src-tauri/src/commands/actions.rs`,
`src-tauri/src/models.rs`, регистрация команд в `src-tauri/src/lib.rs`.
Frontend: `src/lib/types.ts`, `src/lib/api/git.ts`, `src/lib/api/actions.ts`,
`src/lib/components/GitChangesPill.svelte` (новый), `src/lib/components/GitBar.svelte`,
`src/lib/styles/global.css` (стили пилюли/попапа).
