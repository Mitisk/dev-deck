<script lang="ts">
  import type { SearchHit } from "$lib/types";
  import { projects, activeProjectId } from "$lib/stores/projects";
  import { activeTab, showPalette, type Tab } from "$lib/stores/ui";
  import * as search from "$lib/api/search";
  import Icon from "./Icon.svelte";

  let query = $state("");
  let hits = $state<SearchHit[]>([]);
  let sel = $state(0);
  let inputEl = $state<HTMLInputElement | null>(null);
  let timer: ReturnType<typeof setTimeout> | null = null;
  let runId = 0;

  // Пустой запрос → проекты из стора; иначе — бэкенд-поиск.
  const projectHits = $derived(
    $projects
      .filter((p) => p.status !== "archived")
      .map<SearchHit>((p) => ({ kind: "project", projectId: p.id, projectName: p.name, id: p.id, title: p.name, subtitle: "Проект" })),
  );
  const items = $derived(query.trim() ? hits : projectHits);

  // Поиск с debounce при вводе.
  $effect(() => {
    const q = query;
    if (timer) clearTimeout(timer);
    if (!q.trim()) {
      hits = [];
      sel = 0;
      return;
    }
    timer = setTimeout(async () => {
      const my = ++runId;
      try {
        const r = await search.global(q);
        if (my === runId) {
          hits = r;
          sel = 0;
        }
      } catch {
        /* тост из api/client.ts */
      }
    }, 180);
  });

  // При открытии — очистить и сфокусировать.
  $effect(() => {
    if ($showPalette) {
      query = "";
      hits = [];
      sel = 0;
      setTimeout(() => inputEl?.focus(), 10);
    }
  });

  const ICONS: Record<string, string> = {
    project: "box", task: "square-check-big", note: "file-text",
    link: "link", cred: "key-round", command: "terminal", file: "file",
  };
  function tabFor(kind: string): Tab {
    if (kind === "task") return "tasks";
    if (kind === "note") return "notes";
    if (kind === "cred") return "creds";
    return "overview"; // link | command | file | project
  }

  function activate(h: SearchHit | undefined) {
    if (!h) return;
    showPalette.set(false);
    activeProjectId.set(h.projectId);
    if (h.kind !== "project") activeTab.set(tabFor(h.kind));
    else activeTab.set("overview");
  }

  function onKey(e: KeyboardEvent) {
    if (!$showPalette) return;
    if (e.key === "Escape") { e.preventDefault(); showPalette.set(false); }
    else if (e.key === "ArrowDown") { e.preventDefault(); sel = Math.min(sel + 1, items.length - 1); }
    else if (e.key === "ArrowUp") { e.preventDefault(); sel = Math.max(sel - 1, 0); }
    else if (e.key === "Enter") { e.preventDefault(); activate(items[sel]); }
  }
</script>

<svelte:window onkeydown={onKey} />

{#if $showPalette}
  <div class="palette-scrim open" role="presentation" onmousedown={(e) => { if (e.currentTarget === e.target) showPalette.set(false); }}>
    <div class="palette" role="dialog" aria-label="Командный палет">
      <div class="pal-input">
        <Icon name="search" class="ic ic-lg" />
        <input bind:this={inputEl} bind:value={query} placeholder="Поиск проектов и содержимого…" autocomplete="off" />
        <span class="kbd">Esc</span>
      </div>
      <div class="pal-list">
        {#if items.length}
          {#each items as h, i (h.kind + "-" + h.id + "-" + i)}
            <div class="pal-item" class:sel={i === sel}
                 role="button" tabindex="-1"
                 onmousemove={() => (sel = i)} onclick={() => activate(h)}>
              <span class="pi-ico"><Icon name={ICONS[h.kind] ?? "box"} class="ic-sm" /></span>
              <span class="pt">{h.title}<small>{h.subtitle}{h.kind !== "project" ? ` · ${h.projectName}` : ""}</small></span>
              {#if i === sel}<span class="pk"><span class="kbd">↵</span></span>{/if}
            </div>
          {/each}
        {:else}
          <div class="pal-empty">{query.trim() ? "Ничего не найдено" : "Нет проектов"}</div>
        {/if}
      </div>
      <div class="pal-foot">
        <span class="k"><span class="kbd">↑</span><span class="kbd">↓</span> навигация</span>
        <span class="k"><span class="kbd">↵</span> выбрать</span>
        <span class="k"><span class="kbd">Esc</span> закрыть</span>
      </div>
    </div>
  </div>
{/if}
