<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import Icon from "./Icon.svelte";

  let {
    value = $bindable(""),
    id = undefined,
    placeholder = "",
    directory = true,
  }: { value?: string; id?: string; placeholder?: string; directory?: boolean } = $props();

  async function browse() {
    // ~ и относительные пути нативный диалог не понимает — стартуем с пустого
    const cur = value.trim();
    const defaultPath = /^([a-zA-Z]:[\\/]|\/)/.test(cur) ? cur : undefined;
    const sel = await open({ directory, multiple: false, defaultPath });
    if (typeof sel === "string") value = sel;
  }
</script>

<div class="path-input">
  <input {id} class="tin mono" {placeholder} bind:value autocomplete="off" />
  <button class="path-browse" type="button" title={directory ? "Выбрать папку…" : "Выбрать файл…"} onclick={browse}>
    <Icon name="folder-open" class="ic-sm" />
  </button>
</div>
