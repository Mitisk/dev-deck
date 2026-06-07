<script lang="ts">
  import type { Project, Task, TaskColumn, Label } from "$lib/types";
  import * as tasksApi from "$lib/api/tasks";
  import * as columnsApi from "$lib/api/columns";
  import * as labelsApi from "$lib/api/labels";
  import { pushToast } from "$lib/stores/toasts";
  import Icon from "./Icon.svelte";

  let { project }: { project: Project } = $props();

  const PRIORITY = [
    { label: "Низкий", color: "#5b9cff" },
    { label: "Средний", color: "#e0a83a" },
    { label: "Высокий", color: "#f0616d" },
  ];

  const LABEL_COLORS = ["#7c7dff","#c77dff","#3fb863","#e0a83a","#f0616d","#5b9cff","#19c3c0","#ff8b5b"];

  let columns = $state<TaskColumn[]>([]);
  let tasks = $state<Task[]>([]);
  let adding = $state<Record<string, string>>({});
  let dragId = $state<number | null>(null);
  let dragging = $state(false);
  let overCol = $state<string | null>(null);
  let colDragKey = $state<string | null>(null);
  let lblDragId = $state<number | null>(null);

  // метки
  let labels = $state<Label[]>([]);
  let activeLabels = $state<number[]>([]);
  let activePriorities = $state<number[]>([]);
  // менеджер меток
  let showLabels = $state(false);
  let newLabelName = $state("");
  let newLabelColor = $state(LABEL_COLORS[0]);

  // edit task dialog
  let editing = $state<Task | null>(null);
  let eTitle = $state("");
  let eDesc = $state("");
  let ePriority = $state(0);
  let eDue = $state("");
  let eStatus = $state("");
  // выбор меток в edit-диалоге
  let eLabels = $state<number[]>([]);

  // column dialog
  let colEditing = $state<TaskColumn | null>(null);
  let colNew = $state(false);
  let cName = $state("");
  let cDone = $state(false);

  let reqId = 0;
  async function load() {
    const my = ++reqId;
    try {
      const [cols, ts, lbs] = await Promise.all([columnsApi.list(project.id), tasksApi.list(project.id), labelsApi.list(project.id)]);
      if (my === reqId) { columns = cols; tasks = ts; labels = lbs; }
    } catch { /* тост из api/client.ts */ }
  }
  $effect(() => { project.id; load(); });

  function colTasks(key: string): Task[] {
    return tasks.filter((t) => t.status === key).sort((a, b) => a.sortOrder - b.sortOrder);
  }

  function labelById(id: number): Label | undefined { return labels.find((l) => l.id === id); }
  function taskLabels(t: Task): Label[] { return t.labelIds.map(labelById).filter((l): l is Label => !!l); }
  function toggleFilter(id: number) {
    activeLabels = activeLabels.includes(id) ? activeLabels.filter((x) => x !== id) : [...activeLabels, id];
  }
  function togglePri(p: number) {
    activePriorities = activePriorities.includes(p) ? activePriorities.filter((x) => x !== p) : [...activePriorities, p];
  }
  // задачи колонки с учётом фильтра
  function visibleColTasks(key: string): Task[] {
    let list = colTasks(key);
    if (activeLabels.length) list = list.filter((t) => t.labelIds.some((id) => activeLabels.includes(id)));
    if (activePriorities.length) list = list.filter((t) => activePriorities.includes(t.priority));
    return list;
  }

  async function addLabel() {
    if (!newLabelName.trim()) return;
    await labelsApi.create(project.id, newLabelName.trim(), newLabelColor);
    newLabelName = "";
    await load();
  }
  async function deleteLabel(id: number) {
    activeLabels = activeLabels.filter((x) => x !== id);
    await labelsApi.remove(id);
    await load();
  }
  async function dropLabel(targetId: number) {
    const from = lblDragId;
    lblDragId = null;
    if (from == null || from === targetId) return;
    const order = labels.map((l) => l.id);
    const fi = order.indexOf(from), ti = order.indexOf(targetId);
    if (fi < 0 || ti < 0) return;
    order.splice(ti, 0, ...order.splice(fi, 1));
    await labelsApi.reorder(project.id, order);
    await load();
  }
  function toggleEditLabel(id: number) {
    eLabels = eLabels.includes(id) ? eLabels.filter((x) => x !== id) : [...eLabels, id];
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
    if (colDragKey) {
      const from = colDragKey;
      colDragKey = null;
      overCol = null;
      if (from === key) return;
      const order = columns.map((c) => c.key);
      const fi = order.indexOf(from), ti = order.indexOf(key);
      if (fi < 0 || ti < 0) return;
      order.splice(ti, 0, ...order.splice(fi, 1));
      const ids = order
        .map((k) => columns.find((c) => c.key === k)?.id)
        .filter((x): x is number => x != null);
      await columnsApi.reorder(project.id, ids);
      await load();
      return;
    }
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
    eLabels = [...t.labelIds];
  }
  async function saveEdit() {
    if (!editing) return;
    if (!eTitle.trim()) return;
    await tasksApi.update(editing.id, { title: eTitle.trim(), description: eDesc.trim() || null, priority: ePriority, dueDate: eDue.trim() || null, status: eStatus });
    await labelsApi.setTaskLabels(editing.id, eLabels);
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

<div style="display:flex;align-items:center;gap:8px;margin-bottom:10px;flex-wrap:wrap">
  {#each labels as l (l.id)}
    <button class="chip" style="cursor:pointer;border-color:{l.color ?? 'var(--border-2)'};{activeLabels.includes(l.id) ? `background:color-mix(in oklab, ${l.color ?? 'var(--accent)'} 22%, transparent);color:var(--text)` : ''}"
            onclick={() => toggleFilter(l.id)}>{l.name}</button>
  {/each}
  {#each PRIORITY as p, i}
    <button class="chip" style="cursor:pointer;border-color:{p.color};{activePriorities.includes(i) ? `background:color-mix(in oklab, ${p.color} 22%, transparent);color:var(--text)` : ''}"
            onclick={() => togglePri(i)}>{p.label}</button>
  {/each}
  {#if activeLabels.length || activePriorities.length}<button class="chip" onclick={() => { activeLabels = []; activePriorities = []; }}>сбросить</button>{/if}
  <span style="flex:1"></span>
  <button class="gbtn" onclick={() => (showLabels = true)}><Icon name="tag" class="ic-sm" /> Метки</button>
  <button class="gbtn" onclick={openNewCol}><Icon name="plus" class="ic-sm" /> Колонка</button>
</div>

<div class="kanban" style="grid-template-columns:repeat({Math.max(columns.length, 1)}, 1fr)">
  {#each columns as c (c.id)}
    <div class="col" class:drag-over={overCol === c.key} role="list"
         ondragover={(e) => { e.preventDefault(); if (!colDragKey) overCol = c.key; }}
         ondragleave={() => { if (overCol === c.key) overCol = null; }}
         ondrop={() => onDrop(c.key)}>
      <div class="col-head" draggable={true} style="cursor:grab" title="Перетащите, чтобы изменить порядок"
           ondragstart={() => { colDragKey = c.key; }}
           ondragend={() => { colDragKey = null; }}>
        <Icon name="grip-vertical" class="ic-sm" />
        <span class="led" style="background:{c.isDone ? '#3fb863' : 'var(--accent)'}"></span>
        <span class="h">{c.name}</span>
        <span class="n">{visibleColTasks(c.key).length}</span>
        <button class="mini" title="Настроить колонку" style="margin-left:auto" onclick={() => openEditCol(c)}><Icon name="ellipsis" class="ic-sm" /></button>
      </div>
      <div class="col-body">
        {#each visibleColTasks(c.key) as t (t.id)}
          <div class="tcard" class:dragging={dragId === t.id} draggable={true} role="button" tabindex="0"
               ondragstart={() => { dragId = t.id; dragging = true; }}
               ondragend={() => { dragging = false; setTimeout(() => (dragId = null), 0); }}
               onclick={() => openEdit(t)}>
            <div class="tt">{t.title}</div>
            <div class="row">
              <span class="tag-pri" style="color:{PRIORITY[t.priority].color};background:color-mix(in oklab, {PRIORITY[t.priority].color} 14%, transparent)">{PRIORITY[t.priority].label}</span>
              {#if t.dueDate}<span class="due" style={isOverdue(t.dueDate) ? "color:var(--danger)" : ""}><Icon name="calendar" class="ic-sm" /> {fmtDue(t.dueDate)}</span>{/if}
              {#each taskLabels(t) as l}<span class="chip" style="border-color:{l.color ?? 'var(--border-2)'};color:{l.color ?? 'var(--muted)'}">{l.name}</span>{/each}
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
        <div class="field">
          <label>Метки</label>
          <div class="chips">
            {#each labels as l}
              <button class="chip" style="cursor:pointer;border-color:{l.color ?? 'var(--border-2)'};{eLabels.includes(l.id) ? `background:color-mix(in oklab, ${l.color ?? 'var(--accent)'} 22%, transparent);color:var(--text)` : ''}"
                      onclick={() => toggleEditLabel(l.id)}>{l.name}</button>
            {/each}
            {#if !labels.length}<span style="color:var(--muted-2);font-size:12px">Меток нет — создайте через «Метки».</span>{/if}
          </div>
        </div>
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

{#if showLabels}
  <div class="modal-scrim open" role="dialog" tabindex="-1" aria-label="Метки"
       onmousedown={(e) => { if (e.currentTarget === e.target) (showLabels = false); }}
       onkeydown={(e) => { if (e.key === 'Escape') (showLabels = false); }}>
    <div class="modal" style="max-width:440px">
      <div class="modal-head"><span class="t">Метки проекта</span>
        <button class="icon-btn x" onclick={() => (showLabels = false)}><Icon name="x" class="ic" /></button></div>
      <div class="modal-body">
        {#each labels as l (l.id)}
          <div class="link-row" style="padding:6px 4px;cursor:grab" draggable={true}
               ondragstart={() => { lblDragId = l.id; }}
               ondragend={() => { lblDragId = null; }}
               ondragover={(e) => e.preventDefault()}
               ondrop={() => dropLabel(l.id)}>
            <span class="chip" style="border-color:{l.color ?? 'var(--border-2)'};color:{l.color ?? 'var(--muted)'}">{l.name}</span>
            <span style="flex:1"></span>
            <button class="mini" title="Удалить" onclick={() => deleteLabel(l.id)}><Icon name="trash-2" class="ic-sm" /></button>
          </div>
        {/each}
        <div class="erow" style="margin-top:8px">
          <input class="tin" placeholder="Новая метка" bind:value={newLabelName} onkeydown={(e) => { if (e.key === 'Enter') addLabel(); }} />
          <div class="set-pick-color">
            {#each LABEL_COLORS as c}<button class="color-pick" class:sel={newLabelColor === c} style="--c:{c};background:{c}" onclick={() => (newLabelColor = c)} aria-label="цвет"></button>{/each}
          </div>
          <button class="er-del" style="color:var(--accent)" title="Добавить" onclick={addLabel}><Icon name="plus" class="ic-sm" /></button>
        </div>
      </div>
      <div class="modal-foot"><span class="spacer"></span><button class="btn-ghost" onclick={() => (showLabels = false)}>Закрыть</button></div>
    </div>
  </div>
{/if}
