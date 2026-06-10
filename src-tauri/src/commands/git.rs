use crate::error::{AppError, AppResult, ErrorKind};
use crate::models::{AttentionItem, GitChanges, GitFile, GitOpResult, GitStatus};
use crate::state::AppState;
use git2::{BranchType, Repository, Status, StatusOptions};
use std::path::Path;
use std::process::Command;
use tauri::State;

/// Раскрыть ведущий `~` (libgit2 сам это не делает).
fn expand(p: &str) -> String {
    let t = p.trim();
    if let Some(rest) = t.strip_prefix("~/").or_else(|| t.strip_prefix("~\\")) {
        if let Ok(home) = std::env::var("USERPROFILE") {
            return format!("{}\\{}", home.trim_end_matches('\\'), rest.replace('/', "\\"));
        }
    }
    t.to_string()
}

/// Собрать статус из открытого репозитория (вынесено для тестируемости).
fn status_of(repo: &Repository) -> AppResult<GitStatus> {
    let head = repo.head().ok();
    let branch = head.as_ref().and_then(|h| h.shorthand().map(|s| s.to_string()));

    let (last_hash, last_message, last_timestamp) =
        match head.as_ref().and_then(|h| h.peel_to_commit().ok()) {
            Some(c) => {
                let full = c.id().to_string();
                (
                    Some(full.chars().take(7).collect::<String>()),
                    c.summary().map(|s| s.to_string()),
                    Some(c.time().seconds()),
                )
            }
            None => (None, None, None),
        };

    // ahead/behind относительно upstream (если есть).
    let (mut ahead, mut behind) = (0usize, 0usize);
    if let Some(name) = branch.as_deref() {
        if let Ok(local) = repo.find_branch(name, BranchType::Local) {
            if let Ok(upstream) = local.upstream() {
                if let (Some(l), Some(u)) = (local.get().target(), upstream.get().target()) {
                    if let Ok((a, b)) = repo.graph_ahead_behind(l, u) {
                        ahead = a;
                        behind = b;
                    }
                }
            }
        }
    }

    // Рабочее дерево.
    let mut opts = StatusOptions::new();
    opts.include_untracked(true).include_ignored(false);
    let statuses = repo.statuses(Some(&mut opts))?;
    let index_flags = Status::INDEX_NEW
        | Status::INDEX_MODIFIED
        | Status::INDEX_DELETED
        | Status::INDEX_RENAMED
        | Status::INDEX_TYPECHANGE;
    let (mut dirty, mut staged, mut untracked) = (0usize, 0usize, 0usize);
    for e in statuses.iter() {
        let s = e.status();
        if s.contains(Status::IGNORED) {
            continue;
        }
        dirty += 1;
        if s.intersects(index_flags) {
            staged += 1;
        }
        if s.contains(Status::WT_NEW) {
            untracked += 1;
        }
    }

    Ok(GitStatus {
        branch,
        ahead,
        behind,
        dirty,
        staged,
        untracked,
        last_hash,
        last_message,
        last_timestamp,
    })
}

/// Детализация незакоммиченных изменений: diffstat + список файлов.
fn changes_of(repo: &Repository) -> AppResult<GitChanges> {
    // diffstat: HEAD-дерево → рабочее дерево (с учётом индекса). Untracked включаем.
    let head_tree = repo.head().ok().and_then(|h| h.peel_to_tree().ok());
    let mut diff_opts = git2::DiffOptions::new();
    diff_opts.include_untracked(true).recurse_untracked_dirs(true);
    let diff = repo.diff_tree_to_workdir_with_index(head_tree.as_ref(), Some(&mut diff_opts))?;
    let stats = diff.stats()?;
    let insertions = stats.insertions();
    let deletions = stats.deletions();

    // Список файлов из того же statuses(), что и status_of.
    let mut opts = StatusOptions::new();
    opts.include_untracked(true)
        .include_ignored(false)
        .renames_head_to_index(true)
        .renames_index_to_workdir(true);
    let statuses = repo.statuses(Some(&mut opts))?;

    let mut files = Vec::new();
    for e in statuses.iter() {
        let s = e.status();
        if s.contains(Status::IGNORED) {
            continue;
        }
        let path = e.path().unwrap_or("").to_string();
        let index_flags = Status::INDEX_NEW
            | Status::INDEX_MODIFIED
            | Status::INDEX_DELETED
            | Status::INDEX_RENAMED
            | Status::INDEX_TYPECHANGE;
        let staged = s.intersects(index_flags);
        // Буква статуса: рабочее дерево приоритетнее индекса.
        let code = if s.contains(Status::WT_NEW) {
            "?"
        } else if s.intersects(Status::WT_DELETED | Status::INDEX_DELETED) {
            "D"
        } else if s.intersects(Status::WT_RENAMED | Status::INDEX_RENAMED) {
            "R"
        } else if s.intersects(Status::WT_TYPECHANGE | Status::INDEX_TYPECHANGE) {
            "T"
        } else if s.contains(Status::INDEX_NEW) {
            "A"
        } else {
            "M"
        };
        files.push(GitFile { path, code: code.to_string(), staged });
    }

    // Сортировка по категориям: staged → modified → untracked → deleted.
    files.sort_by_key(|f| match (f.staged, f.code.as_str()) {
        (true, _) => 0,
        (false, "?") => 2,
        (false, "D") => 3,
        _ => 1,
    });

    Ok(GitChanges { insertions, deletions, files })
}

