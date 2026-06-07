-- Файлы/папки-ярлыки проекта (открываются ассоциированной программой/в проводнике).
CREATE TABLE files (
    id         INTEGER PRIMARY KEY,
    project_id INTEGER REFERENCES projects(id) ON DELETE CASCADE,
    label      TEXT NOT NULL,
    path       TEXT NOT NULL,
    sort_order INTEGER DEFAULT 0
);

PRAGMA user_version = 2;
