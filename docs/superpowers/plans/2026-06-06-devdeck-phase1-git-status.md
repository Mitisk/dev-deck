# DevDeck Phase 1 — Срез «Git-статус (read-only)» — план реализации

> **Для исполнителя:** ОБЯЗАТЕЛЬНЫЙ ПОД-НАВЫК: superpowers:subagent-driven-development. Шаги — чекбоксы.

**Цель:** Показать git-сводку проекта в шапке: текущая ветка, ahead/behind относительно upstream, число изменённых файлов (dirty / staged / untracked), последний коммит (короткий хэш + сообщение + дата). Только чтение через `git2` — без сети и аутентификации. Если путь не git-репозиторий — git-бар не показываем.

**Архитектура:** Команда `git_status(repo_path)` через `git2` (libgit2, vendored) возвращает `Option<GitStatus>` (None = не репозиторий). Дата коммита отдаётся как unix-timestamp, форматируется на фронте (без chrono в Rust). Компонент `GitBar.svelte` грузит статус при открытии проекта и рисует сводку; встроен в `ProjectView` между шапкой и табами.

**Стек:** + crate `git2` (vendored libgit2, без сетевых фич). Остальное как раньше.

**Решения / границы:**
- **Только чтение.** Сетевые Pull/Push/Fetch и commit-all — отдельным срезом позже (через системный `git`).
- Блок «Требуют внимания» на дашборде (git-статус по всем проектам) — **отложен** (потребует пакетной загрузки; сделаем в срезе сети/дашборда). Сейчас git-бар только в карточке проекта.
- `git2` берём `default-features = false` (отключаем ssh/https → не нужен OpenSSL на Windows; для read-only-статуса сеть не нужна; libgit2 всё равно vendored-сборкой из исходников).
- Путь репозитория: `repoPath` проекта, иначе `path`. Раскрываем ведущий `~`.

**Источники:** `TZ_DevDeck.md` (5.3 — git-интеграция, чтение через git2; 10 — `git_status`). `_prototype/index.html` (`.git-bar` и `gitBarHTML` — разметка-референс). Текущий `ProjectView.svelte`, `Project.repoPath`.

---

## Контекст (после среза «Быстрые действия»)

- Rust: 13 команд (`db_health`, `projects_*`, `open_*`). `error::{AppError, ErrorKind, AppResult}`, `models.rs` (Project, ProjectInput), `state::AppState`.
- Фронт: `Project` имеет `repoPath: string | null`, `path: string | null`. `ProjectView.svelte` — шапка (`.proj-head`) + табы. api: `client.ts` (`call<T>`). Компонент `Icon.svelte`.
- global.css содержит классы git-бара: `.git-bar`, `.branch`, `.aheadbehind` (`.a`/`.b`), `.dirty-count` (`.led`), `.last-commit` (`.hash`/`.msg`), `.git-sep`.

---

## Структура файлов

```
src-tauri/
├─ Cargo.toml                 # MOD: + git2 (vendored, no default features)
└─ src/
   ├─ models.rs               # MOD: + GitStatus (serde camelCase)
   ├─ commands/
   │  ├─ mod.rs               # MOD: + pub mod git;
   │  └─ git.rs               # NEW: git_status + status_of + expand + тест
   └─ lib.rs                  # MOD: регистрация git_status

src/lib/
├─ types.ts                   # MOD: + GitStatus
├─ api/git.ts                 # NEW: status()
└─ components/
   ├─ GitBar.svelte           # NEW: грузит и рисует git-сводку
   └─ ProjectView.svelte      # MOD: вставить <GitBar>
```

---

## Task 1: Rust — git_status через git2

**Files:**
- Modify: `src-tauri/Cargo.toml`, `src-tauri/src/models.rs`, `src-tauri/src/commands/mod.rs`, `src-tauri/src/lib.rs`
- Create: `src-tauri/src/commands/git.rs`

- [ ] **Step 1: Зависимость git2**

