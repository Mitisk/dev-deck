<script lang="ts">
  import type { Project, Checklist } from "$lib/types";
  import * as cl from "$lib/api/checklists";
  import * as templatesApi from "$lib/api/templates";
  import type { ChecklistTemplate } from "$lib/types";
  import { pushToast } from "$lib/stores/toasts";
  import Icon from "./Icon.svelte";

  let { project }: { project: Project } = $props();

  let lists = $state<Checklist[]>([]);
  let newItem = $state<Record<number, string>>({});

  let templates = $state<ChecklistTemplate[]>([]);
  let showTemplates = $state(false);

  async function loadTemplates() {
    try { templates = await templatesApi.list(); } catch { /* */ }
  }
  $effect(() => { loadTemplates(); });

  async function saveAsTemplate(c: { id: number; title: string }) {
    await templatesApi.save(c.id, c.title || "Шаблон");
    await loadTemplates();
    pushToast("Шаблон сохранён", c.title, "ok");
  }
  async function applyTemplate(t: ChecklistTemplate) {
    await templatesApi.apply(t.id, project.id);
    showTemplates = false;
    await load();
    pushToast("Шаблон применён", t.name, "ok");
  }
  async function deleteTemplate(id: number) {
    await templatesApi.remove(id);
    await loadTemplates();
  }

  let reqId = 0;
  async function load() {
    const my = ++reqId;
    try {
      const r = await cl.list(project.id);
      if (my === reqId) lists = r;
    } catch {
      /* тост из api/client.ts */
    }
  }
  $effect(() => {
    project.id;
    load();
  });

  function pct(c: Checklist): number {
    return c.items.length ? Math.round((c.items.filter((i) => i.isDone).length / c.items.length) * 100) : 0;
  }
  function doneCount(c: Checklist): number {
    return c.items.filter((i) => i.isDone).length;
  }

  async function addChecklist() {
    await cl.create(project.id, "Новый чеклист");
    await load();
  }
  async function renameChecklist(id: number, title: string) {
    if (!title.trim()) return;
    await cl.update(id, title.trim());
    // без перезагрузки: заголовок уже в локальном состоянии
  }
  async function deleteChecklist(id: number) {
    await cl.remove(id);
    await load();
  }
  async function toggle(itemId: number, isDone: boolean) {
    await cl.toggleItem(itemId, isDone);
    await load();
  }
  async function editItem(itemId: number, text: string) {
    if (!text.trim()) return;
    await cl.updateItem(itemId, text.trim());
  }
  async function deleteItem(itemId: number) {
    await cl.removeItem(itemId);
    await load();
  }
  async function addItem(checklistId: number) {
    const text = (newItem[checklistId] ?? "").trim();
    if (!text) return;
    newItem[checklistId] = "";
    await cl.addItem(checklistId, text);
    await load();
  }
</script>

<div style="display:flex;align-items:center;gap:8px;margin-bottom:12px;position:relative">
  <span style="flex:1"></span>
  <button class="gbtn" onclick={() => (showTemplates = !showTemplates)}>
    <Icon name="layout-template" class="ic-sm" /> Шаблоны{#if templates.length} ({templates.length}){/if}
  </button>
  {#if showTemplates}
    <div class="card" style="position:absolute;right:0;top:36px;z-index:50;min-width:260px;padding:8px">
      {#if templates.length}
        {#each templates as t (t.id)}
          <div class="link-row" style="padding:6px 8px">
            <div style="flex:1;cursor:pointer" role="button" tabindex="0" onclick={() => applyTemplate(t)}>
              <div class="lt">{t.name}</div>
              <div class="lu">{t.items.length} пунктов</div>
            </div>
            <button class="mini" title="Применить" onclick={() => applyTemplate(t)}><Icon name="plus" class="ic-sm" /></button>
            <button class="mini" title="Удалить" onclick={() => deleteTemplate(t.id)}><Icon name="trash-2" class="ic-sm" /></button>
          </div>
        {/each}
      {:else}
        <div class="erow-empty" style="padding:8px">Нет шаблонов. Сохраните чеклист как шаблон.</div>
      {/if}
    </div>
  {/if}
</div>

<div class="checklists">
  {#each lists as c (c.id)}
    <div class="card checklist">
      <div class="cl-head">
        <input
          class="t tin"
          style="border:0;background:none;padding:0;font-size:14px;font-weight:600"
          value={c.title}
          onchange={(e) => renameChecklist(c.id, (e.currentTarget as HTMLInputElement).value)}
        />
        <span class="pc">{doneCount(c)} / {c.items.length}</span>
        <button class="cl-list-del" title="Сохранить как шаблон" onclick={() => saveAsTemplate(c)}>
          <Icon name="bookmark" class="ic-sm" />
        </button>
        <button class="cl-list-del" title="Удалить чеклист" onclick={() => deleteChecklist(c.id)}>
          <Icon name="trash-2" class="ic-sm" />
        </button>
      </div>
      <div class="progress"><i style="width:{pct(c)}%"></i></div>
      <div class="cl-items">
        {#each c.items as it (it.id)}
          <div class="cl-item" class:done={it.isDone}>
            <button class="cbox" aria-label="переключить" onclick={() => toggle(it.id, !it.isDone)}>
              {#if it.isDone}<Icon name="check" class="ic-sm" />{/if}
            </button>
            <input
              class="lab tin"
              style="border:0;background:none;padding:0;flex:1"
              value={it.text}
              onchange={(e) => editItem(it.id, (e.currentTarget as HTMLInputElement).value)}
            />
            <button class="cl-del" title="Удалить пункт" onclick={() => deleteItem(it.id)}>
              <Icon name="x" class="ic-sm" />
            </button>
          </div>
        {/each}
      </div>
      <div class="cl-add">
        <input
          class="tin"
          placeholder="+ пункт"
          bind:value={newItem[c.id]}
          onkeydown={(e) => { if (e.key === "Enter") addItem(c.id); }}
        />
      </div>
    </div>
  {/each}

  <button class="add-checklist" onclick={addChecklist}>
    <Icon name="plus" class="ic-sm" /> Новый чеклист
  </button>
</div>
