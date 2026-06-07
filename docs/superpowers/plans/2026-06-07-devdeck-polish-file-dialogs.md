# DevDeck — Доработка «Файловые диалоги для экспорта/импорта» — план

> **Для исполнителя:** ОБЯЗАТЕЛЬНЫЙ ПОД-НАВЫК: superpowers:subagent-driven-development. Шаги — чекбоксы.

**Цель:** Экспорт — нативный диалог «Сохранить как…» (выбор пути/имени), импорт — нативный диалог «Открыть файл…». Текстовое поле для вставки JSON остаётся как запасной путь.

**Архитектура:** Чтение/запись файла остаётся в Rust (конвенция: секреты не покидают backend; IO в командах). Плагин `tauri-plugin-dialog` используется ТОЛЬКО для выбора пути на фронте. Новые Rust-команды `export_to_path(path, include_secrets)` и `import_from_path(path)` пишут/читают по выбранному пути и переиспользуют существующие `export_json`/`import_json`. Существующие `export_to_file`/`import_json` остаются (back-compat).

**Стек/границы:** backend — `Cargo.toml` (+плагин), `lib.rs` (init плагина + регистрация 2 команд), `commands/transfer.rs` (+2 команды). Frontend — npm-пакет `@tauri-apps/plugin-dialog`, `capabilities/default.json` (+`dialog:default`), `api/transfer.ts`, `AppSettings.svelte`. Без миграций, без новых Rust-тестов.

---

## Task 1: Rust — плагин dialog + команды по пути

**Files:** Modify `src-tauri/Cargo.toml`, `src-tauri/src/lib.rs`, `src-tauri/src/commands/transfer.rs`, `src-tauri/capabilities/default.json`.

- [ ] **Step 1: Cargo.toml** — в раздел зависимостей рядом с `tauri-plugin-opener = "2"` добавить:
```toml
tauri-plugin-dialog = "2"
```

- [ ] **Step 2: lib.rs — init плагина** — в цепочке билдера после `.plugin(tauri_plugin_opener::init())` добавить:
```rust
        .plugin(tauri_plugin_dialog::init())
```

- [ ] **Step 3: transfer.rs — команды по пути** — добавить после `export_to_file` (и до/после `import_json` — порядок не важен):
```rust
/// Экспорт в выбранный пользователем файл (путь приходит из нативного диалога).
#[tauri::command]
pub fn export_to_path(state: State<AppState>, path: String, include_secrets: bool) -> AppResult<String> {
    let json = export_json(state, include_secrets)?;
    std::fs::write(&path, json)
        .map_err(|e| AppError { kind: ErrorKind::Io, message: format!("Запись файла: {}", e) })?;
    Ok(path)
}

/// Импорт из выбранного пользователем файла (путь приходит из нативного диалога).
#[tauri::command]
pub fn import_from_path(state: State<AppState>, path: String) -> AppResult<ImportSummary> {
    let json = std::fs::read_to_string(&path)
        .map_err(|e| AppError { kind: ErrorKind::Io, message: format!("Чтение файла: {}", e) })?;
    import_json(state, json)
}
```
> `export_json`/`import_json` принимают `State` по значению и потребляют его — здесь это последнее использование `state`, поэтому компилируется (как в существующем `export_to_file`).

- [ ] **Step 4: lib.rs — регистрация** — в `generate_handler![...]` рядом с `commands::transfer::export_to_file,` добавить:
```rust
            commands::transfer::export_to_path,
            commands::transfer::import_from_path,
```

- [ ] **Step 5: capabilities/default.json** — добавить разрешение `dialog:default` в массив `permissions`:
```json
  "permissions": [
    "core:default",
    "opener:default",
    "dialog:default"
  ]
```

- [ ] **Step 6: Сборка + тесты**
```powershell
cargo build --manifest-path src-tauri/Cargo.toml --lib
cargo test --manifest-path src-tauri/Cargo.toml --lib
```
Expected: lib-сборка успешна (плагин подтянется), тесты 35 ok (новых нет). Первая сборка дольше из-за нового крейта — это норм.

