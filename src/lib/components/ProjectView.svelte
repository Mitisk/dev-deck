<script lang="ts">
  import type { Project } from "$lib/types";
  import { activeTab, type Tab } from "$lib/stores/ui";
  import { features } from "$lib/stores/features";
  import { statusLabel } from "$lib/format";
  import * as actions from "$lib/api/actions";
  import Icon from "./Icon.svelte";
  import SettingsTab from "./SettingsTab.svelte";
  import TasksTab from "./TasksTab.svelte";
  import ChecklistsTab from "./ChecklistsTab.svelte";
  import CredsTab from "./CredsTab.svelte";
  import NotesTab from "./NotesTab.svelte";
  import OverviewTab from "./OverviewTab.svelte";
  import GitBar from "./GitBar.svelte";
  import ProjectIcon from "./ProjectIcon.svelte";

  let { project }: { project: Project } = $props();

  function run(action: () => Promise<void>) {
    // Ошибки показывает api/client.ts тостом; здесь просто запускаем.
    void action();
  }

  const allTabs: { key: Tab; label: string; feature?: keyof typeof $features }[] = [
    { key: "overview", label: "Обзор" },
    { key: "tasks", label: "Задачи", feature: "tasks" },
    { key: "checklists", label: "Чеклисты", feature: "checklists" },
    { key: "creds", label: "Креды", feature: "creds" },
    { key: "notes", label: "Заметки" },
    { key: "settings", label: "Настройки" },
  ];

  const tabs = $derived(allTabs.filter((t) => !t.feature || $features[t.feature]));
  const activeLabel = $derived(tabs.find((x) => x.key === $activeTab)?.label ?? "");

  // Если активная вкладка скрыта выключенной опцией — вернуться на «Обзор».
  $effect(() => {
    if (!tabs.some((t) => t.key === $activeTab)) activeTab.set("overview");
  });
</script>

<div class="ws-inner" style="--p-color:{project.color ?? 'var(--accent)'}">
  <div class="proj-head">
    <span class="big-emoji"><ProjectIcon icon={project.icon} size={34} /></span>
    <div>
      <h1>{project.name}</h1>
      <div class="sub">
        <span class="pill">{statusLabel(project.status)}</span>
        {#if project.path}<span class="path mono">{project.path}</span>{/if}
        {#if project.tags.length}
          <span class="chips">{#each project.tags as t}<span class="chip">{t}</span>{/each}</span>
        {/if}
      </div>
    </div>
    <div class="head-actions">
      <button class="act sq" title="Открыть папку"
              disabled={!project.path}
              onclick={() => project.path && run(() => actions.openPath(project.path!))}>
        <Icon name="folder-open" class="ic" />
      </button>
      <button class="act sq" title="Открыть в редакторе"
              disabled={!project.path}
              onclick={() => project.path && run(() => actions.openInEditor(project.path!))}>
        <Icon name="code-xml" class="ic" />
      </button>
      <button class="act sq" title="Открыть терминал здесь"
              disabled={!project.path}
              onclick={() => project.path && run(() => actions.openTerminal(project.path!))}>
        <Icon name="square-terminal" class="ic" />
      </button>
    </div>
  </div>

  <GitBar repoPath={project.repoPath ?? project.path} />

  <div class="tabs">
    {#each tabs as t}
      <button class="tab" class:active={$activeTab === t.key} onclick={() => activeTab.set(t.key)}>{t.label}</button>
    {/each}
  </div>

  <div class="tab-body">
    {#if $activeTab === "settings"}
      <SettingsTab {project} />
    {:else if $activeTab === "tasks"}
      <TasksTab {project} />
    {:else if $activeTab === "checklists"}
      <ChecklistsTab {project} />
    {:else if $activeTab === "creds"}
      <CredsTab {project} />
    {:else if $activeTab === "notes"}
      <NotesTab {project} />
    {:else if $activeTab === "overview"}
      <OverviewTab {project} />
    {:else}
      {#if project.description}<p class="desc">{project.description}</p>{/if}
      <p class="desc" style="color:var(--muted);margin-top:14px">
        Вкладка «{activeLabel}» — содержимое появится в следующих срезах Phase 1.
      </p>
    {/if}
  </div>
</div>
