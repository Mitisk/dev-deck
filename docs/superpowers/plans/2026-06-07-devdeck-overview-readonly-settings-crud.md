# DevDeck — «Обзор read-only + CRUD в настройках проекта» — план

> **Для исполнителя:** ОБЯЗАТЕЛЬНЫЙ ПОД-НАВЫК: superpowers:subagent-driven-development. Шаги — чекбоксы.

**Цель (по прототипу `_prototype/index.html`, `overviewHTML`/`settingsHTML`):**
- Вкладка **«Обзор»** — read-only дашборд: описание; плитки **Команд** (клик = запуск; для фоновых — логи/стоп); 2 колонки: **Быстрые ссылки** (клик = открыть) | **Git-сводка** (ahead/изменено/стейдж + последний коммит); секция **Файлы и папки** (клик = открыть); блок **Ближайшие задачи** со ссылкой «Все задачи →».
- Создание/редактирование **Команд, Ссылок, Файлов** переезжает во вкладку **«Настройки»** проекта (карточки с add/edit/delete).

**Важно:** это перенос существующего CRUD из `OverviewTab.svelte` в `SettingsTab.svelte` + перестройка `OverviewTab` в read-only. Логику запуска/логов/стопа фоновых команд (`runningIds`/`logs`/`logFor`/слушатели событий `cmd-log`/`cmd-exit`/`runCmd`/`stopCmd`/`openLogs`/`appendLog`/`onMount`/`onDestroy`) ОСТАВИТЬ в `OverviewTab` (там происходит запуск). В `SettingsTab` переезжает только редактор команды (модалка) и CRUD ссылок/файлов.

**Стек/границы:** только фронт — `OverviewTab.svelte`, `SettingsTab.svelte`. Без backend, без миграций (все нужные команды/ссылки/файлы CRUD и `git_status`/`tasks_list`/`columns_list` уже есть).

**Справка по api:** `cmdsApi.{list,create,update,remove,run,runBg,stop,running}`; `linksApi.{list,create,remove}` (`create(projectId,label,url)`); `filesApi.{list,create,remove}` (`create(projectId,label,path)`); `gitApi.status(repoPath)` → `GitStatus|null` (`{branch,ahead,behind,dirty,staged,untracked,lastHash,lastMessage,lastTimestamp}`); `tasksApi.list(projectId)` → `Task[]`; `columnsApi.list(projectId)` → `TaskColumn[]`; `actions.openUrl`/`actions.openShortcut`. Навигация на задачи: `import { activeTab } from "$lib/stores/ui"; activeTab.set("tasks")`.

---

## Task 1: SettingsTab — CRUD команд/ссылок/файлов

**Files:** Modify `src/lib/components/SettingsTab.svelte`.

- [ ] **Step 1: Импорты и состояние** — в `<script>` добавить:
```ts
  import type { ProjectCommand, Link, FileShortcut } from "$lib/types";
  import * as cmdsApi from "$lib/api/commands";
  import * as linksApi from "$lib/api/links";
  import * as filesApi from "$lib/api/files";

  let cmds = $state<ProjectCommand[]>([]);
  let links = $state<Link[]>([]);
  let files = $state<FileShortcut[]>([]);

  // редактор команды
  let cmdEditing = $state<ProjectCommand | null>(null);
  let cmdNew = $state(false);
  let cLabel = $state(""); let cCommand = $state(""); let cDir = $state(""); let cRunIn = $state("terminal");
  // строки добавления
  let newLink = $state({ label: "", url: "" });
  let newFile = $state({ label: "", path: "" });

  let extrasReq = 0;
  async function loadExtras() {
    const my = ++extrasReq;
    try {
      const [c, l, f] = await Promise.all([cmdsApi.list(project.id), linksApi.list(project.id), filesApi.list(project.id)]);
      if (my === extrasReq) { cmds = c; links = l; files = f; }
    } catch { /* тост из api/client.ts */ }
  }
```
В существующий `$effect`, который пересоздаёт форму при смене проекта, добавить `loadExtras()` — либо отдельным эффектом:
```ts
  $effect(() => { project.id; loadExtras(); });
```

