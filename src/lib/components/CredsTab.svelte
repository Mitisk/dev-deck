<script lang="ts">
  import type { Project, Credential, CredType } from "$lib/types";
  import * as creds from "$lib/api/creds";
  import { copy, copySecret } from "$lib/clipboard";
  import { pushToast } from "$lib/stores/toasts";
  import Icon from "./Icon.svelte";
  import { reorderIds } from "$lib/credsOrder";
  import * as actions from "$lib/api/actions";
  import { generatePassword, passwordStrength } from "$lib/password";
  import { open } from "@tauri-apps/plugin-dialog";

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

  async function getSecretSafe(id: number): Promise<string | null> {
    try { return await creds.getSecret(id); }
    catch (e) {
      if ((e as { kind?: string }).kind === "locked") pushToast("Заблокировано", "Разблокируйте мастер-паролем в Настройках", "error");
      return null;
    }
  }

  async function toggleReveal(id: number) {
    if (revealed[id] !== undefined) {
      const { [id]: _omit, ...rest } = revealed;
      revealed = rest;
      return;
    }
    const secret = await getSecretSafe(id);
    if (secret === null) return;
    revealed = { ...revealed, [id]: secret };
  }
  async function copyTheSecret(id: number) {
    let secret = revealed[id];
    if (secret === undefined) {
      const s = await getSecretSafe(id);
      if (s === null) return;
      secret = s;
    }
    await copySecret(secret);
  }

  // dialog
  let editing = $state<Credential | null>(null);
  let isNew = $state(false);
  let fLabel = $state(""), fType = $state<CredType>("login"), fUser = $state(""), fUrl = $state(""), fNotes = $state(""), fSecret = $state("");
  let fKeyPath = $state("");
  let fStartupCmd = $state("");
  let fGlobal = $state(false);
  let genLen = $state(20);

  // Видимость полей и подписи зависят от типа креда.
  const showUser = $derived(fType === "login" || fType === "ssh");
  const showUrl = $derived(fType === "login" || fType === "api_key" || fType === "token" || fType === "ssh");
  const showKey = $derived(fType === "ssh");
  const showSecret = $derived(fType !== "note");
  const urlLabel = $derived(
    fType === "api_key" ? "Endpoint" : fType === "ssh" ? "Хост" : fType === "login" ? "Сайт" : "URL",
  );
  const secretLabel = $derived(
    fType === "api_key" ? "Ключ" : fType === "token" ? "Токен" : fType === "conn_string" ? "Строка подключения" : "Пароль",
  );

  async function toggleGlobal() {
    if (!editing || isNew) return;
    const next = !fGlobal;
    await creds.setGlobal(editing.id, next);
    fGlobal = next;
    await load();
  }

  async function pickKey() {
    const sel = await open({
      multiple: false,
      filters: [
        { name: "Ключи", extensions: ["ppk", "pem", "key"] },
        { name: "Все файлы", extensions: ["*"] },
      ],
    });
    if (sel && !Array.isArray(sel)) fKeyPath = sel;
  }

  function openNew() {
    isNew = true;
    editing = { id: 0, projectId: project.id, label: "", type: "login", username: null, url: null, notes: null, sortOrder: 0, hasSecret: false, keyPath: null, isGlobal: false, startupCmd: null };
    fLabel = ""; fType = "login"; fUser = ""; fUrl = ""; fNotes = ""; fSecret = ""; fKeyPath = ""; fStartupCmd = ""; fGlobal = false;
  }
  function openEdit(c: Credential) {
    isNew = false;
    editing = c;
    fLabel = c.label; fType = c.type; fUser = c.username ?? ""; fUrl = c.url ?? ""; fNotes = c.notes ?? ""; fSecret = ""; fKeyPath = c.keyPath ?? ""; fStartupCmd = c.startupCmd ?? ""; fGlobal = c.isGlobal;
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
      keyPath: fKeyPath.trim() || null,
      // команда имеет смысл только для ssh; для других типов не сохраняем
      startupCmd: fType === "ssh" ? fStartupCmd.trim() || null : null,
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

  async function openPutty(id: number) {
    try {
      await actions.launchPutty(id);
    } catch (e) {
      if ((e as { kind?: string }).kind === "locked")
        pushToast("Заблокировано", "Разблокируйте мастер-паролем в Настройках", "error");
      // прочие ошибки уже показал api/client.ts
    }
  }

  // --- drag-and-drop сортировка (только при пустом поиске) ---
  let dragId = $state<number | null>(null);
  let overId = $state<number | null>(null); // карточка под курсором
  let overAfter = $state(false);            // курсор в правой половине → вставка после

  function onGripDragStart(e: DragEvent, id: number) {
    dragId = id;
    if (e.dataTransfer) {
      e.dataTransfer.effectAllowed = "move";
      e.dataTransfer.setData("text/plain", String(id));
    }
  }
  function onCardDragOver(e: DragEvent, id: number) {
    if (dragId === null) return;
    e.preventDefault();
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    overId = id;
    overAfter = e.clientX >= rect.left + rect.width / 2;
  }
  function clearDrag() {
    dragId = null;
    overId = null;
    overAfter = false;
  }
  // Применить перестановку: dragId встаёт перед beforeId (null → в конец).
  async function applyReorder(beforeId: number | null) {
    if (dragId === null || query) { clearDrag(); return; }
    const ids = items.map((c) => c.id);
    const next = reorderIds(ids, dragId, beforeId);
    clearDrag();
    if (next.join(",") === ids.join(",")) return; // порядок не изменился — no-op
    const byId = new Map(items.map((c) => [c.id, c]));
    items = next.map((id) => byId.get(id)!); // оптимистично
    try {
      await creds.reorder(project.id, next);
    } catch {
      await load(); // откат к серверному порядку (тост ошибки уже из client.ts)
    }
  }
  // Бросок на карточку: вставка перед ней (левая половина) или после (правая).
  function commitDrop() {
    if (dragId === null || overId === null) { clearDrag(); return; }
    const ids = items.map((c) => c.id);
    let beforeId: number | null;
    if (!overAfter) {
      beforeId = overId;
    } else {
      const i = ids.indexOf(overId);
      beforeId = i >= 0 && i + 1 < ids.length ? ids[i + 1] : null;
    }
    void applyReorder(beforeId);
  }
  // Бросок в пустую область грида (после всех карточек) → в конец.
  function commitDropEnd(e: DragEvent) {
    if (dragId === null) return;
    if (e.currentTarget !== e.target) return; // сработало пузырьком от карточки — игнор
    void applyReorder(null);
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

<div class="creds" role="list"
     ondragover={(e) => { if (dragId !== null) e.preventDefault(); }}
     ondrop={commitDropEnd}>
  {#each filtered as c (c.id)}
    <div class="card cred"
         class:drop-before={dragId !== null && dragId !== c.id && overId === c.id && !overAfter}
         class:drop-after={dragId !== null && dragId !== c.id && overId === c.id && overAfter}
         role="listitem"
         ondragover={(e) => onCardDragOver(e, c.id)}
         ondrop={commitDrop}>
      <div class="cred-head">
        {#if !query}
          <button class="cred-grip" type="button" draggable={true} title="Перетащите, чтобы изменить порядок"
                  ondragstart={(e) => onGripDragStart(e, c.id)} ondragend={clearDrag}>
            <Icon name="grip-vertical" class="ic-sm" />
          </button>
        {/if}
        <span class="t">{c.label}</span>
        <span class="badge-type" style="color:var(--accent);background:var(--accent-soft)">{typeLabel(c.type)}</span>
        {#if c.isGlobal}<span class="cred-global" title="Во всех проектах"><Icon name="pin" class="ic-sm" /></span>{/if}
        <span class="cred-acts">
          {#if c.url || c.username}
            <button class="mini" title="Открыть в PuTTY" onclick={() => openPutty(c.id)}><Icon name="square-terminal" class="ic-sm" /></button>
          {/if}
          <button class="mini cred-del" title="Редактировать" onclick={() => openEdit(c)}><Icon name="pencil" class="ic-sm" /></button>
        </span>
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
      {#if c.keyPath}
        <div class="cred-row">
          <span class="k">Ключ</span>
          <span class="val mono">{c.keyPath}</span>
          <span class="acts"><button class="mini" title="Копировать" onclick={() => copy(c.keyPath ?? '')}><Icon name="copy" class="ic-sm" /></button></span>
        </div>
      {/if}
      {#if c.startupCmd}
        <div class="cred-row">
          <span class="k">Команда</span>
          <span class="val mono">{c.startupCmd}</span>
          <span class="acts"><button class="mini" title="Копировать" onclick={() => copy(c.startupCmd ?? '')}><Icon name="copy" class="ic-sm" /></button></span>
        </div>
      {/if}
      {#if c.notes}
        <div class="cred-row">
          <span class="k">Заметка</span>
          <span class="val">{c.notes}</span>
          <span class="acts"><button class="mini" title="Копировать" onclick={() => copy(c.notes ?? "")}><Icon name="copy" class="ic-sm" /></button></span>
        </div>
      {/if}
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
          {#if showUser}<div class="field"><label for="c-user">Логин</label><input id="c-user" class="tin" bind:value={fUser} /></div>{/if}
          {#if showUrl}<div class="field"><label for="c-url">{urlLabel}</label><input id="c-url" class="tin mono" bind:value={fUrl} /></div>{/if}
          {#if showKey}
            <div class="field"><label for="c-key">Путь к ключу (.ppk)</label>
              <div class="key-row">
                <input id="c-key" class="tin mono" placeholder="C:\keys\id.ppk" bind:value={fKeyPath} />
                <button class="btn-ghost" type="button" onclick={pickKey} title="Выбрать файл"><Icon name="folder-open" class="ic-sm" /></button>
              </div>
            </div>
            <div class="field"><label for="c-startup">Команда при запуске <span style="color:var(--muted-2)">(выполнится на сервере сразу после входа)</span></label>
              <input id="c-startup" class="tin mono" placeholder="cd /var/www" bind:value={fStartupCmd} />
            </div>
          {/if}
        </div>
        {#if showSecret}
        <div class="field"><label for="c-secret">{secretLabel} {#if !isNew}<span style="color:var(--muted-2)">(пусто = не менять)</span>{/if}</label>
          <div class="secret-row">
            <input id="c-secret" class="tin mono" type="password" bind:value={fSecret} autocomplete="off" />
            <input class="tin gen-len" type="number" min="8" max="64" bind:value={genLen} aria-label="Длина пароля" title="Длина" />
            <button class="btn-ghost" type="button" onclick={() => (fSecret = generatePassword(genLen))} title="Сгенерировать пароль"><Icon name="dices" class="ic-sm" /> Сгенерировать</button>
          </div>
          {#if fSecret}
            {@const score = passwordStrength(fSecret)}
            <div class="pw-meter" data-score={score}>
              {#each [1, 2, 3, 4] as seg}<span class="seg" class:on={score >= seg}></span>{/each}
            </div>
          {/if}
        </div>
        {/if}
        <div class="field"><label for="c-notes">Заметка</label><textarea id="c-notes" class="tin" bind:value={fNotes}></textarea></div>
        {#if !isNew}
          <div class="field">
            <div class="toggle-row">
              <button class="toggle" class:on={fGlobal} onclick={toggleGlobal} aria-pressed={fGlobal} aria-label="Во всех проектах"></button>
              <div class="tl">Показывать во всех проектах<small>Одна и та же запись появится в каждом проекте; правка и удаление — везде.</small></div>
            </div>
          </div>
        {/if}
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
