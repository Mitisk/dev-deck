<script lang="ts">
  import Icon from "./Icon.svelte";

  let {
    open = false,
    title = "Подтвердите",
    message = "",
    confirmLabel = "Удалить",
    onConfirm,
    onCancel,
  }: {
    open?: boolean;
    title?: string;
    message?: string;
    confirmLabel?: string;
    onConfirm: () => void;
    onCancel: () => void;
  } = $props();
</script>

{#if open}
  <div class="modal-scrim open" role="dialog" aria-label={title}
       onmousedown={(e) => { if (e.currentTarget === e.target) onCancel(); }}
       onkeydown={(e) => { if (e.key === "Escape") onCancel(); }} tabindex="-1">
    <div class="modal" style="max-width:440px">
      <div class="modal-head">
        <span class="mh-ico" style="background:color-mix(in oklab,var(--danger) 14%,transparent);color:var(--danger)">
          <Icon name="triangle-alert" class="ic" />
        </span>
        <span class="t">{title}</span>
      </div>
      <div class="modal-body"><p class="desc">{message}</p></div>
      <div class="modal-foot">
        <span class="spacer"></span>
        <button class="btn-ghost" onclick={onCancel}>Отмена</button>
        <button class="btn-danger solid" onclick={onConfirm}>
          <Icon name="trash-2" class="ic-sm" /> {confirmLabel}
        </button>
      </div>
    </div>
  </div>
{/if}
