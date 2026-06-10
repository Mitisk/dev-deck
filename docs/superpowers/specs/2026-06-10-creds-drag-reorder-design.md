# Перетаскивание кредов для сортировки

Дата: 2026-06-10
Статус: согласовано, к реализации

## Проблема

Креды проекта показываются списком в [CredsTab.svelte](../../../src/lib/components/CredsTab.svelte),
отсортированные по `sort_order` (его задаёт только порядок добавления). Изменить
порядок вручную нельзя. Нужна сортировка перетаскиванием (drag-and-drop).

## Решение (обзор)

Карточки кредов получают грип-ручку слева; перетаскивание за ручку меняет порядок,
новый порядок сохраняется командой `creds_reorder` (калька с `labels_reorder`).
Креды уже хранят `sort_order`, а `creds_list` уже сортирует по нему — серверная
часть минимальна.

## 1. Backend

### Команда `creds_reorder`

Новая команда в [creds.rs](../../../src-tauri/src/commands/creds.rs), точная калька с
`labels_reorder` ([labels.rs:73](../../../src-tauri/src/commands/labels.rs#L73)).

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

- `lock(&state)?` и `params!` — те же хелперы/импорты, что уже использует
  `creds.rs` (проверить, что `lock` доступен в этом файле — в `labels.rs` он есть;
  если в `creds.rs` блокировка делается иначе, использовать местный способ).
- Регистрация в `tauri::generate_handler![...]` в
  [lib.rs](../../../src-tauri/src/lib.rs) рядом с прочими `commands::creds::*`.
- Никаких изменений в `creds_list` — он уже `ORDER BY sort_order ASC, id ASC`.

## 2. Frontend API

В [creds.ts](../../../src/lib/api/creds.ts):

```ts
export const reorder = (projectId: number, ids: number[]) =>
  call<void>("creds_reorder", { projectId, ids });
```

## 3. CredsTab — drag-and-drop

Паттерн взят из [TasksTab.svelte](../../../src/lib/components/TasksTab.svelte)
(`dragId`/`dropBeforeId`), адаптирован под РЕШЁТКУ: `.creds` — это
`display:grid; grid-template-columns: repeat(auto-fill, minmax(340px,1fr))`
([global.css:480](../../../src/lib/styles/global.css#L480)), карточки текут
слева-направо с переносом. Поэтому half-детекция — по горизонтали (левая/правая
половина карточки), а индикатор вставки — подсветка края целевой карточки
(вставлять отдельный full-width `drop-line` между ячейками грида нельзя — он
займёт ячейку и сломает раскладку).

### Аффорданс
- Грип-ручка (иконка `grip-vertical`, `cursor:grab`) — первый элемент внутри
  `.cred-head`. `draggable={true}` вешается на саму ручку, НЕ на всю карточку,
  чтобы клики по кнопкам (редактировать/копировать/показать) не конфликтовали
  с drag и не мешало выделение текста.
- Ручка видна и DnD активен ТОЛЬКО когда поиск пустой (`query` === ""). При
  активном поиске список отфильтрован, порядок неоднозначен — ручка скрыта.

### Состояние
- `dragId: number | null` — id перетаскиваемого креда.
- `dropBeforeId: number | null` — id креда, ПЕРЕД которым произойдёт вставка;
  `null` — вставка в конец списка.

### Поведение
- `ondragstart` (на ручке): `dragId = c.id`; `e.dataTransfer.effectAllowed = "move"`,
  `setData("text/plain", String(c.id))`.
- `ondragover` (на карточке): `e.preventDefault()`; по ГОРИЗОНТАЛЬНОЙ позиции
  курсора относительно середины карточки
  (`rect = e.currentTarget.getBoundingClientRect(); before = e.clientX < rect.left + rect.width/2`)
  выставить `dropBeforeId = c.id` (левая половина → вставка перед этой карточкой)
  либо id следующей карточки / `null` (правая половина → вставка после).
- Индикатор: класс на целевой карточке (НЕ отдельный элемент). `.drop-before`
  рисует акцентную полосу у левого края (`box-shadow: inset 3px 0 0 var(--accent)`),
  `.drop-after` — у правого (`inset -3px 0 0 var(--accent)`). Класс ставится
  карточке, над которой курсор, когда `dragId != null && dragId !== c.id`.
- `ondrop` / `ondragend`: вычислить новый порядок (см. ниже), применить
  оптимистично к `items`, затем `await creds.reorder(project.id, items.map(c => c.id))`.
  Сбросить `dragId`/`dropBeforeId`.

### Вычисление нового порядка — чистый хелпер
Чтобы логику можно было протестировать без DOM, вынести в чистую функцию в
`src/lib/credsOrder.ts`:

```ts
// Переместить элемент dragId так, чтобы он встал ПЕРЕД beforeId
// (beforeId === null → в конец). Возвращает новый массив id.
export function reorderIds(ids: number[], dragId: number, beforeId: number | null): number[] {
  const without = ids.filter((id) => id !== dragId);
  if (beforeId === null) return [...without, dragId];
  const idx = without.indexOf(beforeId);
  if (idx < 0) return [...without, dragId];
  return [...without.slice(0, idx), dragId, ...without.slice(idx)];
}
```

CredsTab применяет: `const next = reorderIds(items.map(c=>c.id), dragId, dropBeforeId)`,
затем переставляет `items` в этот порядок (map по id → Credential).

## 4. Ошибки и edge-cases

- Бросок на себя же или на позицию, не меняющую порядок → новый порядок равен
  старому → no-op, бэкенд не дёргаем.
- `items` — полный список кредов проекта (не отфильтрованный), поэтому реордер
  сохраняет позиции всех кредов; DnD доступен только при пустом поиске, так что
  `filtered === items` в момент перетаскивания.
- `creds.reorder` падает → тост ошибки уже идёт из `client.ts`; дополнительно
  вызвать `load()` для отката к серверному порядку.
- Пустой список / один кред → перетаскивать нечего (ручка есть, но drop не меняет
  порядок).

## 5. Тестирование

- **Rust** ([creds.rs](../../../src-tauri/src/commands/creds.rs), `#[cfg(test)]`):
  `creds_reorder` во временной БД — вставить 2–3 креда, переставить, проверить
  `sort_order` через `SELECT ... ORDER BY sort_order`. По образцу
  `reorder_labels_sets_sort_order`.
- **Фронт** (Vitest, node-env): юнит-тесты `reorderIds` — перемещение вверх, вниз,
  в конец (`beforeId === null`), бросок на себя (порядок не меняется), несуществующий
  `beforeId`.
- Рендер компонента не тестируем (нет инфраструктуры) — DnD-связка проверяется
  вручную.

## Затрагиваемые файлы

Backend: `src-tauri/src/commands/creds.rs` (+команда, +тест),
`src-tauri/src/lib.rs` (регистрация).
Frontend: `src/lib/api/creds.ts` (+reorder), `src/lib/credsOrder.ts` (новый,
+тест `src/tests/credsOrder.test.ts`), `src/lib/components/CredsTab.svelte`
(грип-ручка + DnD), `src/lib/styles/global.css` (стиль ручки `.cred-grip` +
индикаторы `.drop-before`/`.drop-after`).
