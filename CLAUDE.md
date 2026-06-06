# DevDeck

Локальный «командный центр» разработчика под Windows. Tauri 2 (Rust) + SvelteKit
(SPA, Svelte 5) + TypeScript + SQLite. Один пользователь, local-first, без облака
и телеметрии.

ТЗ: `TZ_DevDeck.md`. Дизайн реализации: `docs/superpowers/specs/`. Планы:
`docs/superpowers/plans/`. Прототип-референс (вёрстка/мок-данные): `_prototype/index.html`.

## Команды

- `npm run tauri dev` — запуск приложения в дев-режиме (открывает окно).
- `npm run tauri build` — сборка `.msi`/`.exe`.
- `npm run build` — сборка фронта (Vite/SvelteKit → `build/`), без окна.
- `npm run check` — проверка типов (svelte-check).
- `npm test` — Vitest (фронт; добавляется в Task 13).
- `cargo test --manifest-path src-tauri/Cargo.toml` — тесты Rust.

## Структура (SvelteKit SPA)

- `src/routes/` — экраны: `+page.svelte` (главный), `+layout.svelte` (обёртка,
  импорт global.css), `+layout.ts` (`ssr = false`). HTML-оболочка — `src/app.html`.
- `src/lib/` — общие модули (импорт через алиас `$lib`): компоненты —
  `src/lib/components/`, сторы — `src/lib/stores/`, типизированные вызовы бэкенда —
  `src/lib/api/`, дизайн-система — `src/lib/styles/global.css`.
- `src-tauri/src/` — Rust-ядро. Команды по доменам в `commands/`, доступ к БД в
  `db/`, миграции SQL в `src-tauri/migrations/`.

## Конвенции

- Компоненты НЕ вызывают `invoke()` напрямую — только через слой `src/lib/api/`.
- Rust-команды возвращают `Result<T, AppError>`; никаких `unwrap()`/`expect()`
  в командах. `AppError` сериализуется и показывается тостом на фронте.
- Состояние фронта — Svelte-сторы; состояние Rust — `tauri::State<AppState>`
  с `Mutex<Connection>` (rusqlite синхронный, пользователь один).
- Браузерные API (`document`, `localStorage`) в модулях гардить через
  `import { browser } from "$app/environment"` (SPA prerender-safe).
- Миграции БД — нумерованные `.sql` в `src-tauri/migrations/`, применяются
  раннером через `PRAGMA user_version`.
