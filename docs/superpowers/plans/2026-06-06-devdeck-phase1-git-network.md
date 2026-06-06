# DevDeck Phase 1 — Срез «Сетевой git» — план реализации

> **Для исполнителя:** ОБЯЗАТЕЛЬНЫЙ ПОД-НАВЫК: superpowers:subagent-driven-development. Шаги — чекбоксы.

**Цель:** Кнопки Fetch / Pull / Push и быстрый «Commit all» в git-баре проекта. Сетевые операции — через системный `git` (переиспользуют настроенные у пользователя SSH-агент / Git Credential Manager). После операции git-сводка перечитывается; результат — тост «успешно/ошибка + вывод».

**Архитектура:** Команды `git_fetch`/`git_pull`/`git_push`/`git_commit_all` запускают `git` через `std::process::Command` с `current_dir(repo)` — **без shell**, аргументы (включая сообщение коммита) передаются как argv (инъекция исключена). Возвращают `GitOpResult { ok, output }` (вывод stdout+stderr). Фронт показывает busy-состояние на кнопках, по результату — тост и перезагрузку статуса (`git_status` из прошлого среза).

**Стек:** как раньше (системный `git` уже есть в среде; новых crate не нужно).

**Решения / границы:**
- **Системный `git`**, не git2 — чтобы работали уже настроенные креды (SSH/GCM). Чтение статуса осталось на git2 (прошлый срез).
- **Прогресс:** пока синхронный захват вывода + тост (операция выполняется в фоновом потоке Tauri, UI не блокируется). Живой стрим прогресса событиями (`emit`) — будущее улучшение; отмечено.
- Блок «Требуют внимания» на дашборде (git по всем проектам) — по-прежнему отложен.
- Без shell: `Command::new("git").current_dir(repo).args([...])`. Путь — `current_dir` (не интерполируется); сообщение коммита — отдельный arg.

**Источники:** `TZ_DevDeck.md` (5.3 — сетевые операции через системный git, быстрый коммит; 10 — `git_fetch/git_pull/git_push/git_commit_all`). `_prototype/index.html` (`.git-actions`, `.gbtn`, `.commit-field` — разметка-референс). Текущий `GitBar.svelte`, `commands/git.rs` (есть `expand`), `models.rs`.

---

## Контекст (после среза «Git-статус»)

- Rust: 14 команд; `commands/git.rs` содержит `expand(&str)` (раскрытие `~`), `status_of`, `git_status`. `models.rs`: `Project`, `ProjectInput`, `GitStatus`. `error::{AppError, ErrorKind, AppResult}` + `From<git2::Error>`.
- Фронт: `GitBar.svelte` грузит и рисует статус по `repoPath`; api `src/lib/api/git.ts` (`status`). global.css: `.git-actions`, `.gbtn`(`.primary`), `.commit-field`.

---

## Структура файлов

```
src-tauri/src/
├─ models.rs            # MOD: + GitOpResult
├─ commands/git.rs      # MOD: + run_git + 4 команды + тест commit_all
└─ lib.rs               # MOD: регистрация 4 команд

src/lib/
├─ types.ts             # MOD: + GitOpResult
├─ api/git.ts           # MOD: + fetch/pull/push/commitAll
└─ components/GitBar.svelte  # MOD: кнопки Fetch/Pull/Push + Commit all, перезагрузка статуса
```

---

## Task 1: Rust — сетевые git-команды

**Files:**
- Modify: `src-tauri/src/models.rs`, `src-tauri/src/commands/git.rs`, `src-tauri/src/lib.rs`

- [ ] **Step 1: Модель GitOpResult**

В `src-tauri/src/models.rs` добавить (после `GitStatus`):
```rust
/// Результат сетевой/коммит-операции git: успех + объединённый вывод (stdout+stderr).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitOpResult {
    pub ok: bool,
    pub output: String,
}
```

- [ ] **Step 2: Хелпер и команды в commands/git.rs**

В `src-tauri/src/commands/git.rs`:

1. Добавить импорты (вверху, рядом с существующими):
```rust
use crate::error::{AppError, ErrorKind};
use crate::models::GitOpResult;
use std::path::Path;
use std::process::Command;
```
(`use crate::models::GitStatus;` и `use crate::error::AppResult;` уже есть — не дублировать; при необходимости объединить `use crate::error::{AppError, AppResult, ErrorKind};` и `use crate::models::{GitOpResult, GitStatus};`.)