В `src-tauri/Cargo.toml` в `[dependencies]` добавить:
```toml
git2 = { version = "0.20", default-features = false }
```
`default-features = false` отключает ssh/https (не тянем OpenSSL); libgit2 собирается vendored из исходников (нужен C-компилятор — MSVC уже есть). Если `0.20` не резолвится с текущим toolchain — взять последнюю `0.x` и сообщить версию.

- [ ] **Step 2: Модель GitStatus**

В `src-tauri/src/models.rs` добавить (после `ProjectInput`):
```rust
/// Снимок состояния git-репозитория (read-only).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitStatus {
    pub branch: Option<String>,
    pub ahead: usize,
    pub behind: usize,
    pub dirty: usize,
    pub staged: usize,
    pub untracked: usize,
    pub last_hash: Option<String>,
    pub last_message: Option<String>,
    pub last_timestamp: Option<i64>, // unix seconds; форматируется на фронте
}
```

- [ ] **Step 3: Команда и логика**

Create `src-tauri/src/commands/git.rs`:
```rust
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
        let dir = std::env::temp_dir().join(format!("devdeck_git_test_{}", std::process::id()));
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
```

- [ ] **Step 4: Зарегистрировать модуль и команду**

- `src-tauri/src/commands/mod.rs`: добавить `pub mod git;`
- `src-tauri/src/lib.rs`: в `tauri::generate_handler![...]` добавить `commands::git::git_status,` (после команд actions).

- [ ] **Step 5: Тесты + сборка (NON-blocking; не запускать `npm run tauri dev`)**

Run:
```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib
cargo build --manifest-path src-tauri/Cargo.toml
```
Expected: новые тесты `reports_branch_commit_and_dirty`, `clean_repo_has_zero_dirty` + прежние (7) — все ok; build успешен (первая сборка git2/libgit2 из исходников — долгая, это нормально). Если vendored libgit2 не собирается (нет CMake/компилятора) — сообщить точную ошибку (BLOCKED).

- [ ] **Step 6: Commit**

```powershell
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/models.rs src-tauri/src/commands/git.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs
git commit -m @'
feat(backend): read-only git_status via git2

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

## Task 2: Фронт — тип, api, GitBar в шапке

**Files:**
- Modify: `src/lib/types.ts`, `src/lib/components/ProjectView.svelte`
- Create: `src/lib/api/git.ts`, `src/lib/components/GitBar.svelte`

- [ ] **Step 1: Тип GitStatus**

В `src/lib/types.ts` добавить (в конец):
```ts
export type GitStatus = {
  branch: string | null;
  ahead: number;
  behind: number;
  dirty: number;
  staged: number;
  untracked: number;
  lastHash: string | null;
  lastMessage: string | null;
  lastTimestamp: number | null;
};
```

- [ ] **Step 2: api/git.ts**

Create `src/lib/api/git.ts`:
```ts
import { call } from "./client";
import type { GitStatus } from "../types";

// null = путь пуст или не git-репозиторий.
export const status = (repoPath: string) => call<GitStatus | null>("git_status", { repoPath });
```

- [ ] **Step 3: GitBar.svelte**

Create `src/lib/components/GitBar.svelte`:
```svelte
<script lang="ts">
  import type { GitStatus } from "$lib/types";
  import * as git from "$lib/api/git";
  import Icon from "./Icon.svelte";

  let { repoPath }: { repoPath: string | null } = $props();

  let st = $state<GitStatus | null>(null);
  let loaded = $state(false);

  // Перезагружать статус при смене пути.
  $effect(() => {
    const p = repoPath;
    loaded = false;
    st = null;
    if (!p) {
      loaded = true;
      return;
    }
    let cancelled = false;
    git
      .status(p)
      .then((s) => {
        if (!cancelled) {
          st = s;
          loaded = true;
        }
      })
      .catch(() => {
        if (!cancelled) loaded = true;
      });
    return () => {
      cancelled = true;
    };
  });

  function fmtDate(ts: number | null): string {
    if (!ts) return "";
    return new Date(ts * 1000).toLocaleDateString("ru-RU", { day: "numeric", month: "short" });
  }
</script>

