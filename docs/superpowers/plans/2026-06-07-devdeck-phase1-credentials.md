# DevDeck Phase 1 — Срез «Креды (DPAPI)» — план реализации

> **Для исполнителя:** ОБЯЗАТЕЛЬНЫЙ ПОД-НАВЫК: superpowers:subagent-driven-development. Шаги — чекбоксы.

**Цель:** Вкладка «Креды» — несколько кредов на проект с типизацией (логин/пароль, API-ключ, токен, SSH, строка подключения, заметка). Секрет хранится **зашифрованным через Windows DPAPI**; в UI замаскирован, есть «показать» и «копировать в один клик» (с авто-очисткой буфера). Несекретные поля (логин, URL) видны и копируются. Поиск по кредам внутри проекта.

**Архитектура:** Модуль `crypto` шифрует/дешифрует через DPAPI (`CryptProtectData`/`CryptUnprotectData`). Секрет в БД — `secret_encrypted` BLOB. `creds_list` отдаёт метаданные БЕЗ секрета (только `hasSecret`); секрет достаётся отдельной командой `creds_get_secret(id)` по запросу (показать/копировать). Команды `creds_*` лочат БД, шифрование — в `crypto`.

**Безопасность:**
- Секрет шифруется DPAPI (привязка к учётке Windows; без мастер-пароля — режим по умолчанию из ТЗ раздел 7). Мастер-пароль (AES-GCM+Argon2) — отдельный срез v1.0.
- В `creds_list` секрет НЕ возвращается; в открытом виде — только через `creds_get_secret` по явному действию.
- Секрет не логируется. Копирование в буфер — с авто-очисткой через N секунд.

**Решения / границы:**
- Один секрет на кред (по схеме БД: `secret_encrypted` + несекретные `username`/`url`/`notes`). Мульти-поля прототипа не воспроизводим.
- Мастер-пароль — отложен (v1.0). Сейчас только DPAPI.

**Источники:** `TZ_DevDeck.md` (5.2 — креды; 7 — безопасность/DPAPI; 10 — `creds_list/get_secret/create/update/delete`). `_prototype/index.html` (`.creds`/`.cred`/`.cred-row`/`.badge-type` — разметка-референс). Схема `credentials` в миграции 0001.

---

## Контекст

- Rust: 31 команда; `models.rs`, `error::{AppError,ErrorKind,AppResult}`, `state::AppState`, `commands/{...}.rs`. Таблица `credentials`: `id, project_id, label, type, username, url, secret_encrypted BLOB, notes, sort_order, created_at, updated_at` (FK ON DELETE CASCADE).
- Фронт: `ProjectView.svelte` рендерит вкладки; `creds` — плейсхолдер. api `client.ts`. `Icon.svelte`. global.css: `.creds`/`.cred`/`.cred-head`/`.badge-type`/`.cred-row`(`.k`/`.val`(`.secret`)/`.acts`)/`.mini`/`.add-cred`/`.cred-del`; модалка `.modal-*`/`.field`/`.tin`/`.set-grid`.

---

## Структура файлов

```
src-tauri/
├─ Cargo.toml             # MOD: + windows (DPAPI features)
└─ src/
   ├─ crypto.rs           # NEW: encrypt/decrypt через DPAPI + round-trip тест
   ├─ models.rs           # MOD: + Credential, CredInput
   ├─ lib.rs              # MOD: mod crypto; регистрация команд
   └─ commands/
      ├─ mod.rs           # MOD: + pub mod creds;
      └─ creds.rs         # NEW: creds_* + тест create→get_secret

src/lib/
├─ types.ts               # MOD: + Credential, CredType
├─ clipboard.ts           # NEW: copy + copySecret(auto-clear)
├─ api/creds.ts           # NEW
└─ components/
   ├─ CredsTab.svelte     # NEW
   └─ ProjectView.svelte  # MOD: рендер CredsTab на табе "creds"
```

---

## Task 1: Rust — DPAPI-крипто

**Files:** Modify `src-tauri/Cargo.toml`, `src-tauri/src/lib.rs`; Create `src-tauri/src/crypto.rs`.

- [ ] **Step 1: Зависимость windows (DPAPI)**

