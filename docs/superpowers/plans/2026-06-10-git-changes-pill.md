# Git changes pill + file popover — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Заменить непрозрачный счётчик «6 изм.» в git-баре на пилюлю с разбивкой изменений по категориям, раскрывающую попап со списком файлов, diffstat `+X −Y` и открытием файла в редакторе по клику.

**Architecture:** Бэкенд добавляет read-only команду `git_changes` (через `git2`, без внешних процессов) и команду `open_file_in_editor`. Фронтенд: чистый хелпер разбивки счётчиков (тестируемый), новый компонент `GitChangesPill.svelte` (пилюля + ленивый попап), встроенный в `GitBar`.

**Tech Stack:** Rust + `git2`, Tauri 2 commands; SvelteKit (Svelte 5 runes) + TypeScript; Vitest (node-env, pure-logic тесты).

Спека: `docs/superpowers/specs/2026-06-10-git-changes-pill-design.md`.

---

## File Structure

**Backend**
- `src-tauri/src/models.rs` — +структуры `GitChanges`, `GitFile`.
- `src-tauri/src/commands/git.rs` — +`changes_of()` (логика) и `#[tauri::command] git_changes` + тест.
- `src-tauri/src/commands/actions.rs` — +`#[tauri::command] open_file_in_editor` + тест.
- `src-tauri/src/lib.rs` — регистрация двух команд в `generate_handler!`.

**Frontend**
- `src/lib/types.ts` — +типы `GitChanges`, `GitFile`.
- `src/lib/api/git.ts` — +`changes()`.
- `src/lib/api/actions.ts` — +`openFileInEditor()`.
- `src/lib/gitChanges.ts` — новый чистый хелпер `changeCategories()`.
- `src/tests/gitChanges.test.ts` — новый тест хелпера.
- `src/lib/components/GitChangesPill.svelte` — новый компонент.
- `src/lib/components/GitBar.svelte` — подключение пилюли вместо `dirty-count`.
- `src/lib/styles/global.css` — стили пилюли/попапа.

---

## Task 1: Backend — модели GitChanges/GitFile

**Files:**
- Modify: `src-tauri/src/models.rs` (после `GitOpResult`, ~строка 60)

- [ ] **Step 1: Добавить структуры в models.rs**

После определения `GitOpResult` (заканчивается на строке ~60) вставить:

```rust
/// Один изменённый файл рабочего дерева/индекса.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitFile {
    pub path: String,    // относительный путь от корня репозитория (POSIX)
    pub code: String,    // "M" | "A" | "D" | "R" | "T" | "?"
    pub staged: bool,    // присутствует в индексе
}

/// Детализация незакоммиченных изменений (для попапа пилюли).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitChanges {
    pub insertions: usize, // суммарно +строк (staged+unstaged)
    pub deletions: usize,  // суммарно −строк
    pub files: Vec<GitFile>,
}
```

- [ ] **Step 2: Проверить компиляцию**

Run: `cargo build --manifest-path src-tauri/Cargo.toml --lib`
Expected: успешная сборка (структуры пока не используются — допустимо, `#[derive(Serialize)]` не вызывает dead_code-ошибки для pub-полей).

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/models.rs
git commit -m "feat(backend): GitChanges/GitFile models"
```

---

## Task 2: Backend — команда git_changes (TDD)

**Files:**
- Modify: `src-tauri/src/commands/git.rs` (логика + команда; тест в существующем `#[cfg(test)] mod tests`)
- Modify: `src-tauri/src/lib.rs` (регистрация)

- [ ] **Step 1: Написать падающий тест**

В `src-tauri/src/commands/git.rs`, внутри `mod tests` (после теста `clean_repo_has_zero_dirty`), добавить:

```rust
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
```

- [ ] **Step 2: Запустить тест — убедиться, что не компилируется/падает**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib git_changes_reports`
Expected: ошибка компиляции — `changes_of` не найдена.

- [ ] **Step 3: Реализовать changes_of и git_changes**

В `src-tauri/src/commands/git.rs`: добавить `GitChanges, GitFile` в импорт моделей (строка 2):

```rust
use crate::models::{AttentionItem, GitChanges, GitFile, GitOpResult, GitStatus};
```

После функции `status_of` (заканчивается на строке ~88) добавить:

```rust
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
    opts.include_untracked(true).include_ignored(false);
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
```

- [ ] **Step 4: Зарегистрировать команду**

В `src-tauri/src/lib.rs`, в блоке `tauri::generate_handler!`, после строки `commands::git::git_commit_all,` (строка ~119) добавить:

```rust
            commands::git::git_changes,
