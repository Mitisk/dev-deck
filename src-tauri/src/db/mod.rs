pub mod migrations;

use crate::error::AppResult;
use rusqlite::Connection;
use std::path::Path;

/// Открывает (создавая при необходимости) БД по пути и накатывает миграции.
pub fn open(path: &Path) -> AppResult<Connection> {
    let conn = Connection::open(path)?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    migrations::run(&conn)?;
    Ok(conn)
}
