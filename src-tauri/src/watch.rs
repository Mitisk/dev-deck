use crate::error::{AppError, AppResult};
use crate::state::AppState;
use notify_debouncer_mini::notify::RecommendedWatcher;
use notify_debouncer_mini::notify::RecursiveMode;
use notify_debouncer_mini::{new_debouncer, DebounceEventResult, Debouncer};
use rusqlite::Connection;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

const NOISE: &[&str] = &[
    "\\node_modules\\",
    "/node_modules/",
    "\\.git\\",
    "/.git/",
    "\\target\\",
    "/target/",
    "\\dist\\",
    "/dist/",
    "\\build\\",
    "/build/",
    "\\.svelte-kit\\",
    "/.svelte-kit/",
];

/// Считать ли путь «шумовым» (артефакты сборки / VCS).
pub fn is_noise(path: &Path) -> bool {
    let s = path.to_string_lossy();
    NOISE.iter().any(|n| s.contains(n))
}

/// Найти id проекта, чья корневая папка является предком пути.
pub fn project_for_path(roots: &[(PathBuf, i64)], path: &Path) -> Option<i64> {
    roots
        .iter()
        .find(|(root, _)| path.starts_with(root))
        .map(|(_, id)| *id)
}

/// Раскрыть ведущий `~`.
fn expand(p: &str) -> String {
    let t = p.trim();
    if let Some(rest) = t.strip_prefix("~/").or_else(|| t.strip_prefix("~\\")) {
        if let Ok(home) = std::env::var("USERPROFILE") {
            return format!("{}\\{}", home.trim_end_matches('\\'), rest.replace('/', "\\"));
        }
    }
    t.to_string()
}

/// Управляемое состояние вотчера. Debouncer держим живым (drop останавливает слежение).
#[derive(Default)]
pub struct WatchState {
    pub debouncer: Mutex<Option<Debouncer<RecommendedWatcher>>>,
    pub roots: Arc<Mutex<Vec<(PathBuf, i64)>>>,
}

/// Пересобрать вотчер по текущему набору проектов.
pub fn resync(app: &AppHandle) -> AppResult<()> {
    let db_state = app.state::<AppState>();
    let watch_state = app.state::<WatchState>();

    // прочитать пути проектов
    let projects: Vec<(i64, String)> = {
        let guard = db_state
            .db
            .lock()
            .map_err(|_| AppError::internal("db mutex poisoned"))?;
        let conn: &Connection = &guard;
        let mut stmt = conn.prepare(
            "SELECT id, COALESCE(NULLIF(repo_path,''), path) FROM projects WHERE status != 'archived'",
        )?;
        let rows =
            stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Option<String>>(1)?)))?;
        let mut v = Vec::new();
        for row in rows {
            let (id, p) = row?;
            if let Some(p) = p {
                v.push((id, p));
            }
        }
        v
    };

    let roots: Vec<(PathBuf, i64)> = projects
        .into_iter()
        .filter_map(|(id, p)| {
            let pb = PathBuf::from(expand(&p));
            if pb.is_dir() {
                Some((pb, id))
            } else {
                None
            }
        })
        .collect();

    *watch_state
        .roots
        .lock()
        .map_err(|_| AppError::internal("roots poisoned"))? = roots.clone();

    let roots_arc = watch_state.roots.clone();
    let app2 = app.clone();
    let mut deb = new_debouncer(Duration::from_millis(800), move |res: DebounceEventResult| {
        if let Ok(events) = res {
            let roots = match roots_arc.lock() {
                Ok(g) => g,
                Err(_) => return,
            };
            let mut hit: HashSet<i64> = HashSet::new();
            for ev in events {
                if is_noise(&ev.path) {
                    continue;
                }
                if let Some(id) = project_for_path(&roots, &ev.path) {
                    hit.insert(id);
                }
            }
            for id in hit {
                let _ = app2.emit("folder-changed", id);
            }
        }
    })
    .map_err(|e| AppError::internal(format!("watcher: {}", e)))?;

    for (root, _) in roots.iter() {
        let _ = deb.watcher().watch(root, RecursiveMode::Recursive);
    }
    *watch_state
        .debouncer
        .lock()
        .map_err(|_| AppError::internal("debouncer poisoned"))? = Some(deb);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noise_paths_detected() {
        assert!(is_noise(Path::new("C:\\dev\\app\\node_modules\\x\\y.js")));
        assert!(is_noise(Path::new("C:\\dev\\app\\.git\\HEAD")));
        assert!(is_noise(Path::new("C:\\dev\\app\\target\\debug\\app.exe")));
        assert!(!is_noise(Path::new("C:\\dev\\app\\src\\main.rs")));
    }

    #[test]
    fn matches_project_root() {
        let roots = vec![
            (PathBuf::from("C:\\dev\\aurora"), 1i64),
            (PathBuf::from("C:\\dev\\nebula"), 2i64),
        ];
        assert_eq!(
            project_for_path(&roots, Path::new("C:\\dev\\aurora\\src\\a.rs")),
            Some(1)
        );
        assert_eq!(
            project_for_path(&roots, Path::new("C:\\dev\\nebula\\x")),
            Some(2)
        );
        assert_eq!(project_for_path(&roots, Path::new("C:\\other\\z")), None);
    }
}