Проверить, какую версию crate `windows` уже тянет Tauri:
```powershell
Select-String -Path src-tauri/Cargo.lock -Pattern 'name = "windows"' -Context 0,1
```
В `src-tauri/Cargo.toml` `[dependencies]` добавить `windows` ТОЙ ЖЕ мажорной версии (чтобы не плодить второй экземпляр), с фичами крипто/foundation. Пример (подставить актуальную версию из lock, напр. 0.61):
```toml
windows = { version = "0.61", features = ["Win32_Security_Cryptography", "Win32_Foundation"] }
```

- [ ] **Step 2: Модуль crypto.rs**

Create `src-tauri/src/crypto.rs`. Реализовать `encrypt(&[u8]) -> AppResult<Vec<u8>>` и `decrypt(&[u8]) -> AppResult<Vec<u8>>` через DPAPI. Шаблон (ВАЖНО: точные сигнатуры `CryptProtectData`/`CryptUnprotectData`/`LocalFree` и типы Option/указателей зависят от версии crate `windows` — привести в соответствие с актуальной версией; критерий правильности — проходящий round-trip тест ниже):
```rust
use crate::error::{AppError, AppResult};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{LocalFree, HLOCAL};
use windows::Win32::Security::Cryptography::{CryptProtectData, CryptUnprotectData, CRYPT_INTEGER_BLOB};

fn dpapi(
    f: unsafe fn(*const CRYPT_INTEGER_BLOB, PCWSTR, Option<*const CRYPT_INTEGER_BLOB>, Option<*const core::ffi::c_void>, Option<*const windows::Win32::Security::Cryptography::CRYPTPROTECT_PROMPTSTRUCT>, u32, *mut CRYPT_INTEGER_BLOB) -> windows::core::Result<()>,
    data: &[u8],
    what: &str,
) -> AppResult<Vec<u8>> {
    unsafe {
        let mut input = CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr() as *mut u8 };
        let mut output = CRYPT_INTEGER_BLOB::default();
        f(&mut input, PCWSTR::null(), None, None, None, 0, &mut output)
            .map_err(|e| AppError::internal(format!("DPAPI {}: {}", what, e)))?;
        let out = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        let _ = LocalFree(Some(HLOCAL(output.pbData as *mut core::ffi::c_void)));
        Ok(out)
    }
}

/// Зашифровать секрет (DPAPI, привязка к учётке Windows).
pub fn encrypt(plain: &[u8]) -> AppResult<Vec<u8>> {
    dpapi(CryptProtectData, plain, "encrypt")
}

/// Расшифровать секрет.
pub fn decrypt(cipher: &[u8]) -> AppResult<Vec<u8>> {
    dpapi(CryptUnprotectData, cipher, "decrypt")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let secret = b"sk_test_51Hx9pQ2eZvKYlo2C";
        let enc = encrypt(secret).unwrap();
        assert_ne!(enc.as_slice(), secret.as_slice(), "ciphertext must differ from plaintext");
        let dec = decrypt(&enc).unwrap();
        assert_eq!(dec.as_slice(), secret.as_slice());
    }
}
```
> Если сигнатура `CryptProtectData` в установленной версии `windows` отличается (например, optional-параметры не `Option<*const>`, или `LocalFree` принимает `HLOCAL` без `Option`/возвращает иначе) — ПОДСТРОИТЬ под фактическую сигнатуру (помогает `cargo build` с точными ошибками + документация типа). Возможно, обёртка `dpapi` с указателем на функцию не подойдёт — тогда сделать `encrypt`/`decrypt` двумя отдельными `unsafe`-блоками с прямыми вызовами `CryptProtectData`/`CryptUnprotectData`. Главное — round-trip тест зелёный.

- [ ] **Step 3: Подключить модуль**

В `src-tauri/src/lib.rs` добавить `mod crypto;` (рядом с `mod commands; mod db; mod error; mod models; mod state;`).

- [ ] **Step 4: Тест + сборка**
```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib crypto
cargo build --manifest-path src-tauri/Cargo.toml
```
Expected: `encrypt_decrypt_roundtrip` ... ok; build успешен. ЕСЛИ FFI не компилируется после разумной попытки подстройки — STOP, доложить BLOCKED с точной ошибкой компилятора и текущим кодом (не изобретать API вслепую).

