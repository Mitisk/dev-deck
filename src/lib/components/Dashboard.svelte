<script lang="ts">
  import { projects, activeProjectId, projectsLoaded } from "$lib/stores/projects";
  import { recents } from "$lib/stores/recents";
  import { showNewProject } from "$lib/stores/ui";
  import { displayName } from "$lib/stores/user";
  import { statusLabel } from "$lib/format";
  import * as dash from "$lib/api/dashboard";
  import * as actions from "$lib/api/actions";
  import * as git from "$lib/api/git";
  import * as monitor from "$lib/api/monitor";
  import type { HealthResult } from "$lib/api/monitor";
  import { onMount, onDestroy } from "svelte";
  import { pushToast } from "$lib/stores/toasts";
  import type { AttentionItem, AgendaItem } from "$lib/types";
  import Icon from "./Icon.svelte";
  import ProjectIcon from "./ProjectIcon.svelte";

  const visible = $derived($projects.filter((p) => p.status !== "archived"));
  const pinned = $derived(visible.filter((p) => p.pinned));
  const recentProjects = $derived(
    $recents.map((id) => visible.find((p) => p.id === id)).filter((p): p is NonNullable<typeof p> => !!p).slice(0, 6),
  );

  let attention = $state<AttentionItem[]>([]);
  let agenda = $state<AgendaItem[]>([]);
  function todayStr(): string {
    const d = new Date();
    return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
  }
  function fmtDue(due: string): string {
    const d = new Date(due + "T00:00:00");
    return isNaN(d.getTime()) ? due : d.toLocaleDateString("ru-RU", { day: "numeric", month: "short" });
  }
  function overdue(due: string): boolean { return /^\d{4}-\d{2}-\d{2}$/.test(due) && due < todayStr(); }
  async function loadAttention() {
    try {
      attention = await dash.attention();
    } catch {
      /* тост из api/client.ts */
    }
  }
  async function loadAgenda() {
    try { agenda = await dash.agenda(todayStr()); } catch { /* */ }
  }
  $effect(() => {
    // перезагружать при изменении набора проектов
    void $projects.length;
    loadAttention();
    loadAgenda();
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
    if (!r.reachable) return "down";
    if (!r.ok) return "warn";
    // отвечает 2xx, но какая-то зависимость упала → деградация
    if (r.components?.some((c) => c.state === "down")) return "warn";
    return "up";
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
</script>

<div class="ws-inner dash">
  <div style="margin-bottom:22px">
    <div class="dash-hello">Привет, <span>{$displayName}</span></div>
    <div class="dash-sub">{visible.length} {visible.length === 1 ? "проект" : "проектов"} · {attention.length} требуют внимания</div>
  </div>

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
              {#if r?.components?.length}
                <div class="hs-comps">
                  {#each r.components as c}
                    <span class="hs-comp {c.state}" title="{c.name}: {c.label}">{c.name}</span>
                  {/each}
                </div>
              {:else}
                <div class="hs-url">{p.healthUrl}</div>
              {/if}
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
              <span class="be"><ProjectIcon icon={p.icon} size={26} /></span>
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
              <span class="be"><ProjectIcon icon={p.icon} size={26} /></span>
              <div style="min-width:0"><h3>{p.name}</h3><div class="pmeta">{p.path ?? statusLabel(p.status)}</div></div>
            </div>
          </div>
        {/each}
      </div>
    {/if}

    {#if agenda.length}
      <div style="margin-top:26px">
        <h3 class="section-title"><Icon name="calendar-clock" class="ic-sm" /> Задачи на сегодня и просроченные</h3>
        <div class="card att-card">
          {#each agenda as a (a.taskId)}
            <div class="att-row" role="button" tabindex="0" onclick={() => open(a.projectId)}>
              <span class="att-be" style="--p-color:{a.projectColor ?? 'var(--accent)'}"><Icon name="square-check-big" class="ic-sm" /></span>
              <div class="att-main">
                <div class="att-line">
                  <span class="att-name">{a.title}</span>
                  <span class="att-branch">{a.projectName}</span>
                </div>
              </div>
              <span class="att-tag {overdue(a.dueDate) ? 'dirty' : 'ahead'}">{fmtDue(a.dueDate)}</span>
            </div>
          {/each}
        </div>
      </div>
    {/if}

    <div style="margin-top:30px">
      <h3 class="section-title"><Icon name="git-pull-request-arrow" class="ic-sm" /> Требуют внимания</h3>
      <div class="card att-card">
        {#each attention as a (a.projectId)}
          <div class="att-row" role="button" tabindex="0" onclick={() => open(a.projectId)}>
            <span class="att-be" style="--p-color:{a.color ?? 'var(--accent)'}"><ProjectIcon icon={a.icon} size={22} /></span>
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
