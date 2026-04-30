-- FAZ 3.2: Offline-durable queue of CharOp's that the WS client has
-- accepted from the user but hasn't been able to broadcast yet (no
-- live socket, or send failed). On reconnect, ws_client.flush_pending
-- drains rows where pushed=false in chronological order, sends them,
-- and flips pushed=true.
--
-- We keep pushed-true rows around for diagnostics; a separate sweeper
-- (FAZ 3.3 / Final) can prune anything older than N days.

-- entry_date is TEXT (matches diary_entries.date which is also TEXT in
-- this schema — Diary stored ISO strings before the Postgres migration
-- and we kept the column shape so existing data round-trips). A DATE
-- column here would refuse to FK back since Postgres requires identical
-- btree opclasses on both sides.
CREATE TABLE IF NOT EXISTS pending_ops (
    id           BIGSERIAL PRIMARY KEY,
    entry_date   TEXT NOT NULL REFERENCES diary_entries(date) ON DELETE CASCADE,
    field_name   TEXT NOT NULL,
    op_payload   JSONB NOT NULL,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    pushed       BOOLEAN NOT NULL DEFAULT FALSE
);

-- Hot path: list_pending_ops needs to find the next unpushed batch
-- without scanning the whole table.
CREATE INDEX IF NOT EXISTS idx_pending_ops_unpushed
    ON pending_ops(pushed, created_at)
    WHERE pushed = FALSE;
