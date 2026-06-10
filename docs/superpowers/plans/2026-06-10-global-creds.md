# Global (pinned) credentials — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Позволить «закрепить» кред глобально — он показывается во всех проектах как одна запись (правка/удаление везде) с собственной позицией в каждом проекте.

**Architecture:** Флаг `is_global` на credentials + таблица `cred_order(cred_id, project_id, sort_order)` для per-project позиций глобальных кредов. Каждый кред хранит «дом»-проект. DB-логика вынесена в чистые `&Connection`-хелперы (тестируемы без Tauri State). При удалении дом-проекта глобальные креды переселяются (выживают).

**Tech Stack:** Rust + rusqlite (SQLite UPSERT, FK CASCADE); SvelteKit (Svelte 5 runes) + TypeScript.

Спека: `docs/superpowers/specs/2026-06-10-global-creds-design.md`.

---

## File Structure

**Backend**
- `src-tauri/migrations/0007_global_creds.sql` — колонка + таблица.
- `src-tauri/src/db/migrations.rs` — регистрация + версия тестов.
- `src-tauri/src/models.rs` — `is_global` в `Credential`.
- `src-tauri/src/commands/creds.rs` — `is_global` в row_to_cred; хелперы `list_cred_ids`/`reorder_creds`/`set_cred_global` + команды + тесты.
- `src-tauri/src/commands/projects.rs` — `delete_project_in` (выживание) + тест.
- `src-tauri/src/lib.rs` — регистрация `creds_set_global`.

**Frontend**
- `src/lib/types.ts` — `Credential.isGlobal`.
- `src/lib/api/creds.ts` — `setGlobal`.
- `src/lib/components/CredsTab.svelte` — тумблер + бейдж.
- `src/lib/styles/global.css` — бейдж `.cred-global`.

---

## Task 1: Backend — миграция 0007

**Files:**
- Create: `src-tauri/migrations/0007_global_creds.sql`
- Modify: `src-tauri/src/db/migrations.rs`

- [ ] **Step 1: Создать файл миграции**

`src-tauri/migrations/0007_global_creds.sql`:

```sql
ALTER TABLE credentials ADD COLUMN is_global INTEGER NOT NULL DEFAULT 0;

CREATE TABLE cred_order (
    cred_id    INTEGER NOT NULL REFERENCES credentials(id) ON DELETE CASCADE,
    project_id INTEGER NOT NULL REFERENCES projects(id)    ON DELETE CASCADE,
    sort_order INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (cred_id, project_id)
);

PRAGMA user_version = 7;
```

- [ ] **Step 2: Зарегистрировать + обновить тесты версии**

В `src-tauri/src/db/migrations.rs`, в массиве `MIGRATIONS` после строки
`(6, include_str!("../../migrations/0006_cred_key_path.sql")),` добавить:
```rust
    (7, include_str!("../../migrations/0007_global_creds.sql")),
```
В тестах `applies_migrations_on_empty_db` и `run_is_idempotent` заменить
`assert_eq!(schema_version(&conn).unwrap(), 6);` → `7` (обе строки).

- [ ] **Step 3: Прогнать тесты миграций**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib migrations`
Expected: PASS (схема доходит до версии 7).

- [ ] **Step 4: Commit**

```bash
git add src-tauri/migrations/0007_global_creds.sql src-tauri/src/db/migrations.rs
git commit -m "feat(backend): migration 0007 — is_global + cred_order"
```

---

## Task 2: Backend — is_global в модели и row_to_cred

**Files:**
- Modify: `src-tauri/src/models.rs`
- Modify: `src-tauri/src/commands/creds.rs`

- [ ] **Step 1: Поле в модели**

В `src-tauri/src/models.rs`, в `Credential`, после `pub key_path: Option<String>,`
добавить:
```rust
    pub is_global: bool,
```

- [ ] **Step 2: row_to_cred**

В `src-tauri/src/commands/creds.rs`, в `row_to_cred`, заменить SELECT (добавить
`is_global` как столбец 10):
```rust
        "SELECT id, project_id, label, type, username, url, notes, sort_order,
                (secret_encrypted IS NOT NULL AND length(secret_encrypted) > 0),
                key_path, is_global
         FROM credentials WHERE id = ?1",