```

- [ ] **Step 5: Запустить тест — убедиться, что проходит**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib git_changes_reports`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/commands/git.rs src-tauri/src/lib.rs
git commit -m "feat(backend): git_changes command (diffstat + file list)"
```

---

## Task 3: Backend — команда open_file_in_editor (TDD)

**Files:**
- Modify: `src-tauri/src/commands/actions.rs` (команда + тест)
- Modify: `src-tauri/src/lib.rs` (регистрация)

- [ ] **Step 1: Написать падающий тест**

В `src-tauri/src/commands/actions.rs`, внутри `#[cfg(test)] mod tests` (после теста `expands_leading_tilde`), добавить:

```rust
    #[test]
    fn open_file_in_editor_rejects_missing_path() {
        let res = super::open_file_in_editor("Z:\\definitely\\missing\\file.txt".into());
        assert!(res.is_err(), "ожидали ошибку для несуществующего файла");
    }
```

- [ ] **Step 2: Запустить тест — убедиться, что не компилируется**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib open_file_in_editor_rejects`
Expected: ошибка компиляции — `open_file_in_editor` не найдена.

- [ ] **Step 3: Реализовать команду**

В `src-tauri/src/commands/actions.rs`, после функции `open_in_editor` (заканчивается на строке ~71), добавить:

```rust
/// Открыть конкретный файл в редакторе (VS Code). В отличие от open_in_editor
/// (валидирует папку), здесь путь — файл, поэтому require_dir_or_file.
#[tauri::command]
pub fn open_file_in_editor(path: String) -> AppResult<()> {
    let p = require_dir_or_file(&path)?;
    if Command::new("code.cmd").arg(&p).spawn().is_ok() {
        return Ok(());
    }
    let mut cmd = Command::new("cmd");
    cmd.args(["/C", "code", &p]);
    spawn(cmd, "редактор (code)")
}
```

- [ ] **Step 4: Зарегистрировать команду**

В `src-tauri/src/lib.rs`, после строки `commands::actions::open_url,` (строка ~114) добавить:

```rust
            commands::actions::open_file_in_editor,
```

- [ ] **Step 5: Запустить тест — убедиться, что проходит**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib open_file_in_editor_rejects`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/commands/actions.rs src-tauri/src/lib.rs
git commit -m "feat(backend): open_file_in_editor command"
```

---

## Task 4: Frontend — чистый хелпер changeCategories (TDD)

**Files:**
- Create: `src/lib/gitChanges.ts`
- Create: `src/tests/gitChanges.test.ts`

- [ ] **Step 1: Написать падающий тест**

Создать `src/tests/gitChanges.test.ts`:

```ts
import { describe, it, expect } from "vitest";
import { changeCategories } from "../lib/gitChanges";

describe("changeCategories", () => {
  it("разбивает dirty на modified/untracked/staged", () => {
    expect(changeCategories(6, 1, 2)).toEqual({ modified: 3, untracked: 2, staged: 1 });
  });

  it("клампит modified в 0 при пересечениях", () => {
    expect(changeCategories(2, 2, 2)).toEqual({ modified: 0, untracked: 2, staged: 2 });
  });

  it("всё чисто", () => {
    expect(changeCategories(0, 0, 0)).toEqual({ modified: 0, untracked: 0, staged: 0 });
  });
});
```

- [ ] **Step 2: Запустить тест — убедиться, что падает**

Run: `npm test -- gitChanges`
Expected: FAIL — модуль `../lib/gitChanges` не найден.

- [ ] **Step 3: Реализовать хелпер**

Создать `src/lib/gitChanges.ts`:

```ts
// Разбивка счётчиков git-статуса на категории для пилюли изменений.
// modified = всё грязное минус staged минус untracked (клампим в 0).
export type ChangeCategories = { modified: number; untracked: number; staged: number };

