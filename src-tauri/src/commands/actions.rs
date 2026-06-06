use crate::error::{AppError, AppResult, ErrorKind};
use std::process::Command;

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

fn require_path(path: &str) -> AppResult<String> {
    let p = expand_path(path);
    if p.is_empty() {
        return Err(AppError { kind: ErrorKind::Validation, message: "Путь к проекту не задан".into() });
    }
    Ok(p)
}

fn spawn(mut cmd: Command, what: &str) -> AppResult<()> {
    cmd.spawn()
        .map(|_| ())
        .map_err(|e| AppError { kind: ErrorKind::Io, message: format!("Не удалось запустить {}: {}", what, e) })
}

/// Открыть папку в Проводнике. explorer.exe возвращает ненулевой код даже при успехе —
/// поэтому только spawn, без проверки статуса.
#[tauri::command]
pub fn open_path(path: String) -> AppResult<()> {
    let p = require_path(&path)?;
    let mut cmd = Command::new("explorer.exe");
    cmd.arg(&p);
    spawn(cmd, "Проводник")
}

/// Открыть папку в редакторе. По умолчанию VS Code (`code`). На Windows `code` —
/// это code.cmd, поэтому через `cmd /C`. (Настраиваемый редактор — позже, в срезе настроек.)
#[tauri::command]
pub fn open_in_editor(path: String) -> AppResult<()> {
    let p = require_path(&path)?;
    let mut cmd = Command::new("cmd");
    cmd.args(["/C", "code", &p]);
    spawn(cmd, "редактор (code)")
}

/// Открыть терминал в папке: Windows Terminal `wt -d <path>`; при неудаче — cmd в этой папке.
#[tauri::command]
pub fn open_terminal(path: String) -> AppResult<()> {
    let p = require_path(&path)?;
    let mut wt = Command::new("wt.exe");
    wt.args(["-d", &p]);
    if wt.spawn().is_ok() {
        return Ok(());
    }
    // Фолбэк: новое окно cmd с рабочей папкой.
    let mut fallback = Command::new("cmd");
    fallback.args(["/C", "start", "cmd", "/K", "cd", "/d", &p]);
    spawn(fallback, "терминал")
}

/// Открыть URL в браузере по умолчанию.
#[tauri::command]
pub fn open_url(url: String) -> AppResult<()> {
    let u = url.trim();
    if u.is_empty() {
        return Err(AppError { kind: ErrorKind::Validation, message: "URL не задан".into() });
    }
    // Если нет схемы — добавим https:// (localhost:3000, github.com/... и т.п.).
    let full = if u.contains("://") { u.to_string() } else { format!("https://{}", u) };
    let mut cmd = Command::new("cmd");
    cmd.args(["/C", "start", "", &full]);
    spawn(cmd, "браузер")
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
    fn leaves_absolute_paths_untouched() {
        assert_eq!(expand_path("D:\\code\\app"), "D:\\code\\app");
        assert_eq!(expand_path("  C:\\x  "), "C:\\x"); // только trim
    }
}
