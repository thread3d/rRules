-- The audit trail (`api::v1::audit`), when a deployment persists it.
--
-- The in-memory trail is the default and vanishes on restart, which is a poor property for a record
-- whose purpose is to be evidence. This table is append-only by contract: the API never updates or
-- deletes a row, and `id` is the event's UUID.

CREATE TABLE IF NOT EXISTS audit_events (
    id              UUID PRIMARY KEY,
    occurred_at     TIMESTAMPTZ NOT NULL,
    user_id         TEXT,
    action          TEXT NOT NULL,
    resource_type   TEXT NOT NULL,
    resource_id     TEXT NOT NULL,
    resource_name   TEXT,
    details         JSONB NOT NULL DEFAULT '{}'::jsonb,
    ip_address      TEXT,
    user_agent      TEXT
);

-- The trail is read newest-first, and filtered by the three columns a reader narrows on.
CREATE INDEX IF NOT EXISTS audit_events_occurred_at_idx ON audit_events (occurred_at DESC);
CREATE INDEX IF NOT EXISTS audit_events_action_idx ON audit_events (action);
CREATE INDEX IF NOT EXISTS audit_events_resource_idx ON audit_events (resource_type, resource_id);