/// Прочитать детализацию изменений по пути. None — путь пуст или не git-репозиторий.
#[tauri::command]
pub fn git_changes(repo_path: String) -> AppResult<Option<GitChanges>> {
    let p = expand(&repo_path);
    if p.is_empty() {
        return Ok(None);
    }
    match Repository::open(&p) {
        Ok(repo) => Ok(Some(changes_of(&repo)?)),
        Err(_) => Ok(None),
    }
}

/// Прочитать git-статус по пути. None — путь пуст или не git-репозиторий.
#[tauri::command]
pub fn git_status(repo_path: String) -> AppResult<Option<GitStatus>> {
    let p = expand(&repo_path);
    if p.is_empty() {
        return Ok(None);
    }
    match Repository::open(&p) {
        Ok(repo) => Ok(Some(status_of(&repo)?)),
        Err(_) => Ok(None),
    }
}

/// Запустить системный `git` в папке репозитория без shell. Возвращает успех + вывод.
fn run_git(repo: &str, args: &[&str]) -> AppResult<GitOpResult> {
    let dir = expand(repo);
    if !Path::new(&dir).is_dir() {
        return Err(AppError { kind: ErrorKind::Validation, message: "Папка проекта не найдена".into() });
    }
    let out = Command::new("git")
        .current_dir(&dir)
        .args(args)
        .output()
        .map_err(|e| AppError { kind: ErrorKind::Io, message: format!("Не удалось запустить git: {}", e) })?;

    let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
    let err = String::from_utf8_lossy(&out.stderr);
    if !err.trim().is_empty() {
        if !text.trim().is_empty() {
            text.push('\n');
        }
        text.push_str(&err);
    }
    Ok(GitOpResult { ok: out.status.success(), output: text.trim().to_string() })
}

#[tauri::command]
pub fn git_fetch(repo_path: String) -> AppResult<GitOpResult> {
    run_git(&repo_path, &["fetch"])
}

#[tauri::command]
pub fn git_pull(repo_path: String) -> AppResult<GitOpResult> {
    run_git(&repo_path, &["pull"])
}

#[tauri::command]
pub fn git_push(repo_path: String) -> AppResult<GitOpResult> {
    run_git(&repo_path, &["push"])
}

/// `git add -A` затем `git commit -m <message>`. Сообщение — отдельный argv (без shell).
#[tauri::command]
pub fn git_commit_all(repo_path: String, message: String) -> AppResult<GitOpResult> {
    let msg = message.trim();
    if msg.is_empty() {
        return Err(AppError { kind: ErrorKind::Validation, message: "Сообщение коммита пустое".into() });
    }
    let add = run_git(&repo_path, &["add", "-A"])?;
    if !add.ok {
        return Ok(add);
    }
    run_git(&repo_path, &["commit", "-m", msg])
}

