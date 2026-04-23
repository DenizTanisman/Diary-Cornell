-- Cornell Diary Initial Schema (Migration 001)
-- Her gün tek kayıt, 7 sabit cue başlığı, summary + quote.

CREATE TABLE IF NOT EXISTS diary_entries (
    date            TEXT PRIMARY KEY,
    diary           TEXT NOT NULL DEFAULT '',

    title_1         TEXT DEFAULT NULL,
    content_1       TEXT DEFAULT NULL,
    title_2         TEXT DEFAULT NULL,
    content_2       TEXT DEFAULT NULL,
    title_3         TEXT DEFAULT NULL,
    content_3       TEXT DEFAULT NULL,
    title_4         TEXT DEFAULT NULL,
    content_4       TEXT DEFAULT NULL,
    title_5         TEXT DEFAULT NULL,
    content_5       TEXT DEFAULT NULL,
    title_6         TEXT DEFAULT NULL,
    content_6       TEXT DEFAULT NULL,
    title_7         TEXT DEFAULT NULL,
    content_7       TEXT DEFAULT NULL,

    summary         TEXT DEFAULT '',
    quote           TEXT DEFAULT '',

    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL,
    device_id       TEXT,
    version         INTEGER NOT NULL DEFAULT 1,

    CHECK (length(date) = 10),
    CHECK (substr(date, 5, 1) = '-'),
    CHECK (substr(date, 8, 1) = '-')
);

CREATE TABLE IF NOT EXISTS sync_log (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    sync_type       TEXT NOT NULL CHECK (sync_type IN ('export', 'import')),
    method          TEXT NOT NULL CHECK (method IN ('qr', 'json_file')),
    device_id       TEXT NOT NULL,
    peer_device_id  TEXT,
    timestamp       TEXT NOT NULL,
    entry_count     INTEGER NOT NULL DEFAULT 0,
    checksum        TEXT NOT NULL,
    status          TEXT NOT NULL CHECK (status IN ('success', 'partial', 'failed')),
    error_message   TEXT
);

CREATE TABLE IF NOT EXISTS app_settings (
    key             TEXT PRIMARY KEY,
    value           TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_diary_updated ON diary_entries(updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_sync_timestamp ON sync_log(timestamp DESC);

INSERT OR IGNORE INTO app_settings (key, value, updated_at) VALUES
    ('theme', 'auto', datetime('now')),
    ('language', 'tr', datetime('now')),
    ('auto_save_interval_ms', '1500', datetime('now')),
    ('first_launch_date', datetime('now'), datetime('now'));
