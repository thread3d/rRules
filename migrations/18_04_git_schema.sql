-- Phase 18-04: Git Versioning Schema

-- Optional: Track branches in the database for metadata
CREATE TABLE IF NOT EXISTS git_branches (
    id SERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL UNIQUE,
    is_default BOOLEAN DEFAULT FALSE,
    commit_sha VARCHAR(64),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT Now()
);

-- Create index for efficient branch lookups
CREATE INDEX IF NOT EXISTS idx_git_branches_name ON git_branches(name);
CREATE INDEX IF NOT EXISTS idx_git_branches_default ON git_branches(is_default);
