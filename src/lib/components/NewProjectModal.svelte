<script lang="ts">
  import { showNewProject } from "$lib/stores/ui";
  import { loadProjects, activeProjectId } from "$lib/stores/projects";
  import { pushToast } from "$lib/stores/toasts";
  import * as projectsApi from "$lib/api/projects";
  import Icon from "./Icon.svelte";
  import PathInput from "./PathInput.svelte";
  import ProjectIcon from "./ProjectIcon.svelte";
  import { open } from "@tauri-apps/plugin-dialog";

  async function pickIcon() {
    const sel = await open({ multiple: false, filters: [{ name: "Изображения", extensions: ["png", "jpg", "jpeg", "gif", "webp", "svg", "ico", "bmp"] }] });
    if (!sel || Array.isArray(sel)) return;
    try { emoji = await projectsApi.importIcon(sel); } catch { /* тост из api/client.ts */ }
  }

  const EMOJI = ["🚀", "🎨", "📊", "🤖", "🛒", "📱", "⚙️", "🧪", "🔌", "📦", "🌐", "🔥"];
  const COLORS = ["#7c7dff", "#c77dff", "#3fb863", "#e0a83a", "#f0616d", "#5b9cff", "#19c3c0", "#ff8b5b"];

  let name = $state("");
  let path = $state("");
  let tagsRaw = $state("");
  let emoji = $state(EMOJI[0]);
  let color = $state(COLORS[0]);
  let saving = $state(false);

  const canCreate = $derived(name.trim().length > 0 && !saving);

  function close() {
    showNewProject.set(false);
    name = ""; path = ""; tagsRaw = ""; emoji = EMOJI[0]; color = COLORS[0]; saving = false;
  }

  async function create() {
    if (!canCreate) return;
    saving = true;
    try {
      const tags = tagsRaw.split(",").map((t) => t.trim()).filter(Boolean).slice(0, 8);
      const p = await projectsApi.create({
        name: name.trim(),
        path: path.trim() || null,
        repoPath: path.trim() || null,
        icon: emoji,
        color,
        tags,
        status: "active",
      });
      await loadProjects();
      activeProjectId.set(p.id);
      pushToast("Проект создан", p.name, "ok");
      close();
    } catch {
      // тост об ошибке уже показан в api/client.ts
      saving = false;
    }
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape") close();
    if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) create();
  }
</script>

{#if $showNewProject}
  <div class="modal-scrim open" onmousedown={(e) => { if (e.currentTarget === e.target) close(); }}
       onkeydown={onKey} role="dialog" tabindex="-1" aria-label="Новый проект">
    <div class="modal">
      <div class="modal-head">
        <span class="mh-ico"><Icon name="folder-plus" class="ic" /></span>
        <span class="t">Новый проект</span>
        <button class="icon-btn x" onclick={close}><Icon name="x" class="ic" /></button>
      </div>
      <div class="modal-body">
        <div class="field">
          <label for="np-icon-url">Иконка <span style="color:var(--muted-2)">(эмодзи, URL favicon или файл)</span></label>
          <div style="display:flex;align-items:center;gap:12px">
            <span class="icon-preview"><ProjectIcon icon={emoji} size={30} /></span>
            <div class="picker" style="flex:1">
              {#each EMOJI as e}
                <button class="emoji-pick" class:sel={emoji === e} onclick={() => (emoji = e)}>{e}</button>
              {/each}
            </div>
          </div>
          <div class="path-input" style="margin-top:8px">
            <input id="np-icon-url" class="tin mono" placeholder="https://site.com/favicon.ico" bind:value={emoji} />
            <button class="path-browse" type="button" title="Выбрать изображение…" onclick={pickIcon}><Icon name="image" class="ic-sm" /></button>
          </div>
        </div>
        <div class="field">
          <label for="np-name">Название</label>
          <input id="np-name" class="tin" bind:value={name} placeholder="Например, Aurora API" autocomplete="off" />
        </div>
        <div class="field">
          <label for="np-path">Путь к репозиторию</label>
          <PathInput id="np-path" bind:value={path} placeholder="~/dev/my-project" />
        </div>
        <div class="field">
          <label for="np-tags">Теги (через запятую)</label>
          <input id="np-tags" class="tin mono" bind:value={tagsRaw} placeholder="rust, postgres, docker" autocomplete="off" />
        </div>
        <div class="field">
          <label>Цвет проекта</label>
          <div class="picker">
            {#each COLORS as c}
              <button class="color-pick" class:sel={color === c} style="--c:{c};background:{c}" onclick={() => (color = c)} aria-label="цвет"></button>
            {/each}
          </div>
        </div>
      </div>
      <div class="modal-foot">
        <span class="spacer"></span>
        <button class="btn-ghost" onclick={close}>Отмена</button>
        <button class="btn-primary" disabled={!canCreate} onclick={create}>
          <Icon name="check" class="ic ic-sm" /> Создать проект
        </button>
      </div>
    </div>
  </div>
{/if}
