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

/// Результат сетевой/коммит-операции git: успех + объединённый вывод (stdout+stderr).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitOpResult {
    pub ok: bool,
    pub output: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: i64,
    pub project_id: i64,
    pub title: String,
    pub description: Option<String>,
    pub status: String, // todo | doing | done
    pub priority: i64,  // 0 | 1 | 2
    pub due_date: Option<String>,
    pub sort_order: i64,
    pub created_at: String,
    pub completed_at: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskInput {
    pub title: String,
    pub description: Option<String>,
    pub status: Option<String>,
    pub priority: Option<i64>,
    pub due_date: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChecklistItem {
    pub id: i64,
    pub checklist_id: i64,
    pub text: String,
    pub is_done: bool,
    pub sort_order: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Checklist {
    pub id: i64,
    pub project_id: i64,
    pub title: String,
    pub sort_order: i64,
    pub items: Vec<ChecklistItem>,
}

/// Метаданные креда (БЕЗ секрета).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Credential {
    pub id: i64,
    pub project_id: i64,
    pub label: String,
    #[serde(rename = "type")]
    pub kind: String, // login | api_key | token | ssh | conn_string | note
    pub username: Option<String>,
    pub url: Option<String>,
    pub notes: Option<String>,
    pub sort_order: i64,
    pub has_secret: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CredInput {
    pub label: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub username: Option<String>,
    pub url: Option<String>,
    pub notes: Option<String>,
    /// None = не менять секрет (при update); Some("") = очистить; Some(x) = задать.
    pub secret: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    pub id: i64,
    pub project_id: i64,
    pub title: Option<String>,
    pub content_md: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Link {
    pub id: i64,
    pub project_id: i64,
    pub label: String,
    pub url: String,
    pub icon: Option<String>,
    pub sort_order: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileShortcut {
    pub id: i64,
    pub project_id: i64,
    pub label: String,
    pub path: String,
    pub sort_order: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCommand {
    pub id: i64,
    pub project_id: i64,
    pub label: String,
    pub command: String,
    pub working_dir: Option<String>,
    pub run_in: String, // terminal | background
    pub icon: Option<String>,
    pub sort_order: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandInput {
    pub label: String,
    pub command: String,
    pub working_dir: Option<String>,
    pub run_in: Option<String>,
    pub icon: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub kind: String,        // project | task | note | link | cred | command | file
    pub project_id: i64,
    pub project_name: String,
    pub id: i64,             // id сущности (для project — id проекта)
    pub title: String,
    pub subtitle: String,
}
