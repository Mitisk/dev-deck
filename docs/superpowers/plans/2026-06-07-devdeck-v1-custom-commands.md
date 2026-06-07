# DevDeck v1.0 — Срез «Кастомные команды-кнопки» — план реализации

> **Для исполнителя:** ОБЯЗАТЕЛЬНЫЙ ПОД-НАВЫК: superpowers:subagent-driven-development. Шаги — чекбоксы.

**Цель:** «Фишка» из ТЗ — пользователь добавляет кнопки со своими shell-командами (`npm run dev`, `cargo run`, `docker compose up -d`) и запускает их одним кликом. В этом срезе — запуск **в терминале** (новое окно cmd в рабочей папке). Управление командами — в «Обзоре».

**Архитектура:** Команды `commands_*` (таблица `commands`) + `command_run(id)`. `command_run` запускает `cmd /K <command>` в рабочей папке (если пуста — путь проекта) новым консольным окном. Фронт: секция «Команды» в `OverviewTab.svelte` (сетка кнопок-карточек) + диалог добавления/правки.

**Важно (намеренный дизайн):** `command_run` ИСПОЛНЯЕТ shell-команду, которую пользователь сам настроил — это и есть суть фичи (раннер собственных команд). Это не уязвимость инъекции: источник команды — сам пользователь, как в обычном терминале. (Будущий импорт JSON принёс бы команды из недоверенного источника — тогда понадобится подтверждение перед запуском; отмечено.)

**Стек:** как раньше. Без новой миграции (таблица `commands` уже в 0001).

**Решения / границы:**
- Запуск **в терминале** (видно вывод). **Фоновый режим** (захват stdout в панель логов + кнопка «стоп», команды `command_run` background / `command_stop`) — **отложен** (требует реестра дочерних процессов + событий stdout); отдельный срез.
- Иконка кнопки — фикс `play` (пикер — позже). Цвет — из проекта.
- Рабочая папка: из поля `working_dir`; если пусто — `path` проекта.

**Источники:** `TZ_DevDeck.md` (5.4 — кастомные команды; 10 — `commands_*`, `command_run`). `_prototype/index.html` (`.cmd-grid`/`.cmd`/`overviewHTML` — разметка-референс; таблица `commands` в разделе 4). Текущий `OverviewTab.svelte` (из среза ссылок/файлов), `actions.rs` (`CREATE_NEW_CONSOLE`).

---

## Контекст

- Rust: 49 команд; `models.rs`, `commands/{...}.rs`, `error`. Таблица `commands`: `id, project_id, label, command, working_dir, run_in default 'terminal', icon, sort_order`. Константа `CREATE_NEW_CONSOLE` определена приватно в `actions.rs` (продублируем в новом модуле).
- Фронт: `OverviewTab.svelte` (описание + ссылки + файлы). api `client.ts`, `Icon.svelte`. global.css: `.cmd-grid`, `.cmd`(`.ico`/`.lbl`/`.run`/`.play`), модалка.

---

## Структура файлов

```
src-tauri/src/
├─ models.rs            # MOD: + ProjectCommand, CommandInput
├─ commands/cmds.rs     # NEW: commands_* + command_run (terminal)
├─ commands/mod.rs      # MOD: + pub mod cmds;
└─ lib.rs               # MOD: регистрация 5 команд

src/lib/
├─ types.ts             # MOD: + ProjectCommand
├─ api/commands.ts      # NEW
└─ components/
   └─ OverviewTab.svelte# MOD: секция «Команды» + диалог
```

---

## Task 1: Rust — команды и запуск

**Files:** Modify `models.rs`, `commands/mod.rs`, `lib.rs`; Create `commands/cmds.rs`.

- [ ] **Step 1: Модели** (в `models.rs`, после `FileShortcut`):
```rust
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCommand {
    pub id: i64,
    pub project_id: i64,
    pub label: String,
    pub command: String,
    pub working_dir: Option<String>,
    pub run_in: String, // terminal | background
    pub icon: Option<String>,
    pub sort_order: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandInput {
    pub label: String,
    pub command: String,
    pub working_dir: Option<String>,
    pub run_in: Option<String>,
    pub icon: Option<String>,
}
```