```
И в конструкторе, после `key_path: r.get(9)?,` добавить:
```rust
                is_global: r.get::<_, i64>(10)? != 0,
```

- [ ] **Step 3: Проверить компиляцию и тесты**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib`
Expected: всё компилируется и проходит.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/models.rs src-tauri/src/commands/creds.rs
git commit -m "feat(backend): is_global in Credential model + row_to_cred"
```

---

## Task 3: Backend — creds_list объединяет локальные и глобальные (TDD)

**Files:**
- Modify: `src-tauri/src/commands/creds.rs`

- [ ] **Step 1: Написать падающий тест**

В `#[cfg(test)] mod tests` добавить:
```rust
    #[test]
    fn list_cred_ids_merges_local_and_global() {
        let conn = mem();
        conn.execute("INSERT INTO projects(name,status,sort_order) VALUES('P1','active',0)", []).unwrap();
        let p1 = conn.last_insert_rowid();
        conn.execute("INSERT INTO projects(name,status,sort_order) VALUES('P2','active',0)", []).unwrap();
        let p2 = conn.last_insert_rowid();
        conn.execute("INSERT INTO credentials(project_id,label,type,sort_order,is_global) VALUES(?1,'A','note',0,0)", [p1]).unwrap();
        let a = conn.last_insert_rowid();
        conn.execute("INSERT INTO credentials(project_id,label,type,sort_order,is_global) VALUES(?1,'G','note',0,1)", [p1]).unwrap();
        let g = conn.last_insert_rowid();

        let p1_ids = list_cred_ids(&conn, p1).unwrap();
        assert!(p1_ids.contains(&a), "P1 должен видеть локальный A");
        assert!(p1_ids.contains(&g), "P1 должен видеть глобальный G");

        let p2_ids = list_cred_ids(&conn, p2).unwrap();
        assert_eq!(p2_ids, vec![g], "P2 видит только глобальный G");
    }
```

- [ ] **Step 2: Запустить — не компилируется**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib list_cred_ids_merges`
Expected: ошибка компиляции — `list_cred_ids` не найдена.

- [ ] **Step 3: Реализовать хелпер + переключить creds_list**

В `src-tauri/src/commands/creds.rs`, перед `#[tauri::command] pub fn creds_list`,
добавить хелпер:
```rust
/// id кредов проекта: локальные (project_id) + все глобальные. Порядок:
/// локальные — по credentials.sort_order; глобальные — по cred_order проекта,
/// без строки — в конец.
fn list_cred_ids(conn: &Connection, project_id: i64) -> AppResult<Vec<i64>> {
    let mut stmt = conn.prepare(
        "SELECT c.id
         FROM credentials c
         LEFT JOIN cred_order o ON o.cred_id = c.id AND o.project_id = ?1
         WHERE c.project_id = ?1 OR c.is_global = 1
         ORDER BY (CASE WHEN c.is_global = 1
                        THEN COALESCE(o.sort_order, 1000000000)
                        ELSE c.sort_order END) ASC,
                  c.id ASC",
    )?;
    let rows = stmt.query_map([project_id], |r| r.get::<_, i64>(0))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}
```
Заменить тело `creds_list`:
```rust
#[tauri::command]
pub fn creds_list(state: State<AppState>, project_id: i64) -> AppResult<Vec<Credential>> {
    let conn = lock(&state)?;
    let ids = list_cred_ids(&conn, project_id)?;
    let mut out = Vec::new();
    for id in ids {
        out.push(row_to_cred(&conn, id)?);
    }
    Ok(out)
}
```

- [ ] **Step 4: Запустить — проходит**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib list_cred_ids_merges`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/creds.rs
git commit -m "feat(backend): creds_list merges local + global creds"
```

---

## Task 4: Backend — creds_reorder ветвится по типу (TDD)

**Files:**
- Modify: `src-tauri/src/commands/creds.rs`

- [ ] **Step 1: Написать падающий тест**

