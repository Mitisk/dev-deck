<script lang="ts">
  import { projects, activeProjectId, projectsLoaded } from "$lib/stores/projects";
  import { showNewProject } from "$lib/stores/ui";
  import { USER } from "$lib/mock";
  import { statusLabel } from "$lib/format";
  import Icon from "./Icon.svelte";

  const visible = $derived($projects.filter((p) => p.status !== "archived"));
  const pinned = $derived(visible.filter((p) => p.pinned));
  const cards = $derived(pinned.length ? pinned : visible); // если ничего не закреплено — показываем все
</script>

<div class="ws-inner dash">
  <div style="margin-bottom:22px">
    <div class="dash-hello">Привет, <span>{USER.name}</span></div>
    <div class="dash-sub">{visible.length} {visible.length === 1 ? "проект" : "проектов"}</div>
  </div>

  {#if $projectsLoaded && !visible.length}
    <div class="card" style="padding:40px;text-align:center;color:var(--muted)">
      <div style="font-size:15px;color:var(--text);font-weight:600;margin-bottom:6px">Здесь пока пусто</div>
      <div style="margin-bottom:16px">Создайте первый проект, чтобы начать.</div>
      <button class="btn-primary" onclick={() => showNewProject.set(true)}>
        <Icon name="plus" class="ic ic-sm" /> Новый проект
      </button>
    </div>
  {:else}
    <h3 class="section-title"><Icon name="star" class="ic-sm" /> {pinned.length ? "Закреплённые проекты" : "Проекты"}</h3>
    <div class="dash-cards">
      {#each cards as p (p.id)}
        <div class="card dcard" style="--p-color:{p.color ?? 'var(--accent)'}" role="button" tabindex="0"
             onclick={() => activeProjectId.set(p.id)}>
          <div class="top">
            <span class="be">{p.icon ?? "📁"}</span>
            <div style="min-width:0">
              <h3>{p.name}</h3>
              <div class="pmeta">{p.path ?? statusLabel(p.status)}</div>
            </div>
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>
