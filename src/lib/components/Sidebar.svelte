<script lang="ts">
  import { projects, activeProjectId } from "$lib/stores/projects";
  import { theme, toggleTheme } from "$lib/stores/theme";
  import { USER } from "$lib/mock";
  import { paintIcons } from "$lib/icons";

  let query = $state("");

  const filtered = $derived(
    $projects.filter(
      (p) =>
        !query ||
        p.name.toLowerCase().includes(query.toLowerCase()) ||
        p.tags.join(" ").toLowerCase().includes(query.toLowerCase()),
    ),
  );
  const pinned = $derived(filtered.filter((p) => p.pinned));
  const rest = $derived(filtered.filter((p) => !p.pinned));

  $effect(() => {
    filtered;
    $theme;
    paintIcons();
  });

  function select(id: string) {
    activeProjectId.set(id);
  }
</script>

<aside class="sidebar">
  <div class="sb-head">
    <span class="logo"><svg data-lucide="layout-grid"></svg></span>
    <span class="wordmark">Dev<span>Deck</span></span>
    <span class="ver">0.1</span>
  </div>

  <div class="sb-search" class:has-q={query}>
    <svg class="ic" data-lucide="search"></svg>
    <input placeholder="Поиск проектов…" bind:value={query} />
    <span class="kbd">Ctrl K</span>
  </div>

  <button class="sb-new"><svg class="ic ic-sm" data-lucide="plus"></svg> Новый проект</button>

  <div class="sb-scroll">
    {#if pinned.length}
      <div class="sb-section"><span>Закреплённые</span><span class="count">{pinned.length}</span></div>
      {#each pinned as p (p.id)}
        <div class="proj" class:active={$activeProjectId === p.id}
             style="--p-color:{p.color}" role="button" tabindex="0"
             onclick={() => select(p.id)}>
          <span class="emoji">{p.emoji}</span>
          <span class="nm">{p.name}</span>
        </div>
      {/each}
    {/if}

    <div class="sb-section"><span>Все проекты</span><span class="count">{rest.length}</span></div>
    {#each rest as p (p.id)}
      <div class="proj" class:active={$activeProjectId === p.id}
           style="--p-color:{p.color}" role="button" tabindex="0"
           onclick={() => select(p.id)}>
        <span class="dot"></span>
        <span class="nm">{p.name}</span>
      </div>
    {/each}

    {#if !filtered.length}
      <div class="sb-empty">Ничего не найдено</div>
    {/if}
  </div>

  <div class="sb-foot">
    <span class="avatar">{USER.initials}</span>
    <div class="who">{USER.name}<small>{USER.handle}</small></div>
    <button class="icon-btn" onclick={toggleTheme} title="Сменить тему">
      {#if $theme === "dark"}<svg class="ic" data-lucide="sun"></svg>
      {:else}<svg class="ic" data-lucide="moon"></svg>{/if}
    </button>
  </div>
</aside>
