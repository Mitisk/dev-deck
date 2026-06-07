# DevDeck — Доработка «Фильтр по приоритету + смена мастер-пароля» — план

> **Для исполнителя:** ОБЯЗАТЕЛЬНЫЙ ПОД-НАВЫК: superpowers:subagent-driven-development. Шаги — чекбоксы.

**Цель:** (1) В канбане фильтровать задачи по приоритету (вместе с фильтром по меткам). (2) Сменить мастер-пароль без отключения режима (re-encrypt секретов на новый ключ).

**Стек/границы:** backend — одна команда `master_change` + тест; frontend — фильтр приоритета в TasksTab + поле смены пароля в AppSettings. Без миграций.

**Источники:** `commands/security.rs` (verify_password/re_encrypt_all/derive_key/aes_*), `AppSettings.svelte` (секция «Безопасность»), `TasksTab.svelte` (фильтр-бар).

---

## Task 1: Rust — master_change

**Files:** Modify `src-tauri/src/commands/security.rs`, `src-tauri/src/lib.rs`.

- [ ] **Step 1: Команда** — в `security.rs` добавить:
```rust
/// Сменить мастер-пароль: проверить старый, перешифровать секреты на новый ключ.
#[tauri::command]
pub fn master_change(state: State<AppState>, old_password: String, new_password: String) -> AppResult<()> {
    if new_password.trim().len() < 4 {
        return Err(AppError { kind: ErrorKind::Validation, message: "Новый пароль слишком короткий (мин. 4)".into() });
    }
    let conn = state.db.lock().map_err(|_| AppError::internal("db mutex poisoned"))?;
    if current_mode(&conn) != "master" {
        return Err(AppError { kind: ErrorKind::Validation, message: "Мастер-пароль не включён".into() });
    }
    let old_key = verify_password(&conn, &old_password)?;
    let new_salt = crypto::random_salt();
    let new_key = crypto::derive_key(&new_password, &new_salt)?;
    let tx = conn.unchecked_transaction()?;
    re_encrypt_all(&conn, |blob| {
        let plain = crypto::aes_decrypt(&old_key, blob)?;
        crypto::aes_encrypt(&new_key, &plain)
    })?;
    set_setting(&conn, "kdf_salt", &hex::encode(new_salt))?;
    set_setting(&conn, "verifier", &hex::encode(crypto::aes_encrypt(&new_key, VERIFY)?))?;
    tx.commit()?;
    *state.master_key.lock().map_err(|_| AppError::internal("mk poisoned"))? = Some(new_key);
    Ok(())
}
```

- [ ] **Step 2: Тест** — в `security.rs` `mod tests` добавить:
```rust
    #[test]
    fn master_change_reencrypts_secret() {
        let conn = mem();
        conn.execute("INSERT INTO projects(name,status,sort_order) VALUES('P','active',0)", []).unwrap();
        let pid = conn.last_insert_rowid();
        let dpapi = crypto::encrypt(b"sec").unwrap();
        conn.execute("INSERT INTO credentials(project_id,label,type,secret_encrypted,sort_order) VALUES(?1,'C','token',?2,0)", params![pid, dpapi]).unwrap();

        // включить master (pw1)
        let salt = crypto::random_salt();
        let k1 = crypto::derive_key("pw1234", &salt).unwrap();
        re_encrypt_all(&conn, |b| { let p = crypto::decrypt(b)?; crypto::aes_encrypt(&k1, &p) }).unwrap();
        set_setting(&conn, "crypto_mode", "master").unwrap();
        set_setting(&conn, "kdf_salt", &hex::encode(salt)).unwrap();
        set_setting(&conn, "verifier", &hex::encode(crypto::aes_encrypt(&k1, VERIFY).unwrap())).unwrap();

        // сменить на pw2
        let old = verify_password(&conn, "pw1234").unwrap();
        let nsalt = crypto::random_salt();
        let k2 = crypto::derive_key("newpass", &nsalt).unwrap();
        re_encrypt_all(&conn, |b| { let p = crypto::aes_decrypt(&old, b)?; crypto::aes_encrypt(&k2, &p) }).unwrap();
        set_setting(&conn, "kdf_salt", &hex::encode(nsalt)).unwrap();
        set_setting(&conn, "verifier", &hex::encode(crypto::aes_encrypt(&k2, VERIFY).unwrap())).unwrap();

        assert!(verify_password(&conn, "pw1234").is_err());
        let kk = verify_password(&conn, "newpass").unwrap();
        let stored: Vec<u8> = conn.query_row("SELECT secret_encrypted FROM credentials", [], |r| r.get(0)).unwrap();
        let mk = Mutex::new(Some(kk));
        assert_eq!(decrypt_secret(&conn, &mk, &stored).unwrap(), b"sec");
    }
```