- [ ] **Step 2: Функции CRUD** — добавить в `<script>`:
```ts
  function openNewCmd() { cmdNew = true; cmdEditing = { id: 0, projectId: project.id, label: "", command: "", workingDir: null, runIn: "terminal", icon: "play", sortOrder: 0 }; cLabel = ""; cCommand = ""; cDir = ""; cRunIn = "terminal"; }
  function openEditCmd(c: ProjectCommand) { cmdNew = false; cmdEditing = c; cLabel = c.label; cCommand = c.command; cDir = c.workingDir ?? ""; cRunIn = c.runIn ?? "terminal"; }
  async function saveCmd() {
    if (!cmdEditing || !cLabel.trim() || !cCommand.trim()) return;
    const input = { label: cLabel.trim(), command: cCommand.trim(), workingDir: cDir.trim() || null, runIn: cRunIn, icon: "play" };
    if (cmdNew) await cmdsApi.create(project.id, input);
    else await cmdsApi.update(cmdEditing.id, input);
    cmdEditing = null; await loadExtras();
  }
  async function delCmd() { if (!cmdEditing) return; const id = cmdEditing.id; cmdEditing = null; await cmdsApi.remove(id); await loadExtras(); }

  async function addLink() { if (!newLink.label.trim() || !newLink.url.trim()) return; await linksApi.create(project.id, newLink.label.trim(), newLink.url.trim()); newLink = { label: "", url: "" }; await loadExtras(); }
  async function delLink(id: number) { await linksApi.remove(id); await loadExtras(); }
  async function addFile() { if (!newFile.label.trim() || !newFile.path.trim()) return; await filesApi.create(project.id, newFile.label.trim(), newFile.path.trim()); newFile = { label: "", path: "" }; await loadExtras(); }
  async function delFile(id: number) { await filesApi.remove(id); await loadExtras(); }
```

- [ ] **Step 3: Разметка карточек** — между карточкой «Основное» (её закрывающий `</div>` после кнопок Save/Pin) и карточкой «Опасная зона» вставить три карточки:
```svelte
  <div class="card set-card" style="margin-top:22px">
    <h3 class="section-title"><Icon name="terminal" class="ic-sm" /> Команды
      <button class="more" onclick={openNewCmd} style="margin-left:auto">+ команда</button></h3>
    {#if cmds.length}
      <div class="card links">
        {#each cmds as c (c.id)}
          <div class="link-row">
            <span class="lico"><Icon name={c.icon ?? "play"} class="ic-sm" /></span>
            <div style="flex:1;min-width:0">
              <div class="lt">{c.label} <span style="color:var(--muted-2);font-size:11px">· {c.runIn === "background" ? "фон" : "терминал"}</span></div>
              <div class="lu mono">{c.command}</div>
            </div>
            <button class="mini" title="Изменить" onclick={() => openEditCmd(c)}><Icon name="pencil" class="ic-sm" /></button>
          </div>
        {/each}
      </div>
    {:else}
      <button class="add-cred" onclick={openNewCmd}><Icon name="plus" class="ic-sm" /> Добавить команду (напр. npm run dev)</button>
    {/if}
  </div>

  <div class="card set-card" style="margin-top:22px">
    <h3 class="section-title"><Icon name="link" class="ic-sm" /> Быстрые ссылки</h3>
    <div class="card links">
      {#each links as l (l.id)}
        <div class="link-row">
          <span class="lico"><Icon name={l.icon ?? "globe"} class="ic-sm" /></span>
          <div style="flex:1;min-width:0"><div class="lt">{l.label}</div><div class="lu">{l.url}</div></div>
          <button class="mini" title="Удалить" onclick={() => delLink(l.id)}><Icon name="x" class="ic-sm" /></button>
        </div>
      {/each}
      <div class="erow" style="margin:8px 4px 4px">
        <input class="tin" placeholder="Название" bind:value={newLink.label} />
        <input class="tin url" placeholder="localhost:3000 / github.com/…" bind:value={newLink.url} onkeydown={(e) => { if (e.key === 'Enter') addLink(); }} />
        <button class="er-del" style="color:var(--accent)" title="Добавить" onclick={addLink}><Icon name="plus" class="ic-sm" /></button>
      </div>
    </div>
  </div>

  <div class="card set-card" style="margin-top:22px">
    <h3 class="section-title"><Icon name="folder" class="ic-sm" /> Файлы и папки</h3>
    <div class="card links">
      {#each files as f (f.id)}
        <div class="link-row">
          <span class="lico"><Icon name="file" class="ic-sm" /></span>
          <div style="flex:1;min-width:0"><div class="lt">{f.label}</div><div class="lu">{f.path}</div></div>
          <button class="mini" title="Удалить" onclick={() => delFile(f.id)}><Icon name="x" class="ic-sm" /></button>
        </div>
      {/each}
      <div class="erow" style="margin:8px 4px 4px">
        <input class="tin" placeholder="Название" bind:value={newFile.label} />
        <input class="tin url" placeholder="~/dev/proj/.env" bind:value={newFile.path} onkeydown={(e) => { if (e.key === 'Enter') addFile(); }} />
        <button class="er-del" style="color:var(--accent)" title="Добавить" onclick={addFile}><Icon name="plus" class="ic-sm" /></button>
      </div>
    </div>
  </div>
```

