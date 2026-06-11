-- Phase 18-02: Approval Workflow Schema

-- approval_workflows: Template definitions for approval processes
CREATE TABLE IF NOT EXISTS approval_workflows (
    id SERIAL PRIMARY KEY,
    name VARCHAR(100) NOT NULL UNIQUE,
    description TEXT,
    tiers JSONB NOT NULL,
    auto_escape_hours INTEGER DEFAULT 48,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- approval_requests: Individual workflow instances
CREATE TABLE IF NOT EXISTS approval_requests (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workflow_id INTEGER REFERENCES approval_workflows(id),
    rule_id UUID NOT NULL,
    maker_id INTEGER NOT NULL REFERENCES users(id),
    state VARCHAR(50) NOT NULL CHECK (state IN ('draft', 'submitted', 'in_review', 'approved', 'rejected', 'escalated')),
    current_tier INTEGER DEFAULT 1,
    reviewers JSONB,
    required_approvals INTEGER DEFAULT 1,
    approved_by INTEGER[],
    rejected_by INTEGER,
    rejection_reason TEXT,
    submitted_at TIMESTAMP WITH TIME ZONE,
    started_at TIMESTAMP WITH TIME ZONE,
    completed_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- approval_webhooks: Webhook configuration for external notifications
CREATE TABLE IF NOT EXISTS approval_webhooks (
    id SERIAL PRIMARY KEY,
    name VARCHAR(100) NOT NULL,
    url TEXT NOT NULL,
    events JSONB NOT NULL DEFAULT '["submitted", "approved", "rejected"]',
    secret TEXT,
    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Create indexes
CREATE INDEX IF NOT EXISTS idx_approval_requests_state ON approval_requests(state);
CREATE INDEX IF NOT EXISTS idx_approval_requests_maker ON approval_requests(maker_id);
CREATE INDEX IF NOT EXISTS idx_approval_requests_rule ON approval_requests(rule_id);
CREATE INDEX IF NOT EXISTS idx_approval_webhooks_active ON approval_webhooks(is_active);
