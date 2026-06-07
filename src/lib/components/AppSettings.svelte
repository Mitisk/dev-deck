<script lang="ts">
  import { showSettings } from "$lib/stores/ui";
  import { loadProjects } from "$lib/stores/projects";
  import { pushToast } from "$lib/stores/toasts";
  import * as backup from "$lib/api/backup";
  import * as transfer from "$lib/api/transfer";
  import * as security from "$lib/api/security";
  import type { BackupInfo, CryptoStatus } from "$lib/types";
  import Icon from "./Icon.svelte";

  let backups = $state<BackupInfo[]>([]);
  let includeSecrets = $state(false);
  let importText = $state("");
  let busy = $state(false);

  let crypto = $state<CryptoStatus>({ mode: "dpapi", locked: false });
  let pwd = $state("");
  let pwd2 = $state("");

  $effect(() => {
    if ($showSettings) { refreshBackups(); refreshCrypto(); }
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

  async function doBackup() {
    busy = true;
    try { const p = await backup.backupNow(); pushToast("Бэкап создан", p, "ok"); await refreshBackups(); }
    finally { busy = false; }
  }
  async function doExport() {
    busy = true;
    try { const p = await transfer.exportToFile(includeSecrets); pushToast("Экспортировано", p, "ok"); }
    finally { busy = false; }
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
          <button class="btn-ghost" disabled={busy} onclick={doExport}><Icon name="download" class="ic-sm" /> Экспорт в файл (exports/)</button>
        </div>

        <div class="field">
          <label for="imp">Импорт (вставьте JSON)</label>
          <textarea id="imp" class="tin mono" style="min-height:120px" placeholder={'{ "version": 1, "projects": [...] }'} bind:value={importText}></textarea>
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
