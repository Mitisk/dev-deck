# Cred editor: key path + PuTTY + password tools — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Расширить креды полем «путь к ключу», кнопкой запуска PuTTY и инструментами пароля (генератор + индикатор силы) в редакторе.

**Architecture:** Новая миграция добавляет колонку `key_path`. Бэкенд-команда `launch_putty(id)` читает кред, при необходимости расшифровывает секрет (секрет не уходит во фронт) и запускает `putty.exe` с argv без shell. Фронтенд: чистые хелперы пароля (тестируемые) + правки редактора/карточки кредов.

**Tech Stack:** Rust + rusqlite + `std::process::Command`; SvelteKit (Svelte 5 runes) + TypeScript; `@tauri-apps/plugin-dialog`; Vitest.

Спека: `docs/superpowers/specs/2026-06-10-cred-editor-key-putty-design.md`.

---

## File Structure

**Backend**
- `src-tauri/migrations/0006_cred_key_path.sql` — новая колонка.
- `src-tauri/src/db/migrations.rs` — регистрация миграции + правка теста версии.
- `src-tauri/src/models.rs` — `key_path` в `Credential` и `CredInput`.
- `src-tauri/src/commands/creds.rs` — CRUD с `key_path`, `launch_putty` + чистые хелперы + тесты.
- `src-tauri/src/lib.rs` — регистрация `launch_putty`.

**Frontend**
- `src/lib/password.ts` — `generatePassword`, `passwordStrength` (новый).
- `src/tests/password.test.ts` — тесты (новый).
- `src/lib/types.ts` — `Credential.keyPath`.
- `src/lib/api/creds.ts` — `CredInput.keyPath`.
- `src/lib/api/actions.ts` — `launchPutty`.
- `src/lib/components/CredsTab.svelte` — редактор + карточка.
- `src/lib/styles/global.css` — стили.

---

## Task 1: Backend — миграция key_path

**Files:**
- Create: `src-tauri/migrations/0006_cred_key_path.sql`
- Modify: `src-tauri/src/db/migrations.rs`

- [ ] **Step 1: Создать файл миграции**

`src-tauri/migrations/0006_cred_key_path.sql`:

```sql
ALTER TABLE credentials ADD COLUMN key_path TEXT;

PRAGMA user_version = 6;
```

- [ ] **Step 2: Зарегистрировать миграцию**

В `src-tauri/src/db/migrations.rs`, в массиве `MIGRATIONS`, после строки
`(5, include_str!("../../migrations/0005_health.sql")),` добавить:

```rust
    (6, include_str!("../../migrations/0006_cred_key_path.sql")),
```

- [ ] **Step 3: Обновить тест версии схемы**

В том же файле, в тесте `applies_migrations_on_empty_db`, заменить строку
`assert_eq!(schema_version(&conn).unwrap(), 5);` на:

```rust
        assert_eq!(schema_version(&conn).unwrap(), 6);
```

- [ ] **Step 4: Прогнать тест миграций**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib applies_migrations_on_empty_db`
Expected: PASS (схема доходит до версии 6, колонка добавлена).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/migrations/0006_cred_key_path.sql src-tauri/src/db/migrations.rs
git commit -m "feat(backend): migration 0006 — credentials.key_path"
```

---

## Task 2: Backend — key_path в модели и CRUD

**Files:**
- Modify: `src-tauri/src/models.rs`
- Modify: `src-tauri/src/commands/creds.rs`

- [ ] **Step 1: Добавить поле в модели**

В `src-tauri/src/models.rs`, в структуру `Credential` (после `pub has_secret: bool,`)
добавить:

```rust
    pub key_path: Option<String>,
```

В структуру `CredInput` (после `pub secret: Option<String>,`) добавить:

```rust
    pub key_path: Option<String>,
```

- [ ] **Step 2: Обновить row_to_cred**

