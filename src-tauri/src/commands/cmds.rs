use crate::error::{AppError, AppResult, ErrorKind};
use crate::models::{CommandInput, ProjectCommand};
use crate::state::AppState;
use rusqlite::{params, Connection};
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
