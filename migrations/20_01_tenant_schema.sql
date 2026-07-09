-- Phase 20-01: Tenant Schema for Multi-Tenancy Support

-- tenants table: organizations/workspaces that own rules and users
CREATE TABLE IF NOT EXISTS tenants (
    id VARCHAR(36) PRIMARY KEY,
    name VARCHAR(100) UNIQUE NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Create index on name for lookups by tenant name.
CREATE INDEX IF NOT EXISTS idx_tenants_name ON tenants(name);
