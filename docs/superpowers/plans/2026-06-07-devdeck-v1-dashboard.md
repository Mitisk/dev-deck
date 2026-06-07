# DevDeck v1.0 — Срез «Дашборд» — план реализации

> **Для исполнителя:** ОБЯЗАТЕЛЬНЫЙ ПОД-НАВЫК: superpowers:subagent-driven-development. Шаги — чекбоксы.

**Цель:** Оживить стартовый экран (когда проект не выбран): закреплённые проекты крупными карточками с быстрыми действиями (папка/редактор/push), недавно открытые проекты, и блок «Требуют внимания» — проекты с незакоммиченными изменениями или готовые к push (git-статус по всем проектам).

**Архитектура:** Команда `dashboard_attention()` обходит проекты, читает git-статус каждого через `git2` (переиспользует `status_of` из `git.rs`) и возвращает те, у кого dirty>0 или ahead>0. Недавние проекты — фронтовый стор `recents` (localStorage), подписан на `activeProjectId`. Быстрые действия — существующие команды (`open_path`/`open_in_editor`/`git_push`).

**Стек:** как раньше. Без миграций.

**Решения / границы:**
- «Задачи на сегодня/просроченные» — **отложено**: требует структурированных дат у задач (сейчас `due_date` — свободный текст). Сделаем после date-picker в задачах.
- «Недавние» — через localStorage (без поля `last_opened_at` в БД); дедуп, максимум 6.
- Темы уже сохраняются (стор `theme`); отдельной работы не требуется.

**Источники:** `TZ_DevDeck.md` (5.10 — дашборд). `_prototype/index.html` (`renderDashboard`/`.dash`/`.dcard`/`.att-card`/`.att-row` — разметка-референс). Текущий `Dashboard.svelte`, `git.rs` (`status_of`, `expand`), сторы.

---

## Контекст

- Rust: 55 команд; `git.rs` содержит приватные `expand`, `status_of(&Repository)`. `models.rs`. Таблица `projects` (есть `repo_path`, `path`, `color`, `icon`, `pinned`, `status`).
- Фронт: `Dashboard.svelte` показывает закреплённые карточки (клик → открыть). `stores/projects.ts` (`projects`, `activeProjectId`, `projectsLoaded`). `api/actions.ts` (openPath/openInEditor), `api/git.ts` (push). `Icon.svelte`. global.css: `.dash`/`.dash-hello`/`.dash-sub`/`.dash-cards`/`.dcard`(`.top`/`.be`/`.pmeta`/`.gline`/`.quick`/`.qbtn`)/`.section-title`/`.today`/`.att-card`/`.att-row`(`.att-be`/`.att-main`/`.att-line`/`.att-name`/`.att-branch`/`.att-tag`(`.dirty`/`.ahead`)/`.att-msg`/`.att-hash`/`.att-act`(`.push`))/`.att-empty`.

---

## Структура файлов

```
src-tauri/src/
├─ models.rs            # MOD: + AttentionItem
├─ commands/git.rs      # MOD: + dashboard_attention
└─ lib.rs               # MOD: регистрация dashboard_attention

src/lib/
├─ types.ts             # MOD: + AttentionItem
├─ api/dashboard.ts     # NEW
├─ stores/recents.ts    # NEW
└─ components/Dashboard.svelte  # MOD: закреплённые + недавние + «требуют внимания»
```

---

## Task 1: Rust — dashboard_attention

**Files:** Modify `models.rs`, `commands/git.rs`, `lib.rs`.

- [ ] **Step 1: Модель** (в `models.rs`, после `SearchHit`):
```rust
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttentionItem {
    pub project_id: i64,
    pub name: String,
    pub color: Option<String>,
    pub icon: Option<String>,
    pub branch: Option<String>,
    pub ahead: usize,
    pub dirty: usize,
    pub last_hash: Option<String>,
    pub last_message: Option<String>,
}
```