- [ ] **Step 4: Модалка редактора команды** — после `<ConfirmDialog .../>` (в конце файла) добавить:
```svelte
{#if cmdEditing}
  <div class="modal-scrim open" role="dialog" tabindex="-1" aria-label="Команда"
       onmousedown={(e) => { if (e.currentTarget === e.target) (cmdEditing = null); }}
       onkeydown={(e) => { if (e.key === 'Escape') (cmdEditing = null); }}>
    <div class="modal">
      <div class="modal-head"><span class="t">{cmdNew ? "Новая команда" : "Команда"}</span>
        <button class="icon-btn x" onclick={() => (cmdEditing = null)}><Icon name="x" class="ic" /></button></div>
      <div class="modal-body">
        <div class="field"><label for="cm-label">Ярлык</label><input id="cm-label" class="tin" placeholder="Запустить dev" bind:value={cLabel} /></div>
        <div class="field"><label for="cm-cmd">Команда (shell)</label><input id="cm-cmd" class="tin mono" placeholder="npm run dev" bind:value={cCommand} /></div>
        <div class="field"><label for="cm-dir">Рабочая папка <span style="color:var(--muted-2)">(пусто = папка проекта)</span></label>
          <input id="cm-dir" class="tin mono" placeholder={project.path ?? "~/dev/project"} bind:value={cDir} /></div>
        <div class="field"><label for="cm-mode">Режим запуска</label>
          <select id="cm-mode" class="tin" bind:value={cRunIn}>
            <option value="terminal">В терминале (новое окно)</option>
            <option value="background">В фоне (логи + стоп)</option>
          </select></div>
        <p class="desc" style="color:var(--muted-2);font-size:12px">Фоновый режим запускает процесс скрыто и стримит вывод в панель логов; «стоп» завершает дерево процессов. Запуск — во вкладке «Обзор».</p>
      </div>
      <div class="modal-foot">
        {#if !cmdNew}<button class="btn-danger" onclick={delCmd}><Icon name="trash-2" class="ic-sm" /> Удалить</button>{/if}
        <span class="spacer"></span>
        <button class="btn-ghost" onclick={() => (cmdEditing = null)}>Отмена</button>
        <button class="btn-primary" onclick={saveCmd}><Icon name="check" class="ic ic-sm" /> Сохранить</button>
      </div>
    </div>
  </div>
{/if}
```

---

## Task 2: OverviewTab — read-only дашборд

**Files:** Modify `src/lib/components/OverviewTab.svelte`.

> Переписать компонент целиком в read-only вид, СОХРАНИВ логику запуска/логов/стопа фоновых команд. Удалить весь CRUD ссылок/файлов и редактор команды.

