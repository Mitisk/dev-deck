# DevDeck

Локальный «командный центр» разработчика под Windows: проекты, задачи, чеклисты,
креды, заметки, git-статус и быстрые команды. Tauri 2 (Rust) + SvelteKit (Svelte 5)
+ SQLite. Один пользователь, local-first, без облака и телеметрии.

## Разработка

```
npm install
npm run tauri dev          # окно приложения в дев-режиме
npm run check              # svelte-check
npm test                   # Vitest
cargo test --manifest-path src-tauri/Cargo.toml --lib
npm run tauri build        # .msi / .exe в src-tauri/target/release/bundle
```

## Выпуск версии и автообновление

Приложение при запуске (и по кнопке в Настройках) читает
`https://github.com/Mitisk/dev-deck/releases/latest/download/latest.json`,
сравнивает версию со своей и предлагает обновиться. Установщик подписан
minisign-ключом, открытый ключ зашит в `src-tauri/tauri.conf.json`.

Версия берётся из `package.json` (Tauri читает её через `"version": "../package.json"`).

Чтобы выпустить новую версию:

```
npm version minor          # или patch / major: поднимает версию, коммит + тег vX.Y.Z
git push --follow-tags     # тег запускает workflow .github/workflows/release.yml
```

Workflow собирает Windows-сборку, подписывает updater-артефакты, создаёт GitHub
Release с установщиками и `latest.json`. Через несколько минут после публикации
приложения у пользователей увидят обновление в левом нижнем углу.

### Секреты репозитория (один раз)

В GitHub → Settings → Secrets and variables → Actions:

| Секрет | Значение |
|---|---|
| `TAURI_SIGNING_PRIVATE_KEY` | содержимое файла `~/.tauri/devdeck.key` целиком |

Локальная релизная сборка тоже требует ключ (иначе `npm run tauri build` падает на
подписи updater-артефактов), в Git Bash:

```
export TAURI_SIGNING_PRIVATE_KEY="$(cat ~/.tauri/devdeck.key)" TAURI_SIGNING_PRIVATE_KEY_PASSWORD=""
npm run tauri build
```

Закрытый ключ хранится только локально в `~/.tauri/devdeck.key`. Если его потерять,
подписать новые обновления будет нельзя: придётся сгенерировать новую пару
(`npx tauri signer generate -w ~/.tauri/devdeck.key --ci -p ""`), заменить `pubkey`
в `tauri.conf.json` и попросить пользователей переустановить приложение вручную.