- [ ] **Step 2: commands/cmds.rs**

Create `src-tauri/src/commands/cmds.rs`:
```rust
use crate::error::{AppError, AppResult, ErrorKind};
use crate::models::{CommandInput, ProjectCommand};
use crate::state::AppState;
use rusqlite::{params, Connection};
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::Command;
use std::sync::MutexGuard;
use tauri::State;

const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;

fn lock<'a>(state: &'a State<AppState>) -> AppResult<MutexGuard<'a, Connection>> {
    state.db.lock().map_err(|_| AppError::internal("db mutex poisoned"))
}

fn expand(p: &str) -> String {
    let t = p.trim();
    if let Some(rest) = t.strip_prefix("~/").or_else(|| t.strip_prefix("~\\")) {
        if let Ok(home) = std::env::var("USERPROFILE") {
            return format!("{}\\{}", home.trim_end_matches('\\'), rest.replace('/', "\\"));
        }
    }
    t.to_string()
}

fn row_to_cmd(conn: &Connection, id: i64) -> AppResult<ProjectCommand> {
    Ok(conn.query_row(
        "SELECT id, project_id, label, command, working_dir, run_in, icon, sort_order FROM commands WHERE id = ?1",
        [id],
        |r| Ok(ProjectCommand {
            id: r.get(0)?,
            project_id: r.get(1)?,
            label: r.get(2)?,
            command: r.get(3)?,
            working_dir: r.get(4)?,
            run_in: r.get(5)?,
            icon: r.get(6)?,
            sort_order: r.get(7)?,
        }),
    )?)
}

#[tauri::command]
pub fn commands_list(state: State<AppState>, project_id: i64) -> AppResult<Vec<ProjectCommand>> {
    let conn = lock(&state)?;
    let mut stmt = conn.prepare(
        "SELECT id FROM commands WHERE project_id = ?1 ORDER BY sort_order ASC, id ASC",
    )?;
    let ids = stmt.query_map([project_id], |r| r.get::<_, i64>(0))?;
    let mut out = Vec::new();
    for id in ids {
        out.push(row_to_cmd(&conn, id?)?);
    }
    Ok(out)
}

#[tauri::command]
pub fn commands_create(state: State<AppState>, project_id: i64, input: CommandInput) -> AppResult<ProjectCommand> {
    if input.label.trim().is_empty() || input.command.trim().is_empty() {
        return Err(AppError { kind: ErrorKind::Validation, message: "Ярлык и команда обязательны".into() });
    }
    let conn = lock(&state)?;
    let next: i64 = conn.query_row("SELECT COALESCE(MAX(sort_order),0)+1 FROM commands WHERE project_id=?1", [project_id], |r| r.get(0))?;
    conn.execute(
        "INSERT INTO commands(project_id, label, command, working_dir, run_in, icon, sort_order)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            project_id,
            input.label.trim(),
            input.command.trim(),
            input.working_dir,
            input.run_in.as_deref().unwrap_or("terminal"),
            input.icon,
            next,
        ],
    )?;
    row_to_cmd(&conn, conn.last_insert_rowid())
}

#[tauri::command]
pub fn commands_update(state: State<AppState>, id: i64, input: CommandInput) -> AppResult<ProjectCommand> {
    if input.label.trim().is_empty() || input.command.trim().is_empty() {
        return Err(AppError { kind: ErrorKind::Validation, message: "Ярлык и команда обязательны".into() });
    }
    let conn = lock(&state)?;
    conn.execute(
        "UPDATE commands SET label=?2, command=?3, working_dir=?4, run_in=?5, icon=?6 WHERE id=?1",
        params![
            id,
            input.label.trim(),
            input.command.trim(),
            input.working_dir,
            input.run_in.as_deref().unwrap_or("terminal"),
            input.icon,
        ],
    )?;
    row_to_cmd(&conn, id)
}

#[tauri::command]
pub fn commands_delete(state: State<AppState>, id: i64) -> AppResult<()> {
    let conn = lock(&state)?;
    conn.execute("DELETE FROM commands WHERE id=?1", [id])?;
    Ok(())
}

/// Запустить кастомную команду в новом окне терминала (cmd /K) в рабочей папке.
/// Команда — это намеренно shell-строка, настроенная пользователем (раннер своих команд).
#[tauri::command]
pub fn command_run(state: State<AppState>, id: i64) -> AppResult<()> {
    // достаём команду, рабочую папку и путь проекта (для фолбэка) под локом, затем отпускаем лок
    let (command, working_dir, project_path): (String, Option<String>, Option<String>) = {
        let conn = lock(&state)?;
        conn.query_row(
            "SELECT c.command, c.working_dir, p.path
             FROM commands c JOIN projects p ON p.id = c.project_id
             WHERE c.id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?
    };
    if command.trim().is_empty() {
        return Err(AppError { kind: ErrorKind::Validation, message: "Команда пуста".into() });
    }
    // рабочая папка: working_dir, иначе путь проекта
    let raw_dir = working_dir
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .or(project_path.as_deref().map(str::trim).filter(|s| !s.is_empty()));

    let mut cmd = Command::new("cmd");
    cmd.args(["/K", command.trim()]);
    if let Some(d) = raw_dir {
        let dir = expand(d);
        if Path::new(&dir).is_dir() {
            cmd.current_dir(&dir);
        }
    }
    cmd.creation_flags(CREATE_NEW_CONSOLE);
    cmd.spawn()
        .map(|_| ())
        .map_err(|e| AppError { kind: ErrorKind::Io, message: format!("Не удалось запустить команду: {}", e) })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    fn mem() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        db::migrations::run(&conn).unwrap();
        conn
    }

    #[test]
    fn create_list_delete_commands() {
        let conn = mem();
        conn.execute("INSERT INTO projects(name,status,sort_order) VALUES('P','active',0)", []).unwrap();
        let pid = conn.last_insert_rowid();

        conn.execute(
            "INSERT INTO commands(project_id,label,command,run_in,sort_order) VALUES(?1,'Dev','npm run dev','terminal',0)",
            [pid],
        ).unwrap();
        let id = conn.last_insert_rowid();

        let c = row_to_cmd(&conn, id).unwrap();
        assert_eq!(c.label, "Dev");
        assert_eq!(c.command, "npm run dev");
        assert_eq!(c.run_in, "terminal");

        conn.execute("DELETE FROM commands WHERE id=?1", [id]).unwrap();
        let cnt: i64 = conn.query_row("SELECT count(*) FROM commands WHERE project_id=?1", [pid], |r| r.get(0)).unwrap();
        assert_eq!(cnt, 0);
    }
}
```

