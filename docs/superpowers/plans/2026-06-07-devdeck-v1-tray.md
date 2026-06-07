# DevDeck v1.0 — Срез «Системный трей» — план реализации

> **Для исполнителя:** ОБЯЗАТЕЛЬНЫЙ ПОД-НАВЫК: superpowers:subagent-driven-development. Шаги — чекбоксы.

**Цель:** Иконка приложения в системном трее с меню «Открыть DevDeck» / «Выход». Закрытие окна сворачивает в трей (приложение продолжает работать); полный выход — через меню трея. Клик по иконке трея показывает окно.

**Архитектура:** Трей строится в `setup()` (`lib.rs`) через Tauri 2 tray-API. Закрытие окна перехватывается обработчиком `WindowEvent::CloseRequested` → `prevent_close` + `hide`. Нужна фича `tray-icon` у crate `tauri`.

**Стек:** Tauri 2 (фича `tray-icon`). Без миграций, без фронта.

**Решения / границы:**
- **Закреплённые проекты в меню трея** — отложено (динамические пункты меню + перестройка при смене pin сложнее; добавим follow-up'ом). Сейчас меню статичное: Открыть / Выход.
- Сворачивание в трей при закрытии — **включено всегда** (выход через меню трея). Тумблер «сворачивать в трей» — позже, со страницей настроек приложения.
- Проверка — сборкой + ручным GUI (автотеста на трей нет).

**Источники:** `TZ_DevDeck.md` (5.11 — системный трей). Tauri 2 tray API (`tauri::tray::TrayIconBuilder`, `tauri::menu::{Menu, MenuItem}`). Текущий `src-tauri/src/lib.rs` (`setup`), `Cargo.toml`, `tauri.conf.json` (окно без явного label → label `main`).

---

## Контекст

- Rust: `lib.rs` `tauri::Builder::default().plugin(...).setup(|app| {...}).invoke_handler(...).run(...)`. В `setup` уже создаётся каталог данных и `AppState`. Иконки приложения есть (`tauri.conf.json` → `bundle.icon`), `app.default_window_icon()` доступна.
- `Cargo.toml`: `tauri = { version = "2", features = [...] }` — нужно добавить `"tray-icon"`.

---

## Структура файлов

```
src-tauri/
├─ Cargo.toml          # MOD: + фича tray-icon у tauri
└─ src/lib.rs          # MOD: построить трей + обработчик закрытия окна
```

---

## Task 1: Трей + сворачивание в трей

**Files:** Modify `src-tauri/Cargo.toml`, `src-tauri/src/lib.rs`.

- [ ] **Step 1: Включить фичу tray-icon**

В `src-tauri/Cargo.toml` в зависимости `tauri` добавить фичу `"tray-icon"`. Например, если сейчас:
```toml
tauri = { version = "2", features = [] }
```
станет:
```toml
tauri = { version = "2", features = ["tray-icon"] }
```
(Сохранить уже существующие фичи, если они есть — просто дописать `"tray-icon"`.)

- [ ] **Step 2: Построить трей и перехват закрытия в lib.rs**

В `src-tauri/src/lib.rs`:
1. Добавить импорты (вверху, рядом с `use tauri::Manager;`):
```rust
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{WindowEvent};
```
2. Внутри `.setup(|app| { ... })` ПОСЛЕ существующей инициализации БД/AppState добавить построение трея и обработчик окна (перед `Ok(())`):
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

            // --- закрытие окна = сворачивание в трей ---
            if let Some(window) = app.get_webview_window("main") {
                let w = window.clone();
                window.on_window_event(move |event| {
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = w.hide();
                    }
                });
            }
```
3. Добавить хелпер (вне функции `run`, на уровне модуля):
```rust
fn show_main(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.set_focus();
    }
}
```

> ВАЖНО: точные сигнатуры Tauri 2 tray/menu API могут отличаться по версии. Критерий — `cargo build` зелёный и поведение по GUI-смоуку. Если `MenuItem::with_id` / `Menu::with_items` / `TrayIconBuilder` / `on_tray_icon_event` / `TrayIconEvent::Click` отличаются — привести к фактической сигнатуре установленной версии Tauri (подсказывает `cargo build` с точными ошибками). Метод `show_menu_on_left_click` в некоторых версиях называется `menu_on_left_click` — использовать тот, что компилируется (или убрать вызов — не критичен). Если что-то не собирается после разумной попытки — STOP, доложить BLOCKED с точной ошибкой и текущим кодом.

- [ ] **Step 3: Сборка**
```powershell
cargo build --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml --lib
```
Expected: build успешен; прежние 20 тестов ok.

- [ ] **Step 4: Commit**
```powershell
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/lib.rs
git commit -m @'
feat(backend): system tray with minimize-to-tray and open/quit menu

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

## Task 2: Проверка среза

- [ ] **Step 1: Автопроверка**
```powershell
cargo build --manifest-path src-tauri/Cargo.toml ; cargo test --manifest-path src-tauri/Cargo.toml --lib ; npm run check
```
Expected: build ок; Rust 20; типы 0 ошибок.

- [ ] **Step 2: Ручной GUI-смоук (пользователь; `npm run tauri dev`)**
- [ ] В системном трее появилась иконка DevDeck.
- [ ] Закрыть окно (×) → окно скрывается, приложение остаётся в трее (процесс жив).
- [ ] Клик по иконке трея → окно показывается снова.
- [ ] Правый клик по иконке → меню «Открыть DevDeck» / «Выход».
- [ ] «Открыть DevDeck» → окно на переднем плане.
- [ ] «Выход» → приложение полностью закрывается.

---

## Итог среза

Приложение живёт в системном трее: закрытие окна сворачивает в трей, клик по иконке возвращает окно, меню даёт «Открыть»/«Выход». Завершает пункт 10 v1.0 (дашборд/темы/трей). Отложено: закреплённые проекты в меню трея, тумблер «сворачивать в трей» (со страницей настроек). Дальше — пункт 11: бэкап/экспорт-импорт, шаблоны чеклистов, режим мастер-пароля.
```
