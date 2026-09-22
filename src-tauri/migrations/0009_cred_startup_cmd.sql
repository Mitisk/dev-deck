ALTER TABLE credentials ADD COLUMN startup_cmd TEXT;

PRAGMA user_version = 9;
