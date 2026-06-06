<script lang="ts">
  import "$lib/stores/theme"; // активирует подписку темы
  import Sidebar from "$lib/components/Sidebar.svelte";
  import Workspace from "$lib/components/Workspace.svelte";
  import Toasts from "$lib/components/Toasts.svelte";
  import NewProjectModal from "$lib/components/NewProjectModal.svelte";
  import { loadProjects } from "$lib/stores/projects";
  import { onMount } from "svelte";

  onMount(() => {
    loadProjects();
  });
</script>

<div id="app">
  <Sidebar />
  <Workspace />
</div>
<NewProjectModal />
<Toasts />
