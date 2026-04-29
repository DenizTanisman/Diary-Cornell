-- Initial Postgres schema for Cornell Diary.
--
-- Mirrors the live SQLite schema in migrations/001_initial.sql so the
-- one-shot migration in FAZ 1.2 is a column-for-column copy. The only
-- additions are sync-metadata columns (cloud_entry_id, cloud_journal_id,
-- is_dirty, last_synced_at) that earn their keep starting in FAZ 2.

CREATE TABLE IF NOT EXISTS diary_entries (
    -- Mirrors SQLite (handoff Part I §3.3). We could promote `date` to a
    -- native DATE here, but the existing TS frontend round-trips ISO
    -- strings everywhere — keeping TEXT keeps the storage projection
    -- identical and avoids a schema-migration foot-gun for the Reporter
    -- sidecar (it currently pulls the column with no parsing).
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
    version         BIGINT NOT NULL DEFAULT 1,

    -- New sync metadata. is_dirty defaults TRUE so a row that pre-exists
    -- the first sync is treated as "needs to be pushed once" rather than
    -- "already synchronised". cloud_entry_id null = never reached Cloud yet.
    cloud_entry_id      UUID,
    cloud_journal_id    UUID,
    is_dirty            BOOLEAN NOT NULL DEFAULT TRUE,
    last_synced_at      TIMESTAMPTZ,

    CONSTRAINT diary_entries_date_iso CHECK (length(date) = 10
        AND substring(date FROM 5 FOR 1) = '-'
        AND substring(date FROM 8 FOR 1) = '-')
);

CREATE INDEX IF NOT EXISTS idx_diary_entries_updated  ON diary_entries(updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_diary_entries_dirty    ON diary_entries(is_dirty) WHERE is_dirty = TRUE;
CREATE INDEX IF NOT EXISTS idx_diary_entries_cloud_id ON diary_entries(cloud_entry_id);

-- The QR / JSON manual sync log already exists in SQLite (handoff Part I
-- §3.3 row 4). The Cloud sync path appends `'cloud'` to the method enum so
-- both paths share one audit trail.
CREATE TABLE IF NOT EXISTS sync_log (
    id              BIGSERIAL PRIMARY KEY,
    sync_type       TEXT NOT NULL CHECK (sync_type IN ('export', 'import')),
    method          TEXT NOT NULL CHECK (method IN ('qr', 'json_file', 'cloud')),
    device_id       TEXT NOT NULL,
    peer_device_id  TEXT,
    timestamp       TIMESTAMPTZ NOT NULL DEFAULT now(),
    entry_count     INTEGER NOT NULL DEFAULT 0,
    checksum        TEXT NOT NULL,
    status          TEXT NOT NULL CHECK (status IN ('success', 'partial', 'failed')),
    error_message   TEXT
);
CREATE INDEX IF NOT EXISTS idx_sync_log_timestamp ON sync_log(timestamp DESC);

CREATE TABLE IF NOT EXISTS app_settings (
    key             TEXT PRIMARY KEY,
    value           TEXT NOT NULL,
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Bootstrapping settings — same defaults as the SQLite seed so a fresh
-- Postgres DB feels identical to a freshly opened SQLite one.
INSERT INTO app_settings (key, value, updated_at) VALUES
    ('theme', 'auto', now()),
    ('language', 'tr', now()),
    ('auto_save_interval_ms', '1500', now()),
    ('first_launch_date', to_char(now(), 'YYYY-MM-DD'), now())
ON CONFLICT (key) DO NOTHING;
