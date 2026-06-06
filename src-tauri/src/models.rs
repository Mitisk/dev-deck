use serde::{Deserialize, Serialize};

/// Запись проекта (camelCase для фронта).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub status: String, // active | paused | done | archived
    pub color: Option<String>,
    pub icon: Option<String>, // emoji
    pub path: Option<String>,
    pub repo_path: Option<String>,
    pub pinned: bool,
    pub sort_order: i64,
    pub tags: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Вход на создание/обновление проекта.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInput {
    pub name: String,
    pub description: Option<String>,
    pub status: Option<String>,
    pub color: Option<String>,
    pub icon: Option<String>,
    pub path: Option<String>,
    pub repo_path: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Снимок состояния git-репозитория (read-only).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitStatus {
    pub branch: Option<String>,
    pub ahead: usize,
    pub behind: usize,
    pub dirty: usize,
    pub staged: usize,
    pub untracked: usize,
    pub last_hash: Option<String>,
    pub last_message: Option<String>,
    pub last_timestamp: Option<i64>, // unix seconds; форматируется на фронте
}
