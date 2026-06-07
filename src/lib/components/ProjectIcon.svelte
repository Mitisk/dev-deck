<script lang="ts">
  import { isImageIcon, iconSrc } from "$lib/icon";

  let {
    icon = null,
    size = 18,
    fallback = "📁",
  }: { icon?: string | null; size?: number; fallback?: string } = $props();
</script>

{#if icon && isImageIcon(icon)}
  <img class="pi-img" src={iconSrc(icon)} alt="" style="width:{size}px;height:{size}px" />
{:else}{icon || fallback}{/if}
