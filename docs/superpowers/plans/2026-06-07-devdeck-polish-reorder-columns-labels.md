# DevDeck — Доработка «Реордер колонок и меток перетаскиванием» — план

> **Для исполнителя:** ОБЯЗАТЕЛЬНЫЙ ПОД-НАВЫК: superpowers:subagent-driven-development. Шаги — чекбоксы.

**Цель:** Менять порядок колонок канбана (перетаскивая заголовок колонки) и порядок меток (перетаскивая строку в модалке «Метки проекта»). Порядок сохраняется в `sort_order`.

**Архитектура:** Backend — две команды `columns_reorder(project_id, ids)` / `labels_reorder(project_id, ids)`: в транзакции выставляют `sort_order = индекс` по порядку id. Frontend — DnD заголовков колонок (отдельный флаг `colDragKey`, чтобы не конфликтовать с уже существующим перетаскиванием задач) и DnD строк меток в модалке.

**Стек/границы:** backend — `commands/columns.rs`, `commands/labels.rs` (+ по тесту), `lib.rs` (регистрация). Frontend — `api/columns.ts`, `api/labels.ts`, `TasksTab.svelte`. Без миграций.

---

## Task 1: Rust — команды реордера

**Files:** Modify `src-tauri/src/commands/columns.rs`, `src-tauri/src/commands/labels.rs`, `src-tauri/src/lib.rs`.

- [ ] **Step 1: `columns_reorder` в `columns.rs`** — добавить после `column_delete`:
```rust
/// Переставить колонки: выставить sort_order по порядку переданных id.
#[tauri::command]
pub fn columns_reorder(state: State<AppState>, project_id: i64, ids: Vec<i64>) -> AppResult<()> {
    let conn = lock(&state)?;
    let tx = conn.unchecked_transaction()?;
    for (i, id) in ids.iter().enumerate() {
        conn.execute(
            "UPDATE task_columns SET sort_order=?2 WHERE id=?1 AND project_id=?3",
            params![id, i as i64, project_id],
        )?;
    }
    tx.commit()?;
    Ok(())
}
```

- [ ] **Step 2: Тест в `columns.rs`** — добавить в конец файла модуль тестов:
```rust
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
    fn reorder_columns_sets_sort_order() {
        let conn = mem();
        conn.execute("INSERT INTO projects(name,status,sort_order) VALUES('P','active',0)", []).unwrap();
        let pid = conn.last_insert_rowid();
        seed_default_columns(&conn, pid).unwrap();
        let ids: Vec<i64> = {
            let mut s = conn.prepare("SELECT id FROM task_columns WHERE project_id=?1 ORDER BY sort_order").unwrap();
            let r = s.query_map([pid], |r| r.get::<_, i64>(0)).unwrap();
            r.map(|x| x.unwrap()).collect()
        };
        let rev: Vec<i64> = ids.iter().rev().cloned().collect();
        let tx = conn.unchecked_transaction().unwrap();
        for (i, id) in rev.iter().enumerate() {
            conn.execute("UPDATE task_columns SET sort_order=?2 WHERE id=?1 AND project_id=?3", params![id, i as i64, pid]).unwrap();
        }
        tx.commit().unwrap();
        let first: i64 = conn.query_row("SELECT id FROM task_columns WHERE project_id=?1 ORDER BY sort_order LIMIT 1", [pid], |r| r.get(0)).unwrap();
        assert_eq!(first, rev[0]);
    }
}
```

- [ ] **Step 3: `labels_reorder` в `labels.rs`** — добавить после `task_set_labels` (перед `#[cfg(test)]`):
```rust
/// Переставить метки: выставить sort_order по порядку переданных id.
#[tauri::command]
pub fn labels_reorder(state: State<AppState>, project_id: i64, ids: Vec<i64>) -> AppResult<()> {
    let conn = lock(&state)?;
    let tx = conn.unchecked_transaction()?;
    for (i, id) in ids.iter().enumerate() {
        conn.execute(
            "UPDATE labels SET sort_order=?2 WHERE id=?1 AND project_id=?3",
            params![id, i as i64, project_id],
        )?;
    }
    tx.commit()?;
    Ok(())
}
```

- [ ] **Step 4: Тест в `labels.rs`** — в существующий `mod tests` добавить:
```rust
    #[test]
    fn reorder_labels_sets_sort_order() {
        let conn = mem();
        conn.execute("INSERT INTO projects(name,status,sort_order) VALUES('P','active',0)", []).unwrap();
        let pid = conn.last_insert_rowid();
        conn.execute("INSERT INTO labels(project_id,name,color,sort_order) VALUES(?1,'a',NULL,0)", [pid]).unwrap();
        let a = conn.last_insert_rowid();
        conn.execute("INSERT INTO labels(project_id,name,color,sort_order) VALUES(?1,'b',NULL,1)", [pid]).unwrap();
        let b = conn.last_insert_rowid();
        // поменять местами: b, a
        let rev = [b, a];
        let tx = conn.unchecked_transaction().unwrap();
        for (i, id) in rev.iter().enumerate() {
            conn.execute("UPDATE labels SET sort_order=?2 WHERE id=?1 AND project_id=?3", params![id, i as i64, pid]).unwrap();
        }
        tx.commit().unwrap();
        let first: i64 = conn.query_row("SELECT id FROM labels WHERE project_id=?1 ORDER BY sort_order LIMIT 1", [pid], |r| r.get(0)).unwrap();
        assert_eq!(first, b);
    }
```

- [ ] **Step 5: Регистрация** — в `lib.rs` `generate_handler![...]` добавить рядом с прочими columns/labels:
```rust
            commands::columns::columns_reorder,
            commands::labels::labels_reorder,
```

