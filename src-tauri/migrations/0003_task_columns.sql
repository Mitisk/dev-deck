CREATE TABLE task_columns (
    id         INTEGER PRIMARY KEY,
    project_id INTEGER REFERENCES projects(id) ON DELETE CASCADE,
    key        TEXT NOT NULL,
    name       TEXT NOT NULL,
    is_done    INTEGER DEFAULT 0,
    sort_order INTEGER DEFAULT 0,
    UNIQUE(project_id, key)
);

-- дефолтные колонки для уже существующих проектов
INSERT INTO task_columns(project_id, key, name, is_done, sort_order)
    SELECT id, 'todo', 'To Do', 0, 0 FROM projects;
INSERT INTO task_columns(project_id, key, name, is_done, sort_order)
    SELECT id, 'doing', 'In Progress', 0, 1 FROM projects;
INSERT INTO task_columns(project_id, key, name, is_done, sort_order)
    SELECT id, 'done', 'Done', 1, 2 FROM projects;

PRAGMA user_version = 3;
