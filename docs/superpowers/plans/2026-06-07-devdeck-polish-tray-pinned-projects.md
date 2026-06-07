# DevDeck — Доработка «Закреплённые проекты в меню трея» — план

> **Для исполнителя:** ОБЯЗАТЕЛЬНЫЙ ПОД-НАВЫК: superpowers:subagent-driven-development. Шаги — чекбоксы.

**Цель:** В контекстном меню системного трея показывать закреплённые проекты. Клик по пункту проекта — показать окно и открыть этот проект. Меню пересобирается при изменении набора закреплённых (через команду `tray_resync`, которую фронт уже дёргает из `loadProjects`).

**Архитектура:** Новый модуль `src-tauri/src/tray.rs`: `TrayState{ icon: Mutex<Option<TrayIcon>> }` (managed), `build_menu(app)` (читает закреплённые проекты, строит меню Открыть / sep / проекты / sep / Выход), `resync(app)` (пересобрать меню + `icon.set_menu`). `setup` строит меню через `tray::build_menu`, сохраняет `TrayIcon` в `TrayState`, расширяет `on_menu_event` обработкой id `proj:{id}` (показать окно + `emit("tray-open-project", id)`). Команда `tray_resync`. Фронт: `api/tray.ts`, `loadProjects` дёргает `tray.resync()`, `+page.svelte` слушает `tray-open-project` → `activeProjectId.set(id)`.

**Стек/границы:** backend — новый `tray.rs` + `commands/tray.rs` + правки `lib.rs`; frontend — `api/tray.ts` + правки `loadProjects` и `+page.svelte`. Без миграций, без unit-тестов (трей проверяется GUI-смоуком).

---

## Task 1: Rust — модуль трея и динамическое меню

**Files:** Create `src-tauri/src/tray.rs`, `src-tauri/src/commands/tray.rs`; Modify `src-tauri/src/lib.rs`, `src-tauri/src/commands/mod.rs`.

- [ ] **Step 1: Новый модуль `src-tauri/src/tray.rs`**
```rust
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use std::sync::Mutex;
use tauri::menu::{IsMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIcon;
use tauri::{AppHandle, Manager, Wry};

/// Хэндл трея для пересборки меню на лету.
#[derive(Default)]
pub struct TrayState {
    pub icon: Mutex<Option<TrayIcon<Wry>>>,
}

/// Закреплённые (не архивные) проекты для меню трея.
fn pinned_projects(app: &AppHandle) -> AppResult<Vec<(i64, String)>> {
    let st = app.state::<AppState>();
    let conn = st
        .db
        .lock()
        .map_err(|_| AppError::internal("db mutex poisoned"))?;
    let mut stmt = conn.prepare(
        "SELECT id, name FROM projects WHERE pinned=1 AND status != 'archived' \
         ORDER BY sort_order ASC, name COLLATE NOCASE ASC LIMIT 8",
    )?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
    let mut v = Vec::new();
    for row in rows {
        v.push(row?);
    }
    Ok(v)
}

/// Собрать меню трея: Открыть / [sep / закреплённые] / Выход.
pub fn build_menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let open_i = MenuItem::with_id(app, "open", "Открыть DevDeck", true, None::<&str>)?;
    let quit_i = MenuItem::with_id(app, "quit", "Выход", true, None::<&str>)?;
    let pinned = pinned_projects(app).unwrap_or_default();
    let proj_items: Vec<MenuItem<Wry>> = pinned
        .iter()
        .map(|(id, name)| MenuItem::with_id(app, format!("proj:{id}"), name, true, None::<&str>))
        .collect::<tauri::Result<_>>()?;
    let sep1 = PredefinedMenuItem::separator(app)?;
    let sep2 = PredefinedMenuItem::separator(app)?;

    let mut items: Vec<&dyn IsMenuItem<Wry>> = vec![&open_i];
    if !proj_items.is_empty() {
        items.push(&sep1);
        for it in &proj_items {
            items.push(it);
        }
        items.push(&sep2);
    }
    items.push(&quit_i);
    Menu::with_items(app, &items)
}

/// Пересобрать меню трея под актуальный набор закреплённых проектов.
pub fn resync(app: &AppHandle) -> AppResult<()> {
    let menu = build_menu(app).map_err(|e| AppError::internal(format!("tray menu: {e}")))?;
    let st = app.state::<TrayState>();
    let guard = st
        .icon
        .lock()
        .map_err(|_| AppError::internal("tray mutex poisoned"))?;
    if let Some(icon) = guard.as_ref() {
        icon.set_menu(Some(menu))
            .map_err(|e| AppError::internal(format!("set_menu: {e}")))?;
    }
    Ok(())
}
```

- [ ] **Step 2: Команда `src-tauri/src/commands/tray.rs`**
```rust
use crate::error::AppResult;

#[tauri::command]
pub fn tray_resync(app: tauri::AppHandle) -> AppResult<()> {
    crate::tray::resync(&app)
}
```
И в `src-tauri/src/commands/mod.rs` добавить `pub mod tray;` (в алфавитном/логическом порядке среди прочих `pub mod ...;`).

- [ ] **Step 3: Правки `lib.rs`**