- [ ] **Step 5: Commit**
```powershell
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/crypto.rs src-tauri/src/lib.rs
git commit -m @'
feat(backend): DPAPI encrypt/decrypt module

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

## Task 2: Rust — команды кредов

**Files:** Modify `models.rs`, `commands/mod.rs`, `lib.rs`; Create `commands/creds.rs`.

- [ ] **Step 1: Модели** (в `models.rs`, после `Checklist`):
```rust
/// Метаданные креда (БЕЗ секрета).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Credential {
    pub id: i64,
    pub project_id: i64,
    pub label: String,
    #[serde(rename = "type")]
    pub kind: String, // login | api_key | token | ssh | conn_string | note
    pub username: Option<String>,
    pub url: Option<String>,
    pub notes: Option<String>,
    pub sort_order: i64,
    pub has_secret: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CredInput {
    pub label: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub username: Option<String>,
    pub url: Option<String>,
    pub notes: Option<String>,
    /// None = не менять секрет (при update); Some("") = очистить; Some(x) = задать.
    pub secret: Option<String>,
}
```

- [ ] **Step 2: commands/creds.rs**

Create `src-tauri/src/commands/creds.rs`:
```rust
use crate::crypto;
use crate::error::{AppError, AppResult, ErrorKind};
use crate::models::{CredInput, Credential};
use crate::state::AppState;
use rusqlite::{params, Connection};
use std::sync::MutexGuard;
use tauri::State;

fn lock<'a>(state: &'a State<AppState>) -> AppResult<MutexGuard<'a, Connection>> {
    state.db.lock().map_err(|_| AppError::internal("db mutex poisoned"))
}

fn row_to_cred(conn: &Connection, id: i64) -> AppResult<Credential> {
    Ok(conn.query_row(
        "SELECT id, project_id, label, type, username, url, notes, sort_order,
                (secret_encrypted IS NOT NULL AND length(secret_encrypted) > 0)
         FROM credentials WHERE id = ?1",
        [id],
        |r| {
            Ok(Credential {
                id: r.get(0)?,
                project_id: r.get(1)?,
                label: r.get(2)?,
                kind: r.get(3)?,
                username: r.get(4)?,
                url: r.get(5)?,
                notes: r.get(6)?,
                sort_order: r.get(7)?,
                has_secret: r.get::<_, i64>(8)? != 0,
            })
        },
    )?)
}

fn validate(input: &CredInput) -> AppResult<()> {
    if input.label.trim().is_empty() {
        return Err(AppError { kind: ErrorKind::Validation, message: "Название креда пусто".into() });
    }
    Ok(())
}

#[tauri::command]
pub fn creds_list(state: State<AppState>, project_id: i64) -> AppResult<Vec<Credential>> {
    let conn = lock(&state)?;
    let mut stmt = conn.prepare(
        "SELECT id FROM credentials WHERE project_id = ?1 ORDER BY sort_order ASC, id ASC",
    )?;
    let ids = stmt.query_map([project_id], |r| r.get::<_, i64>(0))?;
    let mut out = Vec::new();
    for id in ids {
        out.push(row_to_cred(&conn, id?)?);
    }
    Ok(out)
}

#[tauri::command]
pub fn creds_get_secret(state: State<AppState>, id: i64) -> AppResult<String> {
    let conn = lock(&state)?;
    let blob: Option<Vec<u8>> =
        conn.query_row("SELECT secret_encrypted FROM credentials WHERE id = ?1", [id], |r| r.get(0))?;
    match blob {
        Some(b) if !b.is_empty() => {
            let plain = crypto::decrypt(&b)?;
            Ok(String::from_utf8_lossy(&plain).into_owned())
        }
        _ => Ok(String::new()),
    }
}

#[tauri::command]
pub fn creds_create(state: State<AppState>, project_id: i64, input: CredInput) -> AppResult<Credential> {
    validate(&input)?;
    let conn = lock(&state)?;
    let next: i64 = conn.query_row(
        "SELECT COALESCE(MAX(sort_order),0)+1 FROM credentials WHERE project_id=?1",
        [project_id],
        |r| r.get(0),
    )?;
    let secret_blob: Option<Vec<u8>> = match input.secret.as_deref() {
        Some(s) if !s.is_empty() => Some(crypto::encrypt(s.as_bytes())?),
        _ => None,
    };
    conn.execute(
        "INSERT INTO credentials(project_id, label, type, username, url, secret_encrypted, notes, sort_order)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            project_id,
            input.label.trim(),
            input.kind,
            input.username,
            input.url,
            secret_blob,
            input.notes,
            next,
        ],
    )?;
    row_to_cred(&conn, conn.last_insert_rowid())
}