```rust
    #[test]
    fn reorder_creds_routes_by_global_flag() {
        let conn = mem();
        conn.execute("INSERT INTO projects(name,status,sort_order) VALUES('P','active',0)", []).unwrap();
        let p = conn.last_insert_rowid();
        conn.execute("INSERT INTO credentials(project_id,label,type,sort_order,is_global) VALUES(?1,'L','note',0,0)", [p]).unwrap();
        let l = conn.last_insert_rowid();
        conn.execute("INSERT INTO credentials(project_id,label,type,sort_order,is_global) VALUES(?1,'G','note',0,1)", [p]).unwrap();
        let g = conn.last_insert_rowid();

        reorder_creds(&conn, p, &[g, l]).unwrap();

        // локальный l → credentials.sort_order = 1
        let l_order: i64 = conn.query_row("SELECT sort_order FROM credentials WHERE id=?1", [l], |r| r.get(0)).unwrap();
        assert_eq!(l_order, 1);
        // глобальный g → строка в cred_order = 0
        let g_pos: i64 = conn.query_row("SELECT sort_order FROM cred_order WHERE cred_id=?1 AND project_id=?2", params![g, p], |r| r.get(0)).unwrap();
        assert_eq!(g_pos, 0);
        // глобальный g НЕ трогает credentials.sort_order
        let g_cred: i64 = conn.query_row("SELECT sort_order FROM credentials WHERE id=?1", [g], |r| r.get(0)).unwrap();
        assert_eq!(g_cred, 0);
    }
```

- [ ] **Step 2: Запустить — не компилируется**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib reorder_creds_routes`
Expected: ошибка компиляции — `reorder_creds` не найдена.

- [ ] **Step 3: Реализовать хелпер + переключить creds_reorder**

Перед `#[tauri::command] pub fn creds_reorder` добавить:
```rust
/// Применить порядок: локальные пишут в credentials.sort_order, глобальные — в cred_order.
fn reorder_creds(conn: &Connection, project_id: i64, ids: &[i64]) -> AppResult<()> {
    for (i, id) in ids.iter().enumerate() {
        let is_global: bool = conn.query_row(
            "SELECT is_global FROM credentials WHERE id=?1",
            params![id],
            |r| r.get::<_, i64>(0),
        )? != 0;
        if is_global {
            conn.execute(
                "INSERT INTO cred_order(cred_id, project_id, sort_order) VALUES(?1,?2,?3)
                 ON CONFLICT(cred_id, project_id) DO UPDATE SET sort_order=excluded.sort_order",
                params![id, project_id, i as i64],
            )?;
        } else {
            conn.execute(
                "UPDATE credentials SET sort_order=?2 WHERE id=?1 AND project_id=?3",
                params![id, i as i64, project_id],
            )?;
        }
    }
    Ok(())
}
```
Заменить тело `creds_reorder`:
```rust
#[tauri::command]
pub fn creds_reorder(state: State<AppState>, project_id: i64, ids: Vec<i64>) -> AppResult<()> {
    let conn = lock(&state)?;
    let tx = conn.unchecked_transaction()?;
    reorder_creds(&conn, project_id, &ids)?;
    tx.commit()?;
    Ok(())
}
```

- [ ] **Step 4: Запустить — проходит**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib reorder_creds_routes`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/creds.rs
git commit -m "feat(backend): creds_reorder routes global creds to cred_order"
```

---

## Task 5: Backend — creds_set_global (TDD)

