<script lang="ts">
  import type { Project, Link, FileShortcut } from "$lib/types";
  import * as linksApi from "$lib/api/links";
  import * as filesApi from "$lib/api/files";
  import * as actions from "$lib/api/actions";
  import Icon from "./Icon.svelte";

  let { project }: { project: Project } = $props();

  let links = $state<Link[]>([]);
  let files = $state<FileShortcut[]>([]);
  let newLink = $state({ label: "", url: "" });
  let newFile = $state({ label: "", path: "" });

  let reqId = 0;
  async function load() {
    const my = ++reqId;
    try {
      const [l, f] = await Promise.all([linksApi.list(project.id), filesApi.list(project.id)]);
      if (my === reqId) {
        links = l;
        files = f;
      }
    } catch {
      /* тост из api/client.ts */
    }
  }
  $effect(() => {
    project.id;
    load();
  });

  async function addLink() {
    if (!newLink.label.trim() || !newLink.url.trim()) return;
    await linksApi.create(project.id, newLink.label.trim(), newLink.url.trim());
    newLink = { label: "", url: "" };
    await load();
  }
  async function delLink(id: number) {
    await linksApi.remove(id);
    await load();
  }
  async function addFile() {
    if (!newFile.label.trim() || !newFile.path.trim()) return;
    await filesApi.create(project.id, newFile.label.trim(), newFile.path.trim());
    newFile = { label: "", path: "" };
    await load();
  }
  async function delFile(id: number) {
    await filesApi.remove(id);
    await load();
  }
</script>

<div class="stack">
  {#if project.description}
    <div>
      <h3 class="section-title">Описание</h3>
      <p class="desc">{project.description}</p>
    </div>
  {/if}

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