#[tauri::command]
pub fn creds_update(state: State<AppState>, id: i64, input: CredInput) -> AppResult<Credential> {
    validate(&input)?;
    let conn = lock(&state)?;
    let n = conn.execute(
        "UPDATE credentials SET label=?2, type=?3, username=?4, url=?5, notes=?6, updated_at=datetime('now')
         WHERE id=?1",
        params![id, input.label.trim(), input.kind, input.username, input.url, input.notes],
    )?;
    if n == 0 {
        return Err(AppError { kind: ErrorKind::NotFound, message: "Кред не найден".into() });
    }
    // Секрет меняем только если передан (Some). Some("") = очистить.
    if let Some(s) = input.secret.as_deref() {
        let blob: Option<Vec<u8>> = if s.is_empty() { None } else { Some(crypto::encrypt(s.as_bytes())?) };
        conn.execute("UPDATE credentials SET secret_encrypted=?2 WHERE id=?1", params![id, blob])?;
    }
    row_to_cred(&conn, id)
}

#[tauri::command]
pub fn creds_delete(state: State<AppState>, id: i64) -> AppResult<()> {
    let conn = lock(&state)?;
    conn.execute("DELETE FROM credentials WHERE id=?1", [id])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    fn mem() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        db::migrations::run(&conn).unwrap();
        conn
    }

    #[test]
    fn create_stores_encrypted_and_get_secret_decrypts() {
        let conn = mem();
        conn.execute("INSERT INTO projects(name,status,sort_order) VALUES('P','active',0)", []).unwrap();
        let pid = conn.last_insert_rowid();

        // прямой insert через encrypt (минуя команду, которой нужен State)
        let blob = crypto::encrypt(b"super-secret").unwrap();
        conn.execute(
            "INSERT INTO credentials(project_id,label,type,secret_encrypted,sort_order) VALUES(?1,'Stripe','api_key',?2,0)",
            params![pid, blob],
        ).unwrap();
        let id = conn.last_insert_rowid();

        // секрет в БД зашифрован (не равен plaintext)
        let stored: Vec<u8> = conn.query_row("SELECT secret_encrypted FROM credentials WHERE id=?1", [id], |r| r.get(0)).unwrap();
        assert_ne!(stored.as_slice(), b"super-secret".as_slice());

        // дешифровка возвращает оригинал
        let dec = crypto::decrypt(&stored).unwrap();
        assert_eq!(String::from_utf8_lossy(&dec), "super-secret");

        // метаданные без секрета
        let cred = row_to_cred(&conn, id).unwrap();
        assert_eq!(cred.label, "Stripe");
        assert_eq!(cred.kind, "api_key");
        assert!(cred.has_secret);
    }
}
```

- [ ] **Step 3: Регистрация**
- `commands/mod.rs`: `pub mod creds;`
- `lib.rs` `generate_handler![...]`: добавить
```rust
            commands::creds::creds_list,
            commands::creds::creds_get_secret,
            commands::creds::creds_create,
            commands::creds::creds_update,
            commands::creds::creds_delete,
```

- [ ] **Step 4: Тесты + сборка**
```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib
cargo build --manifest-path src-tauri/Cargo.toml
```
Expected: `create_stores_encrypted_and_get_secret_decrypts` + `crypto::...roundtrip` + прежние (14) → 16 ok; build успешен.

- [ ] **Step 5: Commit**
```powershell
git add src-tauri/src/models.rs src-tauri/src/commands/creds.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs
git commit -m @'
feat(backend): credentials CRUD with DPAPI-encrypted secrets

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

## Task 3: Фронт — CredsTab

**Files:** Modify `types.ts`, `ProjectView.svelte`; Create `clipboard.ts`, `api/creds.ts`, `components/CredsTab.svelte`.

- [ ] **Step 1: Типы** (в конец `types.ts`):
```ts
export type CredType = "login" | "api_key" | "token" | "ssh" | "conn_string" | "note";
export type Credential = {
  id: number;
  projectId: number;
  label: string;
  type: CredType;
  username: string | null;
  url: string | null;
  notes: string | null;
  sortOrder: number;
  hasSecret: boolean;
};
```

- [ ] **Step 2: clipboard.ts**

