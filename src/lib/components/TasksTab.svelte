<script lang="ts">
  import type { Project, Task, TaskColumn } from "$lib/types";
  import * as tasksApi from "$lib/api/tasks";
  import * as columnsApi from "$lib/api/columns";
  import { pushToast } from "$lib/stores/toasts";
  import Icon from "./Icon.svelte";

  let { project }: { project: Project } = $props();

  const PRIORITY = [
    { label: "Низкий", color: "#5b9cff" },
    { label: "Средний", color: "#e0a83a" },
    { label: "Высокий", color: "#f0616d" },
  ];

  let columns = $state<TaskColumn[]>([]);
  let tasks = $state<Task[]>([]);
  let adding = $state<Record<string, string>>({});
  let dragId = $state<number | null>(null);
  let dragging = $state(false);
  let overCol = $state<string | null>(null);

  // edit task dialog
  let editing = $state<Task | null>(null);
  let eTitle = $state("");
  let eDesc = $state("");
  let ePriority = $state(0);
  let eDue = $state("");
  let eStatus = $state("");

  // column dialog
  let colEditing = $state<TaskColumn | null>(null);
  let colNew = $state(false);
  let cName = $state("");
  let cDone = $state(false);

  let reqId = 0;
  async function load() {
    const my = ++reqId;
    try {
      const [cols, ts] = await Promise.all([columnsApi.list(project.id), tasksApi.list(project.id)]);
      if (my === reqId) { columns = cols; tasks = ts; }
    } catch { /* тост из api/client.ts */ }
  }
  $effect(() => { project.id; load(); });

  function colTasks(key: string): Task[] {
    return tasks.filter((t) => t.status === key).sort((a, b) => a.sortOrder - b.sortOrder);
  }

  function todayStr(): string {
    const d = new Date();
    return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
  }
  function fmtDue(due: string | null): string {
    if (!due) return "";
    const d = new Date(due + "T00:00:00");
    return isNaN(d.getTime()) ? due : d.toLocaleDateString("ru-RU", { day: "numeric", month: "short" });
  }
  function isOverdue(due: string | null): boolean {
    return !!due && /^\d{4}-\d{2}-\d{2}$/.test(due) && due < todayStr();
  }

  async function quickAdd(key: string) {
    const title = (adding[key] ?? "").trim();
    if (!title) return;
    adding[key] = "";
    await tasksApi.create(project.id, { title, status: key });
    await load();
  }
  async function onDrop(key: string) {
    overCol = null;
    const id = dragId;
    if (id == null) return;
    const t = tasks.find((x) => x.id === id);
    if (!t || t.status === key) return;
    const nextSort = Math.max(0, ...colTasks(key).map((x) => x.sortOrder)) + 1;
    await tasksApi.move(id, key, nextSort);
    await load();
  }

  function openEdit(t: Task) {
    if (dragging) return;
    editing = t; eTitle = t.title; eDesc = t.description ?? ""; ePriority = t.priority; eDue = t.dueDate ?? ""; eStatus = t.status;
  }
  async function saveEdit() {
    if (!editing) return;
    if (!eTitle.trim()) return;
    await tasksApi.update(editing.id, { title: eTitle.trim(), description: eDesc.trim() || null, priority: ePriority, dueDate: eDue.trim() || null, status: eStatus });
    editing = null; await load();
  }
  async function deleteTask() {
    if (!editing) return;
    const id = editing.id; editing = null; await tasksApi.remove(id); await load();
  }

  // columns management
  function openNewCol() { colNew = true; colEditing = { id: 0, projectId: project.id, key: "", name: "", isDone: false, sortOrder: 0 }; cName = ""; cDone = false; }
  function openEditCol(c: TaskColumn) { colNew = false; colEditing = c; cName = c.name; cDone = c.isDone; }
  async function saveCol() {
    if (!colEditing || !cName.trim()) return;
    if (colNew) await columnsApi.create(project.id, cName.trim(), cDone);
    else await columnsApi.update(colEditing.id, cName.trim(), cDone);
    colEditing = null; await load();
  }
  async function deleteCol() {
    if (!colEditing) return;
    const id = colEditing.id; colEditing = null;
    await columnsApi.remove(id); await load();
  }
</script>

<div style="display:flex;align-items:center;margin-bottom:10px">
  <span style="flex:1"></span>
  <button class="gbtn" onclick={openNewCol}><Icon name="plus" class="ic-sm" /> Колонка</button>
</div>

