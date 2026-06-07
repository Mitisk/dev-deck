# DevDeck — Доработка «Экспорт/импорт колонок и меток» — план

> **Для исполнителя:** ОБЯЗАТЕЛЬНЫЙ ПОД-НАВЫК: superpowers:subagent-driven-development. Шаги — чекбоксы.

**Цель:** JSON-экспорт/импорт переносит кастомные колонки канбана и метки задач (раньше — только дефолтные колонки, метки терялись).

**Архитектура:** Расширить export-DTO в `transfer.rs`: проект += `columns` (key/name/isDone/sortOrder) и `labels` (name/color/sortOrder); задача += `labels` (имена). Импорт: вставлять колонки с их ключами (verbatim, чтобы `task.status` совпал), либо сидировать дефолты если колонок нет; вставлять метки и связывать задачи с ними по имени.

**Стек/границы:** только `commands/transfer.rs` + тест. Без миграций, без фронта.

**Источники:** `commands/transfer.rs` (build_export/import_doc), `commands/columns.rs` (seed_default_columns), схема (task_columns/labels/task_labels).

---

## Task 1: transfer.rs — колонки и метки в экспорте/импорте

**Files:** Modify `src-tauri/src/commands/transfer.rs`.

- [ ] **Step 1: Расширить DTO** — в `transfer.rs` добавить структуры и поля:
```rust
#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct ExportColumn { key: String, name: String, is_done: bool, sort_order: i64 }

#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct ExportLabel { name: String, color: Option<String>, sort_order: i64 }
```
В `ExportTask` добавить поле:
```rust
    #[serde(default)]
    labels: Vec<String>,
```
В `ExportProject` добавить поля:
```rust
    #[serde(default)]
    columns: Vec<ExportColumn>,
    #[serde(default)]
    labels: Vec<ExportLabel>,
```

- [ ] **Step 2: build_export — выгружать колонки/метки/метки-задач**

В `build_export`, в цикле по проектам:
1. Колонки и метки проекта:
```rust
        let columns = col(conn, "SELECT key,name,is_done,sort_order FROM task_columns WHERE project_id=?1 ORDER BY sort_order,id", id,
            |r| Ok(ExportColumn { key: r.get(0)?, name: r.get(1)?, is_done: r.get::<_, i64>(2)? != 0, sort_order: r.get(3)? }))?;
        let proj_labels = col(conn, "SELECT name,color,sort_order FROM labels WHERE project_id=?1 ORDER BY sort_order,id", id,
            |r| Ok(ExportLabel { name: r.get(0)?, color: r.get(1)?, sort_order: r.get(2)? }))?;
```
2. Заменить выгрузку задач так, чтобы у каждой задачи были её метки (по имени). Текущая строка `let tasks = col(conn, "SELECT title,description,status,priority,due_date,sort_order FROM tasks ...", id, |r| Ok(ExportTask{...}))?;` заменить на:
```rust
        let task_rows = col(conn, "SELECT id,title,description,status,priority,due_date,sort_order FROM tasks WHERE project_id=?1 ORDER BY sort_order,id", id,
            |r| Ok((r.get::<_, i64>(0)?, ExportTask {
                title: r.get(1)?, description: r.get(2)?, status: r.get(3)?, priority: r.get(4)?, due_date: r.get(5)?, sort_order: r.get(6)?, labels: Vec::new(),
            })))?;
        let mut tasks = Vec::new();
        for (tid, mut t) in task_rows {
            t.labels = col(conn, "SELECT l.name FROM labels l JOIN task_labels tl ON tl.label_id=l.id WHERE tl.task_id=?1", tid, |r| r.get::<_, String>(0))?;
            tasks.push(t);
        }
```
3. В конструировании `ExportProject { ... }` добавить поля `columns`, `labels: proj_labels` (имя локальной переменной меток проекта — `proj_labels`, чтобы не конфликтовать с полем `labels` структуры).

- [ ] **Step 3: import_doc — восстанавливать колонки/метки/связи**