В `src-tauri/src/commands/creds.rs`, в функции `row_to_cred`, заменить SQL и
конструктор. Текущий SELECT:
```rust
        "SELECT id, project_id, label, type, username, url, notes, sort_order,
                (secret_encrypted IS NOT NULL AND length(secret_encrypted) > 0)
         FROM credentials WHERE id = ?1",
```
на:
```rust
        "SELECT id, project_id, label, type, username, url, notes, sort_order,
                (secret_encrypted IS NOT NULL AND length(secret_encrypted) > 0),
                key_path
         FROM credentials WHERE id = ?1",
```
И в конструкторе `Credential`, после `has_secret: r.get::<_, i64>(8)? != 0,` добавить:
```rust
                key_path: r.get(9)?,
```

- [ ] **Step 3: Обновить creds_create**

В `creds_create`, заменить INSERT (колонки + values). Текущий:
```rust
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
```
на:
```rust
        "INSERT INTO credentials(project_id, label, type, username, url, secret_encrypted, notes, sort_order, key_path)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            project_id,
            input.label.trim(),
            input.kind,
            input.username,
            input.url,
            secret_blob,
            input.notes,
            next,
            input.key_path,
        ],
```

- [ ] **Step 4: Обновить creds_update**

В `creds_update`, заменить UPDATE метаданных. Текущий:
```rust
        "UPDATE credentials SET label=?2, type=?3, username=?4, url=?5, notes=?6, updated_at=datetime('now')
         WHERE id=?1",
        params![id, input.label.trim(), input.kind, input.username, input.url, input.notes],
```
на:
```rust
        "UPDATE credentials SET label=?2, type=?3, username=?4, url=?5, notes=?6, key_path=?7, updated_at=datetime('now')
         WHERE id=?1",
        params![id, input.label.trim(), input.kind, input.username, input.url, input.notes, input.key_path],
```

- [ ] **Step 5: Проверить компиляцию и существующие тесты**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib`
Expected: всё компилируется и проходит (включая creds-тесты и миграции).

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/models.rs src-tauri/src/commands/creds.rs
git commit -m "feat(backend): key_path in Credential model + CRUD"
```

---

## Task 3: Backend — launch_putty + чистые хелперы (TDD)

**Files:**
- Modify: `src-tauri/src/commands/creds.rs` (хелперы, команда, тесты)
- Modify: `src-tauri/src/lib.rs` (регистрация)

- [ ] **Step 1: Написать падающие тесты**

В `src-tauri/src/commands/creds.rs`, внутри `#[cfg(test)] mod tests`, добавить:

```rust
    #[test]
    fn split_host_port_parses_numeric_tail() {
        assert_eq!(split_host_port("root@h:2222"), ("root@h", Some("2222")));
        assert_eq!(split_host_port("host"), ("host", None));
        assert_eq!(split_host_port("host:abc"), ("host:abc", None));
    }

    #[test]
    fn build_putty_args_prefers_key_over_password() {
        let a = build_putty_args("root@1.2.3.4", Some("k.ppk"), Some("pw"));
        assert_eq!(a, vec!["-ssh", "root@1.2.3.4", "-i", "k.ppk"]);
        assert!(!a.iter().any(|x| x == "-pw"));
    }

    #[test]
    fn build_putty_args_uses_password_when_no_key() {
        let a = build_putty_args("host:2222", None, Some("secret"));
        assert_eq!(a, vec!["-ssh", "host", "-P", "2222", "-pw", "secret"]);
    }

    #[test]
    fn build_putty_args_bare_host() {
        assert_eq!(build_putty_args("host", None, None), vec!["-ssh", "host"]);
    }
```

- [ ] **Step 2: Запустить — убедиться, что не компилируется**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib build_putty_args`
Expected: ошибка компиляции — `split_host_port`/`build_putty_args` не найдены.

- [ ] **Step 3: Добавить импорт и реализовать хелперы + команду**

В начало `src-tauri/src/commands/creds.rs`, после `use std::sync::MutexGuard;`
добавить:
```rust
use std::process::Command;
```

После функции `creds_reorder` (или перед `#[cfg(test)]`) добавить:

