<script lang="ts">
  import { showSettings } from "$lib/stores/ui";
  import { loadProjects } from "$lib/stores/projects";
  import { pushToast } from "$lib/stores/toasts";
  import * as backup from "$lib/api/backup";
  import * as transfer from "$lib/api/transfer";
  import * as security from "$lib/api/security";
  import { user, loadUser } from "$lib/stores/user";
  import { features, toggleFeature } from "$lib/stores/features";
  import * as userApi from "$lib/api/user";
  import type { BackupInfo, CryptoStatus } from "$lib/types";
  import { save, open } from "@tauri-apps/plugin-dialog";
  import Icon from "./Icon.svelte";

  let backups = $state<BackupInfo[]>([]);
  let includeSecrets = $state(false);
  let importText = $state("");
  let busy = $state(false);

  let crypto = $state<CryptoStatus>({ mode: "dpapi", locked: false });
  let pwd = $state("");
  let pwd2 = $state("");

  let uName = $state("");
  let uHandle = $state("");
  async function saveProfile() {
    busy = true;
    try {
      await userApi.set(uName, uHandle || null);
      await loadUser();
      pushToast("Профиль сохранён", uName.trim() || "Пользователь", "ok");
    } finally { busy = false; }
  }

  $effect(() => {
    if ($showSettings) { refreshBackups(); refreshCrypto(); uName = $user.name; uHandle = $user.handle ?? ""; }
  });
  async function refreshBackups() {
    try { backups = await backup.backupsList(); } catch { /* */ }
  }
  async function refreshCrypto() {
    try { crypto = await security.status(); } catch { /* */ }
  }

  async function enableMaster() {
    if (pwd.length < 4) { pushToast("Пароль короткий", "минимум 4 символа", "error"); return; }
    if (pwd !== pwd2) { pushToast("Пароли не совпадают", "", "error"); return; }
    busy = true;
    try { await security.enable(pwd); pwd = ""; pwd2 = ""; pushToast("Мастер-пароль включён", "секреты перешифрованы", "ok"); await refreshCrypto(); }
    finally { busy = false; }
  }
  async function unlockMaster() {
    busy = true;
    try { await security.unlock(pwd); pwd = ""; pushToast("Разблокировано", "", "ok"); await refreshCrypto(); }
    finally { busy = false; }
  }
  async function disableMaster() {
    busy = true;
    try { await security.disable(pwd); pwd = ""; pushToast("Мастер-пароль выключен", "секреты на DPAPI", "ok"); await refreshCrypto(); }
    finally { busy = false; }
  }
  async function lockMaster() { await security.lock(); await refreshCrypto(); pushToast("Заблокировано", "", "info"); }

  let chOld = $state("");
  let chNew = $state("");
  async function changeMaster() {
    if (chNew.length < 4) { pushToast("Пароль короткий", "минимум 4 символа", "error"); return; }
    busy = true;
    try { await security.change(chOld, chNew); chOld = ""; chNew = ""; pushToast("Пароль изменён", "секреты перешифрованы", "ok"); }
    finally { busy = false; }
  }

  async function doBackup() {
    busy = true;
    try { const p = await backup.backupNow(); pushToast("Бэкап создан", p, "ok"); await refreshBackups(); }
    finally { busy = false; }
  }
  async function doExport() {
    const path = await save({ defaultPath: "devdeck-export.json", filters: [{ name: "JSON", extensions: ["json"] }] });
    if (!path) return;
    busy = true;
    try { const p = await transfer.exportToPath(path, includeSecrets); pushToast("Экспортировано", p, "ok"); }
    finally { busy = false; }
  }
  async function doImportFile() {
    const sel = await open({ multiple: false, filters: [{ name: "JSON", extensions: ["json"] }] });
    if (!sel || Array.isArray(sel)) return;
    busy = true;
    try {
      const r = await transfer.importFromPath(sel);
      pushToast("Импортировано", `${r.projects} проект(ов)`, "ok");
      await loadProjects();
    } finally { busy = false; }
  }
  async function doImport() {
    if (!importText.trim()) return;
    busy = true;
    try {
      const r = await transfer.importJson(importText);
      pushToast("Импортировано", `${r.projects} проект(ов)`, "ok");
      importText = "";
      await loadProjects();
    } finally { busy = false; }
  }
</script>

