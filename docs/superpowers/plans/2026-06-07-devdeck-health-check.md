# DevDeck — «Health-чек сервисов» — план

> **Для исполнителя:** ОБЯЗАТЕЛЬНЫЙ ПОД-НАВЫК: superpowers:subagent-driven-development. Шаги — чекбоксы.

**Цель:** У проекта можно задать URL health-чека (JSON-страница статуса или просто главная сайта). Если хотя бы у одного проекта он задан — на дашборде показывается «живой» раздел **«Статус сервисов»**: пульсирующий индикатор 🟢/🟠/🔴, HTTP-код, латентность (с цветом), мини-спарклайн последних пингов, авто-обновление и сводка «N/M онлайн».

**Архитектура:** Проверка — в Rust (`ureq`, без облака, без доп. capability). Колонка `health_url` у проектов (миграция 0005). Команда `health_check(url)` делает GET с таймаутом, возвращает `{ok, reachable, status, latencyMs, detail}` (для JSON пытается достать поле status/state/health). Фронт пингует все health-проекты параллельно, копит историю латентности для спарклайна, обновляет каждые 30 c.

**Стек/границы:** backend — миграция, `models.rs`, `commands/projects.rs`, новый `commands/monitor.rs` (+`Cargo.toml` ureq), `lib.rs`, `commands/mod.rs`. frontend — `types.ts`, `api/projects.ts`, новый `api/monitor.ts`, `SettingsTab.svelte`, `Dashboard.svelte`, `global.css`.

---

## Task 1: Rust — колонка, модель, команда проверки

**Files:** Create `src-tauri/migrations/0005_health.sql`, `src-tauri/src/commands/monitor.rs`; Modify `src-tauri/Cargo.toml`, `src-tauri/src/models.rs`, `src-tauri/src/commands/projects.rs`, `src-tauri/src/commands/mod.rs`, `src-tauri/src/lib.rs`.

- [ ] **Step 1: Миграция** — `src-tauri/migrations/0005_health.sql`:
```sql
ALTER TABLE projects ADD COLUMN health_url TEXT;
```

- [ ] **Step 2: Cargo.toml** — в `[dependencies]` добавить:
```toml
ureq = "2"
```

- [ ] **Step 3: models.rs** — в `struct Project` добавить поле после `repo_path`:
```rust
    pub health_url: Option<String>,
```
В `struct ProjectInput` добавить после `repo_path`:
```rust
    pub health_url: Option<String>,
```

- [ ] **Step 4: projects.rs — запросы**
1. `row_to_project`: SELECT и маппинг с `health_url` (индекс 8, остальные сдвигаются):
```rust
    let mut p = conn.query_row(
        "SELECT id, name, description, status, color, icon, path, repo_path, health_url,
                pinned, sort_order, created_at, updated_at
         FROM projects WHERE id = ?1",
        [id],
        |r| {
            Ok(Project {
                id: r.get(0)?,
                name: r.get(1)?,
                description: r.get(2)?,
                status: r.get(3)?,
                color: r.get(4)?,
                icon: r.get(5)?,
                path: r.get(6)?,
                repo_path: r.get(7)?,
                health_url: r.get(8)?,
                pinned: r.get::<_, i64>(9)? != 0,
                sort_order: r.get(10)?,
                tags: Vec::new(),
                created_at: r.get(11)?,
                updated_at: r.get(12)?,
            })
        },
    )?;
```
2. `projects_create` INSERT — добавить `health_url`:
```rust
    conn.execute(
        "INSERT INTO projects(name, description, status, color, icon, path, repo_path, health_url, pinned, sort_order)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0, ?9)",
        params![
            name,
            input.description,
            input.status.as_deref().unwrap_or("active"),
            input.color,
            input.icon,
            input.path,
            input.repo_path,
            input.health_url,
            next_sort,
        ],
    )?;
```
3. `projects_update` UPDATE — добавить `health_url=?9`:
```rust
    let n = conn.execute(
        "UPDATE projects SET name=?2, description=?3, status=?4, color=?5, icon=?6,
                path=?7, repo_path=?8, health_url=?9, updated_at=datetime('now')
         WHERE id=?1",
        params![
            id, name, input.description,
            input.status.as_deref().unwrap_or("active"),
            input.color, input.icon, input.path, input.repo_path, input.health_url,
        ],
    )?;
```

