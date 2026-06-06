use rusqlite::Connection;
use std::sync::Mutex;

/// Глобальное состояние приложения. rusqlite синхронный, пользователь один —
/// Mutex<Connection> достаточно, пул не нужен.
pub struct AppState {
    pub db: Mutex<Connection>,
}