```rust
/// Разбить ssh-цель на host и хвостовой :port (порт — только если все цифры).
fn split_host_port(target: &str) -> (&str, Option<&str>) {
    if let Some(idx) = target.rfind(':') {
        let (h, p) = (&target[..idx], &target[idx + 1..]);
        if !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()) {
            return (h, Some(p));
        }
    }
    (target, None)
}

/// argv для putty (без имени программы). Политика «ключ → опц. пароль»:
/// есть key_path → `-i key` (пароль НЕ передаём); иначе есть пароль → `-pw`.
fn build_putty_args(target: &str, key_path: Option<&str>, password: Option<&str>) -> Vec<String> {
    let (host, port) = split_host_port(target);
    let mut args = vec!["-ssh".to_string(), host.to_string()];
    if let Some(p) = port {
        args.push("-P".to_string());
        args.push(p.to_string());
    }
    if let Some(k) = key_path.filter(|s| !s.trim().is_empty()) {
        args.push("-i".to_string());
        args.push(k.to_string());
    } else if let Some(pw) = password.filter(|s| !s.is_empty()) {
        args.push("-pw".to_string());
        args.push(pw.to_string());
    }
    args
}

/// Запустить putty.exe: PATH → стандартные пути установки.
fn spawn_putty(args: &[String]) -> AppResult<()> {
    const CANDIDATES: [&str; 3] = [
        "putty.exe",
        r"C:\Program Files\PuTTY\putty.exe",
        r"C:\Program Files (x86)\PuTTY\putty.exe",
    ];
    for exe in CANDIDATES {
        if Command::new(exe).args(args).spawn().is_ok() {
            return Ok(());
        }
    }
    Err(AppError {
        kind: ErrorKind::NotFound,
        message: "PuTTY не найден. Установите PuTTY или добавьте putty.exe в PATH.".into(),
    })
}

/// Открыть кред в PuTTY. Секрет расшифровывается на бэке и во фронт не уходит.
#[tauri::command]
pub fn launch_putty(state: State<AppState>, id: i64) -> AppResult<()> {
    let conn = lock(&state)?;
    let (username, key_path): (Option<String>, Option<String>) = conn.query_row(
        "SELECT username, key_path FROM credentials WHERE id=?1",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let target = username.unwrap_or_default();
    if target.trim().is_empty() {
        return Err(AppError {
            kind: ErrorKind::Validation,
            message: "У креда не указан хост (поле «Логин / хост»)".into(),
        });
    }
    let has_key = key_path.as_deref().map(|s| !s.trim().is_empty()).unwrap_or(false);
    let password: Option<String> = if has_key {
        None
    } else {
        let blob: Option<Vec<u8>> =
            conn.query_row("SELECT secret_encrypted FROM credentials WHERE id=?1", [id], |r| r.get(0))?;
        match blob {
            Some(b) if !b.is_empty() => {
                let plain = decrypt_secret(&conn, &state.master_key, &b)?;
                Some(String::from_utf8_lossy(&plain).into_owned())
            }
            _ => None,
        }
    };
    let args = build_putty_args(target.trim(), key_path.as_deref(), password.as_deref());
    spawn_putty(&args)
}
```

- [ ] **Step 4: Зарегистрировать команду**

В `src-tauri/src/lib.rs`, после строки `commands::creds::creds_reorder,` (строка ~155)
добавить:
```rust
            commands::creds::launch_putty,
```

- [ ] **Step 5: Запустить тесты — убедиться, что проходят**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib`
Expected: все PASS, включая `split_host_port_parses_numeric_tail`,
`build_putty_args_prefers_key_over_password`, `build_putty_args_uses_password_when_no_key`,
`build_putty_args_bare_host`.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/commands/creds.rs src-tauri/src/lib.rs
git commit -m "feat(backend): launch_putty command + putty arg builders"
```

---

## Task 4: Frontend — хелперы пароля (TDD)

**Files:**
- Create: `src/lib/password.ts`
- Create: `src/tests/password.test.ts`

- [ ] **Step 1: Написать падающий тест**

`src/tests/password.test.ts`:

