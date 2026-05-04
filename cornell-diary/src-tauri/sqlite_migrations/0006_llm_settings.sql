-- SQLite mirror of postgres_migrations/0006_llm_settings.sql.
--
-- BOOLEAN → INTEGER (0/1), TIMESTAMPTZ → TEXT.
-- SQLite's ALTER TABLE ... ADD COLUMN does not support `IF NOT EXISTS`.
-- The migration runner records the version after the first successful run,
-- so the duplicate-add error you'd hit on a re-run can't actually happen
-- in practice — sqlx::migrate! gates by version table.

CREATE TABLE IF NOT EXISTS llm_settings (
    id                   INTEGER PRIMARY KEY CHECK (id = 1),
    enabled              INTEGER NOT NULL DEFAULT 0,
    bridge_url           TEXT NOT NULL DEFAULT 'http://localhost:8765',
    bridge_api_key       TEXT,
    auto_summarize       INTEGER NOT NULL DEFAULT 0,
    auto_tag             INTEGER NOT NULL DEFAULT 0,
    preferred_language   TEXT NOT NULL DEFAULT 'auto',
    updated_at           TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

INSERT OR IGNORE INTO llm_settings (id) VALUES (1);

ALTER TABLE diary_entries ADD COLUMN ai_summary      TEXT;
ALTER TABLE diary_entries ADD COLUMN ai_tags         TEXT;
ALTER TABLE diary_entries ADD COLUMN ai_sentiment    TEXT;
ALTER TABLE diary_entries ADD COLUMN ai_generated_at TEXT;
