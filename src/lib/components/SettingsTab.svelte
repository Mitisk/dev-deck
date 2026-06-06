<script lang="ts">
  import type { Project } from "$lib/types";
  import { STATUS_OPTIONS } from "$lib/format";
  import { loadProjects, activeProjectId } from "$lib/stores/projects";
  import { pushToast } from "$lib/stores/toasts";
  import * as projectsApi from "$lib/api/projects";
  import Icon from "./Icon.svelte";
  import ConfirmDialog from "./ConfirmDialog.svelte";

  let { project }: { project: Project } = $props();

  const EMOJI = ["🚀", "🎨", "📊", "🤖", "🛒", "📱", "⚙️", "🧪", "🔌", "📦", "🌐", "🔥"];
  const COLORS = ["#7c7dff", "#c77dff", "#3fb863", "#e0a83a", "#f0616d", "#5b9cff", "#19c3c0", "#ff8b5b"];

  // Локальные редактируемые копии (инициализируются от проекта; $derived от project.id — пересоздать при смене проекта).
  let name = $state(project.name);
  let description = $state(project.description ?? "");
  let status = $state(project.status);
  let path = $state(project.path ?? "");
  let repoPath = $state(project.repoPath ?? "");
  let tagsRaw = $state(project.tags.join(", "));
  let icon = $state(project.icon ?? EMOJI[0]);
  let color = $state(project.color ?? COLORS[0]);
  let confirmDelete = $state(false);

  // Если переключили проект — перезаполнить форму.
  let lastId = $state(project.id);
  $effect(() => {
    if (project.id !== lastId) {
      lastId = project.id;
      name = project.name; description = project.description ?? ""; status = project.status;
      path = project.path ?? ""; repoPath = project.repoPath ?? "";
      tagsRaw = project.tags.join(", "); icon = project.icon ?? EMOJI[0]; color = project.color ?? COLORS[0];
    }
  });

  async function save() {
    if (!name.trim()) { pushToast("Нужно имя", "Название проекта не может быть пустым", "error"); return; }
    await projectsApi.update(project.id, {
      name: name.trim(),
      description: description.trim() || null,
      status,
      path: path.trim() || null,
      repoPath: repoPath.trim() || null,
      tags: tagsRaw.split(",").map((t) => t.trim()).filter(Boolean).slice(0, 8),
      icon,
      color,
    });
    await loadProjects();
    pushToast("Сохранено", name.trim(), "ok");
  }

  async function togglePin() {
    await projectsApi.setPinned(project.id, !project.pinned);
    await loadProjects();
  }

  async function doArchive() {
    await projectsApi.archive(project.id);
    await loadProjects();
    activeProjectId.set(null);
    pushToast("Проект в архиве", project.name, "info");
  }

  async function doDelete() {
    confirmDelete = false;
    await projectsApi.remove(project.id);
    await loadProjects();
    activeProjectId.set(null);
    pushToast("Проект удалён", project.name, "ok");
  }
</script>

<div class="settings">
  <div class="card set-card">
    <h3 class="section-title">Основное</h3>
    <div class="set-grid">
      <div class="field">
        <label for="st-name">Название</label>
        <input id="st-name" class="tin" bind:value={name} />
      </div>
      <div class="field">
        <label for="st-status">Статус</label>
        <select id="st-status" class="tin" bind:value={status}>
          {#each STATUS_OPTIONS as o}<option value={o.value}>{o.label}</option>{/each}
        </select>
      </div>
      <div class="field span-2">
        <label for="st-desc">Описание</label>
        <textarea id="st-desc" class="tin" bind:value={description}></textarea>
      </div>
      <div class="field">
        <label for="st-path">Папка проекта</label>
        <input id="st-path" class="tin mono" bind:value={path} placeholder="~/dev/project" />
      </div>
      <div class="field">
        <label for="st-repo">Git-репозиторий</label>
        <input id="st-repo" class="tin mono" bind:value={repoPath} placeholder="~/dev/project" />
      </div>
      <div class="field span-2">
        <label for="st-tags">Теги (через запятую)</label>
        <input id="st-tags" class="tin mono" bind:value={tagsRaw} placeholder="rust, docker" />
      </div>
      <div class="field">
        <label>Иконка</label>
        <div class="set-pick-emoji">
          {#each EMOJI as e}<button class="emoji-pick" class:sel={icon === e} onclick={() => (icon = e)}>{e}</button>{/each}
        </div>
      </div>
      <div class="field">
        <label>Цвет</label>
        <div class="set-pick-color">
          {#each COLORS as c}<button class="color-pick" class:sel={color === c} style="--c:{c};background:{c}" onclick={() => (color = c)} aria-label="цвет"></button>{/each}
        </div>
      </div>
    </div>
    <div style="display:flex;gap:10px;margin-top:16px;align-items:center">
      <button class="btn-primary" onclick={save}><Icon name="check" class="ic ic-sm" /> Сохранить</button>
      <button class="btn-ghost" onclick={togglePin}>
        <Icon name={project.pinned ? "pin-off" : "pin"} class="ic-sm" /> {project.pinned ? "Открепить" : "Закрепить"}
      </button>
    </div>
  </div>

  <div class="card set-card danger-zone" style="margin-top:22px">
    <h3 class="section-title">Опасная зона</h3>
    <div class="dz-row">
      <div class="dz-txt">Архивировать проект<small>Скроет из списка, данные сохранятся.</small></div>
      <span class="spacer"></span>
      <button class="btn-ghost" onclick={doArchive}><Icon name="archive" class="ic-sm" /> В архив</button>
    </div>
    <div class="dz-row" style="margin-top:12px">
      <div class="dz-txt">Удалить проект<small>Безвозвратно удалит проект и все его данные.</small></div>
      <span class="spacer"></span>
      <button class="btn-danger" onclick={() => (confirmDelete = true)}><Icon name="trash-2" class="ic-sm" /> Удалить</button>
    </div>
  </div>
</div>

<ConfirmDialog
  open={confirmDelete}
  title="Удалить проект?"
  message={`«${project.name}» и все связанные данные будут удалены безвозвратно.`}
  confirmLabel="Удалить навсегда"
  onConfirm={doDelete}
  onCancel={() => (confirmDelete = false)} />
