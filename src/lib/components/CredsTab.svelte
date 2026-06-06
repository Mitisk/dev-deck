<script lang="ts">
  import type { Project, Credential, CredType } from "$lib/types";
  import * as creds from "$lib/api/creds";
  import { copy, copySecret } from "$lib/clipboard";
  import Icon from "./Icon.svelte";

  let { project }: { project: Project } = $props();

  const TYPES: { value: CredType; label: string }[] = [
    { value: "login", label: "Логин/пароль" },
    { value: "api_key", label: "API-ключ" },
    { value: "token", label: "Токен" },
    { value: "ssh", label: "SSH" },
    { value: "conn_string", label: "Строка подключения" },
    { value: "note", label: "Заметка" },
  ];
  function typeLabel(t: string): string {
    return TYPES.find((x) => x.value === t)?.label ?? t;
  }

  let items = $state<Credential[]>([]);
  let query = $state("");
  let revealed = $state<Record<number, string>>({}); // id -> расшифрованный секрет (показанный)

  const filtered = $derived(
    items.filter((c) => {
      const q = query.toLowerCase();
      return (
        !q ||
        c.label.toLowerCase().includes(q) ||
        (c.username ?? "").toLowerCase().includes(q) ||
        (c.url ?? "").toLowerCase().includes(q) ||
        typeLabel(c.type).toLowerCase().includes(q)
      );
    }),
  );

  let reqId = 0;
  async function load() {
    const my = ++reqId;
    try {
      const r = await creds.list(project.id);
      if (my === reqId) {
        items = r;
        revealed = {};
      }
    } catch {
      /* тост из api/client.ts */
    }
  }
  $effect(() => {
    project.id;
    load();
  });

  async function toggleReveal(id: number) {
    if (revealed[id] !== undefined) {
      const { [id]: _omit, ...rest } = revealed;
      revealed = rest;
      return;
    }
    const secret = await creds.getSecret(id);
    revealed = { ...revealed, [id]: secret };
  }
  async function copyTheSecret(id: number) {
    const secret = revealed[id] ?? (await creds.getSecret(id));
    await copySecret(secret);
  }

  // dialog
  let editing = $state<Credential | null>(null);
  let isNew = $state(false);
  let fLabel = $state(""), fType = $state<CredType>("login"), fUser = $state(""), fUrl = $state(""), fNotes = $state(""), fSecret = $state("");

  function openNew() {
    isNew = true;
    editing = { id: 0, projectId: project.id, label: "", type: "login", username: null, url: null, notes: null, sortOrder: 0, hasSecret: false };
    fLabel = ""; fType = "login"; fUser = ""; fUrl = ""; fNotes = ""; fSecret = "";
  }
  function openEdit(c: Credential) {
    isNew = false;
    editing = c;
    fLabel = c.label; fType = c.type; fUser = c.username ?? ""; fUrl = c.url ?? ""; fNotes = c.notes ?? ""; fSecret = "";
  }
  async function save() {
    if (!editing) return;
    const label = fLabel.trim();
    if (!label) return;
    const input = {
      label,
      type: fType,
      username: fUser.trim() || null,
      url: fUrl.trim() || null,
      notes: fNotes.trim() || null,
      // при редактировании пустой секрет = «не менять» (undefined); при создании — задать
      secret: isNew ? (fSecret || null) : fSecret ? fSecret : undefined,
    };
    if (isNew) await creds.create(project.id, input);
    else await creds.update(editing.id, input);
    editing = null;
    await load();
  }
  async function del() {
    if (!editing) return;
    const id = editing.id;
    editing = null;
    await creds.remove(id);
    await load();
  }
</script>

<div style="display:flex;align-items:center;gap:10px;margin-bottom:14px">
  <div class="sb-search" style="margin:0;flex:1;max-width:320px">
    <Icon name="search" class="ic" />
    <input placeholder="Поиск кредов…" bind:value={query} />
  </div>
  <span style="flex:1"></span>
  <button class="btn-primary" onclick={openNew}><Icon name="plus" class="ic ic-sm" /> Добавить</button>
</div>

