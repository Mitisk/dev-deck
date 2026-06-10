# Creds drag-reorder — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Дать возможность менять порядок кредов проекта перетаскиванием за грип-ручку; новый порядок сохраняется в `sort_order`.

**Architecture:** Бэкенд получает команду `creds_reorder` (калька с `labels_reorder`) — `UPDATE credentials SET sort_order` по индексу в транзакции. Фронтенд: чистый хелпер `reorderIds` (тестируемый) + нативный HTML5 drag-and-drop в `CredsTab` поверх грид-раскладки `.creds` (детекция левая/правая половина карточки, индикатор — подсветка края).

**Tech Stack:** Rust + rusqlite (Tauri command); SvelteKit (Svelte 5 runes) + TypeScript; Vitest (node-env, pure-logic).

Спека: `docs/superpowers/specs/2026-06-10-creds-drag-reorder-design.md`.

---

## File Structure

**Backend**
- `src-tauri/src/commands/creds.rs` — +`#[tauri::command] creds_reorder` + тест.
- `src-tauri/src/lib.rs` — регистрация команды.

**Frontend**
- `src/lib/credsOrder.ts` — новый чистый хелпер `reorderIds`.
- `src/tests/credsOrder.test.ts` — новый тест хелпера.
- `src/lib/api/creds.ts` — +`reorder()`.
- `src/lib/components/CredsTab.svelte` — грип-ручка + DnD.
- `src/lib/styles/global.css` — `.cred-grip`, `.cred.drop-before`, `.cred.drop-after`.

---

## Task 1: Backend — creds_reorder (TDD)

**Files:**
- Modify: `src-tauri/src/commands/creds.rs` (команда + тест в существующем `#[cfg(test)] mod tests`)
- Modify: `src-tauri/src/lib.rs` (регистрация)

- [ ] **Step 1: Написать падающий тест**

В `src-tauri/src/commands/creds.rs`, внутри `mod tests` (после теста
`create_stores_encrypted_and_get_secret_decrypts`, перед закрывающей `}` модуля),
добавить. Тест повторяет SQL-логику команды на реальной мигрированной схеме
(по образцу `reorder_labels_sets_sort_order` в labels.rs — команда требует `State`,
поэтому в юните проверяется тот же SQL):

```rust
    #[test]
    fn creds_reorder_sets_sort_order() {
        let conn = mem();
        conn.execute("INSERT INTO projects(name,status,sort_order) VALUES('P','active',0)", []).unwrap();
        let pid = conn.last_insert_rowid();

        let blob = crypto::encrypt(b"x").unwrap();
        conn.execute(
            "INSERT INTO credentials(project_id,label,type,secret_encrypted,sort_order) VALUES(?1,'a','note',?2,0)",
            params![pid, blob],
        ).unwrap();
        let a = conn.last_insert_rowid();

        let blob2 = crypto::encrypt(b"y").unwrap();
        conn.execute(
            "INSERT INTO credentials(project_id,label,type,secret_encrypted,sort_order) VALUES(?1,'b','note',?2,1)",
            params![pid, blob2],
        ).unwrap();
        let b = conn.last_insert_rowid();

        // переставляем в порядок [b, a] той же SQL-логикой, что и команда
        let ids = vec![b, a];
        let tx = conn.unchecked_transaction().unwrap();
        for (i, id) in ids.iter().enumerate() {
            conn.execute(
                "UPDATE credentials SET sort_order=?2 WHERE id=?1 AND project_id=?3",
                params![id, i as i64, pid],
            ).unwrap();
        }
        tx.commit().unwrap();

        let order: Vec<i64> = {
            let mut stmt = conn.prepare("SELECT id FROM credentials WHERE project_id=?1 ORDER BY sort_order ASC, id ASC").unwrap();
            let rows = stmt.query_map([pid], |r| r.get(0)).unwrap();
            rows.map(|r| r.unwrap()).collect()
        };
        assert_eq!(order, vec![b, a]);
    }
```

