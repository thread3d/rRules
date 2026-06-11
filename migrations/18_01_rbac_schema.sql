-- Phase 18-01: RBAC Schema
-- Roles, permissions, and user-role assignments

-- roles table: predefined and custom roles
CREATE TABLE IF NOT EXISTS roles (
    id SERIAL PRIMARY KEY,
    name VARCHAR(50) UNIQUE NOT NULL,
    description TEXT,
    is_default BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Insert default roles
INSERT INTO roles (name, description, is_default)
VALUES
    ('admin', 'Full access to all features including system configuration', TRUE),
    ('author', 'Create and edit rules, submit for approval', FALSE),
    ('reviewer', 'Review and approve/reject rule changes', FALSE),
    ('viewer', 'Read-only access to rules and audit logs', FALSE)
ON CONFLICT (name) DO NOTHING;

-- permissions table: granular permission definitions
CREATE TABLE IF NOT EXISTS permissions (
    id SERIAL PRIMARY KEY,
    resource VARCHAR(50) NOT NULL,
    action VARCHAR(50) NOT NULL,
    scope VARCHAR(50) DEFAULT '*',
    UNIQUE(resource, action, scope)
);

-- role_permissions junction table
CREATE TABLE IF NOT EXISTS role_permissions (
    role_id INTEGER REFERENCES roles(id) ON DELETE CASCADE,
    permission_id INTEGER REFERENCES permissions(id) ON DELETE CASCADE,
    PRIMARY KEY (role_id, permission_id)
);

-- Create index on role_permissions for efficient lookups
CREATE INDEX IF NOT EXISTS idx_role_permissions_role ON role_permissions(role_id);
CREATE INDEX IF NOT EXISTS idx_role_permissions_permission ON role_permissions(permission_id);
