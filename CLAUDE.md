# DevDeck

Локальный «командный центр» разработчика под Windows. Tauri 2 (Rust) + SvelteKit
(SPA, Svelte 5) + TypeScript + SQLite. Один пользователь, local-first, без облака
и телеметрии.

ТЗ: `TZ_DevDeck.md`. Дизайн реализации: `docs/superpowers/specs/`. Планы:
`docs/superpowers/plans/`. Прототип-референс (вёрстка/мок-данные): `_prototype/index.html`.
Папка `docs/` в git не отслеживается (в `.gitignore`) — спеки и планы только локальные.

Репозиторий: `https://github.com/Mitisk/dev-deck` (публичный), ветка `main`.
Фичи делаются в ветках `feat/*`, сливаются в `main` через `git merge --no-ff`.

## Команды

- `npm run tauri dev` — запуск приложения в дев-режиме (открывает окно).
- `npm run tauri build` — сборка `.msi`/`.exe`.
- `npm run build` — сборка фронта (Vite/SvelteKit → `build/`), без окна.
- `npm run check` — проверка типов (svelte-check).
- `npm test` — Vitest (фронт).
- `cargo test --manifest-path src-tauri/Cargo.toml --lib` — тесты Rust (`--lib`
  обязателен, пока запущен dev-экземпляр: иначе exe заблокирован Windows).

## Релизы и автообновление

- Версия хранится **только в `package.json`**; `tauri.conf.json` читает её через
  `"version": "../package.json"`. Версию в `src-tauri/Cargo.toml` не трогать.
- Выпуск: `npm version minor` (или `patch`/`major`) → коммит + тег `vX.Y.Z` →
  `git push --follow-tags origin main`. Тег запускает `.github/workflows/release.yml`
  (tauri-action, windows-latest), который собирает MSI/NSIS, подписывает
  updater-артефакты и публикует GitHub Release с `latest.json`.
- `npm version` требует чистого рабочего дерева: если есть незакоммиченные файлы,
  сначала `git stash push <файл>`, после — `git stash pop`.
- Приложение при запуске (через 3 с, если включено в Настройках) и по кнопке
  «Проверить сейчас» читает `https://github.com/Mitisk/dev-deck/releases/latest/download/latest.json`.
  Найденное обновление показывается плашкой в левом нижнем углу сайдбара
  (`Sidebar.svelte`, стор `src/lib/stores/updater.ts`, обёртка `src/lib/api/updater.ts`).
- Подпись: ключ minisign `~/.tauri/devdeck.key` (вне репозитория, без пароля),
  открытый ключ зашит в `tauri.conf.json` → `plugins.updater.pubkey`. В секретах
  GitHub Actions только `TAURI_SIGNING_PRIVATE_KEY` (содержимое файла ключа).
  Потеря ключа = новые обновления подписать нельзя (нужна новая пара + ручная
  переустановка у пользователей).
- Локальный `npm run tauri build` тоже требует ключ, иначе падает после бандла:
  `export TAURI_SIGNING_PRIVATE_KEY="$(cat ~/.tauri/devdeck.key)"` (Git Bash).
  Вариант `TAURI_SIGNING_PRIVATE_KEY_PATH` с tauri-cli 2.11 не сработал.
- Профиль `[profile.release]` в `Cargo.toml` настроен на размер (opt-level "s",
  LTO, strip, panic=abort): релизная сборка идёт ~6 минут, exe ≈ 8 МБ.
- Первый релиз: v0.2.0 (2026-09-22). Установленная вручную 0.1.0 обновляться сама
  не умеет — её нужно один раз переустановить из релиза.

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
