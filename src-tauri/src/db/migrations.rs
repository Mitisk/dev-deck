use crate::error::AppResult;
use rusqlite::Connection;

/// Встроенные миграции: (версия, SQL). Применяются по возрастанию, если
/// текущий user_version меньше.
const MIGRATIONS: &[(i64, &str)] = &[
    (1, include_str!("../../migrations/0001_init.sql")),
    (2, include_str!("../../migrations/0002_files.sql")),
    (3, include_str!("../../migrations/0003_task_columns.sql")),
];

/// Применяет все миграции с номером выше текущего user_version.
pub fn run(conn: &Connection) -> AppResult<()> {
    let current: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    for (version, sql) in MIGRATIONS {
        if *version > current {
            conn.execute_batch(sql)?;
        }
    }
    Ok(())
}

/// Текущая версия схемы.
pub fn schema_version(conn: &Connection) -> AppResult<i64> {
    Ok(conn.query_row("PRAGMA user_version", [], |r| r.get(0))?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applies_migrations_on_empty_db() {
        let conn = Connection::open_in_memory().unwrap();
        assert_eq!(schema_version(&conn).unwrap(), 0);
        run(&conn).unwrap();
        assert_eq!(schema_version(&conn).unwrap(), 3);
        let count: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='projects'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
        let files: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='files'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(files, 1);
        let tc: i64 = conn.query_row("SELECT count(*) FROM sqlite_master WHERE type='table' AND name='task_columns'", [], |r| r.get(0)).unwrap();
        assert_eq!(tc, 1);
    }

    #[test]
    fn run_is_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        run(&conn).unwrap();
        run(&conn).unwrap();
        assert_eq!(schema_version(&conn).unwrap(), 3);
        let tc: i64 = conn.query_row("SELECT count(*) FROM sqlite_master WHERE type='table' AND name='task_columns'", [], |r| r.get(0)).unwrap();
        assert_eq!(tc, 1);
    }
}
