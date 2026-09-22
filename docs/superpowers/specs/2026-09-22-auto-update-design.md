# Автообновление приложения

Дата: 2026-09-22
Статус: согласовано, к реализации

## Решения

- Релизы: публичный репозиторий `https://github.com/Mitisk/dev-deck`, GitHub Releases.
- Сборка: GitHub Actions по тегу `v*` (`tauri-apps/tauri-action`), Windows x64,
  MSI + NSIS, updater-артефакты подписаны; `latest.json` выкладывается в релиз.
- Проверка при запуске **включена** по умолчанию, выключается в настройках.
  Ручная кнопка «Проверить сейчас» доступна всегда.
- Индикатор обновления — в левом нижнем углу сайдбара (над футером).

## Backend / конфиг

- `Cargo.toml`: `tauri-plugin-updater = "2"`, `tauri-plugin-process = "2"`.
- `lib.rs`: `.plugin(tauri_plugin_updater::Builder::new().build())`, `.plugin(tauri_plugin_process::init())`.
- `capabilities/default.json`: `updater:default`, `process:default`.
- `tauri.conf.json`:
  - `"version": "../package.json"` — единственный источник версии; `npm version`
    поднимает её и ставит git-тег.
  - `bundle.createUpdaterArtifacts: true`.
  - `plugins.updater`: `pubkey` (из `~/.tauri/devdeck.key.pub`),
    `endpoints: ["https://github.com/Mitisk/dev-deck/releases/latest/download/latest.json"]`,
    `windows.installMode: "passive"`.
- Ключ подписи: `~/.tauri/devdeck.key` (вне репозитория). В секреты GitHub:
  `TAURI_SIGNING_PRIVATE_KEY` = содержимое файла, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` = пусто.

## Frontend

- `api/updater.ts`: `checkForUpdate() → UpdateInfo | null`, `downloadAndInstall(onProgress)`,
  `relaunchApp()`, `appVersion()`.
- `stores/updater.ts`: `updateState` (idle | checking | none | available | downloading |
  ready | error), `autoCheck` (localStorage `devdeck-auto-update-check`, default true),
  `checkForUpdate({silent})`, `installUpdate()`, `appVersion`.
- `+page.svelte`: после загрузки проектов, если `autoCheck`, через 3 секунды
  фоновая проверка (`silent: true` — без тостов об отсутствии/ошибке).
- `Sidebar.svelte`: блок `.sb-update` над футером, виден при `available` /
  `downloading` / `ready` / `error`:
  - available: иконка, «Доступна версия X», кнопка «Обновить»;
  - downloading: полоса прогресса и процент (или «Скачивание…»);
  - ready: «Перезапуск…»;
  - error: текст и кнопка «Повторить».
- `AppSettings.svelte`: блок «Обновления»: текущая версия, переключатель
  «Проверять при запуске», кнопка «Проверить сейчас», строка статуса.

## Выпуск версии

```
npm version minor        # 0.1.0 → 0.2.0, коммит + тег v0.2.0
git push --follow-tags   # запускает workflow Release
```

## Не-цели

- Каналы beta/stable, дельта-обновления, macOS/Linux.
