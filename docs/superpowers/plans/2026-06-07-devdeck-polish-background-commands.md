# DevDeck — Доработка «Фоновый режим команд (логи + остановка)» — план

> **Для исполнителя:** ОБЯЗАТЕЛЬНЫЙ ПОД-НАВЫК: superpowers:subagent-driven-development. Шаги — чекбоксы.

**Цель:** Команды с `run_in = "background"` запускаются скрыто (без окна терминала), их stdout/stderr стримятся в панель логов в реальном времени, есть кнопка «Стоп» (убивает дерево процессов). Команды с `run_in = "terminal"` работают как раньше (новое окно консоли).

**Архитектура:**
- Backend: managed-реестр `RunningState { procs: Mutex<HashMap<i64, u32>> }` (id команды → OS pid). `command_run_bg` спавнит `cmd /C <команда>` с `CREATE_NO_WINDOW` и пайпами, поток-супервизор читает stdout (+поток stderr), эмитит события `cmd-log {id,line,err}` построчно, по выходу — `cmd-exit {id,code}` и чистит реестр. `command_stop` убивает дерево через `taskkill /PID <pid> /T /F` (нужно для `npm run dev` и т.п.). `command_running` отдаёт список запущенных id.
- Frontend: редактор команды получает выбор режима (терминал/фон). В оверви — для фоновых команд кнопки «логи» и «стоп», индикатор запуска. Панель логов (модалка) слушает `cmd-log`/`cmd-exit`.

**Стек/границы:** backend — `commands/cmds.rs` (+3 команды +RunningState), `lib.rs` (manage + регистрация). Frontend — `api/commands.ts`, `OverviewTab.svelte`. Без миграций (поле `run_in` уже есть). Без новых Rust-тестов (процессы/потоки не юнит-тестируются; проверка — GUI-смоук).

---

## Task 1: Rust — фоновый запуск, стоп, реестр

**Files:** Modify `src-tauri/src/commands/cmds.rs`, `src-tauri/src/lib.rs`.

- [ ] **Step 1: Импорты и константа в `cmds.rs`** — расширить шапку. Текущее:
```rust
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::Command;
use std::sync::MutexGuard;
use tauri::State;

const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;
```
заменить на:
```rust
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, MutexGuard};
use std::thread;
use tauri::{AppHandle, Emitter, Manager, State};

const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Реестр запущенных фоновых команд: id команды → OS pid дочернего процесса.
#[derive(Default)]
pub struct RunningState {
    pub procs: Mutex<HashMap<i64, u32>>,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct LogLine {
    id: i64,
    line: String,
    err: bool,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ExitInfo {
    id: i64,
    code: Option<i32>,
}
```

- [ ] **Step 2: Вынести получение команды/папки в helper** — чтобы переиспользовать в terminal и background. Перед `command_run` добавить:
```rust
/// Достать (команда, рабочая папка с учётом фолбэка на путь проекта) под локом БД.
fn fetch_command(state: &State<AppState>, id: i64) -> AppResult<(String, Option<String>)> {
    let (command, working_dir, project_path): (String, Option<String>, Option<String>) = {
        let conn = lock(state)?;
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
    let dir = working_dir
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .or(project_path.as_deref().map(str::trim).filter(|s| !s.is_empty()))
        .map(|s| s.to_string());
    Ok((command, dir))
}
```
Затем переписать тело `command_run`, чтобы использовать helper (поведение прежнее — новое окно `cmd /K`):
```rust
/// Запустить кастомную команду в новом окне терминала (cmd /K) в рабочей папке.
/// Команда — намеренно shell-строка пользователя (раннер своих команд).
#[tauri::command]
pub fn command_run(state: State<AppState>, id: i64) -> AppResult<()> {
    let (command, dir) = fetch_command(&state, id)?;
    let mut cmd = Command::new("cmd");
    cmd.args(["/K", command.trim()]);
    if let Some(d) = dir {
        let dir = expand(&d);
        if Path::new(&dir).is_dir() {
            cmd.current_dir(&dir);
        }
    }
    cmd.creation_flags(CREATE_NEW_CONSOLE);
    cmd.spawn()
        .map(|_| ())
        .map_err(|e| AppError { kind: ErrorKind::Io, message: format!("Не удалось запустить команду: {}", e) })
}
```

