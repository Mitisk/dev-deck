<script lang="ts">
  import type { GitStatus, GitOpResult } from "$lib/types";
  import * as git from "$lib/api/git";
  import { pushToast } from "$lib/stores/toasts";
  import Icon from "./Icon.svelte";

  let { repoPath }: { repoPath: string | null } = $props();

  let st = $state<GitStatus | null>(null);
  let loaded = $state(false);
  let busy = $state<string | null>(null); // имя текущей операции
  let message = $state("");

  let reqId = 0;
  async function load() {
    if (!repoPath) {
      st = null;
      loaded = true;
      return;
    }
    const my = ++reqId;
    try {
      const s = await git.status(repoPath);
      if (my === reqId) st = s;
    } catch {
      // тост ошибки показывает api/client.ts
    } finally {
      if (my === reqId) loaded = true;
    }
  }

  // Перезагрузка при смене пути.
  $effect(() => {
    repoPath;
    loaded = false;
    st = null;
    load();
  });

  async function op(name: string, fn: () => Promise<GitOpResult>, okMsg: string) {
    if (!repoPath || busy) return;
    busy = name;
    try {
      const r = await fn();
      const tail = r.output.split("\n").filter(Boolean).slice(-2).join(" · ");
      pushToast(r.ok ? okMsg : "Git: ошибка", tail || (r.ok ? "" : "см. вывод git"), r.ok ? "ok" : "error");
      if (r.ok) await load();
    } catch {
      // spawn-ошибка («git не найден») — тост из api/client.ts
    } finally {
      busy = null;
    }
  }

  function commit() {
    const m = message.trim();
    if (!m || !repoPath) return;
    op("commit", () => git.commitAll(repoPath!, m), "Коммит создан").then(() => {
      message = "";
    });
  }

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

    <div class="git-actions">
      <div class="commit-field">
        <input placeholder="Сообщение коммита" bind:value={message} onkeydown={(e) => { if (e.key === 'Enter') commit(); }} />
        <button disabled={!message.trim() || !!busy} onclick={commit}>{busy === "commit" ? "…" : "Commit all"}</button>
      </div>
      <button class="gbtn" disabled={!!busy} onclick={() => op("fetch", () => git.fetch(repoPath!), "Fetch выполнен")}>
        <Icon name="refresh-cw" class="ic-sm" /> {busy === "fetch" ? "…" : "Fetch"}
      </button>
      <button class="gbtn" disabled={!!busy} onclick={() => op("pull", () => git.pull(repoPath!), "Pull выполнен")}>
        <Icon name="arrow-down" class="ic-sm" /> {busy === "pull" ? "…" : "Pull"}
      </button>
      <button class="gbtn primary" disabled={!!busy} onclick={() => op("push", () => git.push(repoPath!), "Push выполнен")}>
        <Icon name="arrow-up" class="ic-sm" /> {busy === "push" ? "…" : "Push"}
      </button>
    </div>
  </div>
{:else if loaded && repoPath}
  <div class="git-bar">
    <span class="last-commit" style="color:var(--muted-2)"><Icon name="git-branch" class="ic-sm" /> Не git-репозиторий</span>
  </div>
{/if}