```ts
import { describe, it, expect } from "vitest";
import { generatePassword, passwordStrength } from "../lib/password";

const CHARSET = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!@#$%^&*-_=+";

describe("passwordStrength", () => {
  it("пустой → 0", () => {
    expect(passwordStrength("")).toBe(0);
  });
  it("слабый < сильный", () => {
    expect(passwordStrength("abc")).toBeLessThan(passwordStrength("Abcd1234!@xyzQ"));
  });
  it("длинный со всеми классами → 4", () => {
    expect(passwordStrength("Abcd1234!@xyzQ")).toBe(4);
  });
});

describe("generatePassword", () => {
  it("длина 20 по умолчанию", () => {
    expect(generatePassword(20).length).toBe(20);
  });
  it("кламп снизу до 8", () => {
    expect(generatePassword(2).length).toBe(8);
  });
  it("кламп сверху до 64", () => {
    expect(generatePassword(100).length).toBe(64);
  });
  it("только символы из CHARSET", () => {
    const pw = generatePassword(40);
    expect([...pw].every((ch) => CHARSET.includes(ch))).toBe(true);
  });
});
```

- [ ] **Step 2: Запустить — убедиться, что падает**

Run: `npm test -- password`
Expected: FAIL — модуль `../lib/password` не найден.

- [ ] **Step 3: Реализовать хелперы**

`src/lib/password.ts`:

```ts
const CHARSET = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!@#$%^&*-_=+";

// Крепкий случайный пароль заданной длины (кламп 8..64, дефолт 20).
export function generatePassword(len: number): string {
  const n = Math.max(8, Math.min(64, Math.floor(len) || 20));
  const arr = new Uint32Array(n);
  crypto.getRandomValues(arr);
  let out = "";
  for (let i = 0; i < n; i++) out += CHARSET[arr[i] % CHARSET.length];
  return out;
}

// Грубая оценка надёжности 0..4 (длина + разнообразие классов символов).
export function passwordStrength(pw: string): number {
  if (!pw) return 0;
  let score = 0;
  if (pw.length >= 8) score++;
  if (pw.length >= 14) score++;
  let classes = 0;
  if (/[a-z]/.test(pw)) classes++;
  if (/[A-Z]/.test(pw)) classes++;
  if (/[0-9]/.test(pw)) classes++;
  if (/[^A-Za-z0-9]/.test(pw)) classes++;
  if (classes >= 3) score++;
  if (classes >= 4 && pw.length >= 12) score++;
  return Math.min(4, score);
}
```

- [ ] **Step 4: Запустить тесты — убедиться, что проходят**

Run: `npm test -- password`
Expected: PASS (7 тестов).

- [ ] **Step 5: Commit**

```bash
git add src/lib/password.ts src/tests/password.test.ts
git commit -m "feat(frontend): password generator + strength helpers"
```

---

## Task 5: Frontend — типы и API

**Files:**
- Modify: `src/lib/types.ts`
- Modify: `src/lib/api/creds.ts`
- Modify: `src/lib/api/actions.ts`

- [ ] **Step 1: Тип Credential**

В `src/lib/types.ts`, в `Credential`, после `hasSecret: boolean;` (строка 95) добавить:
```ts
  keyPath: string | null;
```

- [ ] **Step 2: CredInput**

В `src/lib/api/creds.ts`, в типе `CredInput`, после `secret?: string | null; ...`
(строка 10) добавить:
```ts
  keyPath?: string | null;
```

- [ ] **Step 3: actions.launchPutty**

В `src/lib/api/actions.ts`, после `openFileInEditor` добавить:
```ts
export const launchPutty = (id: number) => call<void>("launch_putty", { id });
```

- [ ] **Step 4: Проверить типы**

Run: `npm run check`
Expected: 0 ошибок.

- [ ] **Step 5: Commit**

```bash
git add src/lib/types.ts src/lib/api/creds.ts src/lib/api/actions.ts
git commit -m "feat(frontend): keyPath type + launchPutty api"
```

---

## Task 6: Frontend — редактор креда (поле ключа, генератор, индикатор)

**Files:**
- Modify: `src/lib/components/CredsTab.svelte`

- [ ] **Step 1: Импорты**

В `<script>` `src/lib/components/CredsTab.svelte`, после
`import { reorderIds } from "$lib/credsOrder";` добавить:
```svelte
  import * as actions from "$lib/api/actions";
  import { generatePassword, passwordStrength } from "$lib/password";
  import { open } from "@tauri-apps/plugin-dialog";
```

