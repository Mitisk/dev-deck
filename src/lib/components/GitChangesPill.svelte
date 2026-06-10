<script lang="ts">
  import type { GitChanges } from "$lib/types";
  import * as git from "$lib/api/git";
  import * as actions from "$lib/api/actions";
  import { browser } from "$app/environment";
  import { changeCategories } from "$lib/gitChanges";
  import Icon from "./Icon.svelte";

  let { repoPath, dirty, staged, untracked }:
    { repoPath: string; dirty: number; staged: number; untracked: number } = $props();

  const cats = $derived(changeCategories(dirty, staged, untracked));

  let open = $state(false);
  let data = $state<GitChanges | null>(null);
  let loading = $state(false);
  let error = $state(false);
  let loadedFor = $state(""); // ключ кэша: repoPath + dirty + staged

  let loadReqId = 0;
  async function loadChanges() {
    // staged входит в ключ: при изменении распределения staged/unstaged
    // (тот же total dirty) список файлов всё равно перезагрузится.
    const key = `${repoPath}:${dirty}:${staged}`;
    if (loadedFor === key && data) return; // кэш свеж
    const my = ++loadReqId;
    loading = true;
    error = false;
    try {
      const result = await git.changes(repoPath);
      if (my !== loadReqId) return; // пришёл более новый запрос — игнорируем
      data = result;
      loadedFor = key;
    } catch {
      if (my === loadReqId) error = true; // тост ошибки уже из client.ts
    } finally {
      if (my === loadReqId) loading = false;
    }
  }

  function toggle() {
    open = !open;
    if (open) void loadChanges();
  }

  function openFile(path: string) {
    // git отдаёт относительный путь через "/"; repoPath на Windows — через "\".
    // Соединяем доминирующим разделителем, чтобы не плодить смешанный путь.
    const base = repoPath.replace(/[\\/]+$/, "");
    const sep = base.includes("\\") ? "\\" : "/";
    const full = base + sep + path.replace(/\//g, sep);
    void actions.openFileInEditor(full);
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape") open = false;
  }
  function onDocMouseDown(e: MouseEvent) {
    const t = e.target as HTMLElement | null;
    if (!t?.closest?.(".gc-wrap")) open = false;
  }
  $effect(() => {
    if (!browser || !open) return;
    window.addEventListener("keydown", onKey);
    window.addEventListener("mousedown", onDocMouseDown);
    return () => {
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("mousedown", onDocMouseDown);
    };
  });

  function splitPath(p: string): { dir: string; name: string } {
    const i = Math.max(p.lastIndexOf("/"), p.lastIndexOf("\\"));
    return i >= 0 ? { dir: p.slice(0, i + 1), name: p.slice(i + 1) } : { dir: "", name: p };
  }
  function codeClass(code: string): string {
    return code === "?" ? "c-new" : `c-${code.toLowerCase()}`;
  }
</script>

<div class="gc-wrap">
  <button class="gc-pill" class:open onclick={toggle} aria-expanded={open} title="Изменения">
    {#if cats.modified > 0}<span class="gc-cat mod">● {cats.modified}</span>{/if}
    {#if cats.untracked > 0}<span class="gc-cat unt">＋ {cats.untracked}</span>{/if}
    {#if cats.staged > 0}<span class="gc-cat stg">✓ {cats.staged}</span>{/if}
    <Icon name="chevron-down" class="ic-sm gc-caret" />
  </button>

  {#if open}
    <div class="gc-pop" role="dialog" aria-label="Изменённые файлы">
      {#if loading}
        <div class="gc-msg">Загрузка…</div>
      {:else if error}
        <div class="gc-msg">Не удалось прочитать изменения</div>
      {:else if data}
        <div class="gc-stat">
          <span class="ins">+{data.insertions}</span>
          <span class="del">−{data.deletions}</span>
        </div>
        {#if data.files.length}
          <div class="gc-list">
            {#each data.files as f (f.path)}
              {@const sp = splitPath(f.path)}
              <button class="gc-file" onclick={() => openFile(f.path)} title={f.path}>
                <span class="gc-code {codeClass(f.code)}">{f.code}</span>
                <span class="gc-path mono"><bdi><span class="dir">{sp.dir}</span>{sp.name}</bdi></span>
                {#if f.staged}<span class="gc-staged">индекс</span>{/if}
              </button>
            {/each}
          </div>
        {:else}
          <div class="gc-msg">Нет изменённых файлов</div>
        {/if}
      {/if}
    </div>
  {/if}
</div>