- [ ] **Step 3: Регистрация**
- `commands/mod.rs`: `pub mod cmds;`
- `lib.rs` `generate_handler![...]`: добавить
```rust
            commands::cmds::commands_list,
            commands::cmds::commands_create,
            commands::cmds::commands_update,
            commands::cmds::commands_delete,
            commands::cmds::command_run,
```

- [ ] **Step 4: Тесты + сборка**
```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib
cargo build --manifest-path src-tauri/Cargo.toml
```
Expected: новый `create_list_delete_commands` + прежние (17) → 18 ok; build успешен.

- [ ] **Step 5: Commit**
```powershell
git add src-tauri/src/models.rs src-tauri/src/commands/cmds.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs
git commit -m @'
feat(backend): custom commands CRUD + run-in-terminal

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

## Task 2: Фронт — секция «Команды» в OverviewTab

**Files:** Modify `types.ts`, `components/OverviewTab.svelte`; Create `api/commands.ts`.

- [ ] **Step 1: Тип** (в конец `types.ts`):
```ts
export type ProjectCommand = {
  id: number;
  projectId: number;
  label: string;
  command: string;
  workingDir: string | null;
  runIn: string;
  icon: string | null;
  sortOrder: number;
};
```

- [ ] **Step 2: api/commands.ts**
```ts
import { call } from "./client";
import type { ProjectCommand } from "../types";