**Files:**
- Modify: `src-tauri/src/commands/creds.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Написать падающий тест**

```rust
    #[test]
    fn set_cred_global_toggles_and_cleans_order() {
        let conn = mem();
        conn.execute("INSERT INTO projects(name,status,sort_order) VALUES('P','active',0)", []).unwrap();
        let p = conn.last_insert_rowid();
        conn.execute("INSERT INTO credentials(project_id,label,type,sort_order,is_global) VALUES(?1,'G','note',0,1)", [p]).unwrap();
        let g = conn.last_insert_rowid();
        conn.execute("INSERT INTO cred_order(cred_id,project_id,sort_order) VALUES(?1,?2,5)", params![g, p]).unwrap();

        // открепить
        set_cred_global(&conn, g, false).unwrap();
        let isg: i64 = conn.query_row("SELECT is_global FROM credentials WHERE id=?1", [g], |r| r.get(0)).unwrap();
        assert_eq!(isg, 0);
        let cnt: i64 = conn.query_row("SELECT count(*) FROM cred_order WHERE cred_id=?1", [g], |r| r.get(0)).unwrap();
        assert_eq!(cnt, 0, "cred_order должен очиститься при откреплении");

        // закрепить обратно
        set_cred_global(&conn, g, true).unwrap();
        let isg2: i64 = conn.query_row("SELECT is_global FROM credentials WHERE id=?1", [g], |r| r.get(0)).unwrap();
        assert_eq!(isg2, 1);
    }
```

- [ ] **Step 2: Запустить — не компилируется**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib set_cred_global_toggles`
Expected: ошибка компиляции — `set_cred_global` не найдена.

- [ ] **Step 3: Реализовать хелпер + команду**

После функции `creds_reorder` добавить:
```rust
/// Поставить/снять флаг «глобальный». При снятии чистим позиции в cred_order.
fn set_cred_global(conn: &Connection, id: i64, is_global: bool) -> AppResult<()> {
    conn.execute(
        "UPDATE credentials SET is_global=?2 WHERE id=?1",
        params![id, if is_global { 1i64 } else { 0 }],
    )?;
    if !is_global {
        conn.execute("DELETE FROM cred_order WHERE cred_id=?1", params![id])?;
    }
    Ok(())
}

/// Закрепить/открепить кред (показывать во всех проектах). Возвращает обновлённый кред.
#[tauri::command]
pub fn creds_set_global(state: State<AppState>, id: i64, is_global: bool) -> AppResult<Credential> {
    let conn = lock(&state)?;
    let tx = conn.unchecked_transaction()?;
    set_cred_global(&conn, id, is_global)?;
    tx.commit()?;
    row_to_cred(&conn, id)
}
```

- [ ] **Step 4: Зарегистрировать команду**

В `src-tauri/src/lib.rs`, после `commands::creds::launch_putty,` добавить:
```rust
            commands::creds::creds_set_global,
```

- [ ] **Step 5: Запустить — проходит**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib set_cred_global_toggles`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/commands/creds.rs src-tauri/src/lib.rs
git commit -m "feat(backend): creds_set_global command (pin/unpin)"
```

---

## Task 6: Backend — projects_delete: выживание глобальных (TDD)

**Files:**
- Modify: `src-tauri/src/commands/projects.rs`

- [ ] **Step 1: Написать падающий тест**

В `#[cfg(test)] mod tests` в `projects.rs` добавить:
```rust
    #[test]
    fn delete_project_reassigns_global_creds() {
        let conn = mem();
        conn.execute("INSERT INTO projects(name,status,sort_order) VALUES('P1','active',0)", []).unwrap();
        let p1 = conn.last_insert_rowid();
        conn.execute("INSERT INTO projects(name,status,sort_order) VALUES('P2','active',0)", []).unwrap();
        let p2 = conn.last_insert_rowid();
        conn.execute("INSERT INTO credentials(project_id,label,type,sort_order,is_global) VALUES(?1,'G','note',0,1)", [p1]).unwrap();
        let g = conn.last_insert_rowid();
        conn.execute("INSERT INTO credentials(project_id,label,type,sort_order,is_global) VALUES(?1,'L','note',0,0)", [p1]).unwrap();
        let l = conn.last_insert_rowid();

        delete_project_in(&conn, p1).unwrap();

        let pcount: i64 = conn.query_row("SELECT count(*) FROM projects WHERE id=?1", [p1], |r| r.get(0)).unwrap();
        assert_eq!(pcount, 0, "P1 удалён");
        let g_home: i64 = conn.query_row("SELECT project_id FROM credentials WHERE id=?1", [g], |r| r.get(0)).unwrap();
        assert_eq!(g_home, p2, "глобальный кред переехал в P2");
        let lcount: i64 = conn.query_row("SELECT count(*) FROM credentials WHERE id=?1", [l], |r| r.get(0)).unwrap();
        assert_eq!(lcount, 0, "локальный кред удалён с проектом");
    }
```