- [ ] **Step 2: Запустить тест — убедиться, что компилируется и проходит SQL-часть**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib creds_reorder_sets_sort_order`
Expected: PASS (тест самодостаточен — проверяет SQL на реальной схеме; команду добавим следующим шагом).

- [ ] **Step 3: Реализовать команду**

В `src-tauri/src/commands/creds.rs`, после функции `creds_delete` (заканчивается
на строке ~125, перед `#[cfg(test)]`), добавить:

```rust
/// Переставить креды: выставить sort_order по порядку переданных id.
#[tauri::command]
pub fn creds_reorder(state: State<AppState>, project_id: i64, ids: Vec<i64>) -> AppResult<()> {
    let conn = lock(&state)?;
    let tx = conn.unchecked_transaction()?;
    for (i, id) in ids.iter().enumerate() {
        conn.execute(
            "UPDATE credentials SET sort_order=?2 WHERE id=?1 AND project_id=?3",
            params![id, i as i64, project_id],
        )?;
    }
    tx.commit()?;
    Ok(())
}
```

- [ ] **Step 4: Зарегистрировать команду**

В `src-tauri/src/lib.rs`, в блоке `tauri::generate_handler![...]`, после строки
`commands::creds::creds_delete,` (строка ~154) добавить:

```rust
            commands::creds::creds_reorder,
```

- [ ] **Step 5: Полный прогон Rust-тестов**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib`
Expected: все тесты PASS, включая `creds_reorder_sets_sort_order`. Сборка проходит
(команда `creds_reorder` компилируется и зарегистрирована).

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/commands/creds.rs src-tauri/src/lib.rs
git commit -m "feat(backend): creds_reorder command"
```

---

## Task 2: Frontend — reorderIds helper (TDD)

**Files:**
- Create: `src/lib/credsOrder.ts`
- Create: `src/tests/credsOrder.test.ts`

- [ ] **Step 1: Написать падающий тест**

Создать `src/tests/credsOrder.test.ts`:

```ts
import { describe, it, expect } from "vitest";
import { reorderIds } from "../lib/credsOrder";

describe("reorderIds", () => {
  it("перемещает элемент перед указанным (вверх)", () => {
    expect(reorderIds([1, 2, 3], 3, 1)).toEqual([3, 1, 2]);
  });

  it("перемещает элемент перед указанным (вниз)", () => {
    expect(reorderIds([1, 2, 3], 1, 3)).toEqual([2, 1, 3]);
  });

  it("beforeId === null → в конец", () => {
    expect(reorderIds([1, 2, 3], 1, null)).toEqual([2, 3, 1]);
  });

  it("бросок на самого себя не меняет порядок", () => {
    expect(reorderIds([1, 2, 3], 2, 2)).toEqual([1, 2, 3]);
  });

  it("несуществующий beforeId → в конец", () => {
    expect(reorderIds([1, 2, 3], 1, 99)).toEqual([2, 3, 1]);
  });
});
```

- [ ] **Step 2: Запустить тест — убедиться, что падает**

Run: `npm test -- credsOrder`
Expected: FAIL — модуль `../lib/credsOrder` не найден.

- [ ] **Step 3: Реализовать хелпер**

Создать `src/lib/credsOrder.ts`. Обрати внимание на ранний выход
`beforeId === dragId` — без него тест «бросок на себя» (`reorderIds([1,2,3], 2, 2)`)
вернул бы `[1,3,2]`, т.к. `indexOf` исключённого элемента даёт −1 → в конец:

```ts
// Переместить элемент dragId так, чтобы он встал ПЕРЕД beforeId
// (beforeId === null или не найден → в конец). Возвращает новый массив id.
export function reorderIds(ids: number[], dragId: number, beforeId: number | null): number[] {
  if (beforeId === dragId) return [...ids]; // бросок перед самим собой — без изменений
  const without = ids.filter((id) => id !== dragId);
  if (beforeId === null) return [...without, dragId];
  const idx = without.indexOf(beforeId);
  if (idx < 0) return [...without, dragId];
  return [...without.slice(0, idx), dragId, ...without.slice(idx)];
}
```