2. Добавить хелпер и команды (после существующего `git_status`):
```rust
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
```

3. Добавить тест в существующий `mod tests` (в конец, рядом с git-тестами). Он создаёт временный репозиторий, локально настраивает identity и проверяет, что `git_commit_all` делает коммит:
```rust
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
```
> `temp_repo()` и `fs` уже используются git-тестами из прошлого среза. `git_commit_all` — обычная функция (атрибут `#[tauri::command]` не мешает прямому вызову), State не требует. Тесту нужен `git` в PATH (есть в среде).

- [ ] **Step 3: Зарегистрировать команды**

В `src-tauri/src/lib.rs` в `tauri::generate_handler![...]` добавить (после `commands::git::git_status,`):
```rust
            commands::git::git_fetch,
            commands::git::git_pull,
            commands::git::git_push,
            commands::git::git_commit_all,
```

- [ ] **Step 4: Тесты + сборка (NON-blocking; не запускать `npm run tauri dev`)**

Run:
```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib
cargo build --manifest-path src-tauri/Cargo.toml
```
Expected: новый тест `commit_all_creates_commit_via_system_git` + прежние (9) → 10 ok; build успешен. (Если в среде не настроен `git` в PATH — тест упадёт; в этой среде git есть.)

- [ ] **Step 5: Commit**

```powershell
git add src-tauri/src/models.rs src-tauri/src/commands/git.rs src-tauri/src/lib.rs
git commit -m @'
feat(backend): network git (fetch/pull/push) and commit-all via system git

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

## Task 2: Фронт — кнопки git-операций в GitBar

**Files:**
- Modify: `src/lib/types.ts`, `src/lib/api/git.ts`, `src/lib/components/GitBar.svelte`

- [ ] **Step 1: Тип GitOpResult**

В `src/lib/types.ts` добавить (в конец):
```ts
export type GitOpResult = { ok: boolean; output: string };
```

- [ ] **Step 2: api-обёртки**

В `src/lib/api/git.ts` добавить (после `status`):
```ts
import type { GitStatus, GitOpResult } from "../types"; // ← заменить существующий import GitStatus на этот

export const fetch = (repoPath: string) => call<GitOpResult>("git_fetch", { repoPath });
export const pull = (repoPath: string) => call<GitOpResult>("git_pull", { repoPath });
export const push = (repoPath: string) => call<GitOpResult>("git_push", { repoPath });
export const commitAll = (repoPath: string, message: string) =>
  call<GitOpResult>("git_commit_all", { repoPath, message });
