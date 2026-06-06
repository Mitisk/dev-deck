<script lang="ts">
  import { projects, activeProjectId } from "$lib/stores/projects";
  import Dashboard from "./Dashboard.svelte";
  import ProjectView from "./ProjectView.svelte";

  const active = $derived($projects.find((p) => p.id === $activeProjectId) ?? null);
</script>

<main class="workspace">
  {#if active}
    <ProjectView project={active} />
  {:else}
    <Dashboard />
  {/if}
</main>
