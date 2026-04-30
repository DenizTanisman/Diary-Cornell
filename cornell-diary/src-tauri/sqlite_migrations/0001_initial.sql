-- SQLite mirror of postgres_migrations/0001_initial.sql.
--
-- Same logical schema, dialect-adapted: BIGSERIAL → INTEGER PRIMARY KEY
-- AUTOINCREMENT, TIMESTAMPTZ → TEXT (RFC3339 strings, the same shape
-- the Postgres path round-trips through `chrono::DateTime<Utc>`),
-- UUID → TEXT (UUIDs serialise as their hyphenated 36-char form on
-- both backends so no app code branches on the storage layer).
--
-- Cornell Diary's `EntryRepository` trait sees the same column shape
-- through both impls, and `DiaryEntry` JSON is byte-for-byte identical
-- across desktop (Postgres) and Android (SQLite).

CREATE TABLE IF NOT EXISTS diary_entries (
    date            TEXT PRIMARY KEY,
    diary           TEXT NOT NULL DEFAULT '',
    title_1         TEXT, content_1 TEXT,
    title_2         TEXT, content_2 TEXT,
    title_3         TEXT, content_3 TEXT,
    title_4         TEXT, content_4 TEXT,
    title_5         TEXT, content_5 TEXT,
    title_6         TEXT, content_6 TEXT,
    title_7         TEXT, content_7 TEXT,
    summary         TEXT NOT NULL DEFAULT '',
    quote           TEXT NOT NULL DEFAULT '',
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL,
    device_id       TEXT,
    version         INTEGER NOT NULL DEFAULT 1,

    cloud_entry_id      TEXT,
    cloud_journal_id    TEXT,
    is_dirty            INTEGER NOT NULL DEFAULT 1,  -- SQLite has no native BOOL; 0/1
    last_synced_at      TEXT,

    -- SQLite's CHECK is identical syntax; substring slicing also works.
    CONSTRAINT diary_entries_date_iso CHECK (length(date) = 10
        AND substr(date, 5, 1) = '-'
        AND substr(date, 8, 1) = '-')
);

CREATE INDEX IF NOT EXISTS idx_diary_entries_updated  ON diary_entries(updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_diary_entries_dirty    ON diary_entries(is_dirty) WHERE is_dirty = 1;
CREATE INDEX IF NOT EXISTS idx_diary_entries_cloud_id ON diary_entries(cloud_entry_id);

CREATE TABLE IF NOT EXISTS sync_log (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    sync_type       TEXT NOT NULL CHECK (sync_type IN ('export', 'import')),
    method          TEXT NOT NULL CHECK (method IN ('qr', 'json_file', 'cloud')),
    device_id       TEXT NOT NULL,
    peer_device_id  TEXT,
    timestamp       TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    entry_count     INTEGER NOT NULL DEFAULT 0,
    checksum        TEXT NOT NULL,
    status          TEXT NOT NULL CHECK (status IN ('success', 'partial', 'failed')),
    error_message   TEXT
);
CREATE INDEX IF NOT EXISTS idx_sync_log_timestamp ON sync_log(timestamp DESC);

CREATE TABLE IF NOT EXISTS app_settings (
    key             TEXT PRIMARY KEY,
    value           TEXT NOT NULL,
    updated_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

INSERT OR IGNORE INTO app_settings (key, value, updated_at) VALUES
    ('theme',                'auto',                                 strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    ('language',             'tr',                                   strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    ('auto_save_interval_ms','1500',                                 strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    ('first_launch_date',    strftime('%Y-%m-%d', 'now'),            strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