- [ ] **Step 3: Регистрация** — `lib.rs` `generate_handler![...]`: добавить `commands::security::master_change,`.

- [ ] **Step 4: Тесты + lib-сборка**
```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib
cargo build --manifest-path src-tauri/Cargo.toml --lib
```
Expected: новый `master_change_reencrypts_secret` + прежние → 33 ok; lib-сборка успешна.

- [ ] **Step 5: Commit**
```powershell
git add src-tauri/src/commands/security.rs src-tauri/src/lib.rs
git commit -m @'
feat(backend): master_change (rotate master password, re-encrypt secrets)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

## Task 2: Фронт — фильтр приоритета + смена пароля

**Files:** Modify `api/security.ts`, `AppSettings.svelte`, `TasksTab.svelte`.

- [ ] **Step 1: api/security.ts** — добавить:
```ts
export const change = (oldPassword: string, newPassword: string) => call<void>("master_change", { oldPassword, newPassword });
```

- [ ] **Step 2: AppSettings — смена пароля**

В `AppSettings.svelte`:
1. В `<script>` добавить состояние и функцию:
```ts
  let chOld = $state("");
  let chNew = $state("");
  async function changeMaster() {
    if (chNew.length < 4) { pushToast("Пароль короткий", "минимум 4 символа", "error"); return; }
    busy = true;
    try { await security.change(chOld, chNew); chOld = ""; chNew = ""; pushToast("Пароль изменён", "секреты перешифрованы", "ok"); }
    finally { busy = false; }
  }
```
2. В блоке «Безопасность», в ветке `{:else}` (режим master + разблокировано) — добавить под кнопками блок смены пароля:
```svelte
              <div style="display:flex;gap:6px;align-items:center;margin-top:6px">
                <input class="tin" type="password" placeholder="Старый пароль" bind:value={chOld} />
                <input class="tin" type="password" placeholder="Новый пароль" bind:value={chNew} />
                <button class="btn-ghost" disabled={busy} onclick={changeMaster}><Icon name="key-round" class="ic-sm" /> Сменить</button>
              </div>
```

- [ ] **Step 3: TasksTab — фильтр по приоритету**

В `src/lib/components/TasksTab.svelte`:
1. Состояние: `let activePriorities = $state<number[]>([]);`
2. Функция toggle: `function togglePri(p: number) { activePriorities = activePriorities.includes(p) ? activePriorities.filter(x => x !== p) : [...activePriorities, p]; }`
3. В `visibleColTasks(key)` добавить фильтр приоритета (после фильтра меток):
```ts
  function visibleColTasks(key: string): Task[] {
    let list = colTasks(key);
    if (activeLabels.length) list = list.filter((t) => t.labelIds.some((id) => activeLabels.includes(id)));
    if (activePriorities.length) list = list.filter((t) => activePriorities.includes(t.priority));
    return list;
  }
```
4. В фильтр-баре (рядом с метками, перед `<span style="flex:1">`) добавить чипы приоритета:
```svelte
  {#each PRIORITY as p, i}
    <button class="chip" style="cursor:pointer;border-color:{p.color};{activePriorities.includes(i) ? `background:color-mix(in oklab, ${p.color} 22%, transparent);color:var(--text)` : ''}"
            onclick={() => togglePri(i)}>{p.label}</button>
  {/each}
```
5. Кнопку «сбросить» расширить, чтобы чистить оба фильтра:
```svelte
  {#if activeLabels.length || activePriorities.length}<button class="chip" onclick={() => { activeLabels = []; activePriorities = []; }}>сбросить</button>{/if}
```
(заменить существующее условие `{#if activeLabels.length}...`).

- [ ] **Step 4: Проверка**
```powershell
npm run check
npm test
npm run build
```
Expected: 0 ошибок типов; Vitest 3; build успешен.

- [ ] **Step 5: Commit**
```powershell
git add src/lib/api/security.ts src/lib/components/AppSettings.svelte src/lib/components/TasksTab.svelte
git commit -m @'
feat(frontend): kanban priority filter and master-password change UI

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

## Task 3: Проверка

- [ ] **Step 1: Автопроверка**
```powershell
npm run check ; npm test ; cargo test --manifest-path src-tauri/Cargo.toml --lib
```
Expected: типы 0; Vitest 3; Rust 33.

- [ ] **Step 2: GUI-смоук (пользователь)**
- [ ] Канбан: чипы приоритета в фильтр-баре → фильтруют задачи; работает вместе с фильтром меток; «сбросить» чистит оба.
- [ ] Настройки → Безопасность (режим master, разблокировано) → «Сменить»: старый+новый пароль → тост; старый пароль больше не разблокирует, новый — да; секреты целы.

---

## Итог

Канбан фильтруется по приоритету (с метками); мастер-пароль меняется без отключения режима. Следующая доработка — закреплённые проекты в меню трея.
```