- [ ] **Step 4: Запустить тест — убедиться, что проходит**

Run: `npm test -- credsOrder`
Expected: PASS (5 тестов).

- [ ] **Step 5: Commit**

```bash
git add src/lib/credsOrder.ts src/tests/credsOrder.test.ts
git commit -m "feat(frontend): reorderIds helper + tests"
```

---

## Task 3: Frontend — api creds.reorder

**Files:**
- Modify: `src/lib/api/creds.ts`

- [ ] **Step 1: Добавить обёртку**

В `src/lib/api/creds.ts`, после строки `export const remove = ...` (строка 17), добавить:

```ts
export const reorder = (projectId: number, ids: number[]) =>
  call<void>("creds_reorder", { projectId, ids });
```

- [ ] **Step 2: Проверить типы**

Run: `npm run check`
Expected: 0 ошибок (warnings на базовом уровне, ~40).

- [ ] **Step 3: Commit**

```bash
git add src/lib/api/creds.ts
git commit -m "feat(frontend): creds.reorder api wrapper"
```

---

## Task 4: Frontend — CredsTab drag-and-drop

**Files:**
- Modify: `src/lib/components/CredsTab.svelte` (импорт, состояние, обработчики, разметка карточки)

- [ ] **Step 1: Импортировать хелпер и API уже есть**

В `src/lib/components/CredsTab.svelte` добавить импорт `reorderIds` после строки
`import Icon from "./Icon.svelte";` (строка 6):

```svelte
  import { reorderIds } from "$lib/credsOrder";
```

(`import * as creds from "$lib/api/creds";` уже есть на строке 3 — `creds.reorder`
доступен.)

- [ ] **Step 2: Добавить состояние и обработчики DnD**

В `<script>`, после функции `del()` (заканчивается на строке ~124, перед `</script>`),
добавить:

```ts
  // --- drag-and-drop сортировка (только при пустом поиске) ---
  let dragId = $state<number | null>(null);
  let overId = $state<number | null>(null); // карточка под курсором
  let overAfter = $state(false);            // курсор в правой половине → вставка после

  function onGripDragStart(e: DragEvent, id: number) {
    dragId = id;
    if (e.dataTransfer) {
      e.dataTransfer.effectAllowed = "move";
      e.dataTransfer.setData("text/plain", String(id));
    }
  }
  function onCardDragOver(e: DragEvent, id: number) {
    if (dragId === null) return;
    e.preventDefault();
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    overId = id;
    overAfter = e.clientX >= rect.left + rect.width / 2;
  }
  function clearDrag() {
    dragId = null;
    overId = null;
    overAfter = false;
  }
  async function commitDrop() {
    if (dragId === null || overId === null) { clearDrag(); return; }
    const ids = items.map((c) => c.id);
    let beforeId: number | null;
    if (!overAfter) {
      beforeId = overId;
    } else {
      const i = ids.indexOf(overId);
      beforeId = i >= 0 && i + 1 < ids.length ? ids[i + 1] : null;
    }
    const next = reorderIds(ids, dragId, beforeId);
    clearDrag();
    if (next.join(",") === ids.join(",")) return; // порядок не изменился — no-op
    const byId = new Map(items.map((c) => [c.id, c]));
    items = next.map((id) => byId.get(id)!); // оптимистично
    try {
      await creds.reorder(project.id, next);
    } catch {
      await load(); // откат к серверному порядку (тост ошибки уже из client.ts)
    }
  }
```

- [ ] **Step 3: Обновить разметку карточки**

В `src/lib/components/CredsTab.svelte` найти открытие карточки и её head (строки ~137-145):

```svelte
  {#each filtered as c (c.id)}
    <div class="card cred">
      <div class="cred-head">
        <span class="t">{c.label}</span>
```

заменить на:

