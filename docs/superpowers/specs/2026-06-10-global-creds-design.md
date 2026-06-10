# Глобальные (закреплённые) креды

Дата: 2026-06-10
Статус: согласовано, к реализации
Sub-project 2 из 2 (первый — редактор кредов: ключ/PuTTY/пароль — уже сделан).

## Проблема / цель

Дать возможность «закрепить» кред так, чтобы он показывался во **всех** проектах,
оставаясь **одной записью** (правка/удаление меняет везде), и при этом в каждом
проекте имел **свою позицию** в списке.

## Архитектура (обзор)

Кред хранит исходный `project_id` как «дом». Флаг `is_global=1` означает «показывать
также во всех остальных проектах». Поскольку креды адресуются по `id`,
`creds_update`/`creds_delete` уже меняют единственную запись везде — нужно лишь,
чтобы список включал глобальные. Per-project порядок глобальных хранится в отдельной
таблице `cred_order`.

## 1. Модель данных — миграция `0007_global_creds.sql`

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

- `db/migrations.rs`: зарегистрировать `(7, include_str!("../../migrations/0007_global_creds.sql"))`
  в `MIGRATIONS`; в тестах `applies_migrations_on_empty_db` и `run_is_idempotent`
  обновить ожидаемую версию `6` → `7`.
- `models.rs`: `Credential` += `pub is_global: bool`.
- `types.ts`: `Credential` += `isGlobal: boolean`.

`cred_order` хранит позиции **только** для глобальных кредов. Локальные продолжают
использовать `credentials.sort_order`.

## 2. `creds_list(project_id)` — объединённый запрос

`row_to_cred` SELECT расширить полем `is_global` (индекс 10, после `key_path`=9):
```sql
SELECT id, project_id, label, type, username, url, notes, sort_order,
       (secret_encrypted IS NOT NULL AND length(secret_encrypted) > 0),
       key_path, is_global
FROM credentials WHERE id = ?1
```
конструктор: `is_global: r.get::<_, i64>(10)? != 0,`.

`creds_list` — один запрос, мёрж локальных и глобальных:
```sql
SELECT c.id
FROM credentials c
LEFT JOIN cred_order o ON o.cred_id = c.id AND o.project_id = ?1
WHERE c.project_id = ?1 OR c.is_global = 1
ORDER BY (CASE WHEN c.is_global = 1
               THEN COALESCE(o.sort_order, 1000000000)
               ELSE c.sort_order END) ASC,
         c.id ASC
```
- Локальный кред (`is_global=0`, `project_id=?1`) — упорядочен по `sort_order`.
- Глобальный кред — в каждом проекте, упорядочен по `cred_order` этого проекта, а без
  строки — в конец (`1000000000`). Глобальный кред «дома» (его `project_id=?1`) попадает
  один раз и тоже использует ветку `cred_order`.

## 3. `creds_reorder(project_id, ids)` — ветвление по типу

Для каждого `id` по индексу `i`: определить `is_global`, затем:
- глобальный → upsert позиции:
  ```sql
  INSERT INTO cred_order(cred_id, project_id, sort_order) VALUES(?1,?2,?3)
  ON CONFLICT(cred_id, project_id) DO UPDATE SET sort_order = excluded.sort_order
  ```
- локальный → как сейчас: `UPDATE credentials SET sort_order=?2 WHERE id=?1 AND project_id=?3`.

Всё в одной транзакции (`unchecked_transaction`/`commit`), как в текущем
`creds_reorder`. (DnD на фронте уже есть — меняется только бэкенд.)

## 4. `creds_set_global(id, is_global)` — закрепление

Новая команда (возвращает обновлённый `Credential`):
- **Закрепить** (`true`): `UPDATE credentials SET is_global=1 WHERE id=?1`. «Дом»
  (`project_id`) не трогаем. Кред появляется во всех проектах в конце списка (дальше
  его можно перетащить — это создаст строки `cred_order`).
- **Открепить** (`false`): `UPDATE credentials SET is_global=0 WHERE id=?1` +
  `DELETE FROM cred_order WHERE cred_id=?1` (чистим позиции). Кред снова локальный
  в своём «доме».

