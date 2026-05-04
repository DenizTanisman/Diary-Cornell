-- Cloud profile switch (MD 03 / Faz 3.1).
--
-- Single-user table; rows are seeded once with `local` + `production`
-- entries so the UI can switch base URLs without ever needing a custom
-- profile. Custom profiles are written through `upsert_cloud_profile`.

CREATE TABLE IF NOT EXISTS cloud_profiles (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL,
    base_url        TEXT NOT NULL DEFAULT '',
    api_key         TEXT,
    is_active       BOOLEAN NOT NULL DEFAULT FALSE,
    last_used_at    TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Exactly one active profile at any time. Postgres expresses this with a
-- partial unique index. The repository layer flips the flag inside a
-- transaction so the index never sees two `TRUE` rows mid-write.
CREATE UNIQUE INDEX IF NOT EXISTS uq_cloud_profiles_one_active
    ON cloud_profiles ((1)) WHERE is_active = TRUE;

INSERT INTO cloud_profiles (id, name, base_url, is_active) VALUES
    ('local',      'Local (LAN)',  'http://localhost:5001', TRUE),
    ('production', 'Production',   '',                       FALSE)
ON CONFLICT (id) DO NOTHING;