{#if loaded && st}
  <div class="git-bar">
    <div class="seg">
      <span class="branch"><Icon name="git-branch" class="ic-sm" /> {st.branch ?? "—"}</span>
      {#if st.ahead > 0 || st.behind > 0}
        <span class="aheadbehind">
          {#if st.ahead > 0}<span class="a">↑{st.ahead}</span>{/if}
          {#if st.behind > 0}<span class="b">↓{st.behind}</span>{/if}
        </span>
      {/if}
    </div>
    <div class="git-sep"></div>
    {#if st.dirty > 0}
      <span class="dirty-count"><span class="led"></span>{st.dirty} изм.{#if st.staged > 0} · {st.staged} в индексе{/if}</span>
    {:else}
      <span class="last-commit"><Icon name="check" class="ic-sm" /> чисто</span>
    {/if}
    {#if st.lastHash}
      <div class="git-sep"></div>
      <span class="last-commit">
        <span class="hash mono">{st.lastHash}</span>
        <span class="msg">{st.lastMessage ?? ""}</span>
        {#if st.lastTimestamp}<span style="color:var(--muted-2)">· {fmtDate(st.lastTimestamp)}</span>{/if}
      </span>
    {/if}
  </div>
{:else if loaded && repoPath}
  <div class="git-bar">
    <span class="last-commit" style="color:var(--muted-2)"><Icon name="git-branch" class="ic-sm" /> Не git-репозиторий</span>
  </div>
{/if}
```
> Классы `.git-bar`/`.branch`/`.aheadbehind`/`.dirty-count`/`.last-commit`/`.git-sep` уже есть в global.css. Иконка `git-branch` — из lucide.

- [ ] **Step 4: Вставить GitBar в ProjectView.svelte**

В `src/lib/components/ProjectView.svelte`:
1. Добавить импорт:
```ts
  import GitBar from "./GitBar.svelte";
```
2. Сразу ПОСЛЕ закрывающего `</div>` блока `<div class="proj-head">...</div>` и ПЕРЕД `<div class="tabs">` вставить:
```svelte
  <GitBar repoPath={project.repoPath ?? project.path} />
```

- [ ] **Step 5: Проверка (NON-blocking; не запускать `npm run tauri dev`)**

Run:
```powershell
npm run check
npm test
npm run build
```
Expected: `npm run check` 0 ОШИБОК (прежние a11y/`node`/`state_referenced_locally` warnings приемлемы); Vitest 3 passed; build успешен.

- [ ] **Step 6: Commit**

```powershell
git add src/lib/types.ts src/lib/api/git.ts src/lib/components/GitBar.svelte src/lib/components/ProjectView.svelte
git commit -m @'
feat(frontend): git summary bar in project header

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

## Task 3: Проверка среза

- [ ] **Step 1: Автопроверка (контроллер)**
```powershell
npm run check ; npm test ; cargo test --manifest-path src-tauri/Cargo.toml --lib
```
Expected: типы 0 ошибок; Vitest 3; Rust 9 (7 прежних + 2 git).

- [ ] **Step 2: Ручной GUI-смоук (пользователь; `npm run tauri dev`)**
- [ ] Создать проект, задать `repoPath` (или `path`) на реальную папку с git-репозиторием (например, саму папку DevDeck).
- [ ] Открыть проект → под шапкой git-бар: ветка (`main`), последний коммит (хэш + сообщение + дата); если есть изменения — счётчик «N изм.», иначе «чисто».
- [ ] Внести изменение в репозитории (создать файл) → переоткрыть проект → счётчик dirty вырос.
- [ ] Указать путь на папку БЕЗ git → «Не git-репозиторий».
- [ ] Проект без пути → git-бар отсутствует.
- [ ] (Если у ветки настроен upstream и есть расхождение) → видно ↑ahead/↓behind.

---

## Итог среза

Git-сводка проекта (ветка, ahead/behind, dirty/staged/untracked, последний коммит) читается через `git2` без сети и показывается в шапке; «не репозиторий» обрабатывается мягко. Готовит почву для среза сетевых операций (Pull/Push/Fetch + commit-all через системный `git`) и блока «Требуют внимания» на дашборде.
```
