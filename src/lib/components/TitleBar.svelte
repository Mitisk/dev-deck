<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { onMount } from "svelte";
  import Icon from "./Icon.svelte";

  const win = getCurrentWindow();
  let maximized = $state(false);

  async function refresh() { try { maximized = await win.isMaximized(); } catch { /* */ } }

  onMount(() => {
    refresh();
    let un: (() => void) | undefined;
    win.onResized(() => refresh()).then((u) => (un = u)).catch(() => {});
    return () => un?.();
  });
</script>

<div class="titlebar" data-tauri-drag-region>
  <div class="tb-drag" data-tauri-drag-region></div>
  <div class="tb-controls">
    <button class="tb-btn" title="Свернуть" onclick={() => win.minimize()}><Icon name="minus" class="ic-sm" /></button>
    <button class="tb-btn" title={maximized ? "Восстановить" : "Развернуть"} onclick={() => win.toggleMaximize()}>
      <Icon name={maximized ? "copy" : "square"} class="ic-sm" />
    </button>
    <button class="tb-btn close" title="Закрыть" onclick={() => win.close()}><Icon name="x" class="ic-sm" /></button>
  </div>
</div>