- [ ] **Step 1: `<script>`** — заменить целиком на:
```ts
<script lang="ts">
  import type { Project, Link, FileShortcut, ProjectCommand, Task, TaskColumn, GitStatus } from "$lib/types";
  import * as linksApi from "$lib/api/links";
  import * as filesApi from "$lib/api/files";
  import * as cmdsApi from "$lib/api/commands";
  import * as tasksApi from "$lib/api/tasks";
  import * as columnsApi from "$lib/api/columns";
  import * as gitApi from "$lib/api/git";
  import * as actions from "$lib/api/actions";
  import { activeTab } from "$lib/stores/ui";
  import { onMount, onDestroy } from "svelte";
  import Icon from "./Icon.svelte";

  let { project }: { project: Project } = $props();

  const PRIORITY = [
    { label: "Низкий", color: "#5b9cff" },
    { label: "Средний", color: "#e0a83a" },
    { label: "Высокий", color: "#f0616d" },
  ];

  let links = $state<Link[]>([]);
  let files = $state<FileShortcut[]>([]);
  let cmds = $state<ProjectCommand[]>([]);
  let tasks = $state<Task[]>([]);
  let columns = $state<TaskColumn[]>([]);
  let git = $state<GitStatus | null>(null);

  // фоновые команды (запуск/логи/стоп)
  let runningIds = $state<number[]>([]);
  let logs = $state<Record<number, string[]>>({});
  let logFor = $state<number | null>(null);
  let unlisten: Array<() => void> = [];

  let reqId = 0;
  async function load() {
    const my = ++reqId;
    try {
      const repo = project.repoPath ?? project.path ?? "";
      const [l, f, c, ts, cols, g] = await Promise.all([
        linksApi.list(project.id),
        filesApi.list(project.id),
        cmdsApi.list(project.id),
        tasksApi.list(project.id),
        columnsApi.list(project.id),
        repo ? gitApi.status(repo) : Promise.resolve(null),
      ]);
      if (my === reqId) { links = l; files = f; cmds = c; tasks = ts; columns = cols; git = g; }
    } catch { /* тост из api/client.ts */ }
  }
  $effect(() => { project.id; load(); });

  // ближайшие задачи: не в «выполненных» колонках, по sortOrder, до 5
  const doneKeys = $derived(new Set(columns.filter((c) => c.isDone).map((c) => c.key)));
  const colName = (key: string) => columns.find((c) => c.key === key)?.name ?? key;
  const upcoming = $derived(
    tasks.filter((t) => !doneKeys.has(t.status)).sort((a, b) => a.sortOrder - b.sortOrder).slice(0, 5)
  );

  function fmtDue(due: string | null): string {
    if (!due) return "";
    const d = new Date(due);
    return isNaN(d.getTime()) ? due : d.toLocaleDateString("ru-RU", { day: "numeric", month: "short" });
  }

  // --- запуск команд ---
  async function runCmd(c: ProjectCommand) {
    if (c.runIn === "background") {
      logs = { ...logs, [c.id]: logs[c.id] ?? [] };
      logFor = c.id;
      try { await cmdsApi.runBg(c.id); if (!runningIds.includes(c.id)) runningIds = [...runningIds, c.id]; }
      catch { /* тост */ }
      return;
    }
    await cmdsApi.run(c.id);
  }
  async function stopCmd(id: number) { try { await cmdsApi.stop(id); } catch { /* тост */ } }
  function openLogs(id: number) { logs = { ...logs, [id]: logs[id] ?? [] }; logFor = id; }
  function appendLog(id: number, line: string) {
    const cur = logs[id] ?? [];
    const next = [...cur, line];
    if (next.length > 500) next.splice(0, next.length - 500);
    logs = { ...logs, [id]: next };
  }

  onMount(async () => {
    try { runningIds = await cmdsApi.running(); } catch { /* нет бэка */ }
    const { listen } = await import("@tauri-apps/api/event");
    unlisten.push(await listen<{ id: number; line: string; err: boolean }>("cmd-log", (e) => {
      appendLog(e.payload.id, (e.payload.err ? "[err] " : "") + e.payload.line);
    }));
    unlisten.push(await listen<{ id: number; code: number | null }>("cmd-exit", (e) => {
      runningIds = runningIds.filter((x) => x !== e.payload.id);
      appendLog(e.payload.id, `— процесс завершён (код ${e.payload.code ?? "?"}) —`);
    }));
  });
  onDestroy(() => { unlisten.forEach((u) => u()); unlisten = []; });
</script>
```