1. В начало (рядом с `mod watch;`) добавить `mod tray;`.
2. В `use tauri::Manager;` рядом добавить `use tauri::Emitter;` (для `app.emit`).
3. В `setup`, после `app.manage(watch::WatchState::default());` добавить:
```rust
            app.manage(tray::TrayState::default());
```
4. Блок системного трея заменить. Текущее (строки ~40–64):
```rust
            // --- системный трей ---
            let open_i = MenuItem::with_id(app, "open", "Открыть DevDeck", true, None::<&str>)?;
            let quit_i = MenuItem::with_id(app, "quit", "Выход", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open_i, &quit_i])?;

            let _tray = TrayIconBuilder::with_id("main-tray")
                .icon(app.default_window_icon().expect("есть иконка окна").clone())
                .tooltip("DevDeck")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "open" => show_main(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    use tauri::tray::TrayIconEvent;
                    if let TrayIconEvent::Click { button, button_state, .. } = event {
                        if button == tauri::tray::MouseButton::Left
                            && button_state == tauri::tray::MouseButtonState::Up
                        {
                            show_main(tray.app_handle());
                        }
                    }
                })
                .build(app)?;
```
заменить на:
```rust
            // --- системный трей ---
            let menu = tray::build_menu(app.handle())?;

            let tray_icon = TrayIconBuilder::with_id("main-tray")
                .icon(app.default_window_icon().expect("есть иконка окна").clone())
                .tooltip("DevDeck")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| {
                    let id = event.id.as_ref();
                    match id {
                        "open" => show_main(app),
                        "quit" => app.exit(0),
                        _ => {
                            if let Some(rest) = id.strip_prefix("proj:") {
                                if let Ok(pid) = rest.parse::<i64>() {
                                    show_main(app);
                                    let _ = app.emit("tray-open-project", pid);
                                }
                            }
                        }
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    use tauri::tray::TrayIconEvent;
                    if let TrayIconEvent::Click { button, button_state, .. } = event {
                        if button == tauri::tray::MouseButton::Left
                            && button_state == tauri::tray::MouseButtonState::Up
                        {
                            show_main(tray.app_handle());
                        }
                    }
                })
                .build(app)?;

            // сохранить хэндл трея для пересборки меню (закреплённые проекты)
            if let Ok(mut g) = app.state::<tray::TrayState>().icon.lock() {
                *g = Some(tray_icon);
            }
```
5. Если после правок `use tauri::menu::{Menu, MenuItem};` стал не нужен в `lib.rs` (Menu/MenuItem больше не используются напрямую) — удалить неиспользуемые импорты, чтобы не было warning. (Проверить: `Menu`/`MenuItem` в `lib.rs` после правки не упоминаются → убрать строку `use tauri::menu::{Menu, MenuItem};`. `TrayIconBuilder` остаётся.)
6. В `generate_handler![...]` добавить `commands::tray::tray_resync,` (рядом с `commands::watch::watch_resync,`).

- [ ] **Step 4: Сборка**
```powershell
cargo build --manifest-path src-tauri/Cargo.toml --lib
cargo test --manifest-path src-tauri/Cargo.toml --lib
```
Expected: lib-сборка успешна, без warning о неиспользуемых импортах; тесты — 33 ok (как было; новых нет). Если есть warning об unused — поправить импорты.

- [ ] **Step 5: Commit**
```powershell
git add src-tauri/src/tray.rs src-tauri/src/commands/tray.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs
git commit -m @'
feat(backend): pinned projects in tray menu + tray_resync

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

## Task 2: Фронт — api трея, пересборка и навигация

**Files:** Create `src/lib/api/tray.ts`; Modify `src/lib/stores/projects.ts`, `src/routes/+page.svelte`.

- [ ] **Step 1: `src/lib/api/tray.ts`**
```ts
import { call } from "./client";

export const resync = () => call<void>("tray_resync");
```

- [ ] **Step 2: `loadProjects` дёргает пересборку меню трея**

В `src/lib/stores/projects.ts`, в конце `loadProjects`, рядом со строкой пересборки вотчера, добавить пересборку трея:
```ts
  // обновить вотчер папок и меню трея под актуальный список (не критично при ошибке)
  import("../api/watch").then((w) => w.resync().catch(() => {}));
  import("../api/tray").then((t) => t.resync().catch(() => {}));
```
(заменить существующий одиночный `import("../api/watch")...` на эти две строки.)

- [ ] **Step 3: `+page.svelte` слушает `tray-open-project`**

В `<script>`:
1. К импорту из stores добавить `activeProjectId`:
```ts
  import { loadProjects, activeProjectId } from "$lib/stores/projects";
```
(сейчас импортируется только `loadProjects` — расширить.)
2. В `onMount`, после `void watchApi.resync();`, добавить подписку:
```ts
    const { listen } = await import("@tauri-apps/api/event");
    await listen<number>("tray-open-project", (e) => activeProjectId.set(e.payload));
```

- [ ] **Step 4: Проверка**
```powershell
npm run check
npm test
npm run build
```
Expected: 0 ошибок типов; Vitest 3 (3 passed); build успешен.

- [ ] **Step 5: Commit**
```powershell
git add src/lib/api/tray.ts src/lib/stores/projects.ts src/routes/+page.svelte
git commit -m @'
feat(frontend): open pinned projects from tray, resync tray menu

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

## Task 3: Проверка

- [ ] **Step 1: Автопроверка**
```powershell
npm run check ; npm test ; cargo test --manifest-path src-tauri/Cargo.toml --lib
```
Expected: типы 0; Vitest 3; Rust 33.

- [ ] **Step 2: GUI-смоук (пользователь)**
- [ ] Закрепить проект → правый клик по иконке трея → проект виден в меню; клик по нему открывает окно и этот проект.
- [ ] Открепить → пункт исчезает из меню (без перезапуска).
- [ ] «Открыть DevDeck» / «Выход» работают как раньше; левый клик по иконке показывает окно.

---

## Итог

Меню трея динамическое: показывает закреплённые проекты, переход по клику, пересобирается при изменении закреплений. Следующая доработка — реордер колонок и меток перетаскиванием.
