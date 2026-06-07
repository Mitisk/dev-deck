use crate::error::{AppError, AppResult, ErrorKind};
use rusqlite::Connection;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const KEEP: usize = 10;
const PREFIX: &str = "devdeck-backup-";
static SEQ: AtomicU64 = AtomicU64::new(0);

fn epoch_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Список файлов бэкапов в папке (полные пути).
fn list_files(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with(PREFIX) && n.ends_with(".db"))
                .unwrap_or(false)
        })
        .collect();
    // новые сверху (по mtime)
    v.sort_by_key(|p| std::cmp::Reverse(mtime(p)));
    v
}

fn mtime(p: &Path) -> SystemTime {
    std::fs::metadata(p).and_then(|m| m.modified()).unwrap_or(UNIX_EPOCH)
}

/// Удалить старые бэкапы сверх `keep` (оставить самые свежие).
fn prune(dir: &Path, keep: usize) {
    let files = list_files(dir);
    for old in files.into_iter().skip(keep) {
        let _ = std::fs::remove_file(old);
    }
}

/// Сделать консистентный снимок БД через VACUUM INTO. Возвращает путь копии.
pub fn make_backup(conn: &Connection, dir: &Path) -> AppResult<PathBuf> {
    std::fs::create_dir_all(dir).map_err(|e| AppError { kind: ErrorKind::Io, message: format!("Папка бэкапов: {}", e) })?;
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    let dest = dir.join(format!("{}{}-{}.db", PREFIX, epoch_secs(), seq));
    // VACUUM INTO принимает строковый литерал — путь контролируется приложением,
    // экранируем одинарные кавычки на всякий случай.
    let path_str = dest.to_string_lossy().replace('\'', "''");
    conn.execute_batch(&format!("VACUUM INTO '{}';", path_str))?;
    prune(dir, KEEP);
    Ok(dest)
}

/// Авто-бэкап при старте: пропустить, если самый свежий бэкап моложе ~20 часов.
pub fn maybe_auto_backup(conn: &Connection, dir: &Path) {
    let recent = list_files(dir)
        .first()
        .map(|p| mtime(p))
        .and_then(|t| SystemTime::now().duration_since(t).ok())
        .map(|age| age < Duration::from_secs(20 * 3600))
        .unwrap_or(false);
    if !recent {
        let _ = make_backup(conn, dir); // тихо: ошибка бэкапа не должна валить старт
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    #[test]
    fn make_backup_creates_file_and_prunes() {
        let conn = Connection::open_in_memory().unwrap();
        db::migrations::run(&conn).unwrap();
        let dir = std::env::temp_dir().join(format!("devdeck_bk_{}_{}", std::process::id(), epoch_secs()));
        let _ = std::fs::remove_dir_all(&dir);

        // создаём больше KEEP бэкапов
        for _ in 0..(KEEP + 3) {
            make_backup(&conn, &dir).unwrap();
        }
        let files = list_files(&dir);
        assert_eq!(files.len(), KEEP, "должно остаться ровно KEEP бэкапов");
        // каждый бэкап — валидная SQLite-БД с таблицей projects
        let any = &files[0];
        let bk = Connection::open(any).unwrap();
        let cnt: i64 = bk
            .query_row("SELECT count(*) FROM sqlite_master WHERE type='table' AND name='projects'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(cnt, 1);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
