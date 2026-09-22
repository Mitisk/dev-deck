<script lang="ts">
  import { projects, activeProjectId, projectsLoaded, loadProjects } from "$lib/stores/projects";
  import { groups, collapsedGroups, toggleCollapsed } from "$lib/stores/groups";
  import { showNewProject, showSettings } from "$lib/stores/ui";
  import { theme, toggleTheme } from "$lib/stores/theme";
  import { changedProjects } from "$lib/stores/changed";
  import { displayName, initials } from "$lib/stores/user";
  import { user } from "$lib/stores/user";
  import * as projectsApi from "$lib/api/projects";
  import * as groupsApi from "$lib/api/groups";
  import { layoutSidebar, planProjectDrop, sectionOf, type Section } from "$lib/sidebarLayout";
  import { reorderIds } from "$lib/credsOrder";
  import type { Project, ProjectGroup } from "$lib/types";
  import Icon from "./Icon.svelte";
  import ProjectIcon from "./ProjectIcon.svelte";
  import ConfirmDialog from "./ConfirmDialog.svelte";

  let query = $state("");

  // Архивные в сайдбаре не показываем.
  const visible = $derived($projects.filter((p) => p.status !== "archived"));
  // Поиск: плоский список без папок.
  const filtered = $derived(
    visible.filter(
      (p) =>
        !query ||
        p.name.toLowerCase().includes(query.toLowerCase()) ||
        p.tags.join(" ").toLowerCase().includes(query.toLowerCase()),
    ),
  );
  const searchPinned = $derived(filtered.filter((p) => p.pinned));
  const searchRest = $derived(filtered.filter((p) => !p.pinned));

  // Обычный режим: закреплённые → папки → корень.
  const layout = $derived(layoutSidebar($projects, $groups));

  function select(id: number) {
    activeProjectId.set(id);
  }

  // --- папки: создание / переименование / удаление ---
  let renamingId = $state<number | null>(null);
  let renameText = $state("");
  let deleting = $state<ProjectGroup | null>(null);

  async function newFolder() {
    try {
      const g = await groupsApi.create("Новая папка");
      groups.update((list) => [...list, g]);
      startRename(g);
    } catch { /* тост из api/client.ts */ }
  }
  function startRename(g: ProjectGroup) {
    renamingId = g.id;
    renameText = g.name;
  }
  async function commitRename() {
    if (renamingId === null) return;
    const id = renamingId;
    const name = renameText.trim();
    renamingId = null;
    const current = $groups.find((g) => g.id === id);
    if (!name || !current || current.name === name) return;
    try {
      const g = await groupsApi.rename(id, name);
      groups.update((list) => list.map((x) => (x.id === id ? g : x)));
    } catch { /* тост из api/client.ts */ }
  }
  function onRenameKey(e: KeyboardEvent) {
    if (e.key === "Enter") { e.preventDefault(); void commitRename(); }
    else if (e.key === "Escape") { renamingId = null; }
  }
  async function confirmDelete() {
    if (!deleting) return;
    const id = deleting.id;
    deleting = null;
    try {
      await groupsApi.remove(id);
      groups.update((list) => list.filter((g) => g.id !== id));
      // проекты этой папки ушли в корень
      projects.update((list) => list.map((p) => (p.groupId === id ? { ...p, groupId: null } : p)));
    } catch { /* тост из api/client.ts */ }
  }
  // svelte action: фокус + выделить всё при появлении инпута переименования
  function focusSelect(node: HTMLInputElement) {
    node.focus();
    node.select();
  }

  // --- drag-and-drop: проекты между/внутри секций, папки между собой (только при пустом поиске) ---
  type Drag = { kind: "project" | "group"; id: number } | null;
  type Over =
    | { type: "row"; id: number; after: boolean }          // строка проекта (before/after)
    | { type: "group"; id: number; after: boolean }        // заголовок папки как цель для папки
    | { type: "into"; groupId: number | null }             // «внутрь» папки / корня (в конец)
    | null;
  let drag = $state<Drag>(null);
  let over = $state<Over>(null);

  const dragIsPinned = $derived(drag?.kind === "project" ? ($projects.find((p) => p.id === drag!.id)?.pinned ?? false) : false);

  function onDragStart(e: DragEvent, kind: "project" | "group", id: number) {
    if (query) { e.preventDefault(); return; }
    drag = { kind, id };
    if (e.dataTransfer) {
      e.dataTransfer.effectAllowed = "move";
      e.dataTransfer.setData("text/plain", `${kind}:${id}`);
    }
  }
  function clearDrag() {
    drag = null;
    over = null;
  }
  function afterHalf(e: DragEvent): boolean {
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    return e.clientY >= rect.top + rect.height / 2;
  }
  // Можно ли бросить перетаскиваемый проект на строку p (закреплённые — только между собой).
  function rowAccepts(p: Project): boolean {
    return drag?.kind === "project" && drag.id !== p.id && p.pinned === dragIsPinned;
  }
  function onRowDragOver(e: DragEvent, p: Project) {
    if (!rowAccepts(p)) return;
    e.preventDefault();
    e.stopPropagation();
    over = { type: "row", id: p.id, after: afterHalf(e) };
  }
  function onGroupDragOver(e: DragEvent, g: ProjectGroup) {
    if (!drag) return;
    if (drag.kind === "project" && dragIsPinned) return;
    if (drag.kind === "group" && drag.id === g.id) return;
    e.preventDefault();
    e.stopPropagation();
    over = drag.kind === "group" ? { type: "group", id: g.id, after: afterHalf(e) } : { type: "into", groupId: g.id };
  }
  function onRootDragOver(e: DragEvent) {
    if (drag?.kind !== "project" || dragIsPinned) return;
    e.preventDefault();
    e.stopPropagation();
    over = { type: "into", groupId: null };
  }
  // Пустая область списка → в корень, в конец. Событие, дошедшее сюда пузырьком от
  // элемента, который бросок не принял, — сбрасываем индикатор.
  function onScrollDragOver(e: DragEvent) {
    if (e.currentTarget !== e.target) { over = null; return; }
    onRootDragOver(e);
  }

  function rowsOfSection(s: Section): Project[] {
    if (s.kind === "pinned") return layout.pinned;
    if (s.kind === "root") return layout.rest;
    return layout.folders.find((f) => f.group.id === s.id)?.items ?? [];
  }

  async function commitDrop(e: DragEvent) {
    e.preventDefault();
    e.stopPropagation();
    const d = drag, o = over;
    clearDrag();
    if (!d || !o) return;

    if (d.kind === "group") {
      if (o.type !== "group") return;
      const ids = layout.folders.map((f) => f.group.id);
      const i = ids.indexOf(o.id);
      const beforeId = o.after ? (i + 1 < ids.length ? ids[i + 1] : null) : o.id;
      const next = reorderIds(ids, d.id, beforeId);
      if (next.join(",") === ids.join(",")) return;
      const pos = new Map(next.map((id, idx) => [id, idx]));
      groups.update((list) => list.map((g) => ({ ...g, sortOrder: pos.get(g.id) ?? g.sortOrder })));
      try { await groupsApi.reorder(next); } catch { await loadProjects(); }
      return;
    }

    // проект
    let section: Section;
    let beforeId: number | null;
    if (o.type === "row") {
      const target = $projects.find((p) => p.id === o.id);
      if (!target) return;
      section = sectionOf(target, $groups);
      const ids = rowsOfSection(section).map((p) => p.id);
      const i = ids.indexOf(o.id);
      beforeId = o.after ? (i + 1 < ids.length ? ids[i + 1] : null) : o.id;
    } else if (o.type === "into") {
      section = o.groupId === null ? { kind: "root" } : { kind: "group", id: o.groupId };
      beforeId = null;
    } else {
      return;
    }
    const plan = planProjectDrop(layout, $groups, d.id, { section, beforeId });
    if (!plan) return;

    // оптимистично: папка + порядок
    const pos = new Map(plan.ids.map((id, idx) => [id, idx]));
    projects.update((list) =>
      list.map((p) => {
        let np = p;
        if (p.id === d.id && plan.setGroup !== undefined) np = { ...np, groupId: plan.setGroup };
        if (pos.has(p.id)) np = { ...np, sortOrder: pos.get(p.id)! };
        return np;
      }),
    );
    try {
      if (plan.setGroup !== undefined) await projectsApi.setGroup(d.id, plan.setGroup);
      await projectsApi.reorder(plan.ids);
    } catch {
      await loadProjects(); // откат к серверному состоянию (тост ошибки уже из client.ts)
    }
  }
