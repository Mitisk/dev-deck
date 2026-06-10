# Креды: путь к ключу, кнопка PuTTY, генератор/индикатор пароля

Дата: 2026-06-10
Статус: согласовано, к реализации
Sub-project 1 из 2 (второй — global/pinned креды, отдельная спека позже).

## Проблема / цель

Расширить редактор и карточку креда:
1. **Путь к ключу** — новое поле `key_path` (например, к `.ppk`/`.pem`) с выбором файла.
2. **Кнопка PuTTY** — для ssh-кредов открыть PuTTY с этими доступами.
3. **Прикольное** — генератор крепкого пароля + индикатор силы в редакторе.

## 1. Data model

Миграция `src-tauri/migrations/0006_cred_key_path.sql`:
```sql
ALTER TABLE credentials ADD COLUMN key_path TEXT;

PRAGMA user_version = 6;
```

- `db/migrations.rs`: зарегистрировать файл в массиве `MIGRATIONS`:
  `(6, include_str!("../../migrations/0006_cred_key_path.sql")),`. БЕЗ этого
  миграция не применяется (файлы встроены через `include_str!`, не сканируются).
- `db/migrations.rs`: в тесте `applies_migrations_on_empty_db` обновить
  `assert_eq!(schema_version(&conn).unwrap(), 5)` → `6` (после добавления миграции).
- `models.rs`: в `Credential` добавить `pub key_path: Option<String>`; в `CredInput`
  добавить `pub key_path: Option<String>`.
- `creds.rs`: `row_to_cred` — добавить `key_path` в SELECT и в конструктор;
  `creds_create` — вставлять `key_path`; `creds_update` — обновлять `key_path`.

## 2. Backend — команда `launch_putty`

В [creds.rs](../../../src-tauri/src/commands/creds.rs) (там доступ к БД, `lock`,
`decrypt_secret`, `state.master_key`). Команда берёт **только `id`** — секрет
расшифровывается на бэке и не уходит во фронт.

### Чистый хелпер `build_putty_args` (тестируемый)
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

/// Собрать argv для putty (без имени программы). Политика «ключ → опц. пароль»:
/// если есть key_path — `-i key` и пароль НЕ передаём; иначе если есть пароль — `-pw`.
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
```

### Поиск и запуск putty.exe
```rust
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
```
(потребуется `use std::process::Command;` в creds.rs)

### Команда
```rust
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
    // Пароль нужен только когда ключа нет.
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
- Регистрация в `tauri::generate_handler![...]` ([lib.rs](../../../src-tauri/src/lib.rs))
  рядом с `commands::creds::*`.
- `locked` мастер-пароль → `decrypt_secret` вернёт `AppError{kind: locked}` → фронт
  показывает свой тост (как `getSecretSafe`).

## 3. Frontend — типы и API

- `types.ts`: `Credential` += `keyPath: string | null`.
- `src/lib/api/creds.ts`: `CredInput` += `keyPath?: string | null`.
- `src/lib/api/actions.ts`: `launchPutty = (id) => call<void>("launch_putty", { id })`.

## 4. Frontend — чистые хелперы `src/lib/password.ts` (тестируемые)

```ts
const CHARSET = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!@#$%^&*-_=+";

// Крепкий случайный пароль заданной длины (8..64, дефолт 20).
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

## 5. Frontend — редактор креда (CredsTab.svelte)

- Состояние: `fKeyPath = $state("")`, `genLen = $state(20)`. В `openNew`/`openEdit`
  выставлять `fKeyPath = c.keyPath ?? ""`. В `save()` — `keyPath: fKeyPath.trim() || null`.
- **Поле «Путь к ключу»** в `set-grid`: input + кнопка «Выбрать…» (через
  `import { open } from "@tauri-apps/plugin-dialog"`; фильтры `ppk/pem/key` + «Все
  файлы»; `multiple:false`; если выбран и не массив — записать в `fKeyPath`).
- **Секрет**: рядом с полем — степпер длины (`number`, 8..64) и кнопка «генерировать»
  (`fSecret = generatePassword(genLen)`); под полем — индикатор силы (4 сегмента,
  закрашены до `passwordStrength(fSecret)`), показывается только при непустом `fSecret`.

## 6. Frontend — карточка креда (CredsTab.svelte)

- **Строка «Ключ»** (если `c.keyPath`): моноширинный путь + кнопка «копировать».
- **Кнопка PuTTY** для `c.type === 'ssh'` — в `.cred-head` рядом с карандашом
  (иконка `terminal`/`square-terminal`); `onclick` → `actions.launchPutty(c.id)`
  с обёрткой, которая ловит `kind === 'locked'` и показывает тост «Разблокируйте
  мастер-паролем».

## 7. CSS (global.css)

- `.pw-meter` + сегменты с цветом по уровню (красный → жёлтый → зелёный, переменные
  `--danger`/`--git-dirty`/`--git-ahead`).
- `.secret-row` (флекс: поле + степпер + кнопка), `.gen-len` (узкий number).
- Кнопка PuTTY — переиспользует `.mini` (как карандаш редактирования).

## 8. Безопасность

- Секрет расшифровывается только на бэке внутри `launch_putty`; в JS при запуске не
  передаётся.
- argv формируется без shell (`Command::new(...).args(...)`) — инъекция через
  путь/хост невозможна.
- Остаточный риск: `-pw <секрет>` виден в командной строке процесса ОС. Применяется
  только когда ключ НЕ задан; озвучено пользователю; приемлемо для локального
  однопользовательского ПК. При наличии ключа пароль не передаётся вовсе.

## 9. Тестирование

- **Rust** ([creds.rs](../../../src-tauri/src/commands/creds.rs), `#[cfg(test)]`):
  - `split_host_port`: `"root@h:2222"` → `("root@h", Some("2222"))`; `"h"` →
    `("h", None)`; `"h:abc"` → `("h:abc", None)` (порт не цифры).
  - `build_putty_args`: ключ задан → есть `-i`, нет `-pw`; ключа нет, есть пароль →
    есть `-pw`, нет `-i`; есть порт → есть `-P`.
- **Фронт** (Vitest, node-env): `passwordStrength("")===0`, слабый < сильный,
  сильный === 4; `generatePassword(20).length===20` и все символы из `CHARSET`;
  кламп длины (`generatePassword(2).length===8`, `generatePassword(100).length===64`).
- Сам `spawn_putty` и запуск PuTTY — ручная проверка.

## Не-цели (вне scope)

- **Экспорт/импорт `key_path`** (`transfer.rs`) НЕ трогаем. `key_path` — это
  локальный путь файловой системы конкретного ПК, переносить его между машинами
  бессмысленно; новый nullable-столбец не ломает существующий export/import
  (он просто не включает это поле, при импорте остаётся NULL).
- Global/pinned креды — отдельный sub-project 2.

## Затрагиваемые файлы

Backend: `src-tauri/migrations/0006_cred_key_path.sql` (новый),
`src-tauri/src/db/migrations.rs` (регистрация + правка теста версии),
`src-tauri/src/models.rs`, `src-tauri/src/commands/creds.rs`, `src-tauri/src/lib.rs`.
Frontend: `src/lib/types.ts`, `src/lib/api/creds.ts`, `src/lib/api/actions.ts`,
`src/lib/password.ts` (новый, +тест `src/tests/password.test.ts`),
`src/lib/components/CredsTab.svelte`, `src/lib/styles/global.css`.
