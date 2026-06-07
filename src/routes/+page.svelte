<script lang="ts">
  import "$lib/stores/theme"; // активирует подписку темы
  import Sidebar from "$lib/components/Sidebar.svelte";
  import Workspace from "$lib/components/Workspace.svelte";
  import Toasts from "$lib/components/Toasts.svelte";
  import NewProjectModal from "$lib/components/NewProjectModal.svelte";
  import CommandPalette from "$lib/components/CommandPalette.svelte";
  import AppSettings from "$lib/components/AppSettings.svelte";
  import { showPalette } from "$lib/stores/ui";
  import { loadProjects } from "$lib/stores/projects";
  import { onMount } from "svelte";

  onMount(() => {
    loadProjects();
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