export type CommandInput = {
  label: string;
  command: string;
  workingDir?: string | null;
  runIn?: string | null;
  icon?: string | null;
};

export const list = (projectId: number) => call<ProjectCommand[]>("commands_list", { projectId });
export const create = (projectId: number, input: CommandInput) => call<ProjectCommand>("commands_create", { projectId, input });
export const update = (id: number, input: CommandInput) => call<ProjectCommand>("commands_update", { id, input });
export const remove = (id: number) => call<void>("commands_delete", { id });
export const run = (id: number) => call<void>("command_run", { id });
```

- [ ] **Step 3: Обновить OverviewTab.svelte (добавить секцию «Команды» вверху + диалог)**

Заменить ВЕСЬ `src/lib/components/OverviewTab.svelte` на (это прежний OverviewTab + новая секция команд и диалог):
```svelte
<script lang="ts">
  import type { Project, Link, FileShortcut, ProjectCommand } from "$lib/types";
  import * as linksApi from "$lib/api/links";
  import * as filesApi from "$lib/api/files";
  import * as cmdsApi from "$lib/api/commands";
  import * as actions from "$lib/api/actions";
  import { pushToast } from "$lib/stores/toasts";
  import Icon from "./Icon.svelte";

  let { project }: { project: Project } = $props();

  let links = $state<Link[]>([]);
  let files = $state<FileShortcut[]>([]);
  let cmds = $state<ProjectCommand[]>([]);
  let newLink = $state({ label: "", url: "" });
  let newFile = $state({ label: "", path: "" });

  let reqId = 0;
  async function load() {
    const my = ++reqId;
    try {
      const [l, f, c] = await Promise.all([
        linksApi.list(project.id),
        filesApi.list(project.id),
        cmdsApi.list(project.id),
      ]);
      if (my === reqId) {
        links = l;
        files = f;
        cmds = c;
      }
    } catch {
      /* тост из api/client.ts */
    }
  }
  $effect(() => {
    project.id;
    load();
  });

  // --- команды ---
  let editing = $state<ProjectCommand | null>(null);
  let isNew = $state(false);
  let eLabel = $state("");
  let eCommand = $state("");
  let eDir = $state("");

  function openNewCmd() {
    isNew = true;
    editing = { id: 0, projectId: project.id, label: "", command: "", workingDir: null, runIn: "terminal", icon: "play", sortOrder: 0 };
    eLabel = ""; eCommand = ""; eDir = "";
  }
  function openEditCmd(c: ProjectCommand) {
    isNew = false;
    editing = c;
    eLabel = c.label; eCommand = c.command; eDir = c.workingDir ?? "";
  }
  async function saveCmd() {
    if (!editing) return;
    if (!eLabel.trim() || !eCommand.trim()) return;
    const input = { label: eLabel.trim(), command: eCommand.trim(), workingDir: eDir.trim() || null, runIn: "terminal", icon: "play" };
    if (isNew) await cmdsApi.create(project.id, input);
    else await cmdsApi.update(editing.id, input);
    editing = null;
    await load();
  }
  async function delCmd() {
    if (!editing) return;
    const id = editing.id;
    editing = null;
    await cmdsApi.remove(id);
    await load();
  }
  async function runCmd(c: ProjectCommand) {
    await cmdsApi.run(c.id);
    pushToast("Запуск", c.command, "info");
  }

  // --- ссылки/файлы ---
  async function addLink() {
    if (!newLink.label.trim() || !newLink.url.trim()) return;
    await linksApi.create(project.id, newLink.label.trim(), newLink.url.trim());
    newLink = { label: "", url: "" };
    await load();
  }
  async function delLink(id: number) { await linksApi.remove(id); await load(); }
  async function addFile() {
    if (!newFile.label.trim() || !newFile.path.trim()) return;
    await filesApi.create(project.id, newFile.label.trim(), newFile.path.trim());
    newFile = { label: "", path: "" };
    await load();
  }
  async function delFile(id: number) { await filesApi.remove(id); await load(); }