- [ ] **Step 5: commands/monitor.rs** — новый модуль:
```rust
use crate::error::AppResult;
use serde::Serialize;
use std::time::{Duration, Instant};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthResult {
    pub ok: bool,        // 2xx
    pub reachable: bool, // получили любой HTTP-ответ
    pub status: u16,     // HTTP-код (0 при сетевой ошибке)
    pub latency_ms: u64,
    pub detail: Option<String>, // status из JSON или текст ошибки
}

/// Проверить доступность URL (health-чек проекта). Сетевые ошибки не падают,
/// а возвращаются как reachable=false (тостов не будет).
#[tauri::command]
pub fn health_check(url: String) -> AppResult<HealthResult> {
    let u = url.trim();
    let u = if u.starts_with("http://") || u.starts_with("https://") {
        u.to_string()
    } else {
        format!("https://{}", u)
    };
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(6))
        .build();
    let start = Instant::now();
    match agent.get(&u).call() {
        Ok(resp) => {
            let status = resp.status();
            let latency_ms = start.elapsed().as_millis() as u64;
            let detail = parse_json_status(resp);
            Ok(HealthResult { ok: (200..300).contains(&status), reachable: true, status, latency_ms, detail })
        }
        Err(ureq::Error::Status(code, _resp)) => {
            let latency_ms = start.elapsed().as_millis() as u64;
            Ok(HealthResult { ok: false, reachable: true, status: code, latency_ms, detail: None })
        }
        Err(ureq::Error::Transport(_)) => {
            let latency_ms = start.elapsed().as_millis() as u64;
            Ok(HealthResult { ok: false, reachable: false, status: 0, latency_ms, detail: Some("сервис недоступен".into()) })
        }
    }
}

/// Если ответ — JSON, вернуть значение поля status/state/health.
fn parse_json_status(resp: ureq::Response) -> Option<String> {
    if !resp.content_type().contains("json") {
        return None;
    }
    let body = resp.into_string().ok()?;
    let v: serde_json::Value = serde_json::from_str(&body).ok()?;
    for key in ["status", "state", "health"] {
        if let Some(s) = v.get(key).and_then(|x| x.as_str()) {
            return Some(s.to_string());
        }
    }
    None
}
```
> `serde_json` уже в зависимостях (используется в transfer). Если `resp.content_type()` в текущей версии ureq возвращает не `&str` — подстроиться (взять до into_string). `ureq::Error` в ureq 2.x имеет варианты `Status` и `Transport`.

- [ ] **Step 6: Регистрация** — в `commands/mod.rs` добавить `pub mod monitor;`. В `lib.rs` `generate_handler![...]` добавить `commands::monitor::health_check,`.

- [ ] **Step 7: Сборка + тесты**
```powershell
cargo build --manifest-path src-tauri/Cargo.toml --lib
cargo test --manifest-path src-tauri/Cargo.toml --lib
```
Expected: lib-сборка успешна (подтянется ureq+rustls — дольше); тесты 37 ok (логику проектов не сломали; миграция 0005 применяется в `mem()` тестов).

- [ ] **Step 8: Commit**
```powershell
git add src-tauri/migrations/0005_health.sql src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/models.rs src-tauri/src/commands/projects.rs src-tauri/src/commands/monitor.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs
git commit -m @'
feat(backend): project health_url + health_check command (ureq)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

## Task 2: Фронт — поле в настройках и раздел статуса

**Files:** Modify `src/lib/types.ts`, `src/lib/api/projects.ts`, `src/lib/components/SettingsTab.svelte`, `src/lib/components/Dashboard.svelte`, `src/lib/styles/global.css`; Create `src/lib/api/monitor.ts`.

- [ ] **Step 1: types.ts** — в `type Project` добавить `healthUrl: string | null;`. В `ProjectInput` (в `api/projects.ts`) добавить `healthUrl?: string | null;`.

- [ ] **Step 2: api/projects.ts** — в тип `ProjectInput` добавить поле:
```ts
  healthUrl?: string | null;
```

- [ ] **Step 3: api/monitor.ts** — новый файл:
```ts
import { call } from "./client";

export type HealthResult = {
  ok: boolean;
  reachable: boolean;
  status: number;
  latencyMs: number;
  detail: string | null;
};

