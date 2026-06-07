<script lang="ts">
  import type { Project, Note } from "$lib/types";
  import * as notesApi from "$lib/api/notes";
  import { marked } from "marked";
  import Icon from "./Icon.svelte";

  let { project }: { project: Project } = $props();

  let notes = $state<Note[]>([]);
  let activeId = $state<number | null>(null);
  let title = $state("");
  let content = $state("");
  let saveTimer: ReturnType<typeof setTimeout> | null = null;

  const active = $derived(notes.find((n) => n.id === activeId) ?? null);
  const preview = $derived(content ? (marked.parse(content) as string) : "");

  let reqId = 0;
  async function load(selectId?: number) {
    const my = ++reqId;
    try {
      const r = await notesApi.list(project.id);
      if (my !== reqId) return;
      notes = r;
      const pick = selectId ?? (r.some((n) => n.id === activeId) ? activeId : r[0]?.id ?? null);
      selectNote(pick, false);
    } catch {
      /* тост из api/client.ts */
    }
  }

  function selectNote(id: number | null, flushFirst = true) {
    if (flushFirst) flush();
    activeId = id;
    const n = notes.find((x) => x.id === id);
    title = n?.title ?? "";
    content = n?.contentMd ?? "";
  }

  // Перезагрузка при смене проекта.
  $effect(() => {
    project.id;
    load();
    return () => flush();
  });

  function scheduleSave() {
    if (saveTimer) clearTimeout(saveTimer);
    saveTimer = setTimeout(flush, 600);
  }

  function flush() {
    if (saveTimer) {
      clearTimeout(saveTimer);
      saveTimer = null;
    }
    const id = activeId;
    if (id == null) return;
    const n = notes.find((x) => x.id === id);
    if (!n) return;
    if ((n.title ?? "") === title && (n.contentMd ?? "") === content) return; // нет изменений
    n.title = title;
    n.contentMd = content;
    void notesApi.update(id, title, content);
  }

  async function newNote() {
    flush();
    const n = await notesApi.create(project.id);
    await load(n.id);
  }

  async function deleteActive() {
    const id = activeId;
    if (id == null) return;
    if (saveTimer) { clearTimeout(saveTimer); saveTimer = null; }
    activeId = null;
    await notesApi.remove(id);
    await load();
  }
</script>

<div style="display:flex;align-items:center;gap:8px;margin-bottom:12px;flex-wrap:wrap">
  {#each notes as n (n.id)}
    <button class="tab" class:active={activeId === n.id} onclick={() => selectNote(n.id)}>
      {n.title || "Без названия"}
    </button>
  {/each}
  <button class="gbtn" onclick={newNote}><Icon name="plus" class="ic-sm" /> Заметка</button>
</div>

{#if active}
  <div style="display:flex;align-items:center;gap:8px;margin-bottom:10px">
    <input
      class="tin"
      style="flex:1;font-weight:600"
      placeholder="Заголовок заметки"
      bind:value={title}
      oninput={scheduleSave}
      onblur={flush}
    />
    <button class="btn-danger" onclick={deleteActive}><Icon name="trash-2" class="ic-sm" /> Удалить</button>
  </div>
  <div class="card notes">
    <div class="note-pane src">
      <div class="pane-bar">Исходник</div>
      <textarea class="note-src" bind:value={content} oninput={scheduleSave} onblur={flush}></textarea>
    </div>
    <div class="note-pane">
      <div class="pane-bar">Превью</div>
      <div class="note-prev">{@html preview}</div>
    </div>
  </div>
{:else}
  <button class="add-cred" onclick={newNote}><Icon name="plus" class="ic-sm" /> Создать первую заметку</button>
{/if}
