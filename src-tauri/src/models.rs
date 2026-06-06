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