{#if $showSettings}
  <div class="modal-scrim open" role="dialog" tabindex="-1" aria-label="Настройки"
       onmousedown={(e) => { if (e.currentTarget === e.target) showSettings.set(false); }}
       onkeydown={(e) => { if (e.key === 'Escape') showSettings.set(false); }}>
    <div class="modal" style="max-width:560px">
      <div class="modal-head">
        <span class="mh-ico"><Icon name="settings" class="ic" /></span>
        <span class="t">Настройки · Данные</span>
        <button class="icon-btn x" onclick={() => showSettings.set(false)}><Icon name="x" class="ic" /></button>
      </div>
      <div class="modal-body">
        <div class="field">
          <label>Профиль</label>
          <div class="set-grid">
            <div class="field"><label for="u-name">Имя</label><input id="u-name" class="tin" placeholder="Ваше имя" bind:value={uName} /></div>
            <div class="field"><label for="u-handle">Хэндл <span style="color:var(--muted-2)">(необязательно)</span></label><input id="u-handle" class="tin mono" placeholder="@nick" bind:value={uHandle} /></div>
          </div>
          <div style="margin-top:8px"><button class="btn-ghost" disabled={busy} onclick={saveProfile}><Icon name="user" class="ic-sm" /> Сохранить профиль</button></div>
        </div>

        <div class="field">
          <label>Разделы проекта</label>
          <div style="color:var(--muted);font-size:12px;margin-bottom:8px">Выключенные вкладки скрываются в карточке проекта.</div>
          <div class="toggle-row">
            <button class="toggle" class:on={$features.tasks} onclick={() => toggleFeature("tasks")} aria-pressed={$features.tasks} aria-label="Задачи"></button>
            <div class="tl">Задачи<small>Доска задач проекта.</small></div>
          </div>
          <div class="toggle-row" style="margin-top:8px">
            <button class="toggle" class:on={$features.checklists} onclick={() => toggleFeature("checklists")} aria-pressed={$features.checklists} aria-label="Чеклисты"></button>
            <div class="tl">Чеклисты<small>Списки шагов и подготовки.</small></div>
          </div>
          <div class="toggle-row" style="margin-top:8px">
            <button class="toggle" class:on={$features.creds} onclick={() => toggleFeature("creds")} aria-pressed={$features.creds} aria-label="Креды"></button>
            <div class="tl">Креды<small>Секреты и доступы проекта.</small></div>
          </div>
        </div>

        <div class="field">
          <label>Безопасность секретов</label>
          <div style="color:var(--muted);font-size:12px;margin-bottom:8px">
            Режим: <b style="color:var(--text)">{crypto.mode === "master" ? "Мастер-пароль" : "DPAPI (Windows)"}</b>
            {#if crypto.mode === "master"} · {crypto.locked ? "🔒 заблокировано" : "🔓 разблокировано"}{/if}
          </div>

          {#if crypto.mode === "dpapi"}
            <div style="display:flex;flex-direction:column;gap:6px">
              <input class="tin" type="password" placeholder="Новый мастер-пароль" bind:value={pwd} />
              <input class="tin" type="password" placeholder="Повторите пароль" bind:value={pwd2} />
              <div><button class="btn-ghost" disabled={busy} onclick={enableMaster}><Icon name="lock" class="ic-sm" /> Включить мастер-пароль</button></div>
              <small style="color:var(--muted-2)">Секреты будут шифроваться AES-256-GCM ключом из пароля (Argon2id). Забытый пароль = секреты не восстановить.</small>
            </div>
          {:else if crypto.locked}
            <div style="display:flex;gap:6px;align-items:center">
              <input class="tin" type="password" placeholder="Мастер-пароль" bind:value={pwd} onkeydown={(e) => { if (e.key==='Enter') unlockMaster(); }} />
              <button class="btn-primary" disabled={busy} onclick={unlockMaster}><Icon name="lock-open" class="ic-sm" /> Разблокировать</button>
            </div>
          {:else}
            <div style="display:flex;flex-direction:column;gap:6px">
              <button class="btn-ghost" onclick={lockMaster}><Icon name="lock" class="ic-sm" /> Заблокировать сейчас</button>
              <div style="display:flex;gap:6px;align-items:center">
                <input class="tin" type="password" placeholder="Текущий пароль" bind:value={pwd} />
                <button class="btn-danger" disabled={busy} onclick={disableMaster}>Выключить мастер-пароль</button>
              </div>
              <div style="display:flex;gap:6px;align-items:center;margin-top:6px">
                <input class="tin" type="password" placeholder="Старый пароль" bind:value={chOld} />
                <input class="tin" type="password" placeholder="Новый пароль" bind:value={chNew} />
                <button class="btn-ghost" disabled={busy} onclick={changeMaster}><Icon name="key-round" class="ic-sm" /> Сменить</button>
              </div>
            </div>
          {/if}
        </div>

        <div class="field">
          <label>Бэкап</label>
          <div style="display:flex;align-items:center;gap:10px">
            <button class="btn-ghost" disabled={busy} onclick={doBackup}><Icon name="database-backup" class="ic-sm" /> Сделать бэкап</button>
            <span style="color:var(--muted);font-size:12px">{backups.length} копий в backups/</span>
          </div>
        </div>

        <div class="field">
          <label>Экспорт</label>
          <div class="toggle-row" style="margin-bottom:8px">
            <button class="toggle" class:on={includeSecrets} onclick={() => (includeSecrets = !includeSecrets)} aria-pressed={includeSecrets} aria-label="секреты"></button>
            <div class="tl">Включить секреты<small>Расшифровать и положить в файл в открытом виде — только для переноса на доверенный ПК.</small></div>
          </div>
          <button class="btn-ghost" disabled={busy} onclick={doExport}><Icon name="download" class="ic-sm" /> Экспорт в файл…</button>
        </div>

        <div class="field">
          <label for="imp">Импорт</label>
          <div style="margin-bottom:8px"><button class="btn-ghost" disabled={busy} onclick={doImportFile}><Icon name="folder-open" class="ic-sm" /> Импорт из файла…</button></div>
          <textarea id="imp" class="tin mono" style="min-height:120px" placeholder={'или вставьте JSON: { "version": 1, "projects": [...] }'} bind:value={importText}></textarea>
          <div style="margin-top:8px"><button class="btn-primary" disabled={busy || !importText.trim()} onclick={doImport}><Icon name="upload" class="ic-sm" /> Импортировать</button></div>
        </div>
      </div>
      <div class="modal-foot">
        <span class="spacer"></span>
        <button class="btn-ghost" onclick={() => showSettings.set(false)}>Закрыть</button>
      </div>
    </div>
  </div>
{/if}
