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
  import { features } from "$lib/stores/features";
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
  async function load(withTasks: boolean) {
    const my = ++reqId;
    try {
      const repo = project.repoPath ?? project.path ?? "";
      const [l, f, c, ts, cols, g] = await Promise.all([
        linksApi.list(project.id),
        filesApi.list(project.id),
        cmdsApi.list(project.id),
        // задачи выключены в настройках → секция скрыта, данные не грузим
        withTasks ? tasksApi.list(project.id) : Promise.resolve([] as Task[]),
        withTasks ? columnsApi.list(project.id) : Promise.resolve([] as TaskColumn[]),
        repo ? gitApi.status(repo) : Promise.resolve(null),
      ]);
      if (my === reqId) { links = l; files = f; cmds = c; tasks = ts; columns = cols; git = g; }
    } catch { /* тост из api/client.ts */ }
  }
  $effect(() => { project.id; load($features.tasks); });

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

<div class="stack">
  {#if project.description}
    <div><p class="desc">{project.description}</p></div>
  {/if}

  {#if cmds.length}
    <div>
      <h3 class="section-title"><Icon name="terminal" class="ic-sm" /> Команды</h3>
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
    </div>
  {/if}

  {#snippet linksBlock()}
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
      </div>
    </div>
  {/snippet}

  {#snippet gitBlock()}
    <div>
      <h3 class="section-title"><Icon name="git-branch" class="ic-sm" /> Git-сводка</h3>
      <div class="gstats">
        <div class="gstat"><div class="k"><Icon name="arrow-up" class="ic-sm" /> Впереди</div><div class="v ahead">{git?.ahead ?? 0}</div></div>
        <div class="gstat"><div class="k"><Icon name="file-diff" class="ic-sm" /> Изменено</div><div class="v" class:dirty={(git?.dirty ?? 0) > 0}>{git?.dirty ?? 0}</div></div>
        <div class="gstat"><div class="k"><Icon name="git-commit-horizontal" class="ic-sm" /> Стейдж</div><div class="v">{git?.staged ?? 0}</div></div>
      </div>
      {#if git?.lastHash}
        <div class="card" style="padding:11px 14px;margin-top:10px;font-size:12px;color:var(--muted)">
          <span class="mono" style="color:var(--text-2)">{git.lastHash.slice(0, 7)}</span> · {git.lastMessage ?? ""}
        </div>
      {/if}
    </div>
  {/snippet}

  {#if links.length && git}
    <div class="grid-2">
      {@render linksBlock()}
      {@render gitBlock()}
    </div>
  {:else if links.length}
    {@render linksBlock()}
  {:else if git}
    {@render gitBlock()}
  {/if}

  {#if files.length}
    <div>
      <h3 class="section-title"><Icon name="folder" class="ic-sm" /> Файлы и папки</h3>
      <div class="card links" style="padding:6px 14px">
        {#each files as f (f.id)}
          <div class="link-row" role="button" tabindex="0" style="cursor:pointer" onclick={() => actions.openShortcut(f.path)}>
            <span class="lico"><Icon name="file" class="ic-sm" /></span>
            <div style="flex:1;min-width:0"><div class="lt">{f.label}</div><div class="lu">{f.path}</div></div>
            {#if f.showTerminal}
              <button class="lrow-act" title="Открыть консоль (Git Bash) здесь" onclick={(e) => { e.stopPropagation(); actions.openGitBash(f.path); }}>
                <Icon name="square-terminal" class="ic-sm" />
              </button>
            {/if}
            <span class="ext"><Icon name="arrow-up-right" class="ic-sm" /></span>
          </div>
        {/each}
      </div>
    </div>
  {/if}

  {#if $features.tasks}
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
  {/if}
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
