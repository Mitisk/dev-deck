use crate::db::migrations::schema_version;
use crate::error::AppResult;
use crate::state::AppState;
use tauri::State;

/// Возвращает версию схемы БД — проверка, что соединение живо и миграции прошли.
#[tauri::command]
pub fn db_health(state: State<AppState>) -> AppResult<i64> {
    let conn = state
        .db
        .lock()
        .map_err(|_| crate::error::AppError::internal("db mutex poisoned"))?;
    schema_version(&conn)
}
