-- SQLite mirror of postgres_migrations/0004_baseline_version.sql.
-- INTEGER replaces BIGINT (SQLite's INTEGER is 64-bit by default).

ALTER TABLE diary_entries
    ADD COLUMN baseline_version INTEGER NOT NULL DEFAULT 0;

UPDATE diary_entries
    SET baseline_version = version
    WHERE baseline_version = 0 AND version > 0;
