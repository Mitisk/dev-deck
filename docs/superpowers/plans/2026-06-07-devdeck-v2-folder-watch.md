# DevDeck v2.0 — Срез «Слежение за папкой проекта» — план реализации

> **Для исполнителя:** ОБЯЗАТЕЛЬНЫЙ ПОД-НАВЫК: superpowers:subagent-driven-development. Шаги — чекбоксы.

**Цель:** Живой индикатор изменений: когда в папке проекта меняются файлы, в сайдбаре у проекта появляется точка «изменено». Индикатор гаснет при открытии проекта.

**Архитектура:** Фоновый вотчер `notify` (через `notify-debouncer-mini`) рекурсивно следит за папками всех проектов. События дебаунсятся, мусорные пути (`node_modules`/`.git`/`target`/`dist`/`build`/`.svelte-kit`) отфильтровываются, остальные сопоставляются с корнем проекта → emit Tauri-события `folder-changed` {projectId}. Фронт слушает событие, помечает проект; пометка снимается при открытии. Вотчер пересобирается при старте и после изменения набора проектов (`watch_resync`).

**Стек:** + crate `notify-debouncer-mini`. Без миграций.

**Решения / границы:**
- Рекурсивный watch + **фильтр шумовых каталогов** (иначе артефакты сборки = постоянные ложные срабатывания).
- Чистые функции `is_noise(path)` и `project_for_path(roots, path)` — тестируемы; обвязка вотчера — по GUI.
- Дебаунс ~800 мс. Индикатор — точка в строке проекта сайдбара; гаснет при `activeProjectId == id`.

**Источники:** `TZ_DevDeck.md` (11 — v2.0, слежение за папкой `notify`). Текущие `lib.rs` (`setup`), `state.rs`, `Sidebar.svelte`, `stores/projects.ts`, `routes/+page.svelte`.

---

## Контекст

- Rust: `lib.rs` `setup` (данные, БД, AppState, трей, авто-бэкап); `state::AppState`. 71 команда. Проекты: `projects(id, repo_path, path, status)`.
- Фронт: `Sidebar.svelte` (строки `.proj`); `stores/projects.ts` (`projects`, `activeProjectId`, `loadProjects`). `+page.svelte` (`onMount(loadProjects)`). `@tauri-apps/api/event` (`listen`) доступен.

---

## Структура файлов

```
src-tauri/
├─ Cargo.toml             # MOD: + notify-debouncer-mini
└─ src/
   ├─ watch.rs            # NEW: WatchState + resync + is_noise/project_for_path + тесты
   ├─ commands/watch.rs   # NEW: watch_resync
   ├─ commands/mod.rs     # MOD: + pub mod watch;
   └─ lib.rs              # MOD: mod watch; WatchState managed; resync в setup; регистрация команды

src/lib/
├─ stores/changed.ts      # NEW: помеченные «изменённые» проекты + listen
├─ api/watch.ts           # NEW: resync()
├─ components/Sidebar.svelte  # MOD: индикатор «изменено»
└─ routes/+page.svelte    # MOD: инициализация changed-стора + resync после загрузки
```

---

## Task 1: Rust — вотчер + чистые помощники

**Files:** Modify `Cargo.toml`; Create `src-tauri/src/watch.rs`.

- [ ] **Step 1: Зависимость** — в `src-tauri/Cargo.toml` `[dependencies]`:
```toml
notify-debouncer-mini = "0.4"
```

- [ ] **Step 2: watch.rs**