```svelte
  {#each filtered as c (c.id)}
    <div class="card cred"
         class:drop-before={dragId !== null && dragId !== c.id && overId === c.id && !overAfter}
         class:drop-after={dragId !== null && dragId !== c.id && overId === c.id && overAfter}
         role="listitem"
         ondragover={(e) => onCardDragOver(e, c.id)}
         ondrop={commitDrop}>
      <div class="cred-head">
        {#if !query}
          <button class="cred-grip" type="button" draggable={true} title="Перетащите, чтобы изменить порядок"
                  ondragstart={(e) => onGripDragStart(e, c.id)} ondragend={clearDrag}>
            <Icon name="grip-vertical" class="ic-sm" />
          </button>
        {/if}
        <span class="t">{c.label}</span>
```

(Закрывающие теги `</div>` карточки и head не меняются.)

- [ ] **Step 4: Добавить role="list" контейнеру**

Найти `<div class="creds">` (строка ~136) и заменить на:

```svelte
<div class="creds" role="list">
```

- [ ] **Step 5: Проверить типы**

Run: `npm run check`
Expected: 0 ошибок. Возможны 1-3 НОВЫХ a11y-warning на обработчиках drag у `.cred`
(как у DnD в TasksTab — они уже в базовых ~40). Это допустимо: 0 ошибок — жёсткое
требование, drag-related a11y-warning — нет. Если появилась ОШИБКА (не warning) —
исправить минимально и сообщить.

- [ ] **Step 6: Commit**

```bash
git add src/lib/components/CredsTab.svelte
git commit -m "feat(frontend): drag-and-drop reorder in CredsTab"
```

---

## Task 5: Frontend — стили грипа и индикаторов

**Files:**
- Modify: `src/lib/styles/global.css` (после блока `.cred-head` / `.cred-del`)

- [ ] **Step 1: Добавить CSS**

В `src/lib/styles/global.css`, после правила `.cred-head .cred-del { margin-left: auto; }`
(строка ~793), добавить:

```css
/* drag-and-drop сортировка кредов */
.cred-grip {
  display: inline-flex; align-items: center; justify-content: center;
  background: transparent; border: 0; padding: 0; color: var(--muted-2);
  cursor: grab; flex: none;
}
.cred-grip:active { cursor: grabbing; }
.cred-grip:hover { color: var(--text-2); }
.cred-grip svg { pointer-events: none; }
.cred.drop-before { box-shadow: inset 3px 0 0 var(--accent); }
.cred.drop-after { box-shadow: inset -3px 0 0 var(--accent); }
```

- [ ] **Step 2: Проверить сборку**

Run: `npm run check`
Expected: 0 ошибок.

- [ ] **Step 3: Commit**

```bash
git add src/lib/styles/global.css
git commit -m "feat(frontend): styles for creds drag handle + drop indicators"
```

---

## Task 6: Полная верификация

- [ ] **Step 1: Rust-тесты**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib`
Expected: все PASS, включая `creds_reorder_sets_sort_order`.
(Флаг `--lib` обязателен, если параллельно запущен `npm run tauri dev`.)

- [ ] **Step 2: Фронт-тесты**

Run: `npm test`
Expected: все PASS, включая 5 тестов `reorderIds`.

- [ ] **Step 3: Проверка типов**

Run: `npm run check`
Expected: 0 ошибок.

- [ ] **Step 4: Ручная проверка в приложении**

Run: `npm run tauri dev`
На проекте с ≥2 кредами (поиск пустой):
- Слева в шапке каждой карточки — грип-ручка (⠿), курсор `grab`.
- Перетаскивание за ручку показывает подсветку края целевой карточки (левый край —
  вставка перед, правый — после).
- Бросок переставляет карточку; порядок сохраняется после переключения вкладок/проекта.
- При вводе в поиск ручка исчезает (DnD недоступен).
- Кнопки редактирования/копирования/показа секрета работают как прежде.

- [ ] **Step 5: Финальный commit (если были правки после ручной проверки)**

```bash
git add -A
git commit -m "fix: creds drag-reorder polish after manual verification"
```

(Если правок нет — пропустить.)
