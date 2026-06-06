use crate::error::AppResult;
use crate::models::GitStatus;
use git2::{BranchType, Repository, Status, StatusOptions};

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
}