<div class="kanban" style="grid-template-columns:repeat({Math.max(columns.length, 1)}, 1fr)">
  {#each columns as c (c.id)}
    <div class="col" class:drag-over={overCol === c.key} role="list"
         ondragover={(e) => { e.preventDefault(); overCol = c.key; }}
         ondragleave={() => { if (overCol === c.key) overCol = null; }}
         ondrop={() => onDrop(c.key)}>
      <div class="col-head">
        <span class="led" style="background:{c.isDone ? '#3fb863' : 'var(--accent)'}"></span>
        <span class="h">{c.name}</span>
        <span class="n">{colTasks(c.key).length}</span>
        <button class="mini" title="Настроить колонку" style="margin-left:auto" onclick={() => openEditCol(c)}><Icon name="ellipsis" class="ic-sm" /></button>
      </div>
      <div class="col-body">
        {#each colTasks(c.key) as t (t.id)}
          <div class="tcard" class:dragging={dragId === t.id} draggable={true} role="button" tabindex="0"
               ondragstart={() => { dragId = t.id; dragging = true; }}
               ondragend={() => { dragging = false; setTimeout(() => (dragId = null), 0); }}
               onclick={() => openEdit(t)}>
            <div class="tt">{t.title}</div>
            <div class="row">
              <span class="tag-pri" style="color:{PRIORITY[t.priority].color};background:color-mix(in oklab, {PRIORITY[t.priority].color} 14%, transparent)">{PRIORITY[t.priority].label}</span>
              {#if t.dueDate}<span class="due" style={isOverdue(t.dueDate) ? "color:var(--danger)" : ""}><Icon name="calendar" class="ic-sm" /> {fmtDue(t.dueDate)}</span>{/if}
            </div>
          </div>
        {/each}
      </div>
      <input class="add-task" placeholder="+ задача" bind:value={adding[c.key]} onkeydown={(e) => { if (e.key === 'Enter') quickAdd(c.key); }} />
    </div>
  {/each}
</div>

{#if editing}
  <div class="modal-scrim open" role="dialog" tabindex="-1" aria-label="Задача"
       onmousedown={(e) => { if (e.currentTarget === e.target) (editing = null); }}
       onkeydown={(e) => { if (e.key === 'Escape') (editing = null); }}>
    <div class="modal">
      <div class="modal-head"><span class="t">Задача</span>
        <button class="icon-btn x" onclick={() => (editing = null)}><Icon name="x" class="ic" /></button></div>
      <div class="modal-body">
        <div class="field"><label for="et-title">Заголовок</label><input id="et-title" class="tin" bind:value={eTitle} /></div>
        <div class="field"><label for="et-desc">Описание</label><textarea id="et-desc" class="tin" bind:value={eDesc}></textarea></div>
        <div class="set-grid">
          <div class="field"><label for="et-pri">Приоритет</label>
            <select id="et-pri" class="tin" bind:value={ePriority}>
              <option value={0}>Низкий</option><option value={1}>Средний</option><option value={2}>Высокий</option>
            </select></div>
          <div class="field"><label for="et-status">Колонка</label>
            <select id="et-status" class="tin" bind:value={eStatus}>
              {#each columns as c}<option value={c.key}>{c.name}</option>{/each}
            </select></div>
        </div>
        <div class="field"><label for="et-due">Срок</label><input id="et-due" class="tin" type="date" bind:value={eDue} /></div>
      </div>
      <div class="modal-foot">
        <button class="btn-danger" onclick={deleteTask}><Icon name="trash-2" class="ic-sm" /> Удалить</button>
        <span class="spacer"></span>
        <button class="btn-ghost" onclick={() => (editing = null)}>Отмена</button>
        <button class="btn-primary" onclick={saveEdit}><Icon name="check" class="ic ic-sm" /> Сохранить</button>
      </div>
    </div>
  </div>
{/if}

{#if colEditing}
  <div class="modal-scrim open" role="dialog" tabindex="-1" aria-label="Колонка"
       onmousedown={(e) => { if (e.currentTarget === e.target) (colEditing = null); }}
       onkeydown={(e) => { if (e.key === 'Escape') (colEditing = null); }}>
    <div class="modal" style="max-width:440px">
      <div class="modal-head"><span class="t">{colNew ? "Новая колонка" : "Колонка"}</span>
        <button class="icon-btn x" onclick={() => (colEditing = null)}><Icon name="x" class="ic" /></button></div>
      <div class="modal-body">
        <div class="field"><label for="c-name">Название</label><input id="c-name" class="tin" bind:value={cName} placeholder="Review" /></div>
        <div class="toggle-row">
          <button class="toggle" class:on={cDone} onclick={() => (cDone = !cDone)} aria-pressed={cDone} aria-label="done"></button>
          <div class="tl">Колонка «выполнено»<small>Задачи здесь считаются завершёнными (проставляется дата выполнения).</small></div>
        </div>
      </div>
      <div class="modal-foot">
        {#if !colNew}<button class="btn-danger" onclick={deleteCol}><Icon name="trash-2" class="ic-sm" /> Удалить</button>{/if}
        <span class="spacer"></span>
        <button class="btn-ghost" onclick={() => (colEditing = null)}>Отмена</button>
        <button class="btn-primary" onclick={saveCol}><Icon name="check" class="ic ic-sm" /> Сохранить</button>
      </div>
    </div>
  </div>
{/if}