Create `src/lib/clipboard.ts`:
```ts
import { pushToast } from "./stores/toasts";

export async function copy(text: string, label = "Скопировано"): Promise<void> {
  try {
    await navigator.clipboard.writeText(text);
    pushToast(label, "", "ok");
  } catch {
    pushToast("Не удалось скопировать", "", "error");
  }
}

// Копировать секрет и очистить буфер через clearMs (по умолчанию 20с).
export async function copySecret(text: string, clearMs = 20000): Promise<void> {
  try {
    await navigator.clipboard.writeText(text);
    pushToast("Секрет скопирован", `Буфер очистится через ${Math.round(clearMs / 1000)} с`, "ok");
    setTimeout(() => {
      navigator.clipboard.writeText("").catch(() => {});
    }, clearMs);
  } catch {
    pushToast("Не удалось скопировать", "", "error");
  }
}
```

- [ ] **Step 3: api/creds.ts**
```ts
import { call } from "./client";
import type { Credential, CredType } from "../types";

export type CredInput = {
  label: string;
  type: CredType;
  username?: string | null;
  url?: string | null;
  notes?: string | null;
  secret?: string | null; // undefined/null = не менять (при update)
};

export const list = (projectId: number) => call<Credential[]>("creds_list", { projectId });
export const getSecret = (id: number) => call<string>("creds_get_secret", { id });
export const create = (projectId: number, input: CredInput) => call<Credential>("creds_create", { projectId, input });
export const update = (id: number, input: CredInput) => call<Credential>("creds_update", { id, input });
export const remove = (id: number) => call<void>("creds_delete", { id });
```

- [ ] **Step 4: components/CredsTab.svelte**

Create `src/lib/components/CredsTab.svelte`:
```svelte
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
```
> Иконки `search`/`plus`/`pencil`/`copy`/`eye`/`eye-off`/`x`/`trash-2`/`check` — из lucide. Классы кредов и модалки — в global.css. Секрет в список не грузится; `creds_get_secret` вызывается только при «показать»/«копировать».

- [ ] **Step 5: Рендер в ProjectView.svelte**
Импорт `import CredsTab from "./CredsTab.svelte";` и ветка в `tab-body`:
```svelte
    {:else if $activeTab === "creds"}
      <CredsTab {project} />
```

- [ ] **Step 6: Проверка**
```powershell
npm run check
npm test
npm run build
```
Expected: 0 ошибок типов; Vitest 3; build успешен.

- [ ] **Step 7: Commit**
```powershell
git add src/lib/types.ts src/lib/clipboard.ts src/lib/api/creds.ts src/lib/components/CredsTab.svelte src/lib/components/ProjectView.svelte
git commit -m @'
feat(frontend): credentials tab (masked secrets, reveal, copy with auto-clear)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

## Task 4: Проверка среза

- [ ] **Step 1: Автопроверка**
```powershell
npm run check ; npm test ; cargo test --manifest-path src-tauri/Cargo.toml --lib
```
Expected: типы 0 ошибок; Vitest 3; Rust 16 (14 прежних + crypto roundtrip + creds).

- [ ] **Step 2: Ручной GUI-смоук (пользователь; `npm run tauri dev`)**
- [ ] Вкладка «Креды» → «Добавить» → заполнить название/тип/логин/URL/секрет → «Сохранить» → карточка появилась; секрет показан как `••••`.
- [ ] «Показать» (глаз) → секрет раскрылся; «Скрыть» → снова замаскирован.
- [ ] «Копировать секрет» → тост «буфер очистится через 20 с»; вставить куда-нибудь — секрет на месте; через 20 с буфер пуст.
- [ ] Копирование логина/URL — свободно.
- [ ] Поиск фильтрует креды по названию/логину/URL/типу.
- [ ] Редактирование: пустой секрет = не менять; ввод нового = заменить. Удаление.
- [ ] Перезапуск приложения → креды на месте, секрет расшифровывается (DPAPI под той же учёткой). Файл БД с секретом, открытый другим пользователем/на другом ПК, не расшифруется.

---

## Итог среза

Креды на проект с типами, секрет зашифрован DPAPI и не покидает бэкенд без явного «показать/копировать»; маскировка, раскрытие, копирование с авто-очисткой буфера, поиск. Закрывает MVP. Отложено: режим мастер-пароля (AES-GCM+Argon2) — отдельный срез v1.0.
```
