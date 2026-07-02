use crate::error::{AppError, AppResult, ErrorKind};
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::Command;
use tauri_plugin_opener::OpenerExt;

// Открыть новое консольное окно (для фолбэка терминала).
const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;

/// Раскрыть ведущий `~` в %USERPROFILE%. Остальной путь не трогаем.
fn expand_path(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed == "~" {
        return std::env::var("USERPROFILE").unwrap_or_else(|_| trimmed.to_string());
    }
    if let Some(rest) = trimmed.strip_prefix("~/").or_else(|| trimmed.strip_prefix("~\\")) {
        if let Ok(home) = std::env::var("USERPROFILE") {
            return format!("{}\\{}", home.trim_end_matches('\\'), rest.replace('/', "\\"));
        }
    }
    trimmed.to_string()
}

/// Раскрыть путь и убедиться, что это существующая папка.
///
/// Проверка `is_dir` — это и корректность (не пытаемся открыть несуществующее),
/// и защита: имя папки в Windows не может содержать `" < > | ?`, поэтому строка,
/// прошедшая эту проверку, не может «выйти» из кавычек/перенаправить команду —
/// shell-инъекция через путь исключена даже там, где ещё используется cmd.
fn require_dir(path: &str) -> AppResult<String> {
    let p = expand_path(path);
    if p.is_empty() {
        return Err(AppError { kind: ErrorKind::Validation, message: "Путь к проекту не задан".into() });
    }
    if !Path::new(&p).is_dir() {
        return Err(AppError { kind: ErrorKind::NotFound, message: format!("Папка не найдена: {}", p) });
    }
    Ok(p)
}

fn spawn(mut cmd: Command, what: &str) -> AppResult<()> {
    cmd.spawn()
        .map(|_| ())
        .map_err(|e| AppError { kind: ErrorKind::Io, message: format!("Не удалось запустить {}: {}", what, e) })
}

/// Открыть папку в Проводнике. Путь — отдельный аргумент (explorer.exe — не shell).
/// explorer возвращает ненулевой код даже при успехе, поэтому только spawn.
#[tauri::command]
pub fn open_path(path: String) -> AppResult<()> {
    let p = require_dir(&path)?;
    let mut cmd = Command::new("explorer.exe");
    cmd.arg(&p);
    spawn(cmd, "Проводник")
}

/// Открыть папку в редакторе (VS Code). Сначала пробуем `code.cmd` напрямую —
/// Rust (1.77+) сам безопасно оборачивает batch-файл и экранирует аргументы.
/// Если не найден — фолбэк через `cmd /C code`, где путь уже провалидирован
/// `require_dir` как существующая папка (без shell-инъекции).
/// (Настраиваемый редактор — позже, в срезе настроек.)
#[tauri::command]
pub fn open_in_editor(path: String) -> AppResult<()> {
    let p = require_dir(&path)?;
    if Command::new("code.cmd").arg(&p).spawn().is_ok() {
        return Ok(());
    }
    let mut cmd = Command::new("cmd");
    cmd.args(["/C", "code", &p]);
    spawn(cmd, "редактор (code)")
}

/// Открыть конкретный файл в редакторе (VS Code). В отличие от open_in_editor
/// (валидирует папку), здесь путь — файл, поэтому require_dir_or_file.
/// `code.cmd` вызываем напрямую: Rust (>=1.77) безопасно экранирует аргументы
/// batch-файла. Shell-фолбэк (`cmd /C`) НЕ используем — он бы пропускал
/// метасимволы (`& ^ % !`) из имени файла в командную строку.
#[tauri::command]
pub fn open_file_in_editor(path: String) -> AppResult<()> {
    let p = require_dir_or_file(&path)?;
    Command::new("code.cmd")
        .arg(&p)
        .spawn()
        .map(|_| ())
        .map_err(|e| AppError {
            kind: ErrorKind::Io,
            message: format!("Не удалось запустить редактор (code): {}", e),
        })
}

/// Открыть системный терминал в папке: Windows Terminal `wt -d`, затем cmd.
fn spawn_terminal_at(dir: &str) -> AppResult<()> {
    let mut wt = Command::new("wt.exe");
    wt.args(["-d", dir]);
    if wt.spawn().is_ok() {
        return Ok(());
    }
    let mut fallback = Command::new("cmd");
    fallback.arg("/K").current_dir(dir).creation_flags(CREATE_NEW_CONSOLE);
    spawn(fallback, "терминал")
}

/// Открыть терминал в папке: Windows Terminal `wt -d <path>` (путь — отдельный аргумент);
/// при неудаче — новое окно `cmd` с рабочей папкой через `current_dir` (без интерполяции
/// пути в командную строку — shell не задействован).
#[tauri::command]
pub fn open_terminal(path: String) -> AppResult<()> {
    let p = require_dir(&path)?;
    spawn_terminal_at(&p)
}