- [ ] **Step 2: Запустить — не компилируется**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib delete_project_reassigns`
Expected: ошибка компиляции — `delete_project_in` не найдена.

- [ ] **Step 3: Реализовать хелпер + переключить projects_delete**

В `src-tauri/src/commands/projects.rs`, перед `#[tauri::command] pub fn projects_delete`
добавить:
```rust
/// Удалить проект. Глобальные креды этого проекта переселяем в другой проект,
/// чтобы они пережили удаление (если других проектов нет — project_id станет NULL).
fn delete_project_in(conn: &Connection, id: i64) -> AppResult<()> {
    conn.execute(
        "UPDATE credentials
         SET project_id = (SELECT MIN(id) FROM projects WHERE id <> ?1)
         WHERE project_id = ?1 AND is_global = 1",
        [id],
    )?;
    conn.execute("DELETE FROM projects WHERE id = ?1", [id])?;
    Ok(())
}
```
Заменить тело `projects_delete`:
```rust
#[tauri::command]
pub fn projects_delete(state: State<AppState>, id: i64) -> AppResult<()> {
    let conn = lock(&state)?;
    let tx = conn.unchecked_transaction()?;
    delete_project_in(&conn, id)?;
    tx.commit()?;
    Ok(())
}
```

- [ ] **Step 4: Запустить — проходит**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib delete_project_reassigns`
Expected: PASS.

- [ ] **Step 5: Полный прогон Rust**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib`
Expected: все тесты PASS.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/commands/projects.rs
git commit -m "feat(backend): global creds survive home-project deletion"
```

---

## Task 7: Frontend — тип и API

**Files:**
- Modify: `src/lib/types.ts`
- Modify: `src/lib/api/creds.ts`

- [ ] **Step 1: Тип**

В `src/lib/types.ts`, в `Credential`, после `keyPath: string | null;` добавить:
```ts
  isGlobal: boolean;
```

- [ ] **Step 2: API**

`src/lib/api/creds.ts` УЖЕ импортирует `Credential` (строка 2:
`import type { Credential, CredType } from "../types";`) — НЕ добавлять import повторно.
После строки `export const reorder = ...` добавить только:
```ts
export const setGlobal = (id: number, isGlobal: boolean) =>
  call<Credential>("creds_set_global", { id, isGlobal });
```

- [ ] **Step 3: Проверить типы**

Run: `npm run check`
Expected: 0 ошибок. (Добавление `isGlobal` к `Credential` может вызвать ошибку у
литерала `editing = {...}` в `openNew` в CredsTab — она чинится в Task 8; если
проверка падает только из-за этого литерала, продолжайте к Task 8 и проверьте там.)

- [ ] **Step 4: Commit**

```bash
git add src/lib/types.ts src/lib/api/creds.ts
git commit -m "feat(frontend): isGlobal type + setGlobal api"
```

---

## Task 8: Frontend — тумблер закрепления + бейдж

**Files:**
- Modify: `src/lib/components/CredsTab.svelte`

- [ ] **Step 1: Состояние + обработчик**

В `<script>`, рядом с состоянием формы (где `fKeyPath`/`genLen`) добавить:
```ts
  let fGlobal = $state(false);
  async function toggleGlobal() {
    if (!editing || isNew) return;
    const next = !fGlobal;
    await creds.setGlobal(editing.id, next);
    fGlobal = next;
    await load();
  }
```

- [ ] **Step 2: Заполнение fGlobal в openNew/openEdit**

В `openNew`, рядом со сбросом полей (`... fKeyPath = "";`) добавить `fGlobal = false;`
в той же строке/рядом:
```ts
    fLabel = ""; fType = "login"; fUser = ""; fUrl = ""; fNotes = ""; fSecret = ""; fKeyPath = ""; fGlobal = false;
