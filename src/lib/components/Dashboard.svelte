<script lang="ts">
  import { projects, activeProjectId } from "$lib/stores/projects";
  import { USER } from "$lib/mock";
  import { paintIcons } from "$lib/icons";

  const pinned = $derived($projects.filter((p) => p.pinned));

  $effect(() => {
    pinned;
    paintIcons();
  });
</script>

<div class="ws-inner dash">
  <div style="margin-bottom:22px">
    <div class="dash-hello">Привет, <span>{USER.name}</span></div>
    <div class="dash-sub">{$projects.length} проектов</div>
  </div>

  <h3 class="section-title"><svg class="ic-sm" data-lucide="star"></svg> Закреплённые проекты</h3>
  <div class="dash-cards">
    {#each pinned as p (p.id)}
      <div class="card dcard" style="--p-color:{p.color}" role="button" tabindex="0"
           onclick={() => activeProjectId.set(p.id)}>
        <div class="top">
          <span class="be">{p.emoji}</span>
          <div style="min-width:0">
            <h3>{p.name}</h3>
            <div class="pmeta">{p.git.branch}</div>
          </div>
        </div>
      </div>
    {/each}
  </div>
</div>