/// Директория для консоли: сама папка, либо родитель файла.
fn dir_of(p: &str, is_dir: bool) -> Option<String> {
    if is_dir {
        Some(p.to_string())
    } else {
        Path::new(p)
            .parent()
            .map(|x| x.to_string_lossy().into_owned())
            .filter(|s| !s.is_empty())
    }
}

/// Кандидаты git-bash.exe: PATH → стандартные пути установки.
fn git_bash_candidates() -> Vec<String> {
    let mut v = vec![
        "git-bash.exe".to_string(),
        r"C:\Program Files\Git\git-bash.exe".to_string(),
        r"C:\Program Files (x86)\Git\git-bash.exe".to_string(),
    ];
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        v.push(format!(r"{}\Programs\Git\git-bash.exe", local.trim_end_matches('\\')));
    }
    v
}

/// Открыть Git Bash в папке ярлыка (файл → родительская папка). Если Git Bash
/// не найден — фолбэк на системный терминал.
#[tauri::command]
pub fn open_git_bash(path: String) -> AppResult<()> {
    let p = require_dir_or_file(&path)?;
    let is_dir = Path::new(&p).is_dir();
    let dir = dir_of(&p, is_dir).ok_or_else(|| AppError {
        kind: ErrorKind::NotFound,
        message: "Не удалось определить папку".into(),
    })?;
    for exe in git_bash_candidates() {
        if Command::new(&exe).arg(format!("--cd={}", dir)).spawn().is_ok() {
            return Ok(());
        }
    }
    spawn_terminal_at(&dir)
}

/// Открыть URL в браузере по умолчанию через системный обработчик (ShellExecuteW
/// в плагине opener — без вызова cmd). Разрешаем только http/https.
#[tauri::command]
pub fn open_url(app: tauri::AppHandle, url: String) -> AppResult<()> {
    let u = url.trim();
    if u.is_empty() {
        return Err(AppError { kind: ErrorKind::Validation, message: "URL не задан".into() });
    }
    // Нет схемы → https:// (localhost:3000, github.com/... и т.п.).
    let full = if u.contains("://") { u.to_string() } else { format!("https://{}", u) };
    let lower = full.to_ascii_lowercase();
    if !(lower.starts_with("http://") || lower.starts_with("https://")) {
        return Err(AppError { kind: ErrorKind::Validation, message: "Поддерживаются только http(s)-ссылки".into() });
    }
    app.opener()
        .open_url(full, None::<&str>)
        .map_err(|e| AppError { kind: ErrorKind::Io, message: format!("Не удалось открыть ссылку: {}", e) })
}

/// Открыть файл/папку-ярлык ассоциированной программой (файл) или в проводнике (папка)
/// через системный обработчик opener — без shell.
#[tauri::command]
pub fn open_shortcut(app: tauri::AppHandle, path: String) -> AppResult<()> {
    let p = require_dir_or_file(&path)?;
    app.opener()
        .open_path(p, None::<&str>)
        .map_err(|e| AppError { kind: ErrorKind::Io, message: format!("Не удалось открыть: {}", e) })
}

/// Раскрыть путь и убедиться, что это существующий файл ИЛИ папка
/// (`require_dir` проверяет только папку, поэтому отдельный хелпер).
fn require_dir_or_file(path: &str) -> AppResult<String> {
    let p = expand_path(path);
    if p.is_empty() {
        return Err(AppError { kind: ErrorKind::Validation, message: "Путь не задан".into() });
    }
    if !Path::new(&p).exists() {
        return Err(AppError { kind: ErrorKind::NotFound, message: format!("Не найдено: {}", p) });
    }
    Ok(p)
}

#[cfg(test)]
mod tests {
    use super::expand_path;

    #[test]
    fn expands_leading_tilde() {
        std::env::set_var("USERPROFILE", "C:\\Users\\test");
        assert_eq!(expand_path("~"), "C:\\Users\\test");
        assert_eq!(expand_path("~/dev/proj"), "C:\\Users\\test\\dev\\proj");
        assert_eq!(expand_path("~\\dev\\proj"), "C:\\Users\\test\\dev\\proj");
    }

    #[test]
    fn dir_of_returns_folder_or_parent() {
        // папка → сама
        assert_eq!(super::dir_of(r"C:\a\b", true), Some(r"C:\a\b".to_string()));
        // файл → родительская папка
        assert_eq!(super::dir_of(r"C:\a\f.txt", false), Some(r"C:\a".to_string()));
        // файл без директории → None
        assert_eq!(super::dir_of("f.txt", false), None);
    }

    #[test]
    fn open_file_in_editor_rejects_missing_path() {
        let res = super::open_file_in_editor("Z:\\definitely\\missing\\file.txt".into());
        assert!(res.is_err(), "ожидали ошибку для несуществующего файла");
    }

    #[test]
    fn leaves_absolute_paths_untouched() {
        assert_eq!(expand_path("D:\\code\\app"), "D:\\code\\app");
        assert_eq!(expand_path("  C:\\x  "), "C:\\x"); // только trim
    }
}