```
В `openEdit`, рядом с заполнением (`... fKeyPath = c.keyPath ?? "";`) добавить:
```ts
    fLabel = c.label; fType = c.type; fUser = c.username ?? ""; fUrl = c.url ?? ""; fNotes = c.notes ?? ""; fSecret = ""; fKeyPath = c.keyPath ?? ""; fGlobal = c.isGlobal;
```

- [ ] **Step 3: Поправить литерал openNew (новый кред)**

В `openNew`, в литерале `editing = {...}`, добавить `isGlobal: false` (после
`sortOrder: 0,` или рядом с `hasSecret: false`):
```ts
    editing = { id: 0, projectId: project.id, label: "", type: "login", username: null, url: null, notes: null, sortOrder: 0, hasSecret: false, keyPath: null, isGlobal: false };
```
(Если в литерале уже есть `keyPath: null` из прошлого среза — добавить только `isGlobal: false`.)

- [ ] **Step 4: Тумблер в модалке (только для существующего креда)**

В теле модалки, после поля «Заметка» (`<div class="field"><label for="c-notes">...`),
перед `</div>` закрытия `modal-body`, добавить:
```svelte
        {#if !isNew}
          <div class="field">
            <div class="toggle-row">
              <button class="toggle" class:on={fGlobal} onclick={toggleGlobal} aria-pressed={fGlobal} aria-label="Во всех проектах"></button>
              <div class="tl">Показывать во всех проектах<small>Одна и та же запись появится в каждом проекте; правка и удаление — везде.</small></div>
            </div>
          </div>
        {/if}
```

- [ ] **Step 5: Бейдж на карточке**

В `.cred-head`, после бейджа типа
```svelte
        <span class="badge-type" style="color:var(--accent);background:var(--accent-soft)">{typeLabel(c.type)}</span>
```
добавить:
```svelte
        {#if c.isGlobal}<span class="cred-global" title="Во всех проектах"><Icon name="pin" class="ic-sm" /></span>{/if}
```

- [ ] **Step 6: Проверить типы**

Run: `npm run check`
Expected: 0 ошибок (новых warning быть не должно).

- [ ] **Step 7: Commit**

```bash
git add src/lib/components/CredsTab.svelte
git commit -m "feat(frontend): global toggle in editor + badge on cred card"
```

---

## Task 9: Frontend — стиль бейджа

**Files:**
- Modify: `src/lib/styles/global.css`

- [ ] **Step 1: Добавить CSS**

В `src/lib/styles/global.css`, рядом с правилами `.cred-head` (после
`.cred-head .cred-acts { ... }`) добавить:
```css
.cred-global { display: inline-flex; align-items: center; color: var(--accent); }
.cred-global svg { width: 13px; height: 13px; }
```

- [ ] **Step 2: Проверить сборку**

Run: `npm run check`
Expected: 0 ошибок.

- [ ] **Step 3: Commit**

```bash
git add src/lib/styles/global.css
git commit -m "feat(frontend): style for global cred badge"
```

---

## Task 10: Полная верификация

- [ ] **Step 1: Rust-тесты**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib`
Expected: все PASS (миграция v7, merge/reorder/set_global/delete-survival).

- [ ] **Step 2: Фронт-тесты**

Run: `npm test`
Expected: все PASS (без новых; глобальные креды без юнит-тестов фронта).

- [ ] **Step 3: Проверка типов**

Run: `npm run check`
Expected: 0 ошибок.

- [ ] **Step 4: Ручная проверка**

Run: `npm run tauri dev`
- В проекте A открыть кред → включить «Показывать во всех проектах» → бейдж-пин появился.
- Переключиться в проект B → этот кред виден; перетащить его на другое место →
  позиция сохраняется именно для B (в A позиция своя).
- Отредактировать кред из B → изменения видны и в A. Удалить → исчезает везде.
- Открепить → кред остаётся только в своём «доме».
- Удалить дом-проект (если есть второй проект) → глобальный кред не пропал, виден
  в остальных.

- [ ] **Step 5: Финальный commit (если были правки)**

```bash
git add -A
git commit -m "fix: global creds polish after manual verification"
```

(Если правок нет — пропустить.)
