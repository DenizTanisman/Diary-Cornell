-- SQLite mirror of postgres_migrations/0003_pending_ops.sql.
-- BIGSERIAL → INTEGER PRIMARY KEY AUTOINCREMENT, JSONB → TEXT (we
-- json.serialize/json.deserialize at the repo boundary so callers see
-- the same Value type regardless of backend).

CREATE TABLE IF NOT EXISTS pending_ops (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    entry_date   TEXT NOT NULL REFERENCES diary_entries(date) ON DELETE CASCADE,
    field_name   TEXT NOT NULL,
    op_payload   TEXT NOT NULL,
    created_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    pushed       INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_pending_ops_unpushed
    ON pending_ops(pushed, created_at)
    WHERE pushed = 0;
