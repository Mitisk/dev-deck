<script lang="ts">
  import { projects, activeProjectId, projectsLoaded } from "$lib/stores/projects";
  import { showNewProject, showSettings } from "$lib/stores/ui";
  import { theme, toggleTheme } from "$lib/stores/theme";
  import { changedProjects } from "$lib/stores/changed";
  import { displayName, initials } from "$lib/stores/user";
  import { user } from "$lib/stores/user";
  import Icon from "./Icon.svelte";
  import ProjectIcon from "./ProjectIcon.svelte";

  let query = $state("");

  // Архивные в сайдбаре не показываем.
  const visible = $derived($projects.filter((p) => p.status !== "archived"));
  const filtered = $derived(
    visible.filter(
      (p) =>
        !query ||
        p.name.toLowerCase().includes(query.toLowerCase()) ||
        p.tags.join(" ").toLowerCase().includes(query.toLowerCase()),
    ),
  );
  const pinned = $derived(filtered.filter((p) => p.pinned));
  const rest = $derived(filtered.filter((p) => !p.pinned));

  function select(id: number) {
    activeProjectId.set(id);
  }
</script>

<aside class="sidebar">
  <div class="sb-head" role="button" tabindex="0" style="cursor:pointer" onclick={() => activeProjectId.set(null)}>
    <span class="logo"><Icon name="layout-grid" class="" /></span>
    <span class="wordmark">Dev<span>Deck</span></span>
    <span class="ver">0.1</span>
  </div>

  <div class="sb-search" class:has-q={query}>
    <Icon name="search" class="ic" />
    <input placeholder="Поиск проектов…" bind:value={query} />
    <span class="kbd">Ctrl K</span>
  </div>

  <button class="sb-new" onclick={() => showNewProject.set(true)}>
    <Icon name="plus" class="ic ic-sm" /> Новый проект
  </button>

  <div class="sb-scroll">
    {#if pinned.length}
      <div class="sb-section"><span>Закреплённые</span><span class="count">{pinned.length}</span></div>
      {#each pinned as p (p.id)}
        <div class="proj" class:active={$activeProjectId === p.id}
             style="--p-color:{p.color ?? 'var(--accent)'}" role="button" tabindex="0"
             onclick={() => select(p.id)}>
          {#if p.icon}<span class="emoji"><ProjectIcon icon={p.icon} size={16} /></span>{:else}<span class="dot"></span>{/if}
          <span class="nm">{p.name}</span>
          {#if $changedProjects.includes(p.id)}<span class="meta dirty" title="Изменения в папке"><span class="dot" style="--p-color:var(--git-dirty)"></span></span>{/if}
        </div>
      {/each}
    {/if}

    <div class="sb-section"><span>Все проекты</span><span class="count">{rest.length}</span></div>
    {#each rest as p (p.id)}
      <div class="proj" class:active={$activeProjectId === p.id}
           style="--p-color:{p.color ?? 'var(--accent)'}" role="button" tabindex="0"
           onclick={() => select(p.id)}>
        {#if p.icon}<span class="emoji">{p.icon}</span>{:else}<span class="dot"></span>{/if}
        <span class="nm">{p.name}</span>
        {#if $changedProjects.includes(p.id)}<span class="meta dirty" title="Изменения в папке"><span class="dot" style="--p-color:var(--git-dirty)"></span></span>{/if}
      </div>
    {/each}

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
