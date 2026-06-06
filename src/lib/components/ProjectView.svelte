<script lang="ts">
  import type { Project } from "$lib/types";
  import { activeTab, type Tab } from "$lib/stores/ui";
  import { pushToast } from "$lib/stores/toasts";
  import { statusLabel } from "$lib/format";
  import Icon from "./Icon.svelte";
  import SettingsTab from "./SettingsTab.svelte";

  let { project }: { project: Project } = $props();

  const tabs: { key: Tab; label: string }[] = [
    { key: "overview", label: "Обзор" },
    { key: "tasks", label: "Задачи" },
    { key: "checklists", label: "Чеклисты" },
    { key: "creds", label: "Креды" },
    { key: "notes", label: "Заметки" },
    { key: "settings", label: "Настройки" },
  ];

  const activeLabel = $derived(tabs.find((x) => x.key === $activeTab)?.label ?? "");
</script>

<div class="ws-inner" style="--p-color:{project.color ?? 'var(--accent)'}">
  <div class="proj-head">
    <span class="big-emoji">{project.icon ?? "📁"}</span>
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
      <button class="act" onclick={() => pushToast("Открываю папку", project.path ?? "путь не задан", "info")}>
        <Icon name="folder-open" class="ic-sm" /> Папка
      </button>
    </div>
  </div>

  <div class="tabs">
    {#each tabs as t}
      <button class="tab" class:active={$activeTab === t.key} onclick={() => activeTab.set(t.key)}>{t.label}</button>
    {/each}
  </div>

  <div class="tab-body">
    {#if $activeTab === "settings"}
      <SettingsTab {project} />
    {:else}
      {#if project.description}<p class="desc">{project.description}</p>{/if}
      <p class="desc" style="color:var(--muted);margin-top:14px">
        Вкладка «{activeLabel}» — содержимое появится в следующих срезах Phase 1.
      </p>
    {/if}
  </div>
</div>
