-- DMN document persistence (`engine_state::PostgresDmnStore`).
--
-- The filesystem store remains authoritative: it is what a deployment's `DMN_STORAGE_PATH` holds and
-- what git tracks. This schema is the database-backed alternative, applied idempotently by
-- `PostgresDmnStore::connect` — the store keeps its own DDL close rather than depending on
-- `sqlx::migrate!`, whose version table cannot express the `18_01`/`18_02`-style names the other
-- files in this directory use.

CREATE TABLE IF NOT EXISTS dmn_documents (
    id          TEXT PRIMARY KEY,
    name        TEXT,
    version     TEXT,
    -- The `.dmn` XML itself, byte for byte: round-tripping it is a requirement, not a convenience.
    content     BYTEA NOT NULL,
    metadata    JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- `list` returns the most recently updated first.
CREATE INDEX IF NOT EXISTS dmn_documents_updated_at_idx
    ON dmn_documents (updated_at DESC);

CREATE TABLE IF NOT EXISTS dmn_files (
    filename    TEXT PRIMARY KEY,
    content     BYTEA NOT NULL,
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
