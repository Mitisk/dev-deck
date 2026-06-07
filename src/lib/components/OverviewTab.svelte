<script lang="ts">
  import type { Project, Link, FileShortcut, ProjectCommand } from "$lib/types";
  import * as linksApi from "$lib/api/links";
  import * as filesApi from "$lib/api/files";
  import * as cmdsApi from "$lib/api/commands";
  import * as actions from "$lib/api/actions";
  import { pushToast } from "$lib/stores/toasts";
  import Icon from "./Icon.svelte";

  let { project }: { project: Project } = $props();

  let links = $state<Link[]>([]);
  let files = $state<FileShortcut[]>([]);
  let cmds = $state<ProjectCommand[]>([]);
  let newLink = $state({ label: "", url: "" });
  let newFile = $state({ label: "", path: "" });

  let reqId = 0;
  async function load() {
    const my = ++reqId;
    try {
      const [l, f, c] = await Promise.all([
        linksApi.list(project.id),
        filesApi.list(project.id),
        cmdsApi.list(project.id),
      ]);
      if (my === reqId) {
        links = l;
        files = f;
        cmds = c;
      }
    } catch {
      /* тост из api/client.ts */
    }
  }
  $effect(() => {
    project.id;
    load();
  });

  // --- команды ---
  let editing = $state<ProjectCommand | null>(null);
  let isNew = $state(false);
  let eLabel = $state("");
  let eCommand = $state("");
  let eDir = $state("");

  function openNewCmd() {
    isNew = true;
    editing = { id: 0, projectId: project.id, label: "", command: "", workingDir: null, runIn: "terminal", icon: "play", sortOrder: 0 };
    eLabel = ""; eCommand = ""; eDir = "";
  }
  function openEditCmd(c: ProjectCommand) {
    isNew = false;
    editing = c;
    eLabel = c.label; eCommand = c.command; eDir = c.workingDir ?? "";
  }
  async function saveCmd() {
    if (!editing) return;
    if (!eLabel.trim() || !eCommand.trim()) return;
    const input = { label: eLabel.trim(), command: eCommand.trim(), workingDir: eDir.trim() || null, runIn: "terminal", icon: "play" };
    if (isNew) await cmdsApi.create(project.id, input);
    else await cmdsApi.update(editing.id, input);
    editing = null;
    await load();
  }
  async function delCmd() {
    if (!editing) return;
    const id = editing.id;
    editing = null;
    await cmdsApi.remove(id);
    await load();
  }
  async function runCmd(c: ProjectCommand) {
    await cmdsApi.run(c.id);
    pushToast("Запуск", c.command, "info");
  }

  // --- ссылки/файлы ---
  async function addLink() {
    if (!newLink.label.trim() || !newLink.url.trim()) return;
    await linksApi.create(project.id, newLink.label.trim(), newLink.url.trim());
    newLink = { label: "", url: "" };
    await load();
  }
  async function delLink(id: number) { await linksApi.remove(id); await load(); }
  async function addFile() {
    if (!newFile.label.trim() || !newFile.path.trim()) return;
    await filesApi.create(project.id, newFile.label.trim(), newFile.path.trim());
    newFile = { label: "", path: "" };
    await load();
  }
  async function delFile(id: number) { await filesApi.remove(id); await load(); }
</script>