export const check = (url: string) => call<HealthResult>("health_check", { url });
```

- [ ] **Step 4: SettingsTab — поле Health-чек**

В `<script>`: добавить состояние `let healthUrl = $state(project.healthUrl ?? "");`. В `$effect`, который пересоздаёт форму при смене проекта, добавить `healthUrl = project.healthUrl ?? "";`. В `save()` в объект `projectsApi.update(...)` добавить `healthUrl: healthUrl.trim() || null,`.

В разметке «Основное», после поля «Git-репозиторий» (перед полем «Теги»), добавить:
```svelte
      <div class="field span-2">
        <label for="st-health">Health-чек (URL) <span style="color:var(--muted-2)">(страница статуса или главная сайта)</span></label>
        <input id="st-health" class="tin mono" bind:value={healthUrl} placeholder="https://api.example.com/health" />
      </div>
```

- [ ] **Step 5: Dashboard — раздел «Статус сервисов»**

В `<script>`:
```ts
  import * as monitor from "$lib/api/monitor";
  import type { HealthResult } from "$lib/api/monitor";
  import { onMount, onDestroy } from "svelte";

  const healthProjects = $derived(visible.filter((p) => p.healthUrl && p.healthUrl.trim()));
  let health = $state<Record<number, HealthResult>>({});
  let latHist = $state<Record<number, number[]>>({});
  let checking = $state(false);
  const onlineCount = $derived(healthProjects.filter((p) => health[p.id]?.ok).length);

  async function checkAll() {
    if (!healthProjects.length) return;
    checking = true;
    await Promise.all(healthProjects.map(async (p) => {
      try {
        const r = await monitor.check(p.healthUrl!.trim());
        health = { ...health, [p.id]: r };
        const h = [...(latHist[p.id] ?? []), r.latencyMs].slice(-14);
        latHist = { ...latHist, [p.id]: h };
      } catch { /* */ }
    }));
    checking = false;
  }
  function hsClass(r: HealthResult | undefined): string {
    if (!r) return "";
    if (r.ok) return "up";
    if (r.reachable) return "warn";
    return "down";
  }
  function latColor(ms: number): string {
    return ms < 300 ? "var(--git-ahead)" : ms < 900 ? "var(--git-dirty)" : "var(--danger)";
  }

  let hcTimer: ReturnType<typeof setInterval> | undefined;
  onMount(() => {
    checkAll();
    hcTimer = setInterval(checkAll, 30000);
  });
  onDestroy(() => { if (hcTimer) clearInterval(hcTimer); });