В `import_doc`, для каждого проекта (после вставки проекта и получения `pid`):
1. Заменить безусловный `seed_default_columns` на условный + вставку экспортированных колонок:
```rust
        if p.columns.is_empty() {
            crate::commands::columns::seed_default_columns(conn, pid)?;
        } else {
            for c in &p.columns {
                conn.execute("INSERT OR IGNORE INTO task_columns(project_id,key,name,is_done,sort_order) VALUES(?1,?2,?3,?4,?5)",
                    params![pid, c.key, c.name, c.is_done as i64, c.sort_order])?;
            }
        }
```
2. Вставить метки проекта и собрать карту имя→id:
```rust
        let mut label_map: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
        for l in &p.labels {
            conn.execute("INSERT INTO labels(project_id,name,color,sort_order) VALUES(?1,?2,?3,?4)", params![pid, l.name, l.color, l.sort_order])?;
            label_map.insert(l.name.clone(), conn.last_insert_rowid());
        }
```
3. В цикле вставки задач — захватить id задачи и связать с метками по имени. Текущую вставку задачи дополнить:
```rust
        for t in &p.tasks {
            conn.execute("INSERT INTO tasks(project_id,title,description,status,priority,due_date,sort_order) VALUES(?1,?2,?3,?4,?5,?6,?7)",
                params![pid, t.title, t.description, t.status, t.priority, t.due_date, t.sort_order])?;
            let tid = conn.last_insert_rowid();
            for lname in &t.labels {
                if let Some(lid) = label_map.get(lname) {
                    conn.execute("INSERT OR IGNORE INTO task_labels(task_id,label_id) VALUES(?1,?2)", params![tid, lid])?;
                }
            }
        }
```
> Если текущий код задач уже использует `last_insert_rowid()` иначе — аккуратно интегрировать. Импортированные задачи сохраняют `status` (ключ колонки), который теперь совпадает с вставленными колонками.

- [ ] **Step 4: Обновить round-trip тест** — в `mod tests` `export_import_roundtrip_with_secret` добавить колонку, метку и помеченную задачу, проверить перенос. Дополнить существующий тест ПОСЛЕ вставки проекта/задачи в `src`:
```rust
        // кастомная колонка + метка + связь
        src.execute("INSERT INTO task_columns(project_id,key,name,is_done,sort_order) VALUES(?1,'review','Review',0,5)", [pid]).unwrap();
        src.execute("INSERT INTO labels(project_id,name,color,sort_order) VALUES(?1,'bug','#f00',0)", [pid]).unwrap();
        let lid = src.last_insert_rowid();
        // привязать к существующей задаче T1
        let t1: i64 = src.query_row("SELECT id FROM tasks WHERE title='T1'", [], |r| r.get(0)).unwrap();
        src.execute("INSERT INTO task_labels(task_id,label_id) VALUES(?1,?2)", params![t1, lid]).unwrap();
```
И в проверках ПОСЛЕ импорта в `dst` добавить:
```rust
        let cols: i64 = dst.query_row("SELECT count(*) FROM task_columns WHERE key='review'", [], |r| r.get(0)).unwrap();
        assert_eq!(cols, 1);
        let lbls: i64 = dst.query_row("SELECT count(*) FROM labels WHERE name='bug'", [], |r| r.get(0)).unwrap();
        assert_eq!(lbls, 1);
        let linked: i64 = dst.query_row("SELECT count(*) FROM task_labels", [], |r| r.get(0)).unwrap();
        assert_eq!(linked, 1);
```
> Тест `mem()` прогоняет миграции (включая 0003/0004), `params!` уже импортирован в тестовом модуле (если нет — добавить `use rusqlite::params;`).

- [ ] **Step 5: Тесты + lib-сборка**
```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib transfer
cargo test --manifest-path src-tauri/Cargo.toml --lib
cargo build --manifest-path src-tauri/Cargo.toml --lib
```
Expected: `export_import_roundtrip_with_secret` (расширенный) + `export_without_secrets_omits_them` + прочие → 32 ok; lib-сборка успешна.

- [ ] **Step 6: Commit**
```powershell
git add src-tauri/src/commands/transfer.rs
git commit -m @'
feat(backend): export/import custom columns and task labels

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

## Task 2: Проверка

- [ ] **Step 1: Автопроверка**
```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib ; npm run check
```
Expected: Rust 32; типы 0 (фронт не менялся).

- [ ] **Step 2: GUI-смоук (пользователь)** — экспортировать проект с кастомной колонкой и помеченными задачами → импортировать → колонки и метки на месте, задачи в правильных колонках и с метками.

---

## Итог

Экспорт/импорт переносит полную модель: проекты, задачи (со статусом-колонкой и метками), кастомные колонки, метки, чеклисты, заметки, ссылки, файлы, команды, креды. Следующая доработка из списка — фильтр по приоритету + смена мастер-пароля.
```
