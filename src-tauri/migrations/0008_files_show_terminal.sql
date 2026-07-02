ALTER TABLE files ADD COLUMN show_terminal INTEGER NOT NULL DEFAULT 1;

PRAGMA user_version = 8;