Create `src-tauri/src/watch.rs`:
```rust
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use notify_debouncer_mini::notify::RecursiveMode;
use notify_debouncer_mini::{new_debouncer, DebounceEventResult, Debouncer};
use notify_debouncer_mini::notify::RecommendedWatcher;
use rusqlite::Connection;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

const NOISE: &[&str] = &[
    "\\node_modules\\", "/node_modules/",
    "\\.git\\", "/.git/",
    "\\target\\", "/target/",
    "\\dist\\", "/dist/",
    "\\build\\", "/build/",
    "\\.svelte-kit\\", "/.svelte-kit/",
];

/// Считать ли путь «шумовым» (артефакты сборки / VCS).
pub fn is_noise(path: &Path) -> bool {
    let s = path.to_string_lossy();
    NOISE.iter().any(|n| s.contains(n))
}

/// Найти id проекта, чья корневая папка является предком пути.
pub fn project_for_path(roots: &[(PathBuf, i64)], path: &Path) -> Option<i64> {
    roots.iter().find(|(root, _)| path.starts_with(root)).map(|(_, id)| *id)
}

/// Раскрыть ведущий `~`.
fn expand(p: &str) -> String {
    let t = p.trim();
    if let Some(rest) = t.strip_prefix("~/").or_else(|| t.strip_prefix("~\\")) {
        if let Ok(home) = std::env::var("USERPROFILE") {
            return format!("{}\\{}", home.trim_end_matches('\\'), rest.replace('/', "\\"));
        }
    }
    t.to_string()
}

/// Управляемое состояние вотчера. Debouncer держим живым (drop останавливает слежение).
#[derive(Default)]
pub struct WatchState {
    pub debouncer: Mutex<Option<Debouncer<RecommendedWatcher>>>,
    pub roots: Arc<Mutex<Vec<(PathBuf, i64)>>>,
}

/// Пересобрать вотчер по текущему набору проектов.
pub fn resync(app: &AppHandle) -> AppResult<()> {
    let db_state = app.state::<AppState>();
    let watch_state = app.state::<WatchState>();

    // прочитать пути проектов
    let projects: Vec<(i64, String)> = {
        let conn: &Connection = &db_state.db.lock().map_err(|_| AppError::internal("db mutex poisoned"))?;
        let mut stmt = conn.prepare(
            "SELECT id, COALESCE(NULLIF(repo_path,''), path) FROM projects WHERE status != 'archived'",
        )?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Option<String>>(1)?)))?;
        let mut v = Vec::new();
        for row in rows {
            let (id, p) = row?;
            if let Some(p) = p {
                v.push((id, p));
            }
        }
        v
    };

    let roots: Vec<(PathBuf, i64)> = projects
        .into_iter()
        .filter_map(|(id, p)| {
            let pb = PathBuf::from(expand(&p));
            if pb.is_dir() { Some((pb, id)) } else { None }
        })
        .collect();

    *watch_state.roots.lock().map_err(|_| AppError::internal("roots poisoned"))? = roots.clone();

    let roots_arc = watch_state.roots.clone();
    let app2 = app.clone();
    let mut deb = new_debouncer(Duration::from_millis(800), move |res: DebounceEventResult| {
        if let Ok(events) = res {
            let roots = match roots_arc.lock() {
                Ok(g) => g,
                Err(_) => return,
            };
            let mut hit: HashSet<i64> = HashSet::new();
            for ev in events {
                if is_noise(&ev.path) {
                    continue;
                }
                if let Some(id) = project_for_path(&roots, &ev.path) {
                    hit.insert(id);
                }
            }
            for id in hit {
                let _ = app2.emit("folder-changed", id);
            }
        }
    })
    .map_err(|e| AppError::internal(format!("watcher: {}", e)))?;

    for (root, _) in roots.iter() {
        let _ = deb.watcher().watch(root, RecursiveMode::Recursive);
    }
    *watch_state.debouncer.lock().map_err(|_| AppError::internal("debouncer poisoned"))? = Some(deb);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noise_paths_detected() {
        assert!(is_noise(Path::new("C:\\dev\\app\\node_modules\\x\\y.js")));
        assert!(is_noise(Path::new("C:\\dev\\app\\.git\\HEAD")));
        assert!(is_noise(Path::new("C:\\dev\\app\\target\\debug\\app.exe")));
        assert!(!is_noise(Path::new("C:\\dev\\app\\src\\main.rs")));
    }

    #[test]
    fn matches_project_root() {
        let roots = vec![
            (PathBuf::from("C:\\dev\\aurora"), 1i64),
            (PathBuf::from("C:\\dev\\nebula"), 2i64),
        ];
        assert_eq!(project_for_path(&roots, Path::new("C:\\dev\\aurora\\src\\a.rs")), Some(1));
        assert_eq!(project_for_path(&roots, Path::new("C:\\dev\\nebula\\x")), Some(2));
        assert_eq!(project_for_path(&roots, Path::new("C:\\other\\z")), None);
    }
}
```
> ВНИМАНИЕ: API `notify-debouncer-mini 0.4` версионно-чувствителен (`new_debouncer`, `DebounceEventResult`, `DebouncedEvent.path`, `RecursiveMode`, `Debouncer<RecommendedWatcher>`, реэкспорт `notify`). Привести к фактической версии (подсказывает `cargo build`). Критерий — `cargo build --lib` зелёный и тесты `is_noise`/`project_for_path` проходят. Если debouncer-API сильно отличается — STOP, доложить BLOCKED.

