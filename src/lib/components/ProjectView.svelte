<script lang="ts">
  import type { Project } from "$lib/types";
  import { activeTab, type Tab } from "$lib/stores/ui";
  import { pushToast } from "$lib/stores/toasts";
  import Icon from "./Icon.svelte";

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

<div class="ws-inner" style="--p-color:{project.color}">
  <div class="proj-head">
    <span class="big-emoji">{project.emoji}</span>
    <div>
      <h1>{project.name}</h1>
      <div class="sub">
        <span class="pill">{project.status}</span>
        <span class="path mono">{project.path}</span>
      </div>
    </div>
    <div class="head-actions">
      <button class="act" onclick={() => pushToast("Открываю папку", project.path, "info")}>
        <Icon name="folder-open" class="ic-sm" /> Папка
      </button>
    </div>
  </div>

  <div class="tabs">
    {#each tabs as t}
      <button class="tab" class:active={$activeTab === t.key} onclick={() => activeTab.set(t.key)}>
        {t.label}
      </button>
    {/each}
  </div>

  <div class="tab-body">
    <p class="desc">{project.desc}</p>
    <p class="desc" style="color:var(--muted);margin-top:14px">
      Вкладка «{activeLabel}» — содержимое появится в срезах Phase 1.
    </p>
  </div>
</div>
