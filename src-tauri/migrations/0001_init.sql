-- Проекты
CREATE TABLE projects (
    id           INTEGER PRIMARY KEY,
    name         TEXT NOT NULL,
    description  TEXT,
    status       TEXT DEFAULT 'active',   -- active | paused | done | archived
    color        TEXT,                    -- HEX для акцента карточки
    icon         TEXT,                    -- emoji или имя иконки
    path         TEXT,                    -- корневая папка проекта
    repo_path    TEXT,                    -- путь к git-репо (часто = path)
    pinned       INTEGER DEFAULT 0,
    sort_order   INTEGER DEFAULT 0,
    created_at   TEXT DEFAULT (datetime('now')),
    updated_at   TEXT DEFAULT (datetime('now'))
);

-- Теги
CREATE TABLE tags (
    id    INTEGER PRIMARY KEY,
    name  TEXT NOT NULL UNIQUE,
    color TEXT
);
CREATE TABLE project_tags (
    project_id INTEGER REFERENCES projects(id) ON DELETE CASCADE,
    tag_id     INTEGER REFERENCES tags(id) ON DELETE CASCADE,
    PRIMARY KEY (project_id, tag_id)
);

-- Креды (секрет хранится зашифрованным)
CREATE TABLE credentials (
    id               INTEGER PRIMARY KEY,
    project_id       INTEGER REFERENCES projects(id) ON DELETE CASCADE,
    label            TEXT NOT NULL,           -- "Прод БД", "Stripe API" и т.п.
    type             TEXT NOT NULL,           -- login | api_key | token | ssh | conn_string | note
    username         TEXT,                    -- логин/хост (не секрет)
    url              TEXT,
    secret_encrypted BLOB,                    -- зашифрованный секрет
    notes            TEXT,
    sort_order       INTEGER DEFAULT 0,
    created_at       TEXT DEFAULT (datetime('now')),
    updated_at       TEXT DEFAULT (datetime('now'))
);

-- Задачи (мини-канбан)
CREATE TABLE tasks (
    id           INTEGER PRIMARY KEY,
    project_id   INTEGER REFERENCES projects(id) ON DELETE CASCADE,
    title        TEXT NOT NULL,
    description  TEXT,
    status       TEXT DEFAULT 'todo',     -- todo | doing | done
    priority     INTEGER DEFAULT 0,       -- 0 норм, 1 важно, 2 срочно
    due_date     TEXT,
    sort_order   INTEGER DEFAULT 0,
    created_at   TEXT DEFAULT (datetime('now')),
    completed_at TEXT
);

-- Чеклисты
CREATE TABLE checklists (
    id         INTEGER PRIMARY KEY,
    project_id INTEGER REFERENCES projects(id) ON DELETE CASCADE,
    title      TEXT NOT NULL,
    sort_order INTEGER DEFAULT 0
);
CREATE TABLE checklist_items (
    id           INTEGER PRIMARY KEY,
    checklist_id INTEGER REFERENCES checklists(id) ON DELETE CASCADE,
    text         TEXT NOT NULL,
    is_done      INTEGER DEFAULT 0,
    sort_order   INTEGER DEFAULT 0
);
-- Шаблоны чеклистов (переиспользуемые, не привязаны к проекту)
CREATE TABLE checklist_templates (
    id         INTEGER PRIMARY KEY,
    name       TEXT NOT NULL,
    items_json TEXT NOT NULL            -- JSON-массив строк
);

-- Заметки (Markdown), может быть несколько на проект
CREATE TABLE notes (
    id         INTEGER PRIMARY KEY,
    project_id INTEGER REFERENCES projects(id) ON DELETE CASCADE,
    title      TEXT,
    content_md TEXT,
    updated_at TEXT DEFAULT (datetime('now'))
);

-- Быстрые ссылки/закладки (localhost, репо, дашборды деплоя, доки)
CREATE TABLE links (
    id         INTEGER PRIMARY KEY,
    project_id INTEGER REFERENCES projects(id) ON DELETE CASCADE,
    label      TEXT NOT NULL,
    url        TEXT NOT NULL,
    icon       TEXT,
    sort_order INTEGER DEFAULT 0
);

-- Кастомные команды-кнопки (npm run dev, cargo run, docker compose up)
CREATE TABLE commands (
    id          INTEGER PRIMARY KEY,
    project_id  INTEGER REFERENCES projects(id) ON DELETE CASCADE,
    label       TEXT NOT NULL,
    command     TEXT NOT NULL,           -- shell-команда
    working_dir TEXT,                    -- по умолчанию path проекта
    run_in      TEXT DEFAULT 'terminal', -- terminal | background
    icon        TEXT,
    sort_order  INTEGER DEFAULT 0
);

-- Настройки приложения (ключ-значение)
CREATE TABLE settings (
    key   TEXT PRIMARY KEY,
    value TEXT
);

PRAGMA user_version = 1;