</script>

<div class="stack">
  {#if project.description}
    <div>
      <h3 class="section-title">Описание</h3>
      <p class="desc">{project.description}</p>
    </div>
  {/if}

  <div>
    <h3 class="section-title">
      <Icon name="terminal" class="ic-sm" /> Команды
      <button class="more" onclick={openNewCmd}>+ команда</button>
    </h3>
    {#if cmds.length}
      <div class="cmd-grid">
        {#each cmds as c (c.id)}
          <button class="cmd" style="--c-tint:{project.color ?? 'var(--accent)'}" onclick={() => runCmd(c)}>
            <span class="ico"><Icon name={c.icon ?? "play"} class="ic" /></span>
            <span class="lbl">{c.label}</span>
            <span class="run mono">{c.command}</span>
            <span class="play" role="button" tabindex="-1"
                  onclick={(e) => { e.stopPropagation(); openEditCmd(c); }}><Icon name="pencil" class="ic-sm" /></span>
          </button>
        {/each}
      </div>
    {:else}
      <button class="add-cred" onclick={openNewCmd}><Icon name="plus" class="ic-sm" /> Добавить команду (напр. npm run dev)</button>
    {/if}
  </div>

  <div>
    <h3 class="section-title"><Icon name="link" class="ic-sm" /> Ссылки</h3>
    <div class="card links">
      {#each links as l (l.id)}
        <div class="link-row">
          <span class="lico" role="button" tabindex="0" onclick={() => actions.openUrl(l.url)}><Icon name={l.icon ?? "globe"} class="ic-sm" /></span>
          <div style="flex:1;min-width:0;cursor:pointer" role="button" tabindex="0" onclick={() => actions.openUrl(l.url)}>
            <div class="lt">{l.label}</div>
            <div class="lu">{l.url}</div>
          </div>
          <button class="mini" title="Открыть" onclick={() => actions.openUrl(l.url)}><Icon name="external-link" class="ic-sm" /></button>
          <button class="mini" title="Удалить" onclick={() => delLink(l.id)}><Icon name="x" class="ic-sm" /></button>
        </div>
      {/each}
      <div class="erow" style="margin:8px 4px 4px">
        <input class="tin" placeholder="Название" bind:value={newLink.label} />
        <input class="tin url" placeholder="localhost:3000 / github.com/…" bind:value={newLink.url}
               onkeydown={(e) => { if (e.key === 'Enter') addLink(); }} />
        <button class="er-del" style="color:var(--accent)" title="Добавить" onclick={addLink}><Icon name="plus" class="ic-sm" /></button>
      </div>
    </div>
  </div>

  <div>
    <h3 class="section-title"><Icon name="folder" class="ic-sm" /> Файлы и папки</h3>
    <div class="card links">
      {#each files as f (f.id)}
        <div class="link-row">
          <span class="lico" role="button" tabindex="0" onclick={() => actions.openShortcut(f.path)}><Icon name="file" class="ic-sm" /></span>
          <div style="flex:1;min-width:0;cursor:pointer" role="button" tabindex="0" onclick={() => actions.openShortcut(f.path)}>
            <div class="lt">{f.label}</div>
            <div class="lu">{f.path}</div>
          </div>
          <button class="mini" title="Открыть" onclick={() => actions.openShortcut(f.path)}><Icon name="external-link" class="ic-sm" /></button>
          <button class="mini" title="Удалить" onclick={() => delFile(f.id)}><Icon name="x" class="ic-sm" /></button>
        </div>
      {/each}
      <div class="erow" style="margin:8px 4px 4px">
        <input class="tin" placeholder="Название" bind:value={newFile.label} />
        <input class="tin url" placeholder="~/dev/proj/.env" bind:value={newFile.path}
               onkeydown={(e) => { if (e.key === 'Enter') addFile(); }} />
        <button class="er-del" style="color:var(--accent)" title="Добавить" onclick={addFile}><Icon name="plus" class="ic-sm" /></button>
      </div>
    </div>
  </div>
</div>

{#if editing}
  <div class="modal-scrim open" role="dialog" tabindex="-1" aria-label="Команда"
       onmousedown={(e) => { if (e.currentTarget === e.target) (editing = null); }}
       onkeydown={(e) => { if (e.key === 'Escape') (editing = null); }}>
    <div class="modal">
      <div class="modal-head">
        <span class="t">{isNew ? "Новая команда" : "Команда"}</span>
        <button class="icon-btn x" onclick={() => (editing = null)}><Icon name="x" class="ic" /></button>
      </div>
      <div class="modal-body">
        <div class="field"><label for="cm-label">Ярлык</label><input id="cm-label" class="tin" placeholder="Запустить dev" bind:value={eLabel} /></div>
        <div class="field"><label for="cm-cmd">Команда (shell)</label><input id="cm-cmd" class="tin mono" placeholder="npm run dev" bind:value={eCommand} /></div>
        <div class="field"><label for="cm-dir">Рабочая папка <span style="color:var(--muted-2)">(пусто = папка проекта)</span></label>
          <input id="cm-dir" class="tin mono" placeholder={project.path ?? "~/dev/project"} bind:value={eDir} /></div>
        <p class="desc" style="color:var(--muted-2);font-size:12px">Запуск открывает новое окно терминала. Фоновый режим с логами — позже.</p>
      </div>
      <div class="modal-foot">
        {#if !isNew}<button class="btn-danger" onclick={delCmd}><Icon name="trash-2" class="ic-sm" /> Удалить</button>{/if}
        <span class="spacer"></span>
        <button class="btn-ghost" onclick={() => (editing = null)}>Отмена</button>
        <button class="btn-primary" onclick={saveCmd}><Icon name="check" class="ic ic-sm" /> Сохранить</button>
      </div>
    </div>
  </div>
{/if}
```
> Иконки `terminal`/`play`/`pencil`/`plus`/`link`/`globe`/`folder`/`file`/`external-link`/`x`/`trash-2`/`check` — из lucide. Классы `.cmd-grid`/`.cmd`(`.ico`/`.lbl`/`.run`/`.play`)/`.section-title .more`/`.add-cred`/модалка — в global.css.

- [ ] **Step 4: Проверка**
```powershell
npm run check
npm test
npm run build
```
Expected: 0 ошибок типов; Vitest 3 (при редком транзиенте — перезапустить); build успешен.

- [ ] **Step 5: Commit**
```powershell
git add src/lib/types.ts src/lib/api/commands.ts src/lib/components/OverviewTab.svelte
git commit -m @'
feat(frontend): custom command buttons in overview (run in terminal)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

## Task 3: Проверка среза

- [ ] **Step 1: Автопроверка**
```powershell
npm run check ; npm test ; cargo test --manifest-path src-tauri/Cargo.toml --lib
```
Expected: типы 0 ошибок; Vitest 3; Rust 18.

- [ ] **Step 2: Ручной GUI-смоук (пользователь; `npm run tauri dev`)**
- [ ] «Обзор» → раздел «Команды» → «+ команда» → ярлык «Dev», команда `npm run dev` (или `dir`/`echo hi` для теста), рабочая папка пустая → «Сохранить».
- [ ] Клик по карточке команды → открывается новое окно терминала в папке проекта и выполняет команду; тост «Запуск».
- [ ] Иконка-карандаш на карточке → правка/удаление команды.
- [ ] Рабочая папка задана явно → терминал открывается в ней.
- [ ] Перезапуск приложения → команды на месте.

---

## Итог среза

«Фишка» работает: кнопки своих shell-команд в «Обзоре», запуск одним кликом в терминале (в папке проекта или заданной). Команды/ссылки/файлы наполнили «Обзор». Отложено: фоновый режим (захват stdout в панель логов + «стоп»), пикер иконок/цвета кнопки, подтверждение запуска для импортированных команд.
```
