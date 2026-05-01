-- Faz 1.1: per-entry baseline_version. Tracks the server `version`
-- the last successful sync observed; the next push attaches it so
-- Cloud's CRDT-aware merge path can detect a concurrent writer that
-- landed since baseline.
--
-- Existing rows default to their current `version` so they're treated
-- as "baseline = current" (no inferred concurrent change). New rows
-- default to 0 → first push is unconditional.

ALTER TABLE diary_entries
    ADD COLUMN IF NOT EXISTS baseline_version BIGINT NOT NULL DEFAULT 0;

UPDATE diary_entries
    SET baseline_version = version
    WHERE baseline_version = 0 AND version > 0;