export function changeCategories(dirty: number, staged: number, untracked: number): ChangeCategories {
  return {
    modified: Math.max(0, dirty - staged - untracked),
    untracked,
    staged,
  };
}
```

- [ ] **Step 4: Запустить тест — убедиться, что проходит**

Run: `npm test -- gitChanges`
Expected: PASS (3 теста).

- [ ] **Step 5: Commit**

```bash
git add src/lib/gitChanges.ts src/tests/gitChanges.test.ts
git commit -m "feat(frontend): changeCategories helper + tests"
```

---

## Task 5: Frontend — типы и API-обёртки

**Files:**
- Modify: `src/lib/types.ts` (после `GitOpResult`, строка ~38)
- Modify: `src/lib/api/git.ts`
- Modify: `src/lib/api/actions.ts`

- [ ] **Step 1: Добавить типы**

В `src/lib/types.ts`, после строки `export type GitOpResult = { ok: boolean; output: string };` (строка 38), добавить:

```ts
export type GitFile = {
  path: string;
  code: string; // M | A | D | R | T | ?
  staged: boolean;
};

export type GitChanges = {
  insertions: number;
  deletions: number;
  files: GitFile[];
};
```

- [ ] **Step 2: Добавить вызов git.changes**

В `src/lib/api/git.ts`: заменить строку импорта типов (строка 2) на:

```ts
import type { GitStatus, GitOpResult, GitChanges } from "../types";
```

После строки `export const status = ...` (строка 5) добавить:

```ts
// null = путь пуст или не git-репозиторий.
export const changes = (repoPath: string) => call<GitChanges | null>("git_changes", { repoPath });
```

- [ ] **Step 3: Добавить вызов actions.openFileInEditor**

В `src/lib/api/actions.ts`, после строки `export const openInEditor = ...` (строка 4), добавить:

```ts
export const openFileInEditor = (path: string) => call<void>("open_file_in_editor", { path });
```

- [ ] **Step 4: Проверить типы**

Run: `npm run check`
Expected: 0 ошибок (число warnings — на базовом уровне проекта, ~40).

- [ ] **Step 5: Commit**

```bash
git add src/lib/types.ts src/lib/api/git.ts src/lib/api/actions.ts
git commit -m "feat(frontend): GitChanges types + api wrappers"
```

---

## Task 6: Frontend — компонент GitChangesPill.svelte

**Files:**
- Create: `src/lib/components/GitChangesPill.svelte`

- [ ] **Step 1: Создать компонент**

Создать `src/lib/components/GitChangesPill.svelte`:

```svelte
<script lang="ts">
  import type { GitChanges } from "$lib/types";
  import * as git from "$lib/api/git";
  import * as actions from "$lib/api/actions";
  import { browser } from "$app/environment";
  import { changeCategories } from "$lib/gitChanges";
  import Icon from "./Icon.svelte";

  let { repoPath, dirty, staged, untracked }:
    { repoPath: string; dirty: number; staged: number; untracked: number } = $props();

  const cats = $derived(changeCategories(dirty, staged, untracked));

  let open = $state(false);
  let data = $state<GitChanges | null>(null);
  let loading = $state(false);
  let error = $state(false);
  let loadedFor = $state(""); // ключ кэша: repoPath + dirty

  async function loadChanges() {
    const key = `${repoPath}:${dirty}`;
    if (loadedFor === key && data) return; // кэш свеж
    loading = true;
    error = false;
    try {
      data = await git.changes(repoPath);
      loadedFor = key;
    } catch {
      error = true; // тост ошибки уже из client.ts
    } finally {
      loading = false;
    }
  }

  function toggle() {
    open = !open;
    if (open) void loadChanges();
  }

  function openFile(path: string) {
    const full = repoPath.replace(/[\\/]+$/, "") + "/" + path;
    void actions.openFileInEditor(full);
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape") open = false;
  }
  function onDocMouseDown(e: MouseEvent) {
    const t = e.target as HTMLElement | null;
    if (!t?.closest?.(".gc-wrap")) open = false;
  }
  $effect(() => {
    if (!browser || !open) return;
    window.addEventListener("keydown", onKey);
    window.addEventListener("mousedown", onDocMouseDown);
    return () => {
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("mousedown", onDocMouseDown);
    };
  });

  function splitPath(p: string): { dir: string; name: string } {
    const i = Math.max(p.lastIndexOf("/"), p.lastIndexOf("\\"));
    return i >= 0 ? { dir: p.slice(0, i + 1), name: p.slice(i + 1) } : { dir: "", name: p };
  }
  function codeClass(code: string): string {
    return code === "?" ? "c-new" : `c-${code.toLowerCase()}`;
  }
</script>