- [ ] **Step 2: Команда в git.rs**

В `src-tauri/src/commands/git.rs` добавить (нужны импорты: `use crate::models::AttentionItem;` — дополнить существующий `use crate::models::{...};`; `use crate::state::AppState; use tauri::State;` — добавить, если ещё нет; `git2::Repository` уже импортирован):
```rust
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
```

- [ ] **Step 3: Регистрация** — в `lib.rs` `generate_handler![...]` добавить `commands::git::dashboard_attention,`.

- [ ] **Step 4: Сборка** (юнит-тест не делаем — требует реальных репозиториев; покрыто GUI-смоуком)
```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib
cargo build --manifest-path src-tauri/Cargo.toml
```
Expected: прежние 20 тестов ok; build успешен.

- [ ] **Step 5: Commit**
```powershell
git add src-tauri/src/models.rs src-tauri/src/commands/git.rs src-tauri/src/lib.rs
git commit -m @'
feat(backend): dashboard_attention (projects with dirty/ahead git state)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

## Task 2: Фронт — дашборд

**Files:** Modify `types.ts`, `Dashboard.svelte`; Create `api/dashboard.ts`, `stores/recents.ts`.

- [ ] **Step 1: Тип** (в конец `types.ts`):
```ts
export type AttentionItem = {
  projectId: number;
  name: string;
  color: string | null;
  icon: string | null;
  branch: string | null;
  ahead: number;
  dirty: number;
  lastHash: string | null;
  lastMessage: string | null;
};
```

- [ ] **Step 2: api/dashboard.ts**
```ts
import { call } from "./client";
import type { AttentionItem } from "../types";

export const attention = () => call<AttentionItem[]>("dashboard_attention");
```

- [ ] **Step 3: stores/recents.ts**

Create `src/lib/stores/recents.ts`:
```ts
import { writable } from "svelte/store";
import { browser } from "$app/environment";
import { activeProjectId } from "./projects";

const KEY = "devdeck-recents";
const MAX = 6;

function initial(): number[] {
  if (!browser) return [];
  try {
    const raw = localStorage.getItem(KEY);
    return raw ? (JSON.parse(raw) as number[]) : [];
  } catch {
    return [];
  }
}

export const recents = writable<number[]>(initial());

recents.subscribe((v) => {
  if (browser) localStorage.setItem(KEY, JSON.stringify(v));
});

// При открытии проекта — поднять его в начало списка недавних.
activeProjectId.subscribe((id) => {
  if (id == null) return;
  recents.update((list) => [id, ...list.filter((x) => x !== id)].slice(0, MAX));
});
```

- [ ] **Step 4: Dashboard.svelte**

Заменить ВЕСЬ `src/lib/components/Dashboard.svelte` на:
```svelte
<script lang="ts">
  import { projects, activeProjectId, projectsLoaded } from "$lib/stores/projects";
  import { recents } from "$lib/stores/recents";
  import { showNewProject } from "$lib/stores/ui";
  import { USER } from "$lib/mock";
  import { statusLabel } from "$lib/format";
  import * as dash from "$lib/api/dashboard";
  import * as actions from "$lib/api/actions";
  import * as git from "$lib/api/git";
  import { pushToast } from "$lib/stores/toasts";
  import type { AttentionItem } from "$lib/types";
  import Icon from "./Icon.svelte";

  const visible = $derived($projects.filter((p) => p.status !== "archived"));
  const pinned = $derived(visible.filter((p) => p.pinned));
  const recentProjects = $derived(
    $recents.map((id) => visible.find((p) => p.id === id)).filter((p): p is NonNullable<typeof p> => !!p).slice(0, 6),
  );

  let attention = $state<AttentionItem[]>([]);
  async function loadAttention() {
    try {
      attention = await dash.attention();
    } catch {
      /* тост из api/client.ts */
    }
  }
  $effect(() => {
    // перезагружать при изменении набора проектов
    void $projects.length;
    loadAttention();
  });

  function open(id: number) {
    activeProjectId.set(id);
  }
  function quick(e: MouseEvent, fn: () => Promise<unknown>) {
    e.stopPropagation();
    void fn();
  }
  async function pushProject(item: AttentionItem) {
    const p = visible.find((x) => x.id === item.projectId);
    const repo = p?.repoPath ?? p?.path;
    if (!repo) return;
    const r = await git.push(repo);
    pushToast(r.ok ? "Push выполнен" : "Git: ошибка", r.output.split("\n").slice(-1).join(""), r.ok ? "ok" : "error");
    await loadAttention();
  }
  function effRepo(id: number): string | null {
    const p = visible.find((x) => x.id === id);
    return p?.repoPath ?? p?.path ?? null;
  }
  function effPath(id: number): string | null {
    const p = visible.find((x) => x.id === id);
    return p?.path ?? null;
  }