</script>

{#snippet projectRow(p: Project)}
  <div class="proj" class:active={$activeProjectId === p.id}
       class:drop-before={over?.type === "row" && over.id === p.id && !over.after}
       class:drop-after={over?.type === "row" && over.id === p.id && over.after}
       class:dragging={drag?.kind === "project" && drag.id === p.id}
       style="--p-color:{p.color ?? 'var(--accent)'}" role="button" tabindex="0"
       draggable={!query}
       ondragstart={(e) => onDragStart(e, "project", p.id)} ondragend={clearDrag}
       ondragover={(e) => onRowDragOver(e, p)} ondrop={commitDrop}
       onclick={() => select(p.id)}>
    {#if p.icon}<span class="emoji"><ProjectIcon icon={p.icon} size={16} /></span>{:else}<span class="dot"></span>{/if}
    <span class="nm">{p.name}</span>
    {#if $changedProjects.includes(p.id)}<span class="meta dirty" title="Изменения в папке"><span class="dot" style="--p-color:var(--git-dirty)"></span></span>{/if}
  </div>
{/snippet}

<aside class="sidebar">
  <div class="sb-head" role="button" tabindex="0" style="cursor:pointer" onclick={() => activeProjectId.set(null)}>
    <span class="logo"><Icon name="layout-grid" class="" /></span>
    <span class="wordmark">Dev<span>Deck</span></span>
  </div>

  <div class="sb-search" class:has-q={query}>
    <Icon name="search" class="ic" />
    <input placeholder="Поиск проектов…" bind:value={query} />
    <span class="kbd">Ctrl K</span>
  </div>

  <div class="sb-new-row">
    <button class="sb-new" onclick={() => showNewProject.set(true)}>
      <Icon name="plus" class="ic ic-sm" /> Новый проект
    </button>
    <button class="sb-new-folder" title="Новая папка" aria-label="Новая папка" onclick={newFolder}>
      <Icon name="folder-plus" class="ic ic-sm" />
    </button>
  </div>

  <div class="sb-scroll" role="list" ondragover={onScrollDragOver} ondrop={commitDrop}>
    {#if query}
      <!-- режим поиска: плоский список -->
      {#if searchPinned.length}
        <div class="sb-section"><span>Закреплённые</span><span class="count">{searchPinned.length}</span></div>
        {#each searchPinned as p (p.id)}{@render projectRow(p)}{/each}
      {/if}
      <div class="sb-section"><span>Все проекты</span><span class="count">{searchRest.length}</span></div>
      {#each searchRest as p (p.id)}{@render projectRow(p)}{/each}
    {:else}
      {#if layout.pinned.length}
        <div class="sb-section"><span>Закреплённые</span><span class="count">{layout.pinned.length}</span></div>
        {#each layout.pinned as p (p.id)}{@render projectRow(p)}{/each}
      {/if}

      {#each layout.folders as f (f.group.id)}
        {@const collapsed = $collapsedGroups.includes(f.group.id)}
        <div class="sb-folder" class:collapsed
             class:drop-into={over?.type === "into" && over.groupId === f.group.id}
             class:drop-before={over?.type === "group" && over.id === f.group.id && !over.after}
             class:drop-after={over?.type === "group" && over.id === f.group.id && over.after}
             class:dragging={drag?.kind === "group" && drag.id === f.group.id}
             role="button" tabindex="0" draggable={renamingId !== f.group.id}
             ondragstart={(e) => onDragStart(e, "group", f.group.id)} ondragend={clearDrag}
             ondragover={(e) => onGroupDragOver(e, f.group)} ondrop={commitDrop}
             onclick={() => { if (renamingId !== f.group.id) toggleCollapsed(f.group.id); }}>
          <Icon name={collapsed ? "chevron-right" : "chevron-down"} class="ic-sm chev" />
          <Icon name={collapsed ? "folder" : "folder-open"} class="ic-sm fico" />
          {#if renamingId === f.group.id}
            <input class="sb-folder-input" bind:value={renameText} use:focusSelect
                   onkeydown={onRenameKey} onblur={commitRename}
                   onclick={(e) => e.stopPropagation()} aria-label="Имя папки" />
          {:else}
            <span class="nm" title="Двойной клик — переименовать" role="presentation"
                  ondblclick={(e) => { e.stopPropagation(); startRename(f.group); }}>{f.group.name}</span>
            <span class="count">{f.items.length}</span>
            <button class="mini fdel" title="Удалить папку (проекты останутся)"
                    onclick={(e) => { e.stopPropagation(); deleting = f.group; }}>
              <Icon name="x" class="ic-sm" />
            </button>
          {/if}
        </div>
        {#if !collapsed}
          <div class="sb-folder-body">
            {#each f.items as p (p.id)}{@render projectRow(p)}{/each}
            {#if !f.items.length}
              <div class="sb-folder-empty" role="presentation"
                   ondragover={(e) => onGroupDragOver(e, f.group)} ondrop={commitDrop}>Перетащите проект сюда</div>
            {/if}
          </div>
        {/if}
      {/each}

      <div class="sb-section" class:drop-into={over?.type === "into" && over.groupId === null}
           role="presentation" ondragover={onRootDragOver} ondrop={commitDrop}>
        <span>Все проекты</span><span class="count">{layout.rest.length}</span>
      </div>
      {#each layout.rest as p (p.id)}{@render projectRow(p)}{/each}
    {/if}

    {#if $projectsLoaded && !visible.length}
      <div class="sb-empty">Пока нет проектов.<br />Создайте первый.</div>
    {:else if !filtered.length && query}
      <div class="sb-empty">Ничего не найдено</div>
    {/if}
  </div>

  <div class="sb-foot">
    <span class="avatar">{$initials}</span>
    <div class="who">{$displayName}{#if $user.handle}<small>{$user.handle}</small>{/if}</div>
    <button class="icon-btn" onclick={toggleTheme} title="Сменить тему">
      {#if $theme === "dark"}<Icon name="sun" class="ic" />{:else}<Icon name="moon" class="ic" />{/if}
    </button>
    <button class="icon-btn" onclick={() => showSettings.set(true)} title="Настройки"><Icon name="settings" class="ic" /></button>
  </div>
</aside>

<ConfirmDialog
  open={deleting !== null}
  title="Удалить папку?"
  message={deleting ? `Папка «${deleting.name}» будет удалена. Проекты останутся и вернутся в общий список.` : ""}
  confirmLabel="Удалить папку"
  onConfirm={confirmDelete}
  onCancel={() => (deleting = null)} />