<div class="gc-wrap">
  <button class="gc-pill" class:open onclick={toggle} aria-expanded={open} title="Изменения">
    {#if cats.modified > 0}<span class="gc-cat mod">● {cats.modified}</span>{/if}
    {#if cats.untracked > 0}<span class="gc-cat unt">＋ {cats.untracked}</span>{/if}
    {#if cats.staged > 0}<span class="gc-cat stg">✓ {cats.staged}</span>{/if}
    <Icon name="chevron-down" class="ic-sm gc-caret" />
  </button>

  {#if open}
    <div class="gc-pop" role="dialog" aria-label="Изменённые файлы">
      {#if loading}
        <div class="gc-msg">Загрузка…</div>
      {:else if error}
        <div class="gc-msg">Не удалось прочитать изменения</div>
      {:else if data}
        <div class="gc-stat">
          <span class="ins">+{data.insertions}</span>
          <span class="del">−{data.deletions}</span>
        </div>
        {#if data.files.length}
          <div class="gc-list">
            {#each data.files as f (f.path)}
              {@const sp = splitPath(f.path)}
              <button class="gc-file" onclick={() => openFile(f.path)} title={f.path}>
                <span class="gc-code {codeClass(f.code)}">{f.code}</span>
                <span class="gc-path mono"><span class="dir">{sp.dir}</span>{sp.name}</span>
                {#if f.staged}<span class="gc-staged">индекс</span>{/if}
              </button>
            {/each}
          </div>
        {:else}
          <div class="gc-msg">Нет изменённых файлов</div>
        {/if}
      {/if}
    </div>
  {/if}
</div>
```

- [ ] **Step 2: Проверить типы**

Run: `npm run check`
Expected: 0 ошибок. (Допустимы a11y-warnings уровня проекта; если на `.gc-file`/`.gc-pill` появятся новые a11y-warnings — это `<button>`, они не должны возникать.)

- [ ] **Step 3: Commit**

```bash
git add src/lib/components/GitChangesPill.svelte
git commit -m "feat(frontend): GitChangesPill component"
```

---

## Task 7: Frontend — встроить пилюлю в GitBar

**Files:**
- Modify: `src/lib/components/GitBar.svelte` (импорт + замена блока `dirty-count`, строки ~81-85)

- [ ] **Step 1: Импортировать компонент**

В `src/lib/components/GitBar.svelte`, после строки `import Icon from "./Icon.svelte";` (строка 5), добавить:

```svelte
  import GitChangesPill from "./GitChangesPill.svelte";
```

- [ ] **Step 2: Заменить блок dirty-count**

Заменить существующий блок (строки ~81-85):

```svelte
    {#if st.dirty > 0}
      <span class="dirty-count"><span class="led"></span>{st.dirty} изм.{#if st.staged > 0} · {st.staged} в индексе{/if}</span>
    {:else}
      <span class="last-commit"><Icon name="check" class="ic-sm" /> чисто</span>
    {/if}
```

на:

```svelte
    {#if st.dirty > 0}
      <GitChangesPill repoPath={repoPath!} dirty={st.dirty} staged={st.staged} untracked={st.untracked} />
    {:else}
      <span class="last-commit"><Icon name="check" class="ic-sm" /> чисто</span>
    {/if}
```

(`repoPath!` безопасно: блок внутри `{#if loaded && st}`, а `st` получен из непустого `repoPath`.)

- [ ] **Step 3: Проверить типы**

Run: `npm run check`
Expected: 0 ошибок.

- [ ] **Step 4: Commit**

```bash
git add src/lib/components/GitBar.svelte
git commit -m "feat(frontend): wire GitChangesPill into git bar"
```

---

## Task 8: Frontend — стили пилюли и попапа

**Files:**
- Modify: `src/lib/styles/global.css` (после блока `.dirty-count`, строка ~280)

- [ ] **Step 1: Добавить CSS**

В `src/lib/styles/global.css`, после правила `.dirty-count .led { ... }` (строка ~280), добавить:

```css
/* Git changes — пилюля + попап */
.gc-wrap { position: relative; display: inline-flex; }
.gc-pill {
  display: inline-flex; align-items: center; gap: 8px;
  font-size: 12px; font-weight: 500; cursor: pointer;
  background: var(--surface-2); border: 1px solid var(--border);
  border-radius: 99px; padding: 3px 9px; color: var(--text-2);
}
.gc-pill:hover, .gc-pill.open { border-color: var(--git-dirty); }
.gc-cat { display: inline-flex; align-items: center; gap: 3px; font-family: var(--font-mono); }
.gc-cat.mod { color: var(--git-dirty); }
.gc-cat.unt { color: var(--accent); }
.gc-cat.stg { color: var(--git-ahead); }
.gc-pill .gc-caret { color: var(--muted); transition: transform .15s; }
.gc-pill.open .gc-caret { transform: rotate(180deg); }

.gc-pop {
  position: absolute; top: calc(100% + 6px); left: 0; z-index: 40;
  min-width: 280px; max-width: 440px; max-height: 320px; overflow: auto;
  background: var(--surface); border: 1px solid var(--border);
  border-radius: var(--r-lg); box-shadow: var(--shadow); padding: 8px;
}
.gc-msg { font-size: 12px; color: var(--muted); padding: 8px 6px; }
.gc-stat {
  display: flex; gap: 12px; font-family: var(--font-mono); font-size: 12px;
  padding: 4px 6px 8px; border-bottom: 1px solid var(--border); margin-bottom: 6px;
}
.gc-stat .ins { color: var(--git-ahead); }
.gc-stat .del { color: var(--danger); }
.gc-list { display: flex; flex-direction: column; gap: 1px; }
.gc-file {
  display: flex; align-items: center; gap: 8px; width: 100%;
  background: transparent; border: 0; cursor: pointer; text-align: left;
  padding: 4px 6px; border-radius: 7px; color: var(--text-2);
}
.gc-file:hover { background: var(--surface-2); }
.gc-code {
  flex: none; width: 18px; height: 18px; border-radius: 5px;
  display: inline-flex; align-items: center; justify-content: center;
  font-family: var(--font-mono); font-size: 11px; font-weight: 700;
}
.gc-code.c-m { color: var(--git-dirty); background: color-mix(in oklab, var(--git-dirty) 16%, transparent); }
.gc-code.c-a { color: var(--git-ahead); background: color-mix(in oklab, var(--git-ahead) 16%, transparent); }
.gc-code.c-new { color: var(--accent); background: color-mix(in oklab, var(--accent) 16%, transparent); }
.gc-code.c-d { color: var(--danger); background: color-mix(in oklab, var(--danger) 16%, transparent); }
.gc-code.c-r, .gc-code.c-t { color: var(--muted); background: var(--surface-2); }
.gc-path {
  flex: 1; min-width: 0; font-size: 12px; white-space: nowrap;
  overflow: hidden; text-overflow: ellipsis; direction: rtl; text-align: left;
}
.gc-path .dir { color: var(--muted-2); }
.gc-staged {
  flex: none; font-size: 10px; color: var(--git-ahead);
  background: color-mix(in oklab, var(--git-ahead) 14%, transparent);
  border-radius: 5px; padding: 1px 6px;
}
```

(`direction: rtl` на `.gc-path` обрезает длинный путь слева — имя файла всегда видно справа.)

- [ ] **Step 2: Проверить типы/сборку**

Run: `npm run check`
Expected: 0 ошибок.

- [ ] **Step 3: Commit**

```bash
git add src/lib/styles/global.css
git commit -m "feat(frontend): styles for git changes pill + popover"
```

---

## Task 9: Полная верификация

- [ ] **Step 1: Rust-тесты**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib`
Expected: все тесты PASS, включая `git_changes_reports_files_and_insertions` и `open_file_in_editor_rejects_missing_path`.
(Примечание: флаг `--lib` обязателен, если параллельно запущено `npm run tauri dev` — иначе блокировка файла бинарника.)

- [ ] **Step 2: Фронт-тесты**

Run: `npm test`
Expected: все тесты PASS, включая 3 теста `changeCategories`.

- [ ] **Step 3: Проверка типов**

Run: `npm run check`
Expected: 0 ошибок; число warnings — на базовом уровне проекта (~40).

- [ ] **Step 4: Ручная проверка в приложении**

Run: `npm run tauri dev`
Проверить на проекте с git-изменениями:
- Пилюля показывает категории (например `● 3  ＋ 2  ✓ 1`) вместо «6 изм.».
- Клик раскрывает попап: `+X −Y`, список файлов с цветными буквами статуса.
- Клик по строке файла открывает файл в VS Code.
- Escape / клик вне — попап закрывается.
- На «чистом» репозитории пилюли нет (показано «✓ чисто»).

- [ ] **Step 5: Финальный commit (если остались правки после ручной проверки)**

```bash
git add -A
git commit -m "fix: git changes pill polish after manual verification"
```

(Если правок нет — пропустить.)
