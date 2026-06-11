-- Phase 18-03a: Audit Event Types Schema
-- This file contains the audit_events table definition used by all governance features

-- Create audit_events table (append-only event sourcing)
CREATE TABLE IF NOT EXISTS audit_events (
    id SERIAL PRIMARY KEY,
    event_id UUID NOT NULL UNIQUE DEFAULT gen_random_uuid(),
    event_type VARCHAR(100) NOT NULL,
    actor_id INTEGER REFERENCES users(id),
    subject_id UUID,
    subject_type VARCHAR(50),
    payload JSONB NOT NULL,
    occurred_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    source_ip INET,
    user_agent TEXT
);

-- Create indexes for common query patterns
CREATE INDEX IF NOT EXISTS idx_audit_event_type ON audit_events(event_type);
CREATE INDEX IF NOT EXISTS idx_audit_occurred_at ON audit_events(occurred_at DESC);
CREATE INDEX IF NOT EXISTS idx_audit_actor_id ON audit_events(actor_id);
CREATE INDEX IF NOT EXISTS idx_audit_subject ON audit_events(subject_type, subject_id);

-- Create immutable trigger to prevent updates/deletes
CREATE OR REPLACE FUNCTION immutable_audit_log()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'Audit log is append-only - historical records cannot be modified';
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_audit_immutable ON audit_events;
CREATE TRIGGER trg_audit_immutable
    BEFORE UPDATE OR DELETE ON audit_events
    FOR EACH ROW
    EXECUTE FUNCTION immutable_audit_log();