- [ ] **Step 3: `command_run_bg` / `command_stop` / `command_running`** — добавить после `command_run`:
```rust
/// Запустить команду в фоне: скрытый процесс, stdout/stderr стримятся событиями
/// `cmd-log`, по завершении — `cmd-exit`. Идемпотентность по id: повторный запуск
/// уже работающей команды отклоняется.
#[tauri::command]
pub fn command_run_bg(
    state: State<AppState>,
    running: State<RunningState>,
    app: AppHandle,
    id: i64,
) -> AppResult<()> {
    {
        let map = running.procs.lock().map_err(|_| AppError::internal("proc mutex poisoned"))?;
        if map.contains_key(&id) {
            return Err(AppError { kind: ErrorKind::Validation, message: "Команда уже запущена".into() });
        }
    }
    let (command, dir) = fetch_command(&state, id)?;

    let mut cmd = Command::new("cmd");
    cmd.args(["/C", command.trim()]);
    if let Some(d) = dir {
        let dir = expand(&d);
        if Path::new(&dir).is_dir() {
            cmd.current_dir(&dir);
        }
    }
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    cmd.creation_flags(CREATE_NO_WINDOW);

    let mut child: Child = cmd
        .spawn()
        .map_err(|e| AppError { kind: ErrorKind::Io, message: format!("Не удалось запустить команду: {}", e) })?;
    let pid = child.id();
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    {
        let mut map = running.procs.lock().map_err(|_| AppError::internal("proc mutex poisoned"))?;
        map.insert(id, pid);
    }

    // поток чтения stderr
    let app_err = app.clone();
    let stderr_handle = thread::spawn(move || {
        if let Some(out) = stderr {
            let reader = BufReader::new(out);
            for line in reader.lines().map_while(Result::ok) {
                let _ = app_err.emit("cmd-log", LogLine { id, line, err: true });
            }
        }
    });

    // поток-супервизор: читает stdout, ждёт stderr, reap, эмитит exit, чистит реестр
    let app2 = app.clone();
    thread::spawn(move || {
        if let Some(out) = stdout {
            let reader = BufReader::new(out);
            for line in reader.lines().map_while(Result::ok) {
                let _ = app2.emit("cmd-log", LogLine { id, line, err: false });
            }
        }
        let _ = stderr_handle.join();
        let code = child.wait().ok().and_then(|s| s.code());
        if let Ok(mut map) = app2.state::<RunningState>().procs.lock() {
            map.remove(&id);
        }
        let _ = app2.emit("cmd-exit", ExitInfo { id, code });
    });

    Ok(())
}

/// Остановить фоновую команду: убить дерево процессов по pid (taskkill /T /F).
#[tauri::command]
pub fn command_stop(running: State<RunningState>, id: i64) -> AppResult<()> {
    let pid = {
        let map = running.procs.lock().map_err(|_| AppError::internal("proc mutex poisoned"))?;
        map.get(&id).copied()
    };
    let Some(pid) = pid else {
        return Err(AppError { kind: ErrorKind::NotFound, message: "Команда не запущена".into() });
    };
    Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| AppError { kind: ErrorKind::Io, message: format!("taskkill: {}", e) })?;
    // запись из реестра уберёт поток-супервизор после закрытия пайпов
    Ok(())
}

/// Список id запущенных фоновых команд (для восстановления состояния UI).
#[tauri::command]
pub fn command_running(running: State<RunningState>) -> AppResult<Vec<i64>> {
    let map = running.procs.lock().map_err(|_| AppError::internal("proc mutex poisoned"))?;
    Ok(map.keys().copied().collect())
}
```

- [ ] **Step 4: lib.rs — manage + регистрация**
1. В `setup`, после `app.manage(tray::TrayState::default());` (или рядом с прочими manage) добавить:
```rust
            app.manage(commands::cmds::RunningState::default());
```
2. В `generate_handler![...]` рядом с `commands::cmds::command_run,` добавить:
```rust
            commands::cmds::command_run_bg,
            commands::cmds::command_stop,
            commands::cmds::command_running,
```

- [ ] **Step 5: Сборка + тесты**
```powershell
cargo build --manifest-path src-tauri/Cargo.toml --lib
cargo test --manifest-path src-tauri/Cargo.toml --lib
```
Expected: lib-сборка успешна, без warning; тесты 35 ok (новых нет). Если `command_run` после рефактора триггерит unused — проверить, что helper используется обоими.

