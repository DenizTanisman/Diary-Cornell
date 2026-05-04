-- SQLite mirror of postgres_migrations/0005_cloud_profiles.sql.
--
-- BOOLEAN → INTEGER (0/1), TIMESTAMPTZ → TEXT (ISO-8601 UTC).
-- Partial unique index uses SQLite's WHERE clause syntax, identical
-- semantics to the Postgres definition.

CREATE TABLE IF NOT EXISTS cloud_profiles (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL,
    base_url        TEXT NOT NULL DEFAULT '',
    api_key         TEXT,
    is_active       INTEGER NOT NULL DEFAULT 0,
    last_used_at    TEXT,
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE UNIQUE INDEX IF NOT EXISTS uq_cloud_profiles_one_active
    ON cloud_profiles (is_active) WHERE is_active = 1;

INSERT OR IGNORE INTO cloud_profiles (id, name, base_url, is_active) VALUES
    ('local',      'Local (LAN)',  'http://localhost:5001', 1),
    ('production', 'Production',   '',                       0);
