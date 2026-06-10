ALTER TABLE credentials ADD COLUMN is_global INTEGER NOT NULL DEFAULT 0;

CREATE TABLE cred_order (
    cred_id    INTEGER NOT NULL REFERENCES credentials(id) ON DELETE CASCADE,
    project_id INTEGER NOT NULL REFERENCES projects(id)    ON DELETE CASCADE,
    sort_order INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (cred_id, project_id)
);

PRAGMA user_version = 7;
