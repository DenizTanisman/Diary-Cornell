-- LLM Bridge settings (MD 03 / Faz 3.2).
--
-- Singleton row (id = 1). Bridge URL + per-deployment API key live here so
-- the UI can flip features without an app restart. AI outputs are NOT
-- synced to Cloud — see `ai_*` columns on diary_entries.

CREATE TABLE IF NOT EXISTS llm_settings (
    id                   INTEGER PRIMARY KEY CHECK (id = 1),
    enabled              BOOLEAN NOT NULL DEFAULT FALSE,
    bridge_url           TEXT NOT NULL DEFAULT 'http://localhost:8765',
    bridge_api_key       TEXT,
    auto_summarize       BOOLEAN NOT NULL DEFAULT FALSE,
    auto_tag             BOOLEAN NOT NULL DEFAULT FALSE,
    preferred_language   TEXT NOT NULL DEFAULT 'auto',
    updated_at           TIMESTAMPTZ NOT NULL DEFAULT now()
);

INSERT INTO llm_settings (id) VALUES (1) ON CONFLICT (id) DO NOTHING;

-- Per-entry AI artefacts. None of these columns are ever pushed to the
-- Cloud sync surface — Faz 3.2 keeps AI outputs device-local for privacy.
ALTER TABLE diary_entries ADD COLUMN IF NOT EXISTS ai_summary       TEXT;
ALTER TABLE diary_entries ADD COLUMN IF NOT EXISTS ai_tags          TEXT;
ALTER TABLE diary_entries ADD COLUMN IF NOT EXISTS ai_sentiment     TEXT;
ALTER TABLE diary_entries ADD COLUMN IF NOT EXISTS ai_generated_at  TIMESTAMPTZ;
