-- Singleton sync_metadata row holding tokens, peer id, last sync timestamps.
-- The CHECK keeps it a single row — Diary is local-first, one device per
-- install. Multi-account would expand this into a non-singleton table, but
-- that's out of scope for FAZ 2.

CREATE TABLE IF NOT EXISTS sync_metadata (
    id                 INTEGER PRIMARY KEY DEFAULT 1,
    peer_id            TEXT NOT NULL DEFAULT '',
    cloud_user_id      UUID,
    cloud_journal_id   UUID,
    access_token       TEXT,
    refresh_token      TEXT,
    token_expires_at   TIMESTAMPTZ,
    last_pull_at       TIMESTAMPTZ,
    last_push_at       TIMESTAMPTZ,
    last_full_sync_at  TIMESTAMPTZ,
    sync_enabled       BOOLEAN NOT NULL DEFAULT FALSE,
    device_label       TEXT,
    CONSTRAINT sync_metadata_singleton CHECK (id = 1)
);

INSERT INTO sync_metadata (id) VALUES (1) ON CONFLICT (id) DO NOTHING;