<div class="creds">
  {#each filtered as c (c.id)}
    <div class="card cred">
      <div class="cred-head">
        <span class="t">{c.label}</span>
        <span class="badge-type" style="color:var(--accent);background:var(--accent-soft)">{typeLabel(c.type)}</span>
        <button class="mini cred-del" style="margin-left:auto" title="Редактировать" onclick={() => openEdit(c)}>
          <Icon name="pencil" class="ic-sm" />
        </button>
      </div>
      {#if c.username}
        <div class="cred-row">
          <span class="k">Логин</span>
          <span class="val">{c.username}</span>
          <span class="acts"><button class="mini" title="Копировать" onclick={() => copy(c.username ?? "")}><Icon name="copy" class="ic-sm" /></button></span>
        </div>
      {/if}
      {#if c.url}
        <div class="cred-row">
          <span class="k">URL</span>
          <span class="val">{c.url}</span>
          <span class="acts"><button class="mini" title="Копировать" onclick={() => copy(c.url ?? "")}><Icon name="copy" class="ic-sm" /></button></span>
        </div>
      {/if}
      {#if c.hasSecret}
        <div class="cred-row">
          <span class="k">Секрет</span>
          <span class="val secret">{revealed[c.id] ?? "••••••••••••"}</span>
          <span class="acts">
            <button class="mini" title={revealed[c.id] !== undefined ? "Скрыть" : "Показать"} onclick={() => toggleReveal(c.id)}>
              <Icon name={revealed[c.id] !== undefined ? "eye-off" : "eye"} class="ic-sm" />
            </button>
            <button class="mini" title="Копировать секрет (буфер очистится)" onclick={() => copyTheSecret(c.id)}>
              <Icon name="copy" class="ic-sm" />
            </button>
          </span>
        </div>
      {/if}
      {#if c.notes}<div class="cred-row"><span class="k">Заметка</span><span class="val">{c.notes}</span></div>{/if}
    </div>
  {/each}

  {#if !filtered.length}
    <button class="add-cred" onclick={openNew}><Icon name="plus" class="ic-sm" /> {items.length ? "Ничего не найдено" : "Добавить первый кред"}</button>
  {/if}
</div>

{#if editing}
  <div class="modal-scrim open" role="dialog" tabindex="-1" aria-label="Кред"
       onmousedown={(e) => { if (e.currentTarget === e.target) (editing = null); }}
       onkeydown={(e) => { if (e.key === "Escape") (editing = null); }}>
    <div class="modal">
      <div class="modal-head">
        <span class="t">{isNew ? "Новый кред" : "Кред"}</span>
        <button class="icon-btn x" onclick={() => (editing = null)}><Icon name="x" class="ic" /></button>
      </div>
      <div class="modal-body">
        <div class="set-grid">
          <div class="field"><label for="c-label">Название</label><input id="c-label" class="tin" bind:value={fLabel} /></div>
          <div class="field"><label for="c-type">Тип</label>
            <select id="c-type" class="tin" bind:value={fType}>
              {#each TYPES as t}<option value={t.value}>{t.label}</option>{/each}
            </select>
          </div>
          <div class="field"><label for="c-user">Логин / хост</label><input id="c-user" class="tin" bind:value={fUser} /></div>
          <div class="field"><label for="c-url">URL</label><input id="c-url" class="tin mono" bind:value={fUrl} /></div>
        </div>
        <div class="field"><label for="c-secret">Секрет {#if !isNew}<span style="color:var(--muted-2)">(пусто = не менять)</span>{/if}</label>
          <input id="c-secret" class="tin mono" type="password" bind:value={fSecret} autocomplete="off" /></div>
        <div class="field"><label for="c-notes">Заметка</label><textarea id="c-notes" class="tin" bind:value={fNotes}></textarea></div>
      </div>
      <div class="modal-foot">
        {#if !isNew}<button class="btn-danger" onclick={del}><Icon name="trash-2" class="ic-sm" /> Удалить</button>{/if}
        <span class="spacer"></span>
        <button class="btn-ghost" onclick={() => (editing = null)}>Отмена</button>
        <button class="btn-primary" onclick={save}><Icon name="check" class="ic ic-sm" /> Сохранить</button>
      </div>
    </div>
  </div>
{/if}