- [ ] **Step 2: Состояние редактора**

Найти строку с состоянием формы:
```ts
  let fLabel = $state(""), fType = $state<CredType>("login"), fUser = $state(""), fUrl = $state(""), fNotes = $state(""), fSecret = $state("");
```
заменить на (добавлены `fKeyPath`, `genLen`):
```ts
  let fLabel = $state(""), fType = $state<CredType>("login"), fUser = $state(""), fUrl = $state(""), fNotes = $state(""), fSecret = $state("");
  let fKeyPath = $state("");
  let genLen = $state(20);

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
```

- [ ] **Step 3: Заполнение/сброс fKeyPath**

В `openNew`, в строке сброса полей
```ts
    fLabel = ""; fType = "login"; fUser = ""; fUrl = ""; fNotes = ""; fSecret = "";
```
заменить на:
```ts
    fLabel = ""; fType = "login"; fUser = ""; fUrl = ""; fNotes = ""; fSecret = ""; fKeyPath = "";
```
В `openEdit`, в строке заполнения
```ts
    fLabel = c.label; fType = c.type; fUser = c.username ?? ""; fUrl = c.url ?? ""; fNotes = c.notes ?? ""; fSecret = "";
```
заменить на:
```ts
    fLabel = c.label; fType = c.type; fUser = c.username ?? ""; fUrl = c.url ?? ""; fNotes = c.notes ?? ""; fSecret = ""; fKeyPath = c.keyPath ?? "";
```

- [ ] **Step 4: keyPath в save()**

В `save()`, в объекте `input`, после `notes: fNotes.trim() || null,` добавить:
```ts
      keyPath: fKeyPath.trim() || null,
```

- [ ] **Step 5: Поле ключа в разметке**

В модалке, в `<div class="set-grid">`, после поля URL
```svelte
          <div class="field"><label for="c-url">URL</label><input id="c-url" class="tin mono" bind:value={fUrl} /></div>
```
добавить:
```svelte
          <div class="field"><label for="c-key">Путь к ключу</label>
            <div class="key-row">
              <input id="c-key" class="tin mono" placeholder="C:\keys\id.ppk" bind:value={fKeyPath} />
              <button class="btn-ghost" type="button" onclick={pickKey} title="Выбрать файл"><Icon name="folder-open" class="ic-sm" /></button>
            </div>
          </div>
```

- [ ] **Step 6: Секрет — генератор + индикатор**

Заменить блок поля секрета:
```svelte
        <div class="field"><label for="c-secret">Секрет {#if !isNew}<span style="color:var(--muted-2)">(пусто = не менять)</span>{/if}</label>
          <input id="c-secret" class="tin mono" type="password" bind:value={fSecret} autocomplete="off" /></div>
```
на:
```svelte
        <div class="field"><label for="c-secret">Секрет {#if !isNew}<span style="color:var(--muted-2)">(пусто = не менять)</span>{/if}</label>
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
```

- [ ] **Step 7: Проверить типы**

Run: `npm run check`
Expected: 0 ошибок (новых warning быть не должно).

- [ ] **Step 8: Commit**

```bash
git add src/lib/components/CredsTab.svelte
git commit -m "feat(frontend): key path field + password generator/meter in cred editor"
```

---

## Task 7: Frontend — карточка креда (ключ + кнопка PuTTY)

**Files:**
- Modify: `src/lib/components/CredsTab.svelte`

- [ ] **Step 1: Обёртка openPutty**

В `<script>`, после функции `del()` (или рядом с другими async-хелперами) добавить:
```ts
  async function openPutty(id: number) {
    try {
      await actions.launchPutty(id);
    } catch (e) {
      if ((e as { kind?: string }).kind === "locked")
        pushToast("Заблокировано", "Разблокируйте мастер-паролем в Настройках", "error");
      // прочие ошибки уже показал api/client.ts
    }
  }
```

- [ ] **Step 2: Кнопки в шапке карточки (PuTTY + редактирование)**

