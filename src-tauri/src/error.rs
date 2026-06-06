use serde::Serialize;

/// Единый тип ошибки команд. Сериализуется во фронт как { kind, message }.
#[derive(Debug, Serialize)]
pub struct AppError {
    pub kind: ErrorKind,
    pub message: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    Db,
    Io,
    NotFound,
    Validation,
    Internal,
}

impl AppError {
    pub fn db(msg: impl Into<String>) -> Self {
        Self { kind: ErrorKind::Db, message: msg.into() }
    }
    pub fn internal(msg: impl Into<String>) -> Self {
        Self { kind: ErrorKind::Internal, message: msg.into() }
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(e: rusqlite::Error) -> Self {
        AppError::db(e.to_string())
    }
}

impl From<git2::Error> for AppError {
    fn from(e: git2::Error) -> Self {
        AppError::internal(e.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;