- [ ] **Step 6: Тесты + сборка**
```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib
cargo build --manifest-path src-tauri/Cargo.toml --lib
```
Expected: 35 ok (33 + 2 новых); lib-сборка успешна.

- [ ] **Step 7: Commit**
```powershell
git add src-tauri/src/commands/columns.rs src-tauri/src/commands/labels.rs src-tauri/src/lib.rs
git commit -m @'
feat(backend): columns_reorder and labels_reorder

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

## Task 2: Фронт — DnD реордер

**Files:** Modify `src/lib/api/columns.ts`, `src/lib/api/labels.ts`, `src/lib/components/TasksTab.svelte`.

- [ ] **Step 1: api** —
В `src/lib/api/columns.ts` добавить:
```ts
export const reorder = (projectId: number, ids: number[]) => call<void>("columns_reorder", { projectId, ids });
```
В `src/lib/api/labels.ts` добавить:
```ts
export const reorder = (projectId: number, ids: number[]) => call<void>("labels_reorder", { projectId, ids });
```

- [ ] **Step 2: TasksTab — состояние и функции**

В `<script>` `TasksTab.svelte`:
1. Рядом с `let overCol` добавить:
```ts
  let colDragKey = $state<string | null>(null);
  let lblDragId = $state<number | null>(null);
```
2. В функции `onDrop(key)` — в самое начало (до `overCol = null;`) добавить ветку реордера колонок:
```ts
  async function onDrop(key: string) {
    if (colDragKey) {
      const from = colDragKey;
      colDragKey = null;
      overCol = null;
      if (from === key) return;
      const order = columns.map((c) => c.key);
      const fi = order.indexOf(from), ti = order.indexOf(key);
      if (fi < 0 || ti < 0) return;
      order.splice(ti, 0, ...order.splice(fi, 1));
      const ids = order
        .map((k) => columns.find((c) => c.key === k)?.id)
        .filter((x): x is number => x != null);
      await columnsApi.reorder(project.id, ids);
      await load();
      return;
    }
    overCol = null;
    // ↓ существующий код перемещения задачи без изменений
    const id = dragId;
    ...
  }
```
(остальное тело `onDrop` оставить как есть.)
3. Добавить функцию реордера меток (рядом с `deleteLabel`):
```ts
  async function dropLabel(targetId: number) {
    const from = lblDragId;
    lblDragId = null;
    if (from == null || from === targetId) return;
    const order = labels.map((l) => l.id);
    const fi = order.indexOf(from), ti = order.indexOf(targetId);
    if (fi < 0 || ti < 0) return;
    order.splice(ti, 0, ...order.splice(fi, 1));
    await labelsApi.reorder(project.id, order);
    await load();
  }
```

- [ ] **Step 3: TasksTab — заголовок колонки draggable**

1. У `.col` подавить подсветку drop при перетаскивании колонки. Заменить:
```svelte
         ondragover={(e) => { e.preventDefault(); overCol = c.key; }}
```
на:
```svelte
         ondragover={(e) => { e.preventDefault(); if (!colDragKey) overCol = c.key; }}
```
2. Сделать `.col-head` перетаскиваемым и добавить грип-иконку. Заменить блок:
```svelte
      <div class="col-head">
        <span class="led" style="background:{c.isDone ? '#3fb863' : 'var(--accent)'}"></span>
```
на:
```svelte
      <div class="col-head" draggable={true} style="cursor:grab" title="Перетащите, чтобы изменить порядок"
           ondragstart={() => { colDragKey = c.key; }}
           ondragend={() => { colDragKey = null; }}>
        <Icon name="grip-vertical" class="ic-sm" />
        <span class="led" style="background:{c.isDone ? '#3fb863' : 'var(--accent)'}"></span>
```

- [ ] **Step 4: TasksTab — строки меток draggable**

В модалке «Метки проекта» заменить строку метки:
```svelte
        {#each labels as l (l.id)}
          <div class="link-row" style="padding:6px 4px">
```
на:
```svelte
        {#each labels as l (l.id)}
          <div class="link-row" style="padding:6px 4px;cursor:grab" draggable={true}
               ondragstart={() => { lblDragId = l.id; }}
               ondragend={() => { lblDragId = null; }}
               ondragover={(e) => e.preventDefault()}
               ondrop={() => dropLabel(l.id)}>
```
(закрывающий `</div>` строки не трогать.)

- [ ] **Step 5: Проверка**
```powershell
npm run check
npm test
npm run build
```
Expected: 0 ошибок типов (`grip-vertical` — валидная lucide-иконка); Vitest 3 (3 passed); build успешен.

- [ ] **Step 6: Commit**
```powershell
git add src/lib/api/columns.ts src/lib/api/labels.ts src/lib/components/TasksTab.svelte
git commit -m @'
feat(frontend): drag-reorder kanban columns and labels

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

## Task 3: Проверка

- [ ] **Step 1: Автопроверка**
```powershell
npm run check ; npm test ; cargo test --manifest-path src-tauri/Cargo.toml --lib
```
Expected: типы 0; Vitest 3; Rust 35.

- [ ] **Step 2: GUI-смоук (пользователь)**
- [ ] Канбан: перетащить заголовок колонки на другую → порядок меняется и сохраняется (после перезагрузки тот же). Перетаскивание карточек задач между колонками по-прежнему работает.
- [ ] Модалка «Метки»: перетащить строку метки → порядок меняется; чипы-фильтры и порядок в edit-диалоге следуют новому порядку.

---

## Итог

Колонки и метки переставляются перетаскиванием, порядок сохраняется в `sort_order`. Следующая доработка — файловые диалоги для экспорта/импорта.