- [ ] **Step 2: Разметка (read-only)** — заменить весь шаблон (всё после `</script>`) на:
```svelte
<div class="stack">
  {#if project.description}
    <div><p class="desc">{project.description}</p></div>
  {/if}

  <div>
    <h3 class="section-title"><Icon name="terminal" class="ic-sm" /> Команды</h3>
    {#if cmds.length}
      <div class="cmd-grid">
        {#each cmds as c (c.id)}
          <button class="cmd" style="--c-tint:{project.color ?? 'var(--accent)'}" onclick={() => runCmd(c)}>
            <span class="ico"><Icon name={c.icon ?? "play"} class="ic" /></span>
            <span class="lbl">{c.label}</span>
            <span class="run mono">{c.command}</span>
            {#if c.runIn === "background"}
              <span class="play" role="button" tabindex="-1" title="Логи"
                    onclick={(e) => { e.stopPropagation(); openLogs(c.id); }}><Icon name="scroll-text" class="ic-sm" /></span>
              {#if runningIds.includes(c.id)}
                <span class="play" role="button" tabindex="-1" title="Остановить" style="color:var(--danger);right:34px"
                      onclick={(e) => { e.stopPropagation(); stopCmd(c.id); }}><Icon name="square" class="ic-sm" /></span>
              {/if}
            {/if}
          </button>
        {/each}
      </div>
    {:else}
      <p class="desc" style="color:var(--muted-2);font-size:13px">Команд нет — добавьте во вкладке «Настройки».</p>
    {/if}
  </div>

  <div class="grid-2">
    <div>
      <h3 class="section-title"><Icon name="link" class="ic-sm" /> Быстрые ссылки</h3>
      <div class="card links" style="padding:6px 14px">
        {#each links as l (l.id)}
          <div class="link-row" role="button" tabindex="0" style="cursor:pointer" onclick={() => actions.openUrl(l.url)}>
            <span class="lico"><Icon name={l.icon ?? "globe"} class="ic-sm" /></span>
            <div style="flex:1;min-width:0"><div class="lt">{l.label}</div><div class="lu">{l.url}</div></div>
            <span class="ext"><Icon name="arrow-up-right" class="ic-sm" /></span>
          </div>
        {/each}
        {#if !links.length}<div style="padding:12px;color:var(--muted-2);font-size:13px">Ссылок нет.</div>{/if}
      </div>
    </div>
    <div>
      <h3 class="section-title"><Icon name="git-branch" class="ic-sm" /> Git-сводка</h3>
      {#if git}
        <div class="gstats">
          <div class="gstat"><div class="k"><Icon name="arrow-up" class="ic-sm" /> Впереди</div><div class="v ahead">{git.ahead}</div></div>
          <div class="gstat"><div class="k"><Icon name="file-diff" class="ic-sm" /> Изменено</div><div class="v" class:dirty={git.dirty > 0}>{git.dirty}</div></div>
          <div class="gstat"><div class="k"><Icon name="git-commit-horizontal" class="ic-sm" /> Стейдж</div><div class="v">{git.staged}</div></div>
        </div>
        {#if git.lastHash}
          <div class="card" style="padding:11px 14px;margin-top:10px;font-size:12px;color:var(--muted)">
            <span class="mono" style="color:var(--text-2)">{git.lastHash.slice(0, 7)}</span> · {git.lastMessage ?? ""}
          </div>
        {/if}
      {:else}
        <div class="card" style="padding:12px 14px;color:var(--muted-2);font-size:13px">Не git-репозиторий (укажите путь в настройках).</div>
      {/if}
    </div>
  </div>

  <div>
    <h3 class="section-title"><Icon name="folder" class="ic-sm" /> Файлы и папки</h3>
    <div class="card links" style="padding:6px 14px">
      {#each files as f (f.id)}
        <div class="link-row" role="button" tabindex="0" style="cursor:pointer" onclick={() => actions.openShortcut(f.path)}>
          <span class="lico"><Icon name="file" class="ic-sm" /></span>
          <div style="flex:1;min-width:0"><div class="lt">{f.label}</div><div class="lu">{f.path}</div></div>
          <span class="ext"><Icon name="arrow-up-right" class="ic-sm" /></span>
        </div>
      {/each}
      {#if !files.length}<div style="padding:12px;color:var(--muted-2);font-size:13px">Файлов нет.</div>{/if}
    </div>
  </div>

  <div>
    <h3 class="section-title"><Icon name="list-checks" class="ic-sm" /> Ближайшие задачи
      <button class="more" onclick={() => activeTab.set("tasks")} style="margin-left:auto">Все задачи →</button></h3>
    <div class="card" style="padding:4px 16px">
      <div class="tasklist">
        {#each upcoming as t (t.id)}
          <div class="tl-row">
            <span class="pri" style="background:{PRIORITY[t.priority]?.color ?? 'var(--muted)'}"></span>
            <span class="tt">{t.title}</span>
            <span class="st">{colName(t.status)}</span>
            <span class="due">{fmtDue(t.dueDate)}</span>
          </div>
        {/each}
        {#if !upcoming.length}<div style="padding:16px;color:var(--muted)">Нет активных задач</div>{/if}
      </div>
    </div>
  </div>
</div>

{#if logFor !== null}
  <div class="modal-scrim open" role="dialog" tabindex="-1" aria-label="Логи команды"
       onmousedown={(e) => { if (e.currentTarget === e.target) (logFor = null); }}
       onkeydown={(e) => { if (e.key === 'Escape') (logFor = null); }}>
    <div class="modal" style="max-width:760px">
      <div class="modal-head">
        <span class="t">Логи: {cmds.find((c) => c.id === logFor)?.label ?? ""}</span>
        <button class="icon-btn x" onclick={() => (logFor = null)}><Icon name="x" class="ic" /></button>
      </div>
      <div class="modal-body">
        <pre class="mono" style="margin:0;max-height:50vh;overflow:auto;white-space:pre-wrap;font-size:12px;padding:8px;border-radius:8px">{(logs[logFor] ?? []).join("\n") || "— нет вывода —"}</pre>
      </div>
      <div class="modal-foot">
        {#if runningIds.includes(logFor)}
          <button class="btn-danger" onclick={() => logFor !== null && stopCmd(logFor)}><Icon name="square" class="ic-sm" /> Остановить</button>
        {/if}
        <span class="spacer"></span>
        <button class="btn-ghost" onclick={() => (logFor = null)}>Закрыть</button>
      </div>
    </div>
  </div>
{/if}
```

