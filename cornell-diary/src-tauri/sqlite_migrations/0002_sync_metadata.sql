-- SQLite mirror of postgres_migrations/0002_sync_metadata.sql.
-- UUIDs and timestamps are TEXT for SQLite; semantically identical.

CREATE TABLE IF NOT EXISTS sync_metadata (
    id                 INTEGER PRIMARY KEY DEFAULT 1,
    peer_id            TEXT NOT NULL DEFAULT '',
    cloud_user_id      TEXT,
    cloud_journal_id   TEXT,
    access_token       TEXT,
    refresh_token      TEXT,
    token_expires_at   TEXT,
    last_pull_at       TEXT,
    last_push_at       TEXT,
    last_full_sync_at  TEXT,
    sync_enabled       INTEGER NOT NULL DEFAULT 0,
    device_label       TEXT,
    CONSTRAINT sync_metadata_singleton CHECK (id = 1)
);

INSERT OR IGNORE INTO sync_metadata (id) VALUES (1);
