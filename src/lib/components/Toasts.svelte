<script lang="ts">
  import { toasts, dismissToast } from "$lib/stores/toasts";
  import { ico, paintIcons } from "$lib/icons";

  // Перерисовать иконки после изменения списка тостов.
  $effect(() => {
    $toasts; // зависимость: перезапуск эффекта при изменении списка
    paintIcons();
  });
</script>

<div id="toasts">
  {#each $toasts as t (t.id)}
    <div class="toast {t.kind}" role="status" onclick={() => dismissToast(t.id)}>
      <span class="ti">
        {#if t.kind === "ok"}{@html ico("check")}{:else}{@html ico("info")}{/if}
      </span>
      <div class="tx">
        {t.text}
        {#if t.sub}<small>{t.sub}</small>{/if}
      </div>
    </div>
  {/each}
</div>
