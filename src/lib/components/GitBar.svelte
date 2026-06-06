<script lang="ts">
  import type { GitStatus } from "$lib/types";
  import * as git from "$lib/api/git";
  import Icon from "./Icon.svelte";

  let { repoPath }: { repoPath: string | null } = $props();

  let st = $state<GitStatus | null>(null);
  let loaded = $state(false);

  // Перезагружать статус при смене пути.
  $effect(() => {
    const p = repoPath;
    loaded = false;
    st = null;
    if (!p) {
      loaded = true;
      return;
    }
    let cancelled = false;
    git
      .status(p)
      .then((s) => {
        if (!cancelled) {
          st = s;
          loaded = true;
        }
      })
      .catch(() => {
        if (!cancelled) loaded = true;
      });
    return () => {
      cancelled = true;
    };
  });

  function fmtDate(ts: number | null): string {
    if (!ts) return "";
    return new Date(ts * 1000).toLocaleDateString("ru-RU", { day: "numeric", month: "short" });
  }
</script>

{#if loaded && st}
  <div class="git-bar">
    <div class="seg">
      <span class="branch"><Icon name="git-branch" class="ic-sm" /> {st.branch ?? "—"}</span>
      {#if st.ahead > 0 || st.behind > 0}
        <span class="aheadbehind">
          {#if st.ahead > 0}<span class="a">↑{st.ahead}</span>{/if}
          {#if st.behind > 0}<span class="b">↓{st.behind}</span>{/if}
        </span>
      {/if}
    </div>
    <div class="git-sep"></div>
    {#if st.dirty > 0}
      <span class="dirty-count"><span class="led"></span>{st.dirty} изм.{#if st.staged > 0} · {st.staged} в индексе{/if}</span>
    {:else}
      <span class="last-commit"><Icon name="check" class="ic-sm" /> чисто</span>
    {/if}
    {#if st.lastHash}
      <div class="git-sep"></div>
      <span class="last-commit">
        <span class="hash mono">{st.lastHash}</span>
        <span class="msg">{st.lastMessage ?? ""}</span>
        {#if st.lastTimestamp}<span style="color:var(--muted-2)">· {fmtDate(st.lastTimestamp)}</span>{/if}
      </span>
    {/if}
  </div>
{:else if loaded && repoPath}
  <div class="git-bar">
    <span class="last-commit" style="color:var(--muted-2)"><Icon name="git-branch" class="ic-sm" /> Не git-репозиторий</span>
  </div>
{/if}
