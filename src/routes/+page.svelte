<script lang="ts">
  import "$lib/stores/theme"; // активирует подписку темы
  import Sidebar from "$lib/components/Sidebar.svelte";
  import Workspace from "$lib/components/Workspace.svelte";
  import Toasts from "$lib/components/Toasts.svelte";
  import { onMount } from "svelte";
  import { paintIcons } from "$lib/icons";

  onMount(() => paintIcons());
</script>

<div id="app">
  <Sidebar />
  <Workspace />
</div>
<Toasts />
