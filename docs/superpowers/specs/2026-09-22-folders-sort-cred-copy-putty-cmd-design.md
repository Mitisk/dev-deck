# Папки и сортировка проектов, копирование кредов, команда PuTTY, скрытие задач на Обзоре

Дата: 2026-09-22
Статус: согласовано, к реализации

Пять доработок, согласованных в диалоге. Реализуются по порядку 1 → 5 → 4 → 2+3,
каждая отдельным коммитом в одной ветке.

## 1. Скрывать «Ближайшие задачи», если фича задач выключена

Стор `features` ([features.ts](../../../src/lib/stores/features.ts)) уже прячет
вкладку «Задачи». На [OverviewTab.svelte](../../../src/lib/components/OverviewTab.svelte):

- секция «Ближайшие задачи» оборачивается в `{#if $features.tasks}`;
- в `load()` запросы `tasksApi.list` и `columnsApi.list` выполняются только при
  включённой фиче (иначе `tasks = []`, `columns = []`);
- `$effect` перезагружает данные при смене `project.id` **и** `$features.tasks`.

Дашборд не трогаем (решение пользователя).

## 5. Команда при запуске PuTTY

### Data model — миграция `0009_cred_startup_cmd.sql`

```sql
ALTER TABLE credentials ADD COLUMN startup_cmd TEXT;
PRAGMA user_version = 9;
```

`Credential` и `CredInput` (Rust и TS) += `startup_cmd` / `startupCmd: string | null`.
`row_to_cred`, `creds_create`, `creds_update` читают/пишут колонку.

### Backend — запуск

У PuTTY нет опции «выполнить команду и остаться в сессии». Используем `-t -m <file>`:
файл содержит `"{cmd}; exec $SHELL -l\n"`. Разделитель `;`, чтобы при ошибке
команды окно не закрылось. Хвост добавляется бэкендом; пользователь пишет только
`cd /home`.

```rust
/// Содержимое скрипта для `putty -m`: команда пользователя + интерактивная оболочка.
fn startup_script(cmd: &str) -> String  // "cd /home; exec $SHELL -l\n"
```

`build_putty_args(target, key_path, password, script_path: Option<&str>)` добавляет
`-t -m <path>` в конец, если `script_path` задан. Файл пишется в
`std::env::temp_dir()/devdeck-putty-<pid>-<nanos>.txt` и удаляется фоновым потоком
через 15 секунд (PuTTY читает файл при старте). Ошибка записи файла → `AppError`.

### Frontend

- В редакторе креда для типа `ssh` поле «Команда при запуске» (`placeholder="cd /var/www"`),
  подсказка: «Выполнится на сервере сразу после входа».
- В карточке креда строка `Команда` (mono) с кнопкой копирования, если задана.

## 4. Копирование креда в другой проект

### Backend — `creds_copy(id, target_project_id) -> Credential`

- Кред с `is_global = 1` → `Validation`: «Глобальный кред нельзя копировать».
- Целевой проект не найден → `NotFound`.
- `INSERT INTO credentials(project_id, label, type, username, url, secret_encrypted,
  notes, sort_order, key_path, startup_cmd, is_global) SELECT ?target, label, type, …,
  <next sort>, key_path, startup_cmd, 0 FROM credentials WHERE id = ?id`.
  Шифрование не привязано к id строки, поэтому blob копируется как есть.
- Копия — независимая запись; правки оригинала её не касаются.

### Frontend

- `creds.ts`: `copyTo = (id, targetProjectId) => call<Credential>("creds_copy", { id, targetProjectId })`.
- В `.cred-acts` карточки (только для `!c.isGlobal`) кнопка «Копировать в проект…»
  (иконка `copy-plus`). Открывает модалку со списком неархивных проектов, кроме текущего.
  Клик по проекту → `copyTo` → тост «Скопировано в <имя проекта>» → модалка закрывается.
  Если других проектов нет — модалка показывает «Нет других проектов».

## 2. Папки проектов (только сайдбар)

### Data model — миграция `0010_project_groups.sql`

