-- Папки проектов (один уровень). Удаление папки возвращает проекты в корень.
CREATE TABLE project_groups (
    id         INTEGER PRIMARY KEY,
    name       TEXT NOT NULL,
    sort_order INTEGER DEFAULT 0
);
ALTER TABLE projects ADD COLUMN group_id INTEGER REFERENCES project_groups(id) ON DELETE SET NULL;

PRAGMA user_version = 10;
