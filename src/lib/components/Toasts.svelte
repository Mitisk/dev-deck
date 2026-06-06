<script lang="ts">
  import { toasts, dismissToast } from "$lib/stores/toasts";
  import Icon from "./Icon.svelte";
</script>

<div id="toasts">
  {#each $toasts as t (t.id)}
    <div class="toast {t.kind}" role="status" onclick={() => dismissToast(t.id)}>
      <span class="ti">
        {#if t.kind === "ok"}<Icon name="check" />{:else}<Icon name="info" />{/if}
      </span>
      <div class="tx">
        {t.text}
        {#if t.sub}<small>{t.sub}</small>{/if}
      </div>
    </div>
  {/each}
</div>