/// Проекты, требующие внимания: незакоммиченные изменения (dirty>0) или готовые к push (ahead>0).
#[tauri::command]
pub fn dashboard_attention(state: State<AppState>) -> AppResult<Vec<AttentionItem>> {
    // Сначала под локом читаем список проектов, затем отпускаем лок и идём в git2.
    let projects: Vec<(i64, String, Option<String>, Option<String>, Option<String>)> = {
        let conn = state.db.lock().map_err(|_| crate::error::AppError::internal("db mutex poisoned"))?;
        let mut stmt = conn.prepare(
            "SELECT id, name, color, icon, COALESCE(NULLIF(repo_path,''), path)
             FROM projects WHERE status != 'archived' ORDER BY pinned DESC, sort_order ASC, name ASC",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })?;
        let mut v = Vec::new();
        for row in rows {
            v.push(row?);
        }
        v
    };

    let mut out = Vec::new();
    for (id, name, color, icon, repo) in projects {
        let Some(repo_path) = repo else { continue };
        let p = expand(&repo_path);
        if p.trim().is_empty() {
            continue;
        }
        let Ok(repo) = Repository::open(&p) else { continue };
        let Ok(st) = status_of(&repo) else { continue };
        if st.dirty > 0 || st.ahead > 0 {
            out.push(AttentionItem {
                project_id: id,
                name,
                color,
                icon,
                branch: st.branch,
                ahead: st.ahead,
                dirty: st.dirty,
                last_hash: st.last_hash,
                last_message: st.last_message,
            });
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    fn temp_repo() -> (std::path::PathBuf, Repository) {
        use std::sync::atomic::{AtomicU32, Ordering};
        static SEQ: AtomicU32 = AtomicU32::new(0);
        let n = SEQ.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "devdeck_git_test_{}_{}",
            std::process::id(),
            n
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let repo = Repository::init(&dir).unwrap();
        (dir, repo)
    }

    fn commit_file(repo: &Repository, name: &str, content: &str) {
        let wd = repo.workdir().unwrap().to_path_buf();
        fs::write(wd.join(name), content).unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new(name)).unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let sig = git2::Signature::now("Test", "test@example.com").unwrap();
        let parent = repo.head().ok().and_then(|h| h.peel_to_commit().ok());
        let parents: Vec<&git2::Commit> = parent.iter().collect();
        repo.commit(Some("HEAD"), &sig, &sig, "initial", &tree, &parents).unwrap();
    }

    #[test]
    fn reports_branch_commit_and_dirty() {
        let (dir, repo) = temp_repo();
        commit_file(&repo, "a.txt", "hello");
        // изменить закоммиченный файл + добавить неотслеживаемый
        fs::write(dir.join("a.txt"), "changed").unwrap();
        fs::write(dir.join("b.txt"), "new").unwrap();

        let st = status_of(&repo).unwrap();
        assert!(st.branch.is_some());
        assert!(st.last_hash.is_some());
        assert_eq!(st.last_message.as_deref(), Some("initial"));
        assert!(st.dirty >= 2, "dirty was {}", st.dirty);
        assert!(st.untracked >= 1, "untracked was {}", st.untracked);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn clean_repo_has_zero_dirty() {
        let (dir, repo) = temp_repo();
        commit_file(&repo, "a.txt", "hello");
        let st = status_of(&repo).unwrap();
        assert_eq!(st.dirty, 0);
        assert_eq!(st.untracked, 0);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn git_changes_reports_files_and_insertions() {
        let (dir, repo) = temp_repo();
        commit_file(&repo, "a.txt", "line1\n");
        // модифицируем закоммиченный файл
        fs::write(dir.join("a.txt"), "line1\nline2\n").unwrap();
        // новый неотслеживаемый файл
        fs::write(dir.join("b.txt"), "new\n").unwrap();
        // staged-файл
        {
            fs::write(dir.join("c.txt"), "staged\n").unwrap();
            let mut index = repo.index().unwrap();
            index.add_path(Path::new("c.txt")).unwrap();
            index.write().unwrap();
        }

        let ch = changes_of(&repo).unwrap();
        assert!(ch.insertions > 0, "insertions was {}", ch.insertions);

        let a = ch.files.iter().find(|f| f.path == "a.txt").expect("a.txt");
        assert_eq!(a.code, "M");
        assert!(!a.staged);

        let b = ch.files.iter().find(|f| f.path == "b.txt").expect("b.txt");
        assert_eq!(b.code, "?");

        let c = ch.files.iter().find(|f| f.path == "c.txt").expect("c.txt");
        assert_eq!(c.code, "A");
        assert!(c.staged);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn commit_all_creates_commit_via_system_git() {
        let (dir, _repo) = temp_repo();
        // локальная identity, чтобы не зависеть от глобального git-конфига
        let cfg = |args: &[&str]| {
            std::process::Command::new("git").current_dir(&dir).args(args).output().unwrap();
        };
        cfg(&["config", "user.email", "t@example.com"]);
        cfg(&["config", "user.name", "Test"]);
        fs::write(dir.join("a.txt"), "x").unwrap();

        let res = super::git_commit_all(dir.to_string_lossy().into_owned(), "первый коммит".into()).unwrap();
        assert!(res.ok, "commit output: {}", res.output);

        let log = std::process::Command::new("git").current_dir(&dir).args(["log", "--oneline"]).output().unwrap();
        assert!(String::from_utf8_lossy(&log.stdout).contains("первый коммит"));

        let _ = fs::remove_dir_all(&dir);
    }
}