</script>

<div class="ws-inner dash">
  <div style="margin-bottom:22px">
    <div class="dash-hello">Привет, <span>{USER.name}</span></div>
    <div class="dash-sub">{visible.length} {visible.length === 1 ? "проект" : "проектов"} · {attention.length} требуют внимания</div>
  </div>

  {#if $projectsLoaded && !visible.length}
    <div class="card" style="padding:40px;text-align:center;color:var(--muted)">
      <div style="font-size:15px;color:var(--text);font-weight:600;margin-bottom:6px">Здесь пока пусто</div>
      <div style="margin-bottom:16px">Создайте первый проект, чтобы начать.</div>
      <button class="btn-primary" onclick={() => showNewProject.set(true)}><Icon name="plus" class="ic ic-sm" /> Новый проект</button>
    </div>
  {:else}
    {#if pinned.length}
      <h3 class="section-title"><Icon name="star" class="ic-sm" /> Закреплённые</h3>
      <div class="dash-cards">
        {#each pinned as p (p.id)}
          <div class="card dcard" style="--p-color:{p.color ?? 'var(--accent)'}" role="button" tabindex="0" onclick={() => open(p.id)}>
            <div class="top">
              <span class="be">{p.icon ?? "📁"}</span>
              <div style="min-width:0"><h3>{p.name}</h3><div class="pmeta">{p.path ?? statusLabel(p.status)}</div></div>
            </div>
            <div class="quick">
              <button class="qbtn" disabled={!effPath(p.id)} onclick={(e) => quick(e, () => actions.openPath(effPath(p.id)!))}><Icon name="folder-open" class="ic-sm" /> Папка</button>
              <button class="qbtn" disabled={!effPath(p.id)} onclick={(e) => quick(e, () => actions.openInEditor(effPath(p.id)!))}><Icon name="code-xml" class="ic-sm" /> Код</button>
              <button class="qbtn" disabled={!effRepo(p.id)} onclick={(e) => quick(e, async () => { const r = await git.push(effRepo(p.id)!); pushToast(r.ok ? 'Push выполнен' : 'Git: ошибка', r.output.split('\n').slice(-1).join(''), r.ok ? 'ok' : 'error'); await loadAttention(); })}><Icon name="arrow-up" class="ic-sm" /> Push</button>
            </div>
          </div>
        {/each}
      </div>
    {/if}

    {#if recentProjects.length}
      <h3 class="section-title" style="margin-top:26px"><Icon name="history" class="ic-sm" /> Недавние</h3>
      <div class="dash-cards">
        {#each recentProjects as p (p.id)}
          <div class="card dcard" style="--p-color:{p.color ?? 'var(--accent)'}" role="button" tabindex="0" onclick={() => open(p.id)}>
            <div class="top">
              <span class="be">{p.icon ?? "📁"}</span>
              <div style="min-width:0"><h3>{p.name}</h3><div class="pmeta">{p.path ?? statusLabel(p.status)}</div></div>
            </div>
          </div>
        {/each}
      </div>
    {/if}

    <div style="margin-top:30px">
      <h3 class="section-title"><Icon name="git-pull-request-arrow" class="ic-sm" /> Требуют внимания</h3>
      <div class="card att-card">
        {#each attention as a (a.projectId)}
          <div class="att-row" role="button" tabindex="0" onclick={() => open(a.projectId)}>
            <span class="att-be" style="--p-color:{a.color ?? 'var(--accent)'}">{a.icon ?? "📁"}</span>
            <div class="att-main">
              <div class="att-line">
                <span class="att-name">{a.name}</span>
                {#if a.branch}<span class="att-branch"><Icon name="git-branch" class="ic-sm" /> {a.branch}</span>{/if}
                {#if a.dirty > 0}<span class="att-tag dirty"><Icon name="dot" class="ic-sm" />{a.dirty} изм.</span>
                {:else}<span class="att-tag ahead">↑{a.ahead} к push</span>{/if}
              </div>
              {#if a.lastHash}<div class="att-msg"><span class="att-hash mono">{a.lastHash}</span> {a.lastMessage ?? ""}</div>{/if}
            </div>
            {#if a.dirty > 0}
              <button class="att-act" onclick={(e) => { e.stopPropagation(); open(a.projectId); }}><Icon name="git-commit-horizontal" class="ic-sm" /> Открыть</button>
            {:else}
              <button class="att-act push" onclick={(e) => { e.stopPropagation(); pushProject(a); }}><Icon name="arrow-up" class="ic-sm" /> Push</button>
            {/if}
          </div>
        {/each}
        {#if !attention.length}
          <div class="att-empty"><Icon name="check" class="ic-sm" /> Всё закоммичено и запушено — рабочие деревья чистые</div>
        {/if}
      </div>
    </div>
  {/if}
</div>
```
> Иконки `plus`/`star`/`folder-open`/`code-xml`/`arrow-up`/`history`/`git-pull-request-arrow`/`git-branch`/`dot`/`git-commit-horizontal`/`check` — из lucide. Все классы дашборда — в global.css.

- [ ] **Step 5: Проверка**
```powershell
npm run check
npm test
npm run build
```
Expected: 0 ошибок типов; Vitest 3; build успешен.

- [ ] **Step 6: Commit**
```powershell
git add src/lib/types.ts src/lib/api/dashboard.ts src/lib/stores/recents.ts src/lib/components/Dashboard.svelte
git commit -m @'
feat(frontend): dashboard with pinned/recent projects and attention block

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

## Task 3: Проверка среза

- [ ] **Step 1: Автопроверка**
```powershell
npm run check ; npm test ; cargo test --manifest-path src-tauri/Cargo.toml --lib
```
Expected: типы 0 ошибок; Vitest 3; Rust 20.

- [ ] **Step 2: Ручной GUI-смоук (пользователь; `npm run tauri dev`)**
- [ ] Закрыть карточку проекта (вернуться на дашборд) — клик по логотипу/пустому состоянию... (или просто при старте без выбранного проекта).
- [ ] Дашборд: «Закреплённые» с кнопками Папка/Код/Push; «Недавние» (проекты, которые открывал); «Требуют внимания».
- [ ] Кнопки на закреплённой карточке: Папка/Код открывают; Push пушит (тост).
- [ ] Проект с git-репо и изменениями → появляется в «Требуют внимания» с «N изм.»; с ahead>0 → кнопка Push.
- [ ] Push из блока внимания → проект уходит из блока после успешного push.
- [ ] Нет проектов с изменениями → «Всё закоммичено…».

---

## Итог среза

Дашборд показывает закреплённые проекты с быстрыми действиями, недавно открытые и блок «Требуют внимания» (git dirty/ahead по всем репозиториям). Отложено: «задачи на сегодня/просроченные» (нужны структурированные даты задач), быстрые действия в блоке внимания сверх push. Дальше — системный трей.
```