<div class="stack">
  {#if project.description}
    <div>
      <h3 class="section-title">Описание</h3>
      <p class="desc">{project.description}</p>
    </div>
  {/if}

  <div>
    <h3 class="section-title">
      <Icon name="terminal" class="ic-sm" /> Команды
      <button class="more" onclick={openNewCmd}>+ команда</button>
    </h3>
    {#if cmds.length}
      <div class="cmd-grid">
        {#each cmds as c (c.id)}
          <button class="cmd" style="--c-tint:{project.color ?? 'var(--accent)'}" onclick={() => runCmd(c)}>
            <span class="ico"><Icon name={c.icon ?? "play"} class="ic" /></span>
            <span class="lbl">{c.label}</span>
            <span class="run mono">{c.command}</span>
            <span class="play" role="button" tabindex="-1"
                  onclick={(e) => { e.stopPropagation(); openEditCmd(c); }}><Icon name="pencil" class="ic-sm" /></span>
          </button>
        {/each}
      </div>
    {:else}
      <button class="add-cred" onclick={openNewCmd}><Icon name="plus" class="ic-sm" /> Добавить команду (напр. npm run dev)</button>
    {/if}
  </div>

  <div>
    <h3 class="section-title"><Icon name="link" class="ic-sm" /> Ссылки</h3>
    <div class="card links">
      {#each links as l (l.id)}
        <div class="link-row">
          <span class="lico" role="button" tabindex="0" onclick={() => actions.openUrl(l.url)}><Icon name={l.icon ?? "globe"} class="ic-sm" /></span>
          <div style="flex:1;min-width:0;cursor:pointer" role="button" tabindex="0" onclick={() => actions.openUrl(l.url)}>
            <div class="lt">{l.label}</div>
            <div class="lu">{l.url}</div>
          </div>
          <button class="mini" title="Открыть" onclick={() => actions.openUrl(l.url)}><Icon name="external-link" class="ic-sm" /></button>
          <button class="mini" title="Удалить" onclick={() => delLink(l.id)}><Icon name="x" class="ic-sm" /></button>
        </div>
      {/each}
      <div class="erow" style="margin:8px 4px 4px">
        <input class="tin" placeholder="Название" bind:value={newLink.label} />
        <input class="tin url" placeholder="localhost:3000 / github.com/…" bind:value={newLink.url}
               onkeydown={(e) => { if (e.key === 'Enter') addLink(); }} />
        <button class="er-del" style="color:var(--accent)" title="Добавить" onclick={addLink}><Icon name="plus" class="ic-sm" /></button>
      </div>
    </div>
  </div>

  <div>
    <h3 class="section-title"><Icon name="folder" class="ic-sm" /> Файлы и папки</h3>
    <div class="card links">
      {#each files as f (f.id)}
        <div class="link-row">
          <span class="lico" role="button" tabindex="0" onclick={() => actions.openShortcut(f.path)}><Icon name="file" class="ic-sm" /></span>
          <div style="flex:1;min-width:0;cursor:pointer" role="button" tabindex="0" onclick={() => actions.openShortcut(f.path)}>
            <div class="lt">{f.label}</div>
            <div class="lu">{f.path}</div>
          </div>
          <button class="mini" title="Открыть" onclick={() => actions.openShortcut(f.path)}><Icon name="external-link" class="ic-sm" /></button>
          <button class="mini" title="Удалить" onclick={() => delFile(f.id)}><Icon name="x" class="ic-sm" /></button>
        </div>
      {/each}
      <div class="erow" style="margin:8px 4px 4px">
        <input class="tin" placeholder="Название" bind:value={newFile.label} />
        <input class="tin url" placeholder="~/dev/proj/.env" bind:value={newFile.path}
               onkeydown={(e) => { if (e.key === 'Enter') addFile(); }} />
        <button class="er-del" style="color:var(--accent)" title="Добавить" onclick={addFile}><Icon name="plus" class="ic-sm" /></button>
      </div>
    </div>
  </div>
</div>

{#if editing}
  <div class="modal-scrim open" role="dialog" tabindex="-1" aria-label="Команда"
       onmousedown={(e) => { if (e.currentTarget === e.target) (editing = null); }}
       onkeydown={(e) => { if (e.key === 'Escape') (editing = null); }}>
    <div class="modal">
      <div class="modal-head">
        <span class="t">{isNew ? "Новая команда" : "Команда"}</span>
        <button class="icon-btn x" onclick={() => (editing = null)}><Icon name="x" class="ic" /></button>
      </div>
      <div class="modal-body">
        <div class="field"><label for="cm-label">Ярлык</label><input id="cm-label" class="tin" placeholder="Запустить dev" bind:value={eLabel} /></div>
        <div class="field"><label for="cm-cmd">Команда (shell)</label><input id="cm-cmd" class="tin mono" placeholder="npm run dev" bind:value={eCommand} /></div>
        <div class="field"><label for="cm-dir">Рабочая папка <span style="color:var(--muted-2)">(пусто = папка проекта)</span></label>
          <input id="cm-dir" class="tin mono" placeholder={project.path ?? "~/dev/project"} bind:value={eDir} /></div>
        <p class="desc" style="color:var(--muted-2);font-size:12px">Запуск открывает новое окно терминала. Фоновый режим с логами — позже.</p>
      </div>
      <div class="modal-foot">
        {#if !isNew}<button class="btn-danger" onclick={delCmd}><Icon name="trash-2" class="ic-sm" /> Удалить</button>{/if}
        <span class="spacer"></span>
        <button class="btn-ghost" onclick={() => (editing = null)}>Отмена</button>
        <button class="btn-primary" onclick={saveCmd}><Icon name="check" class="ic ic-sm" /> Сохранить</button>
      </div>
    </div>
  </div>
{/if}