- [ ] **Step 6: Commit**
```powershell
git add src-tauri/src/commands/cmds.rs src-tauri/src/lib.rs
git commit -m @'
feat(backend): background command runner with log streaming and stop

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

## Task 2: Фронт — режим, панель логов, стоп

**Files:** Modify `src/lib/api/commands.ts`, `src/lib/components/OverviewTab.svelte`.

- [ ] **Step 1: api/commands.ts** — добавить:
```ts
export const runBg = (id: number) => call<void>("command_run_bg", { id });
export const stop = (id: number) => call<void>("command_stop", { id });
export const running = () => call<number[]>("command_running");
```

- [ ] **Step 2: OverviewTab — состояние, режим, события**

В `<script>`:
1. Добавить импорт `onMount`/`onDestroy`:
```ts
  import { onMount, onDestroy } from "svelte";
```
2. Состояние режима в редакторе и для фоновых команд (рядом с `eDir`):
```ts
  let eRunIn = $state("terminal");
  // фоновые команды
  let runningIds = $state<number[]>([]);
  let logs = $state<Record<number, string[]>>({});
  let logFor = $state<number | null>(null);
  let unlisten: Array<() => void> = [];
```
3. В `openNewCmd`/`openEditCmd` выставлять режим:
```ts
  function openNewCmd() {
    isNew = true;
    editing = { id: 0, projectId: project.id, label: "", command: "", workingDir: null, runIn: "terminal", icon: "play", sortOrder: 0 };
    eLabel = ""; eCommand = ""; eDir = ""; eRunIn = "terminal";
  }
  function openEditCmd(c: ProjectCommand) {
    isNew = false;
    editing = c;
    eLabel = c.label; eCommand = c.command; eDir = c.workingDir ?? ""; eRunIn = c.runIn ?? "terminal";
  }
```
4. В `saveCmd` использовать `eRunIn`:
```ts
    const input = { label: eLabel.trim(), command: eCommand.trim(), workingDir: eDir.trim() || null, runIn: eRunIn, icon: "play" };
```
5. Переписать `runCmd` и добавить стоп/логи:
```ts
  async function runCmd(c: ProjectCommand) {
    if (c.runIn === "background") {
      logs = { ...logs, [c.id]: logs[c.id] ?? [] };
      logFor = c.id;
      try {
        await cmdsApi.runBg(c.id);
        if (!runningIds.includes(c.id)) runningIds = [...runningIds, c.id];
      } catch { /* тост из api/client.ts */ }
      return;
    }
    await cmdsApi.run(c.id);
    pushToast("Запуск", c.command, "info");
  }
  async function stopCmd(id: number) {
    try { await cmdsApi.stop(id); } catch { /* тост */ }
  }
  function openLogs(id: number) {
    logs = { ...logs, [id]: logs[id] ?? [] };
    logFor = id;
  }
  function appendLog(id: number, line: string) {
    const cur = logs[id] ?? [];
    const next = [...cur, line];
    if (next.length > 500) next.splice(0, next.length - 500);
    logs = { ...logs, [id]: next };
  }
```
6. Подписка на события и восстановление состояния:
```ts
  onMount(async () => {
    try { runningIds = await cmdsApi.running(); } catch { /* нет бэка — игнор */ }
    const { listen } = await import("@tauri-apps/api/event");
    unlisten.push(await listen<{ id: number; line: string; err: boolean }>("cmd-log", (e) => {
      appendLog(e.payload.id, (e.payload.err ? "[err] " : "") + e.payload.line);
    }));
    unlisten.push(await listen<{ id: number; code: number | null }>("cmd-exit", (e) => {
      runningIds = runningIds.filter((x) => x !== e.payload.id);
      appendLog(e.payload.id, `— процесс завершён (код ${e.payload.code ?? "?"}) —`);
    }));
  });
  onDestroy(() => { unlisten.forEach((u) => u()); unlisten = []; });
```

- [ ] **Step 3: OverviewTab — разметка кнопок команды**

В блоке `.cmd-grid`, в `<button class="cmd" ...>` — добавить рядом с существующим `.play` (карандаш) кнопки логов/стопа для фоновых команд. Заменить:
```svelte
            <span class="play" role="button" tabindex="-1"
                  onclick={(e) => { e.stopPropagation(); openEditCmd(c); }}><Icon name="pencil" class="ic-sm" /></span>