Заменить кнопку редактирования в `.cred-head`:
```svelte
        <button class="mini cred-del" style="margin-left:auto" title="Редактировать" onclick={() => openEdit(c)}>
          <Icon name="pencil" class="ic-sm" />
        </button>
```
на обёртку `.cred-acts` с опциональной кнопкой PuTTY:
```svelte
        <span class="cred-acts">
          {#if c.type === 'ssh'}
            <button class="mini" title="Открыть в PuTTY" onclick={() => openPutty(c.id)}><Icon name="square-terminal" class="ic-sm" /></button>
          {/if}
          <button class="mini cred-del" title="Редактировать" onclick={() => openEdit(c)}><Icon name="pencil" class="ic-sm" /></button>
        </span>
```

- [ ] **Step 3: Строка «Ключ» в теле карточки**

После блока секрета (`{#if c.hasSecret}...{/if}`) и перед строкой заметки
(`{#if c.notes}...`) добавить:
```svelte
      {#if c.keyPath}
        <div class="cred-row">
          <span class="k">Ключ</span>
          <span class="val mono">{c.keyPath}</span>
          <span class="acts"><button class="mini" title="Копировать" onclick={() => copy(c.keyPath ?? '')}><Icon name="copy" class="ic-sm" /></button></span>
        </div>
      {/if}
```

- [ ] **Step 4: Проверить типы**

Run: `npm run check`
Expected: 0 ошибок.

- [ ] **Step 5: Commit**

```bash
git add src/lib/components/CredsTab.svelte
git commit -m "feat(frontend): PuTTY button + key path row in cred card"
```

---

## Task 8: Frontend — стили

**Files:**
- Modify: `src/lib/styles/global.css`

- [ ] **Step 1: Заменить правило выравнивания и добавить стили**

В `src/lib/styles/global.css` найти правило (строка ~793):
```css
.cred-head .cred-del { margin-left: auto; }
```
заменить на:
```css
.cred-head .cred-acts { display: inline-flex; align-items: center; gap: 6px; margin-left: auto; }
.secret-row { display: flex; gap: 8px; align-items: center; }
.secret-row > input.tin:first-child { flex: 1; min-width: 0; }
.gen-len { width: 64px; flex: none; text-align: center; }
.key-row { display: flex; gap: 8px; align-items: center; }
.key-row > input.tin { flex: 1; min-width: 0; }
.pw-meter { display: flex; gap: 4px; margin-top: 6px; }
.pw-meter .seg { height: 4px; flex: 1; border-radius: 2px; background: var(--surface-3); }
.pw-meter[data-score="1"] .seg.on { background: var(--danger); }
.pw-meter[data-score="2"] .seg.on { background: var(--git-dirty); }
.pw-meter[data-score="3"] .seg.on { background: var(--git-dirty); }
.pw-meter[data-score="4"] .seg.on { background: var(--git-ahead); }
```

- [ ] **Step 2: Проверить сборку**

Run: `npm run check`
Expected: 0 ошибок.

- [ ] **Step 3: Commit**

```bash
git add src/lib/styles/global.css
git commit -m "feat(frontend): styles for key path, generator, strength meter, cred actions"
```

---

## Task 9: Полная верификация

- [ ] **Step 1: Rust-тесты**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib`
Expected: все PASS (миграция v6, putty-хелперы, существующие creds-тесты).

- [ ] **Step 2: Фронт-тесты**

Run: `npm test`
Expected: все PASS (включая 7 тестов password).

- [ ] **Step 3: Проверка типов**

Run: `npm run check`
Expected: 0 ошибок.

- [ ] **Step 4: Ручная проверка**

Run: `npm run tauri dev`
- Создать/редактировать кред: поле «Путь к ключу» + «Выбрать файл» работает.
- Кнопка «Сгенерировать» заполняет секрет; индикатор силы реагирует на ввод.
- На ssh-креде в карточке есть кнопка PuTTY; при наличии PuTTY в системе она
  открывает сессию (ключ → `-i`, иначе пароль → `-pw`). Без PuTTY — тост «не найден».
- Строка «Ключ» показывается в карточке, копирование пути работает.

- [ ] **Step 5: Финальный commit (если были правки)**

```bash
git add -A
git commit -m "fix: cred editor polish after manual verification"
```

(Если правок нет — пропустить.)