Транзакция; в конце `row_to_cred(&conn, id)`. Регистрация в `generate_handler!`.

## 5. Удаление проекта — выживание глобальных

В `projects_delete` **перед** `DELETE FROM projects` переселить глобальные креды
удаляемого проекта в другой проект (в одной транзакции):
```rust
conn.execute(
    "UPDATE credentials
     SET project_id = (SELECT MIN(id) FROM projects WHERE id <> ?1)
     WHERE project_id = ?1 AND is_global = 1",
    [id],
)?;
conn.execute("DELETE FROM projects WHERE id = ?1", [id])?;
```
- Если других проектов нет → подзапрос вернёт NULL → `project_id` станет NULL
  (кред-сирота переживёт и всплывёт при создании нового проекта). Приемлемый
  крайний случай.
- `cred_order`-строки удаляемого проекта уходят по FK `ON DELETE CASCADE`.
- Локальные креды удаляемого проекта по-прежнему удаляются вместе с ним
  (FK CASCADE по `credentials.project_id`).

## 6. Что НЕ ломается / вне scope

- **Search** (`search.rs`): `JOIN projects p ON p.id=c.project_id` — у глобального
  креда «дом» существует, поэтому он остаётся в поиске (один раз). Не трогаем.
- **Перешифровка секретов** (`security.rs`): фильтрует по `secret_encrypted`, не по
  проекту — глобальные секреты перешифровываются корректно. Не трогаем.
- **Экспорт/импорт** (`transfer.rs`): экспорт пер-проектный; флаг `is_global` и
  таблицу `cred_order` НЕ экспортируем (вне scope, как и `key_path`). Глобальный кред
  при экспорте «дома» уедет как обычный кред — приемлемо.

## 7. Frontend

- `src/lib/api/creds.ts`: `setGlobal = (id, isGlobal) => call<Credential>("creds_set_global", { id, isGlobal })`.
- **Редактор** (CredsTab): только для существующего креда (`!isNew`) — тумблер
  «Показывать во всех проектах» (стиль `.toggle`, как в AppSettings). Состояние
  `fGlobal = $state(false)`, выставляется в `openEdit` из `c.isGlobal`. По клику:
  `await creds.setGlobal(editing.id, next); fGlobal = next; await load();`.
- **Карточка**: бейдж-пин у глобального креда — в `.cred-head` рядом с типом:
  `{#if c.isGlobal}<span class="cred-global" title="Во всех проектах"><Icon name="pin" class="ic-sm" /></span>{/if}`.

## 8. Тестирование

- **Rust** (`creds.rs`/`projects.rs` `#[cfg(test)]`):
  - `creds_list` мёрж: проект P1 (локальный A + глобальный G дома P1), проект P2
    (видит только G). Порядок: G без `cred_order` → в конец.
  - `creds_reorder`: глобальный id пишет в `cred_order` (а не в `credentials.sort_order`);
    локальный — в `credentials.sort_order`.
  - `creds_set_global`: pin ставит `is_global=1`; unpin ставит `0` и удаляет
    `cred_order`-строки креда.
  - `projects_delete`: глобальный кред «дома» P1 при удалении P1 переезжает в P2
    (выживает); локальный — удаляется.
- **Фронт**: рендер-тестов компонента нет (нет инфраструктуры) — тумблер и бейдж
  проверяются вручную.

## Затрагиваемые файлы

Backend: `src-tauri/migrations/0007_global_creds.sql` (новый),
`src-tauri/src/db/migrations.rs`, `src-tauri/src/models.rs`,
`src-tauri/src/commands/creds.rs` (list/reorder/set_global/row_to_cred + тесты),
`src-tauri/src/commands/projects.rs` (delete + тест), `src-tauri/src/lib.rs`.
Frontend: `src/lib/types.ts`, `src/lib/api/creds.ts`,
`src/lib/components/CredsTab.svelte`, `src/lib/styles/global.css` (бейдж/тумблер).
