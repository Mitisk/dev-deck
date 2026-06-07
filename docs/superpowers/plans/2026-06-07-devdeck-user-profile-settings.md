# DevDeck — «Имя пользователя в настройках» — план

> **Для исполнителя:** ОБЯЗАТЕЛЬНЫЙ ПОД-НАВЫК: superpowers:subagent-driven-development. Шаги — чекбоксы.

**Цель:** Убрать захардкоженного «Артёма» (`src/lib/mock.ts`) и хранить имя/хэндл пользователя в настройках приложения (таблица `settings`), редактируемые в модалке «Настройки». Sidebar и Dashboard показывают имя из стора; инициалы выводятся из имени.

**Архитектура:** Backend — команды `user_get`/`user_set` в `commands/security.rs` (там живут helper'ы `get_setting`/`set_setting`/`del_setting` и таблица `settings`), хранят ключи `user_name`/`user_handle`. Frontend — `api/user.ts`, стор `stores/user.ts` (грузится при старте), правка `Sidebar`/`Dashboard`/`AppSettings`, удаление `mock.ts`.

**Стек/границы:** backend — `models.rs` (+DTO), `commands/security.rs` (+2 команды), `lib.rs` (регистрация). Frontend — `types.ts`, `api/user.ts`, `stores/user.ts`, `routes/+page.svelte`, `Sidebar.svelte`, `Dashboard.svelte`, `AppSettings.svelte`; удалить `mock.ts`. Без миграций (таблица `settings` уже есть).

---

## Task 1: Rust — профиль в settings

**Files:** Modify `src-tauri/src/models.rs`, `src-tauri/src/commands/security.rs`, `src-tauri/src/lib.rs`.

- [ ] **Step 1: DTO** — в `models.rs` добавить:
```rust
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserProfile {
    pub name: String,
    pub handle: Option<String>,
}
```
(если в `models.rs` уже импортированы `Serialize`/`Deserialize` через `use serde::...`, использовать их без префикса — подстроиться под файл.)

- [ ] **Step 2: Команды** — в `commands/security.rs` добавить (рядом с прочими `#[tauri::command]`, helper'ы `get_setting`/`set_setting`/`del_setting` уже в этом файле):
```rust
#[tauri::command]
pub fn user_get(state: State<AppState>) -> AppResult<crate::models::UserProfile> {
    let conn = state.db.lock().map_err(|_| AppError::internal("db mutex poisoned"))?;
    let name = get_setting(&conn, "user_name")?.unwrap_or_default();
    let handle = get_setting(&conn, "user_handle")?;
    Ok(crate::models::UserProfile { name, handle })
}

#[tauri::command]
pub fn user_set(state: State<AppState>, name: String, handle: Option<String>) -> AppResult<()> {
    let conn = state.db.lock().map_err(|_| AppError::internal("db mutex poisoned"))?;
    set_setting(&conn, "user_name", name.trim())?;
    match handle.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(h) => set_setting(&conn, "user_handle", h)?,
        None => del_setting(&conn, "user_handle")?,
    }
    Ok(())
}
```
> Если `del_setting` помечен `#[allow(dead_code)]` — снять атрибут (теперь используется).

- [ ] **Step 3: Регистрация** — в `lib.rs` `generate_handler![...]` добавить:
```rust
            commands::security::user_get,
            commands::security::user_set,
```

- [ ] **Step 4: Сборка + тесты**
```powershell
cargo build --manifest-path src-tauri/Cargo.toml --lib
cargo test --manifest-path src-tauri/Cargo.toml --lib
```
Expected: lib-сборка успешна; 35 тестов ok.

- [ ] **Step 5: Commit**
```powershell
git add src-tauri/src/models.rs src-tauri/src/commands/security.rs src-tauri/src/lib.rs
git commit -m @'
feat(backend): user profile (name/handle) in settings

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

## Task 2: Фронт — стор профиля и правка UI

**Files:** Modify `src/lib/types.ts`, `src/routes/+page.svelte`, `src/lib/components/Sidebar.svelte`, `src/lib/components/Dashboard.svelte`, `src/lib/components/AppSettings.svelte`; Create `src/lib/api/user.ts`, `src/lib/stores/user.ts`; Delete `src/lib/mock.ts`.

- [ ] **Step 1: Тип** — в `src/lib/types.ts` добавить:
```ts
export type UserProfile = { name: string; handle: string | null };
```

- [ ] **Step 2: api/user.ts**
```ts
import { call } from "./client";
import type { UserProfile } from "../types";

export const get = () => call<UserProfile>("user_get");
export const set = (name: string, handle: string | null) => call<void>("user_set", { name, handle });
```

- [ ] **Step 3: stores/user.ts**
```ts
import { writable, derived } from "svelte/store";
import type { UserProfile } from "../types";
import * as userApi from "../api/user";

export const user = writable<UserProfile>({ name: "", handle: null });

export const displayName = derived(user, ($u) => $u.name.trim() || "Пользователь");
export const initials = derived(user, ($u) => {
  const parts = $u.name.trim().split(/\s+/).filter(Boolean);
  if (!parts.length) return "·";
  const a = parts[0][0] ?? "";
  const b = parts.length > 1 ? (parts[1][0] ?? "") : "";
  return (a + b).toUpperCase();
});

export async function loadUser(): Promise<void> {
  try { user.set(await userApi.get()); } catch { /* нет бэка — оставить дефолт */ }
}
```

- [ ] **Step 4: +page.svelte — загрузка при старте**

В `<script>` добавить импорт и вызов в `onMount` (рядом с `loadProjects()`):
```ts
  import { loadUser } from "$lib/stores/user";
```
и в `onMount` первой строкой:
```ts
    await loadUser();
```

- [ ] **Step 5: Sidebar.svelte** — заменить mock на стор.
1. Удалить `import { USER } from "$lib/mock";`, добавить:
```ts
  import { displayName, initials } from "$lib/stores/user";
  import { user } from "$lib/stores/user";
```
2. В разметке заменить:
```svelte
    <span class="avatar">{USER.initials}</span>
    <div class="who">{USER.name}<small>{USER.handle}</small></div>
```
на:
```svelte
    <span class="avatar">{$initials}</span>
    <div class="who">{$displayName}{#if $user.handle}<small>{$user.handle}</small>{/if}</div>
```

- [ ] **Step 6: Dashboard.svelte** — заменить mock.
1. Удалить `import { USER } from "$lib/mock";`, добавить `import { displayName } from "$lib/stores/user";`.
2. Заменить `Привет, <span>{USER.name}</span>` на `Привет, <span>{$displayName}</span>`.

- [ ] **Step 7: AppSettings.svelte — карточка профиля**
1. В `<script>` добавить импорт и состояние:
```ts
  import { user, loadUser } from "$lib/stores/user";
  import * as userApi from "$lib/api/user";
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
```
2. В `$effect`, который реагирует на открытие настроек, подставлять текущие значения. Заменить:
```ts
  $effect(() => {
    if ($showSettings) { refreshBackups(); refreshCrypto(); }
  });
```
на:
```ts
  $effect(() => {
    if ($showSettings) { refreshBackups(); refreshCrypto(); uName = $user.name; uHandle = $user.handle ?? ""; }
  });
```
3. В начало `<div class="modal-body">` (перед полем «Безопасность секретов») добавить карточку профиля:
```svelte
        <div class="field">
          <label>Профиль</label>
          <div class="set-grid">
            <div class="field"><label for="u-name">Имя</label><input id="u-name" class="tin" placeholder="Ваше имя" bind:value={uName} /></div>
            <div class="field"><label for="u-handle">Хэндл <span style="color:var(--muted-2)">(необязательно)</span></label><input id="u-handle" class="tin mono" placeholder="@nick" bind:value={uHandle} /></div>
          </div>
          <div style="margin-top:8px"><button class="btn-ghost" disabled={busy} onclick={saveProfile}><Icon name="user" class="ic-sm" /> Сохранить профиль</button></div>
        </div>
```

- [ ] **Step 8: Удалить mock.ts**
```powershell
Remove-Item src/lib/mock.ts
```
Убедиться, что больше нет импортов из `$lib/mock` (grep). Если есть прочие — поправить.

- [ ] **Step 9: Проверка**
```powershell
npm run check
npm test
npm run build
```
Expected: 0 ошибок типов (нет висящих импортов `mock`); Vitest 3 (3 passed); build успешен. `user` иконка — валидная lucide.

- [ ] **Step 10: Commit**
```powershell
git add src/lib/types.ts src/lib/api/user.ts src/lib/stores/user.ts src/routes/+page.svelte src/lib/components/Sidebar.svelte src/lib/components/Dashboard.svelte src/lib/components/AppSettings.svelte
git rm src/lib/mock.ts
git commit -m @'
feat(frontend): editable user profile (name/handle) in app settings

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

## Task 3: Проверка

- [ ] **Step 1: Автопроверка**
```powershell
npm run check ; npm test ; cargo test --manifest-path src-tauri/Cargo.toml --lib
```
Expected: типы 0; Vitest 3; Rust 35.

- [ ] **Step 2: GUI-смоук (пользователь)**
- [ ] Настройки → Профиль → ввести имя/хэндл → Сохранить → Sidebar и Dashboard сразу показывают новое имя; инициалы выводятся из имени.
- [ ] Перезапуск приложения сохраняет имя.

---

## Итог

Имя пользователя редактируется в настройках и хранится в БД; «Артём» и `mock.ts` удалены.
