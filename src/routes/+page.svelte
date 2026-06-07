<script lang="ts">
  import "$lib/stores/theme"; // активирует подписку темы
  import Sidebar from "$lib/components/Sidebar.svelte";
  import Workspace from "$lib/components/Workspace.svelte";
  import Toasts from "$lib/components/Toasts.svelte";
  import NewProjectModal from "$lib/components/NewProjectModal.svelte";
  import CommandPalette from "$lib/components/CommandPalette.svelte";
  import AppSettings from "$lib/components/AppSettings.svelte";
  import { showPalette } from "$lib/stores/ui";
  import { loadProjects, activeProjectId } from "$lib/stores/projects";
  import { startWatchEvents } from "$lib/stores/changed";
  import { loadUser } from "$lib/stores/user";
  import * as watchApi from "$lib/api/watch";
  import { onMount } from "svelte";

  onMount(async () => {
    await loadUser();
    await loadProjects();
    await startWatchEvents();
    void watchApi.resync(); // пересобрать вотчер под текущие проекты
    const { listen } = await import("@tauri-apps/api/event");
    await listen<number>("tray-open-project", (e) => activeProjectId.set(e.payload));
  });
</script>

<svelte:window onkeydown={(e) => {
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
    e.preventDefault();
    showPalette.update((v) => !v);
  }
}} />

<div id="app">
  <Sidebar />
  <Workspace />
</div>
<NewProjectModal />
<CommandPalette />
<AppSettings />
<Toasts />