```
> `onMount`/`onDestroy` — если их ещё нет в импортах Dashboard, добавить из `"svelte"`.

В разметке — вставить раздел сразу после блока приветствия (после закрывающего `</div>` блока `dash-hello`/`dash-sub`, перед `{#if $projectsLoaded && !visible.length}`):
```svelte
  {#if healthProjects.length}
    <div style="margin-bottom:26px">
      <h3 class="section-title"><Icon name="activity" class="ic-sm" /> Статус сервисов
        <span class="hs-summary">{onlineCount}/{healthProjects.length} онлайн</span>
        <button class="more" style="margin-left:10px" disabled={checking} onclick={checkAll}>
          <Icon name="refresh-cw" class="ic-sm" /> {checking ? "проверка…" : "обновить"}
        </button>
      </h3>
      <div class="card health-card">
        {#each healthProjects as p (p.id)}
          {@const r = health[p.id]}
          <div class="hs-row" role="button" tabindex="0" onclick={() => p.healthUrl && actions.openUrl(p.healthUrl)}>
            <span class="hs-dot {hsClass(r)}"></span>
            <span class="att-be" style="--p-color:{p.color ?? 'var(--accent)'};width:30px;height:30px;font-size:15px"><ProjectIcon icon={p.icon} size={18} /></span>
            <div class="hs-main">
              <div class="hs-name">{p.name}{#if r?.detail}<span class="hs-detail">· {r.detail}</span>{/if}</div>
              <div class="hs-url">{p.healthUrl}</div>
            </div>
            <div class="hs-spark">
              {#each (latHist[p.id] ?? []) as ms}
                {@const mx = Math.max(1, ...(latHist[p.id] ?? [1]))}
                <i style="height:{Math.max(10, Math.round((ms / mx) * 100))}%;background:{latColor(ms)}"></i>
              {/each}
            </div>
            <div class="hs-meta">
              {#if r}
                <span class="hs-lat" style="color:{latColor(r.latencyMs)}">{r.latencyMs} мс</span>
                <span class="hs-code">{r.reachable ? r.status : "—"}</span>
              {:else}
                <span class="hs-lat" style="color:var(--muted-2)">…</span>
              {/if}
            </div>
          </div>
        {/each}
      </div>
    </div>
  {/if}
```
> `actions` и `ProjectIcon` уже импортированы в Dashboard. Иконки `activity`/`refresh-cw` — валидные lucide.

- [ ] **Step 6: global.css — стили раздела**
```css
/* health-чек сервисов */
.hs-summary { margin-left: 10px; font-size: 11px; font-weight: 600; color: var(--muted); }
.health-card { padding: 4px 6px; }
.hs-row { display: flex; align-items: center; gap: 12px; padding: 10px 12px; border-radius: var(--r-md); border-bottom: 1px solid var(--border); cursor: pointer; transition: var(--t); }
.hs-row:last-child { border-bottom: 0; }
.hs-row:hover { background: var(--hover); }
.hs-dot { width: 11px; height: 11px; border-radius: 50%; flex: none; background: var(--muted-2); }
.hs-dot.up { background: var(--git-ahead); box-shadow: 0 0 0 0 color-mix(in oklab, var(--git-ahead) 70%, transparent); animation: hs-pulse 1.8s infinite; }
.hs-dot.warn { background: var(--git-dirty); }
.hs-dot.down { background: var(--danger); }
@keyframes hs-pulse {
  0% { box-shadow: 0 0 0 0 color-mix(in oklab, var(--git-ahead) 60%, transparent); }
  70% { box-shadow: 0 0 0 7px transparent; }
  100% { box-shadow: 0 0 0 0 transparent; }
}
.hs-main { flex: 1; min-width: 0; }
.hs-name { font-size: 13px; font-weight: 600; }
.hs-detail { margin-left: 6px; font-size: 11px; font-weight: 500; color: var(--muted); }
.hs-url { font-family: var(--font-mono); font-size: 11px; color: var(--muted); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.hs-spark { display: flex; align-items: flex-end; gap: 2px; height: 26px; width: 64px; flex: none; }
.hs-spark i { flex: 1; min-width: 2px; border-radius: 1px; opacity: .85; }
.hs-meta { display: flex; flex-direction: column; align-items: flex-end; gap: 2px; flex: none; min-width: 56px; }
.hs-lat { font-family: var(--font-mono); font-size: 12px; font-weight: 600; }
.hs-code { font-family: var(--font-mono); font-size: 11px; color: var(--muted-2); }
```

- [ ] **Step 7: Проверка**
```powershell
npm run check
npm test
npm run build
```
Expected: 0 ошибок типов; Vitest 3 (3 passed); build успешен.

- [ ] **Step 8: Commit**
```powershell
git add src/lib/types.ts src/lib/api/projects.ts src/lib/api/monitor.ts src/lib/components/SettingsTab.svelte src/lib/components/Dashboard.svelte src/lib/styles/global.css
git commit -m @'
feat(frontend): live service health dashboard with status, latency sparkline, auto-refresh

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

## Task 3: Проверка

- [ ] **Step 1: Автопроверка**
```powershell
npm run check ; npm test ; cargo test --manifest-path src-tauri/Cargo.toml --lib
```
Expected: типы 0; Vitest 3; Rust 37.

- [ ] **Step 2: GUI-смоук (пользователь)**
- [ ] Настройки проекта → Health-чек (URL) → указать `https://...` (JSON-статус или главную) → сохранить.
- [ ] Дашборд: появился раздел «Статус сервисов» (т.к. ≥1 проект с URL); индикатор 🟢/🟠/🔴, код, латентность с цветом, спарклайн; «обновить» и авто-обновление каждые 30 c; клик по строке открывает URL.

---

## Итог

Health-чек: URL в настройках проекта + «живой» раздел статуса на дашборде (пульс, латентность, спарклайн, авто-рефреш, сводка). Проверки в Rust через ureq — без облака и без доп. прав.