```
на:
```svelte
            {#if c.runIn === "background"}
              <span class="play" role="button" tabindex="-1" title="Логи"
                    onclick={(e) => { e.stopPropagation(); openLogs(c.id); }}><Icon name="scroll-text" class="ic-sm" /></span>
              {#if runningIds.includes(c.id)}
                <span class="play" role="button" tabindex="-1" title="Остановить" style="color:var(--danger)"
                      onclick={(e) => { e.stopPropagation(); stopCmd(c.id); }}><Icon name="square" class="ic-sm" /></span>
              {/if}
            {/if}
            <span class="play" role="button" tabindex="-1" title="Изменить"
                  onclick={(e) => { e.stopPropagation(); openEditCmd(c); }}><Icon name="pencil" class="ic-sm" /></span>
```
(Опционально для индикации запуска можно добавить класс, но достаточно появления кнопки «стоп».)

- [ ] **Step 4: OverviewTab — выбор режима в редакторе**

В модалке команды заменить подсказку:
```svelte
        <p class="desc" style="color:var(--muted-2);font-size:12px">Запуск открывает новое окно терминала. Фоновый режим с логами — позже.</p>
```
на выбор режима + актуальную подсказку:
```svelte
        <div class="field"><label for="cm-mode">Режим запуска</label>
          <select id="cm-mode" class="tin" bind:value={eRunIn}>
            <option value="terminal">В терминале (новое окно)</option>
            <option value="background">В фоне (логи + стоп)</option>
          </select></div>
        <p class="desc" style="color:var(--muted-2);font-size:12px">Фоновый режим запускает процесс скрыто и стримит вывод в панель логов; «стоп» завершает дерево процессов.</p>
```

- [ ] **Step 5: OverviewTab — панель логов (модалка)**

Перед закрывающим тегом компонента (после модалки команды `{#if editing}…{/if}`) добавить:
```svelte
{#if logFor !== null}
  <div class="modal-scrim open" role="dialog" tabindex="-1" aria-label="Логи команды"
       onmousedown={(e) => { if (e.currentTarget === e.target) (logFor = null); }}
       onkeydown={(e) => { if (e.key === 'Escape') (logFor = null); }}>
    <div class="modal" style="max-width:760px">
      <div class="modal-head">
        <span class="t">Логи: {cmds.find((c) => c.id === logFor)?.label ?? ""}</span>
        <button class="icon-btn x" onclick={() => (logFor = null)}><Icon name="x" class="ic" /></button>
      </div>
      <div class="modal-body">
        <pre class="mono" style="margin:0;max-height:50vh;overflow:auto;white-space:pre-wrap;font-size:12px;background:var(--bg-2,#0000);padding:8px;border-radius:8px">{(logs[logFor] ?? []).join("\n") || "— нет вывода —"}</pre>
      </div>
      <div class="modal-foot">
        {#if runningIds.includes(logFor)}
          <button class="btn-danger" onclick={() => logFor !== null && stopCmd(logFor)}><Icon name="square" class="ic-sm" /> Остановить</button>
        {/if}
        <span class="spacer"></span>
        <button class="btn-ghost" onclick={() => (logFor = null)}>Закрыть</button>
      </div>
    </div>
  </div>
{/if}
```

- [ ] **Step 6: Проверка**
```powershell
npm run check
npm test
npm run build
```
Expected: 0 ошибок типов (`scroll-text`/`square` — валидные lucide-иконки); Vitest 3 (3 passed); build успешен.

- [ ] **Step 7: Commit**
```powershell
git add src/lib/api/commands.ts src/lib/components/OverviewTab.svelte
git commit -m @'
feat(frontend): background command mode with live log panel and stop

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
- [ ] Создать команду с режимом «В фоне» (напр. `ping -n 5 127.0.0.1`) → запуск открывает панель логов, строки появляются в реальном времени; по завершении — маркер «процесс завершён».
- [ ] Долгая команда (`npm run dev`) в фоне → кнопка «стоп» завершает её (дерево процессов), `cmd-exit` чистит индикатор.
- [ ] Команда в режиме «В терминале» по-прежнему открывает новое окно консоли.
- [ ] Кнопка «логи» открывает панель без перезапуска.

---

## Итог

Команды умеют работать в фоне со стримингом логов и остановкой дерева процессов; терминальный режим сохранён. Это завершает список доработок-полировки v1.