- [ ] **Step 7: Commit**
```powershell
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/lib.rs src-tauri/src/commands/transfer.rs src-tauri/capabilities/default.json
git commit -m @'
feat(backend): export_to_path/import_from_path + dialog plugin

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

## Task 2: Фронт — нативные диалоги в настройках

**Files:** `package.json` (npm install), Modify `src/lib/api/transfer.ts`, `src/lib/components/AppSettings.svelte`.

- [ ] **Step 1: npm-пакет**
```powershell
npm install @tauri-apps/plugin-dialog@^2
```
(должно добавить зависимость в `package.json`/`package-lock.json`.)

- [ ] **Step 2: api/transfer.ts** — добавить:
```ts
export const exportToPath = (path: string, includeSecrets: boolean) => call<string>("export_to_path", { path, includeSecrets });
export const importFromPath = (path: string) => call<ImportSummary>("import_from_path", { path });
```

- [ ] **Step 3: AppSettings.svelte — диалоги**

1. В `<script>` добавить импорт плагина (рядом с прочими import):
```ts
  import { save, open } from "@tauri-apps/plugin-dialog";
```
2. Заменить `doExport` на версию с диалогом «Сохранить как…»:
```ts
  async function doExport() {
    const path = await save({ defaultPath: "devdeck-export.json", filters: [{ name: "JSON", extensions: ["json"] }] });
    if (!path) return;
    busy = true;
    try { const p = await transfer.exportToPath(path, includeSecrets); pushToast("Экспортировано", p, "ok"); }
    finally { busy = false; }
  }
```
3. Добавить функцию импорта из файла (рядом с `doImport`):
```ts
  async function doImportFile() {
    const sel = await open({ multiple: false, filters: [{ name: "JSON", extensions: ["json"] }] });
    if (!sel || Array.isArray(sel)) return;
    busy = true;
    try {
      const r = await transfer.importFromPath(sel);
      pushToast("Импортировано", `${r.projects} проект(ов)`, "ok");
      await loadProjects();
    } finally { busy = false; }
  }
```

- [ ] **Step 4: AppSettings.svelte — разметка**

1. Кнопку экспорта (label «Экспорт в файл (exports/)») заменить на:
```svelte
          <button class="btn-ghost" disabled={busy} onclick={doExport}><Icon name="download" class="ic-sm" /> Экспорт в файл…</button>
```
2. В поле импорта над `<textarea>` добавить кнопку «Импорт из файла…». Заменить:
```svelte
        <div class="field">
          <label for="imp">Импорт (вставьте JSON)</label>
          <textarea id="imp" class="tin mono" style="min-height:120px" placeholder={'{ "version": 1, "projects": [...] }'} bind:value={importText}></textarea>
```
на:
```svelte
        <div class="field">
          <label for="imp">Импорт</label>
          <div style="margin-bottom:8px"><button class="btn-ghost" disabled={busy} onclick={doImportFile}><Icon name="folder-open" class="ic-sm" /> Импорт из файла…</button></div>
          <textarea id="imp" class="tin mono" style="min-height:120px" placeholder={'или вставьте JSON: { "version": 1, "projects": [...] }'} bind:value={importText}></textarea>
```

- [ ] **Step 5: Проверка**
```powershell
npm run check
npm test
npm run build
```
Expected: 0 ошибок типов (`@tauri-apps/plugin-dialog` ставит свои типы; `folder-open`/`download` — валидные lucide); Vitest 3 (3 passed); build успешен.

- [ ] **Step 6: Commit**
```powershell
git add package.json package-lock.json src/lib/api/transfer.ts src/lib/components/AppSettings.svelte
git commit -m @'
feat(frontend): native save/open dialogs for export/import

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

## Task 3: Проверка

- [ ] **Step 1: Автопроверка**
```powershell
npm run check ; npm test ; cargo test --manifest-path src-tauri/Cargo.toml --lib
```
Expected: типы 0; Vitest 3; Rust 35.

- [ ] **Step 2: GUI-смоук (пользователь)**
- [ ] Настройки → Экспорт в файл… → нативный «Сохранить как» → файл создаётся по выбранному пути; тост с путём.
- [ ] Настройки → Импорт из файла… → выбрать ранее экспортированный JSON → проекты импортируются; список обновляется.
- [ ] Вставка JSON в textarea + «Импортировать» по-прежнему работает.

---

## Итог

Экспорт/импорт через нативные файловые диалоги (плагин dialog), запись/чтение остаются в Rust. Следующая доработка — фоновый режим команд (логи + остановка).
