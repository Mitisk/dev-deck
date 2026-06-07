<script lang="ts">
  import type { Project, Task, TaskStatus } from "$lib/types";
  import * as tasksApi from "$lib/api/tasks";
  import Icon from "./Icon.svelte";

  let { project }: { project: Project } = $props();

  const COLUMNS: { key: TaskStatus; label: string; led: string }[] = [
    { key: "todo", label: "To Do", led: "#5b9cff" },
    { key: "doing", label: "In Progress", led: "#e0a83a" },
    { key: "done", label: "Done", led: "#3fb863" },
  ];
  const PRIORITY = [
    { label: "Низкий", color: "#5b9cff" },
    { label: "Средний", color: "#e0a83a" },
    { label: "Высокий", color: "#f0616d" },
  ];

  let tasks = $state<Task[]>([]);
  let adding = $state<Record<TaskStatus, string>>({ todo: "", doing: "", done: "" });
  let dragId = $state<number | null>(null);
  let dragging = $state(false);
  let overCol = $state<TaskStatus | null>(null);

  // edit dialog
  let editing = $state<Task | null>(null);
  let eTitle = $state("");
  let eDesc = $state("");
  let ePriority = $state(0);
  let eDue = $state("");
  let eStatus = $state<TaskStatus>("todo");

  let reqId = 0;
  async function load() {
    const my = ++reqId;
    try {
      const list = await tasksApi.list(project.id);
      if (my === reqId) tasks = list;
    } catch {
      /* тост из api/client.ts */
    }
  }
  $effect(() => {
    project.id;
    load();
  });

  function col(status: TaskStatus): Task[] {
    return tasks.filter((t) => t.status === status).sort((a, b) => a.sortOrder - b.sortOrder);
  }

  function todayStr(): string {
    const d = new Date();
    return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
  }
  function fmtDue(due: string | null): string {
    if (!due) return "";
    const d = new Date(due + "T00:00:00");
    if (isNaN(d.getTime())) return due; // не ISO — показать как есть
    return d.toLocaleDateString("ru-RU", { day: "numeric", month: "short" });
  }
  function isOverdue(due: string | null): boolean {
    return !!due && /^\d{4}-\d{2}-\d{2}$/.test(due) && due < todayStr();
  }

  async function quickAdd(status: TaskStatus) {
    const title = (adding[status] ?? "").trim();
    if (!title) return;
    adding[status] = "";
    await tasksApi.create(project.id, { title, status });
    await load();
  }

  async function onDrop(status: TaskStatus) {
    overCol = null;
    const id = dragId;
    if (id == null) return;
    const t = tasks.find((x) => x.id === id);
    if (!t || t.status === status) return;
    const nextSort = Math.max(0, ...col(status).map((x) => x.sortOrder)) + 1;
    await tasksApi.move(id, status, nextSort);
    await load();
  }

  function openEdit(t: Task) {
    if (dragging) return;
    editing = t;
    eTitle = t.title;
    eDesc = t.description ?? "";
    ePriority = t.priority;
    eDue = t.dueDate ?? "";
    eStatus = t.status;
  }

  async function saveEdit() {
    if (!editing) return;
    const title = eTitle.trim();
    if (!title) return;
    await tasksApi.update(editing.id, {
      title,
      description: eDesc.trim() || null,
      priority: ePriority,
      dueDate: eDue.trim() || null,
      status: eStatus,
    });
    editing = null;
    await load();
  }

  async function deleteTask() {
    if (!editing) return;
    const id = editing.id;
    editing = null;
    await tasksApi.remove(id);
    await load();
  }
</script>

<div class="kanban">
  {#each COLUMNS as c (c.key)}
    <div
      class="col"
      class:drag-over={overCol === c.key}
      role="list"
      ondragover={(e) => { e.preventDefault(); overCol = c.key; }}
      ondragleave={() => { if (overCol === c.key) overCol = null; }}
      ondrop={() => onDrop(c.key)}
    >
      <div class="col-head">
        <span class="led" style="background:{c.led}"></span>
        <span class="h">{c.label}</span>
        <span class="n">{col(c.key).length}</span>
      </div>
      <div class="col-body">
        {#each col(c.key) as t (t.id)}
          <div
            class="tcard"
            class:dragging={dragId === t.id}
            draggable={true}
            role="button"
            tabindex="0"
            ondragstart={() => { dragId = t.id; dragging = true; }}
            ondragend={() => { dragging = false; setTimeout(() => (dragId = null), 0); }}
            onclick={() => openEdit(t)}
          >
            <div class="tt">{t.title}</div>
            <div class="row">
              <span class="tag-pri" style="color:{PRIORITY[t.priority].color};background:color-mix(in oklab, {PRIORITY[t.priority].color} 14%, transparent)">
                {PRIORITY[t.priority].label}
              </span>
              {#if t.dueDate}<span class="due" style={isOverdue(t.dueDate) ? "color:var(--danger)" : ""}><Icon name="calendar" class="ic-sm" /> {fmtDue(t.dueDate)}</span>{/if}
            </div>
          </div>
        {/each}
      </div>
      <input
        class="add-task"
        placeholder="+ задача"
        bind:value={adding[c.key]}
        onkeydown={(e) => { if (e.key === "Enter") quickAdd(c.key); }}
      />
    </div>
  {/each}
</div>

{#if editing}
  <div
    class="modal-scrim open"
    role="dialog"
    tabindex="-1"
    aria-label="Задача"
    onmousedown={(e) => { if (e.currentTarget === e.target) (editing = null); }}
    onkeydown={(e) => { if (e.key === "Escape") (editing = null); }}
  >
    <div class="modal">
      <div class="modal-head">
        <span class="t">Задача</span>
        <button class="icon-btn x" onclick={() => (editing = null)}><Icon name="x" class="ic" /></button>
      </div>
      <div class="modal-body">
        <div class="field">
          <label for="et-title">Заголовок</label>
          <input id="et-title" class="tin" bind:value={eTitle} />
        </div>
        <div class="field">
          <label for="et-desc">Описание</label>
          <textarea id="et-desc" class="tin" bind:value={eDesc}></textarea>
        </div>
        <div class="set-grid">
          <div class="field">
            <label for="et-pri">Приоритет</label>
            <select id="et-pri" class="tin" bind:value={ePriority}>
              <option value={0}>Низкий</option>
              <option value={1}>Средний</option>
              <option value={2}>Высокий</option>
            </select>
          </div>
          <div class="field">
            <label for="et-status">Статус</label>
            <select id="et-status" class="tin" bind:value={eStatus}>
              <option value="todo">To Do</option>
              <option value="doing">In Progress</option>
              <option value="done">Done</option>
            </select>
          </div>
        </div>
        <div class="field">
          <label for="et-due">Срок</label>
          <input id="et-due" class="tin" type="date" bind:value={eDue} />
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