> Стили `.grid-2`, `.gstats`/`.gstat`, `.tasklist`/`.tl-row`/`.pri`/`.tt`/`.st`/`.due`, `.cmd`/`.cmd-grid`/`.ext`/`.links`/`.link-row` уже есть в `global.css` (взяты из прототипа). Если `npm run check`/визуальная проверка покажут отсутствие какого-то класса — оставить как есть (классы прототипа присутствуют в global.css), не изобретать новых.

- [ ] **Step 3: Проверка**
```powershell
npm run check
npm test
npm run build
```
Expected: 0 ошибок типов; Vitest 3 (3 passed); build успешен. Иконки `scroll-text`/`square`/`arrow-up-right`/`git-branch`/`file-diff`/`git-commit-horizontal`/`arrow-up`/`list-checks` — валидные lucide.

- [ ] **Step 4: Commit**
```powershell
git add src/lib/components/OverviewTab.svelte src/lib/components/SettingsTab.svelte
git commit -m @'
feat(frontend): read-only project Overview; move commands/links/files CRUD to project Settings

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

## Task 3: Проверка

- [ ] **Step 1: Автопроверка**
```powershell
npm run check ; npm test ; npm run build
```
Expected: типы 0; Vitest 3; build успешен.

- [ ] **Step 2: GUI-смоук (пользователь)**
- [ ] «Обзор» — read-only: клик по плитке команды запускает её; фоновая команда показывает логи/стоп; клик по ссылке/файлу открывает; git-сводка и ближайшие задачи отображаются; «Все задачи →» переключает на вкладку задач.
- [ ] «Настройки» проекта — добавление/редактирование/удаление команд (с выбором режима), ссылок, файлов; изменения видны в «Обзоре».

---

## Итог

«Обзор» соответствует прототипу (read-only дашборд: команды-запуск, ссылки, git-сводка, файлы, ближайшие задачи); вся правка команд/ссылок/файлов — в настройках проекта.