- [ ] **Step 3: lib-сборка + тесты помощников**
```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib watch
cargo build --manifest-path src-tauri/Cargo.toml --lib
```
Expected: `noise_paths_detected`/`matches_project_root` ok; lib-сборка успешна (вотчер пока не вызывается — подключим в Task 2).

---

## Task 2: Rust — команда + регистрация + старт

**Files:** Create `commands/watch.rs`; Modify `commands/mod.rs`, `lib.rs`.

- [ ] **Step 1: commands/watch.rs**
```rust
use crate::error::AppResult;
use crate::watch;

#[tauri::command]
pub fn watch_resync(app: tauri::AppHandle) -> AppResult<()> {
    watch::resync(&app)
}
```

- [ ] **Step 2: Регистрация модулей**
- `commands/mod.rs`: `pub mod watch;`
- `lib.rs`: добавить `mod watch;` (рядом с `mod backup; mod crypto; ...`).
- `lib.rs` `generate_handler![...]`: добавить `commands::watch::watch_resync,`.

- [ ] **Step 3: Managed WatchState + старт в setup** — в `lib.rs`:
1. Зарегистрировать состояние: `.manage(watch::WatchState::default())` в билдере (рядом с `.manage(AppState{...})` — но AppState管ится в setup; WatchState можно тоже в setup: `app.manage(watch::WatchState::default());`). Поставить ПОСЛЕ `app.manage(AppState {...})`.
2. В конце `setup` (после трея/бэкапа, перед `Ok(())`) — первичная синхронизация (не падать при ошибке):
```rust
            let _ = watch::resync(app.handle());
```
> `app.handle()` даёт `&AppHandle`; `resync` берёт `&AppHandle`. Состояния `AppState`/`WatchState` уже managed к этому моменту.

- [ ] **Step 4: Тесты + lib-сборка**
```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib
cargo build --manifest-path src-tauri/Cargo.toml --lib
```
Expected: новые watch-тесты + прежние (29) → 31 ok; lib-сборка успешна.

- [ ] **Step 5: Commit (бэкенд)**
```powershell
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/watch.rs src-tauri/src/commands/watch.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs
git commit -m @'
feat(backend): project folder watching (notify) with noise filtering

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

## Task 3: Фронт — индикатор «изменено»

**Files:** Modify `Sidebar.svelte`, `routes/+page.svelte`; Create `stores/changed.ts`, `api/watch.ts`.

- [ ] **Step 1: api/watch.ts**
```ts
import { call } from "./client";
export const resync = () => call<void>("watch_resync");
```

- [ ] **Step 2: stores/changed.ts**

Create `src/lib/stores/changed.ts`:
```ts
import { writable, get } from "svelte/store";
import { browser } from "$app/environment";
import { activeProjectId } from "./projects";

// id проектов с изменениями в папке (не открытых после изменения).
export const changedProjects = writable<number[]>([]);