```
> Существующая строка `import type { GitStatus } from "../types";` заменяется на объединённый импорт с `GitOpResult`. `status` остаётся как был.

- [ ] **Step 3: GitBar.svelte — действия + перезагрузка статуса**

Заменить ВЕСЬ `src/lib/components/GitBar.svelte` на:
```svelte
<script lang="ts">
  import type { GitStatus, GitOpResult } from "$lib/types";
  import * as git from "$lib/api/git";
  import { pushToast } from "$lib/stores/toasts";
  import Icon from "./Icon.svelte";

  let { repoPath }: { repoPath: string | null } = $props();

  let st = $state<GitStatus | null>(null);
  let loaded = $state(false);
  let busy = $state<string | null>(null); // имя текущей операции
  let message = $state("");

  let reqId = 0;
  async function load() {
    if (!repoPath) {
      st = null;
      loaded = true;
      return;
    }
    const my = ++reqId;
    try {
      const s = await git.status(repoPath);
      if (my === reqId) st = s;
    } catch {
      // тост ошибки показывает api/client.ts
    } finally {
      if (my === reqId) loaded = true;
    }
  }

  // Перезагрузка при смене пути.
  $effect(() => {
    repoPath;
    loaded = false;
    st = null;
    load();
  });

  async function op(name: string, fn: () => Promise<GitOpResult>, okMsg: string) {
    if (!repoPath || busy) return;
    busy = name;
    try {
      const r = await fn();
      const tail = r.output.split("\n").filter(Boolean).slice(-2).join(" · ");
      pushToast(r.ok ? okMsg : "Git: ошибка", tail || (r.ok ? "" : "см. вывод git"), r.ok ? "ok" : "error");
      if (r.ok) await load();
    } catch {
      // spawn-ошибка («git не найден») — тост из api/client.ts
    } finally {
      busy = null;
    }
  }

  function commit() {
    const m = message.trim();
    if (!m || !repoPath) return;
    op("commit", () => git.commitAll(repoPath!, m), "Коммит создан").then(() => {
      message = "";
    });
  }

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

    <div class="git-actions">
      <div class="commit-field">
        <input placeholder="Сообщение коммита" bind:value={message} onkeydown={(e) => { if (e.key === 'Enter') commit(); }} />
        <button disabled={!message.trim() || !!busy} onclick={commit}>{busy === "commit" ? "…" : "Commit all"}</button>
      </div>
      <button class="gbtn" disabled={!!busy} onclick={() => op("fetch", () => git.fetch(repoPath!), "Fetch выполнен")}>
        <Icon name="refresh-cw" class="ic-sm" /> {busy === "fetch" ? "…" : "Fetch"}
      </button>
      <button class="gbtn" disabled={!!busy} onclick={() => op("pull", () => git.pull(repoPath!), "Pull выполнен")}>
        <Icon name="arrow-down" class="ic-sm" /> {busy === "pull" ? "…" : "Pull"}
      </button>
      <button class="gbtn primary" disabled={!!busy} onclick={() => op("push", () => git.push(repoPath!), "Push выполнен")}>
        <Icon name="arrow-up" class="ic-sm" /> {busy === "push" ? "…" : "Push"}
      </button>
    </div>
  </div>
{:else if loaded && repoPath}
  <div class="git-bar">
    <span class="last-commit" style="color:var(--muted-2)"><Icon name="git-branch" class="ic-sm" /> Не git-репозиторий</span>
  </div>
{/if}
```
> Иконки `refresh-cw`/`arrow-down`/`arrow-up`/`git-branch`/`check` — из lucide. Классы `.git-actions`/`.gbtn`/`.commit-field` — в global.css.

- [ ] **Step 4: Проверка (NON-blocking; не запускать `npm run tauri dev`)**

Run:
```powershell
npm run check
npm test
npm run build
```
Expected: `npm run check` 0 ОШИБОК (a11y/`node`/`state_referenced_locally` warnings приемлемы); Vitest 3 passed; build успешен.

- [ ] **Step 5: Commit**

```powershell
git add src/lib/types.ts src/lib/api/git.ts src/lib/components/GitBar.svelte
git commit -m @'
feat(frontend): git fetch/pull/push and commit-all buttons in GitBar

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

## Task 3: Проверка среза

- [ ] **Step 1: Автопроверка (контроллер)**
```powershell
npm run check ; npm test ; cargo test --manifest-path src-tauri/Cargo.toml --lib
```
Expected: типы 0 ошибок; Vitest 3; Rust 10 (9 прежних + commit_all).

- [ ] **Step 2: Ручной GUI-смоук (пользователь; `npm run tauri dev`)**
- [ ] Проект с реальным git-репо (с настроенным remote) → в git-баре кнопки Fetch/Pull/Push и поле «Сообщение коммита».
- [ ] Внести изменение → ввести сообщение → «Commit all» → тост «Коммит создан»; счётчик dirty обнулился, ahead вырос; последний коммит обновился.
- [ ] «Push» → тост «Push выполнен» (или ошибка с выводом git, если нет upstream/прав); ahead обнулился.
- [ ] «Fetch»/«Pull» → тост с результатом; статус перечитался.
- [ ] Пока операция идёт — кнопки задизейблены, на активной «…».
- [ ] Ошибка (нет remote/коммитить нечего) → тост с хвостом вывода git, приложение не падает.

---

## Итог среза

Полный быстрый git-цикл из шапки проекта: Commit all → Push, плюс Fetch/Pull, через системный `git` (с кредами пользователя), без shell. Git-сводка перечитывается после операций. Дальше по Phase 1 — наполнение вкладок проекта (**заметки / чеклисты / задачи**) и позже блок «Требуют внимания» + живой стрим прогресса git.
```
