<script lang="ts">
  import type { Project, ProjectCommand, Link, FileShortcut } from "$lib/types";
  import { STATUS_OPTIONS } from "$lib/format";
  import { loadProjects, activeProjectId } from "$lib/stores/projects";
  import { pushToast } from "$lib/stores/toasts";
  import * as projectsApi from "$lib/api/projects";
  import * as cmdsApi from "$lib/api/commands";
  import * as linksApi from "$lib/api/links";
  import * as filesApi from "$lib/api/files";
  import * as tasksApi from "$lib/api/tasks";
  import Icon from "./Icon.svelte";
  import ConfirmDialog from "./ConfirmDialog.svelte";

  let { project }: { project: Project } = $props();

  let cmds = $state<ProjectCommand[]>([]);
  let links = $state<Link[]>([]);
  let files = $state<FileShortcut[]>([]);

  // редактор команды
  let cmdEditing = $state<ProjectCommand | null>(null);
  let cmdNew = $state(false);
  let cLabel = $state(""); let cCommand = $state(""); let cDir = $state(""); let cRunIn = $state("terminal");
  // строки добавления
  let newLink = $state({ label: "", url: "" });
  let newFile = $state({ label: "", path: "" });

  let extrasReq = 0;
  async function loadExtras() {
    const my = ++extrasReq;
    try {
      const [c, l, f] = await Promise.all([cmdsApi.list(project.id), linksApi.list(project.id), filesApi.list(project.id)]);
      if (my === extrasReq) { cmds = c; links = l; files = f; }
    } catch { /* тост из api/client.ts */ }
  }
  $effect(() => { project.id; loadExtras(); });

  function openNewCmd() { cmdNew = true; cmdEditing = { id: 0, projectId: project.id, label: "", command: "", workingDir: null, runIn: "terminal", icon: "play", sortOrder: 0 }; cLabel = ""; cCommand = ""; cDir = ""; cRunIn = "terminal"; }
  function openEditCmd(c: ProjectCommand) { cmdNew = false; cmdEditing = c; cLabel = c.label; cCommand = c.command; cDir = c.workingDir ?? ""; cRunIn = c.runIn ?? "terminal"; }
  async function saveCmd() {
    if (!cmdEditing || !cLabel.trim() || !cCommand.trim()) return;
    const input = { label: cLabel.trim(), command: cCommand.trim(), workingDir: cDir.trim() || null, runIn: cRunIn, icon: "play" };
    if (cmdNew) await cmdsApi.create(project.id, input);
    else await cmdsApi.update(cmdEditing.id, input);
    cmdEditing = null; await loadExtras();
  }
  async function delCmd() { if (!cmdEditing) return; const id = cmdEditing.id; cmdEditing = null; await cmdsApi.remove(id); await loadExtras(); }

  async function addLink() { if (!newLink.label.trim() || !newLink.url.trim()) return; await linksApi.create(project.id, newLink.label.trim(), newLink.url.trim()); newLink = { label: "", url: "" }; await loadExtras(); }
  async function delLink(id: number) { await linksApi.remove(id); await loadExtras(); }
  async function addFile() { if (!newFile.label.trim() || !newFile.path.trim()) return; await filesApi.create(project.id, newFile.label.trim(), newFile.path.trim()); newFile = { label: "", path: "" }; await loadExtras(); }
  async function delFile(id: number) { await filesApi.remove(id); await loadExtras(); }

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
  let confirmClearDone = $state(false);

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

  async function doClearDone() {
    confirmClearDone = false;
    const n = await tasksApi.deleteCompleted(project.id);
    pushToast(n ? "Завершённые удалены" : "Нечего удалять", n ? `Удалено задач: ${n}` : "Завершённых задач нет", n ? "ok" : "info");
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

  <div class="card set-card" style="margin-top:22px">
    <h3 class="section-title"><Icon name="terminal" class="ic-sm" /> Команды
      <button class="more" onclick={openNewCmd} style="margin-left:auto">+ команда</button></h3>
    {#if cmds.length}
      <div class="card links">
        {#each cmds as c (c.id)}
          <div class="link-row">
            <span class="lico"><Icon name={c.icon ?? "play"} class="ic-sm" /></span>
            <div style="flex:1;min-width:0">
              <div class="lt">{c.label} <span style="color:var(--muted-2);font-size:11px">· {c.runIn === "background" ? "фон" : "терминал"}</span></div>
              <div class="lu mono">{c.command}</div>
            </div>
            <button class="mini" title="Изменить" onclick={() => openEditCmd(c)}><Icon name="pencil" class="ic-sm" /></button>
          </div>
        {/each}
      </div>
    {:else}
      <button class="add-cred" onclick={openNewCmd}><Icon name="plus" class="ic-sm" /> Добавить команду (напр. npm run dev)</button>
    {/if}
  </div>

  <div class="card set-card" style="margin-top:22px">
    <h3 class="section-title"><Icon name="link" class="ic-sm" /> Быстрые ссылки</h3>
    <div class="card links">
      {#each links as l (l.id)}
        <div class="link-row">
          <span class="lico"><Icon name={l.icon ?? "globe"} class="ic-sm" /></span>
          <div style="flex:1;min-width:0"><div class="lt">{l.label}</div><div class="lu">{l.url}</div></div>
          <button class="mini" title="Удалить" onclick={() => delLink(l.id)}><Icon name="x" class="ic-sm" /></button>
        </div>
      {/each}
      <div class="erow" style="margin:8px 4px 4px">
        <input class="tin" placeholder="Название" bind:value={newLink.label} />
        <input class="tin url" placeholder="localhost:3000 / github.com/…" bind:value={newLink.url} onkeydown={(e) => { if (e.key === 'Enter') addLink(); }} />
        <button class="er-del" style="color:var(--accent)" title="Добавить" onclick={addLink}><Icon name="plus" class="ic-sm" /></button>
      </div>
    </div>
  </div>

  <div class="card set-card" style="margin-top:22px">
    <h3 class="section-title"><Icon name="folder" class="ic-sm" /> Файлы и папки</h3>
    <div class="card links">
      {#each files as f (f.id)}
        <div class="link-row">
          <span class="lico"><Icon name="file" class="ic-sm" /></span>
          <div style="flex:1;min-width:0"><div class="lt">{f.label}</div><div class="lu">{f.path}</div></div>
          <button class="mini" title="Удалить" onclick={() => delFile(f.id)}><Icon name="x" class="ic-sm" /></button>
        </div>
      {/each}
      <div class="erow" style="margin:8px 4px 4px">
        <input class="tin" placeholder="Название" bind:value={newFile.label} />
        <input class="tin url" placeholder="~/dev/proj/.env" bind:value={newFile.path} onkeydown={(e) => { if (e.key === 'Enter') addFile(); }} />
        <button class="er-del" style="color:var(--accent)" title="Добавить" onclick={addFile}><Icon name="plus" class="ic-sm" /></button>
      </div>
    </div>
  </div>

  <div class="card set-card danger-zone" style="margin-top:22px">
    <h3 class="section-title">Опасная зона</h3>
    <div class="dz-row">
      <div class="dz-txt">Удалить завершённые задачи<small>Безвозвратно удалит все выполненные задачи проекта.</small></div>
      <span class="spacer"></span>
      <button class="btn-ghost" onclick={() => (confirmClearDone = true)}><Icon name="check-check" class="ic-sm" /> Очистить</button>
    </div>
    <div class="dz-row" style="margin-top:12px">
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

<ConfirmDialog
  open={confirmClearDone}
  title="Удалить завершённые задачи?"
  message="Все выполненные задачи проекта будут удалены безвозвратно."
  confirmLabel="Удалить"
  onConfirm={doClearDone}
  onCancel={() => (confirmClearDone = false)} />

{#if cmdEditing}
  <div class="modal-scrim open" role="dialog" tabindex="-1" aria-label="Команда"
       onmousedown={(e) => { if (e.currentTarget === e.target) (cmdEditing = null); }}
       onkeydown={(e) => { if (e.key === 'Escape') (cmdEditing = null); }}>
    <div class="modal">
      <div class="modal-head"><span class="t">{cmdNew ? "Новая команда" : "Команда"}</span>
        <button class="icon-btn x" onclick={() => (cmdEditing = null)}><Icon name="x" class="ic" /></button></div>
      <div class="modal-body">
        <div class="field"><label for="cm-label">Ярлык</label><input id="cm-label" class="tin" placeholder="Запустить dev" bind:value={cLabel} /></div>
        <div class="field"><label for="cm-cmd">Команда (shell)</label><input id="cm-cmd" class="tin mono" placeholder="npm run dev" bind:value={cCommand} /></div>
        <div class="field"><label for="cm-dir">Рабочая папка <span style="color:var(--muted-2)">(пусто = папка проекта)</span></label>
          <input id="cm-dir" class="tin mono" placeholder={project.path ?? "~/dev/project"} bind:value={cDir} /></div>
        <div class="field"><label for="cm-mode">Режим запуска</label>
          <select id="cm-mode" class="tin" bind:value={cRunIn}>
            <option value="terminal">В терминале (новое окно)</option>
            <option value="background">В фоне (логи + стоп)</option>
          </select></div>
        <p class="desc" style="color:var(--muted-2);font-size:12px">Фоновый режим запускает процесс скрыто и стримит вывод в панель логов; «стоп» завершает дерево процессов. Запуск — во вкладке «Обзор».</p>
      </div>
      <div class="modal-foot">
        {#if !cmdNew}<button class="btn-danger" onclick={delCmd}><Icon name="trash-2" class="ic-sm" /> Удалить</button>{/if}
        <span class="spacer"></span>
        <button class="btn-ghost" onclick={() => (cmdEditing = null)}>Отмена</button>
        <button class="btn-primary" onclick={saveCmd}><Icon name="check" class="ic ic-sm" /> Сохранить</button>
      </div>
    </div>
  </div>
{/if}