export function markChanged(id: number) {
  // не помечаем открытый проект
  if (get(activeProjectId) === id) return;
  changedProjects.update((list) => (list.includes(id) ? list : [...list, id]));
}

// При открытии проекта — снять пометку.
activeProjectId.subscribe((id) => {
  if (id == null) return;
  changedProjects.update((list) => list.filter((x) => x !== id));
});

// Подписка на событие бэкенда (один раз).
let started = false;
export async function startWatchEvents() {
  if (!browser || started) return;
  started = true;
  const { listen } = await import("@tauri-apps/api/event");
  await listen<number>("folder-changed", (e) => markChanged(e.payload));
}
```

- [ ] **Step 3: Индикатор в Sidebar.svelte**

В `src/lib/components/Sidebar.svelte`:
1. Импорт: `import { changedProjects } from "$lib/stores/changed";`
2. В строке проекта (`.proj`, и в pinned, и в rest) добавить точку, если проект помечен — рядом с `.nm` (после неё):
```svelte
          {#if $changedProjects.includes(p.id)}<span class="meta dirty" title="Изменения в папке"><span class="dot" style="--p-color:var(--git-dirty)"></span></span>{/if}
```
> Класс `.proj .meta.dirty` есть в global.css. Вставить в ОБА `{#each}` (закреплённые и все).

- [ ] **Step 4: +page.svelte — старт событий + resync**

В `src/routes/+page.svelte` в `onMount` (где уже `loadProjects()`):
```ts
  import { startWatchEvents } from "$lib/stores/changed";
  import * as watchApi from "$lib/api/watch";

  onMount(async () => {
    await loadProjects();
    await startWatchEvents();
    void watchApi.resync(); // пересобрать вотчер под текущие проекты
  });
```
(если `onMount` уже async/иной — аккуратно дополнить, сохранив существующее.)

- [ ] **Step 5: resync после изменения проектов** — чтобы вотчер знал о новых/удалённых проектах: в `src/lib/stores/projects.ts` в конце `loadProjects()` (после `projectsLoaded.set(true)`) добавить «мягкий» resync:
```ts
  // обновить вотчер папок под актуальный список (не критично при ошибке)
  import("../api/watch").then((w) => w.resync().catch(() => {}));
```
> Динамический import во избежание циклов; ошибки глушим.

- [ ] **Step 6: Проверка**
```powershell
npm run check
npm test
npm run build
```
Expected: 0 ошибок типов; Vitest 3; build успешен.

- [ ] **Step 7: Commit**
```powershell
git add src/lib/api/watch.ts src/lib/stores/changed.ts src/lib/components/Sidebar.svelte src/routes/+page.svelte src/lib/stores/projects.ts
git commit -m @'
feat(frontend): folder-change indicator in sidebar

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

## Task 4: Проверка среза

- [ ] **Step 1: Автопроверка**
```powershell
npm run check ; npm test ; cargo test --manifest-path src-tauri/Cargo.toml --lib
```
Expected: типы 0; Vitest 3; Rust 31.

- [ ] **Step 2: Ручной GUI-смоук (пользователь; перезапустить `npm run tauri dev`)**
- [ ] У проекта задан реальный путь к существующей папке.
- [ ] Изменить файл в этой папке (НЕ в node_modules/.git/target) во внешнем редакторе → у проекта в сайдбаре появляется точка «изменено» (через ~1 с).
- [ ] Открыть проект → точка гаснет.
- [ ] Изменения в `node_modules`/`.git`/`target` → точка НЕ появляется (фильтр шума).
- [ ] Создать новый проект с путём → его папка тоже отслеживается (resync).

---

## Итог среза

Папки проектов отслеживаются в фоне (`notify`) с фильтром артефактов; изменения подсвечиваются точкой в сайдбаре, гаснут при открытии. Первый v2.0-срез. Отложено: индикатор на дашборде, счётчик изменений, настройка списка игнорируемых папок, дебаунс-настройка.
```