```sql
CREATE TABLE project_groups (
    id         INTEGER PRIMARY KEY,
    name       TEXT NOT NULL,
    sort_order INTEGER DEFAULT 0
);
ALTER TABLE projects ADD COLUMN group_id INTEGER REFERENCES project_groups(id) ON DELETE SET NULL;
PRAGMA user_version = 10;
```

`Project` (Rust/TS) += `group_id` / `groupId: number | null`. Один уровень, без
иконок и цветов.

### Backend — `commands/groups.rs`

- `groups_list() -> Vec<ProjectGroup>` — по `sort_order, id`.
- `groups_create(name) -> ProjectGroup` — `sort_order = max+1`; пустое имя → `Validation`.
- `groups_rename(id, name) -> ProjectGroup`.
- `groups_delete(id)` — проекты уходят в корень (`ON DELETE SET NULL`).
- `groups_reorder(ids)` — `sort_order = index`.
- В `projects.rs`: `project_set_group(id, group_id: Option<i64>)`,
  `projects_reorder(ids)` — `sort_order = index` для переданных id (перестановка
  внутри одной секции: закреплённые, папка или корень).

### Frontend

- `types.ts`: `ProjectGroup = { id; name; sortOrder }`.
- `api/groups.ts`, стор `stores/groups.ts` (`groups`, `loadGroups`); `loadProjects`
  также грузит папки.
- Свёрнутость папок: `localStorage` `devdeck-collapsed-groups` (массив id).

### Sidebar

Кнопка «Новый проект» становится split-button: слева прежняя кнопка, справа узкая
кнопка с иконкой `folder-plus` другого оттенка. Клик → `groups_create("Новая папка")`
и папка сразу переходит в режим переименования (инлайн-инпут, Enter — сохранить,
Esc — отмена).

Структура списка при пустом поиске:

1. «Закреплённые» (как сейчас).
2. Папки по `sort_order`: заголовок папки (шеврон, имя, счётчик) — клик сворачивает;
   двойной клик по имени — переименование; кнопка `x` на hover — удаление с
   `ConfirmDialog` («Проекты останутся, папка удалится»).
3. «Все проекты» — проекты без папки.

Закреплённые не показываются внутри папок. Архивные не показываются нигде.
При поиске папки игнорируются — плоский список как сейчас.

## 3. Сортировка проектов перетаскиванием

HTML5 DnD по образцу [CredsTab.svelte](../../../src/lib/components/CredsTab.svelte)
и хелпера `reorderIds` ([credsOrder.ts](../../../src/lib/credsOrder.ts)).

- Проект перетаскивается за всю строку (`draggable`). Индикатор — линия сверху/снизу
  целевой строки (по `clientY` относительно середины).
- Бросок в пределах той же секции → `projects_reorder(ids секции)`.
- Бросок на заголовок папки или внутрь другой папки → `project_set_group(id, groupId)`,
  затем `projects_reorder` для целевой секции (проект встаёт на место броска или в конец).
- Бросок в «Все проекты» из папки → `project_set_group(id, null)`.
- Бросок в «Закреплённые» из другой секции — не поддерживается (закрепление — через
  настройки проекта); индикатор не показывается.
- Заголовки папок перетаскиваются друг относительно друга → `groups_reorder`.
- Оптимистичное обновление стора; при ошибке — `loadProjects()`.

Порядок на бэке остаётся `pinned DESC, sort_order, name`; сортировка по секциям
делается на фронте.

## Тестирование

- Rust: миграции доходят до v10; `startup_script`; `build_putty_args` с `-t -m`;
  `creds_copy` копирует поля и blob, ставит `is_global = 0`, отказывает глобальному;
  `groups_delete` обнуляет `group_id`; `projects_reorder`/`groups_reorder` выставляют
  индексы.
- Vitest: чистая функция раскладки сайдбара `groupProjects(projects, groups)` →
  `{ pinned, folders: [{group, items}], rest }`; повторное использование `reorderIds`.
- Ручная проверка: DnD в сайдбаре, PuTTY с командой, копирование креда.

## Не-цели

- Экспорт/импорт папок и `startup_cmd` в `transfer.rs`.
- Папки на Дашборде, вложенные папки, сортировка по имени/дате.
