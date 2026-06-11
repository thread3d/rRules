# Phase 18-01: RBAC Implementation - Plan

**Phase:** 18.01  
**Goal:** Production-ready Role-Based Access Control with PostgreSQL storage and Actix Web middleware  
**Depends on:** Phase 17 frontend foundation  

---

## Overview

Implement enterprise-grade RBAC for rRules BRMS:
- **Roles:** admin, author, reviewer, viewer
- **Storage:** PostgreSQL database (roles, permissions, assignments)
- **Auth:** JWT with `actix-jwt` middleware + custom RBAC validation
- **Frontend:** Route guards and permission hooks

---

## Success Criteria (What Must Be TRUE)

1. Users have distinct roles (admin/author/reviewer/viewer) with different API access levels
2. Rule creation/editing requires appropriate permissions (author role)
3. Rule approval workflow is restricted to reviewer/admin roles
4. Audit events record role assignments and permission changes
5. Frontend hides unauthorized features based on user role

---

## Implementation Tasks

### Backend: Database Schema (Wave 0)

| File | Purpose |
|------|---------|
| `migrations/18_01_rbac_schema.sql` | Create roles, permissions, role_assignments tables |

```sql
-- roles table: predefined and custom roles
CREATE TABLE roles (
    id SERIAL PRIMARY KEY,
    name VARCHAR(50) UNIQUE NOT NULL,
    description TEXT,
    is_default BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- permissions table: granular permission definitions
CREATE TABLE permissions (
    id SERIAL PRIMARY KEY,
    resource VARCHAR(50) NOT NULL,  -- e.g., "rule", "approval", "user"
    action VARCHAR(50) NOT NULL,     -- e.g., "read", "create", "delete"
    scope VARCHAR(50) DEFAULT '*',   -- "*" or specific category/owner_id
    UNIQUE(resource, action, scope)
);

-- role_permissions junction table
CREATE TABLE role_permissions (
    role_id INTEGER REFERENCES roles(id) ON DELETE CASCADE,
    permission_id INTEGER REFERENCES permissions(id) ON DELETE CASCADE,
    PRIMARY KEY (role_id, permission_id)
);

-- users table (extend existing if exists)
ALTER TABLE users ADD COLUMN default_role_id INTEGER REFERENCES roles(id);

-- role_assignments for user-role mapping
CREATE TABLE role_assignments (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role_id INTEGER NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
    assigned_by INTEGER REFERENCES users(id),
    assigned_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    UNIQUE(user_id, role_id)
);
```

---

### Backend: RBAC Domain Module (Wave 1)

| File | Purpose |
|------|---------|
| `src/governance/rbac/mod.rs` | Module exports and types |
| `src/governance/rbac/roles.rs` | Role management (CRUD, queries) |
| `src/governance/rbac/permissions.rs` | Permission enums and checks |
| `src/governance/rbac/store.rs` | PostgreSQL-backed storage operations |

```rust
// src/governance/rbac/types.rs
#[derive(Clone, Serialize, Deserialize)]
pub enum Role {
    Admin,      // Full access to all features
    Author,     // Create/edit rules, submit for approval
    Reviewer,   // Approve/reject rule changes
    Viewer,     // Read-only access
}

impl Role {
    pub fn name(&self) -> &'static str { /* ... */ }
    pub fn description(&self) -> &'static str { /* ... */ }
}

// Permission checking trait
pub trait HasPermissions {
    fn has_permission(&self, resource: &str, action: &str, scope: &str) -> bool;
}
```

---

### Backend: JWT + RBAC Middleware (Wave 1)

| File | Purpose |
|------|---------|
| `src/api/v1/middleware/auth.rs` | JWT authentication wrapper |
| `src/api/v1/middleware/rbac.rs` | Role/permission validation |

```rust
// src/api/v1/middleware/rbac.rs
use actix_web::{
    dev::{Service, ServiceRequest, ServiceResponse, Transform},
    Error,
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct RbacClaims {
    pub sub: String,                    // user_id
    pub roles: Vec<String>,            // role names
    pub permissions: Vec<String>,      // resolved permission strings
}

pub struct RbacMiddleware {
    required_role: Option<String>,
    required_permission: Option<(String, String)>,
}

impl<S> Transform<S, ServiceRequest> for RbacMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse, Error = Error>,
{
    type Response = ServiceResponse;
    type Error = Error;
    type InitError = ();
    type Transform = RbacMiddlewareService<S>;
    type Future = Result<Self::Transform, Self::InitError>;

    fn new_transform(&self, service: S) -> Self::Future {
        Ok(RbacMiddlewareService { service, ... })
    }
}

pub struct RbacMiddlewareService<S> {
    service: S,
    required_role: Option<String>,
    required_permission: Option<(String, String)>,
}
```

---

### Backend: REST API Endpoints (Wave 2)

| Endpoint | Method | Role Required | Purpose |
|----------|--------|---------------|---------|
| `/api/v1/rbac/roles` | GET | admin | List all roles |
| `/api/v1/rbac/roles/{id}` | GET | admin | Get role details |
| `/api/v1/rbac/roles` | POST | admin | Create new role |
| `/api/v1/rbac/roles/{id}` | PUT | admin | Update role |
| `/api/v1/rbac/roles/{id}` | DELETE | admin | Delete role |
| `/api/v1/rbac/users/{user_id}/roles` | GET | self or admin | Get user roles |
| `/api/v1/rbac/users/{user_id}/roles` | POST | admin | Assign role to user |

---

### Frontend: Route Guards (Wave 2)

| File | Purpose |
|------|---------|
| `frontend/src/features/governance/rbac/ProtectedRoute.tsx` | Route protection component |
| `frontend/src/features/governance/rbac/usePermission.ts` | Permission checking hook |
| `frontend/src/lib/auth-context.tsx` | Update with role-based route guards |

```typescript
// frontend/src/features/governance/rbac/usePermission.ts
import { useContext } from 'react';
import { AuthContext } from '@/lib/auth-context';

export function usePermission() {
  const { user, isAuthenticated } = useContext(AuthContext);
  
  const hasPermission = (resource: string, action: string, scope: string = '*') => {
    if (!isAuthenticated || !user) return false;
    if (user.role === 'admin') return true; // Admin has everything
    
    // Check user's permissions
    return user.permissions.some(
      p => p.resource === resource && 
           (p.action === action || p.action === '*') &&
           (p.scope === scope || p.scope === '*')
    );
  };

  const canCreateRules = () => hasPermission('rule', 'create');
  const canApproveRules = () => hasPermission('approval', 'approve');
  
  return { hasPermission, canCreateRules, canApproveRules };
}
```

---

### Frontend: Role Management UI (Wave 3)

| File | Purpose |
|------|---------|
| `frontend/src/features/governance/rbac/RoleManager.tsx` | Role CRUD interface |
| `frontend/src/features/governance/rbac/UserList.tsx` | User list with role badges |
| `frontend/src/features/governance/rbac/PermissionEditor.tsx` | Permission editing |

---

## Testing

### Unit Tests

```rust
// tests/governance/rbac/mod.rs
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_admin_has_all_permissions() {
        let claims = RbacClaims { sub: "1".into(), roles: vec!["admin".into()], permissions: vec![] };
        assert!(claims.has_permission("rule", "create"));
        assert!(claims.has_permission("approval", "approve"));
    }
    
    #[test]
    fn test_viewer_cannot_edit_rules() {
        let claims = RbacClaims { sub: "2".into(), roles: vec!["viewer".into()], permissions: vec![] };
        assert!(!claims.has_permission("rule", "create"));
        assert!(!claims.has_permission("approval", "approve"));
    }
}
```

### Integration Tests

| Test | Endpoint/Component | Pass Condition |
|------|-------------------|----------------|
| RBAC-01-01 | GET `/api/v1/rbac/roles` (admin) | Returns 200 with all roles |
| RBAC-01-02 | POST `/api/v1/rbac/roles` (viewer) | Returns 403 Forbidden |
| RBAC-01-03 | Protected route (author) | Route accessible |
| RBAC-01-04 | Protected route (viewer) | Route returns 403 |

---

## Acceptance Checklist

- [x] Database schema created and migrated
- [x] RBAC module compiles with `cargo check --lib governance::rbac`
- [ ] JWT authentication works with custom claims
- [ ] RbacMiddleware blocks unauthorized access (403 response)
- [ ] Admin can create roles via API
- [ ] Frontend route guards hide unauthorized features
- [ ] Test suite passes: `cargo test governance::rbac`

**Status: PARTIAL - Backend complete, middleware and frontend pending**

---

## Dependencies

### Cargo.toml additions
```toml
[dependencies]
actix-jwt = "0.3.0"
serde_json = "1.0"

[dev-dependencies]
mockall = "0.13"
```

### Frontend dependencies (already in package.json)
- `@tanstack/react-router` - Routing with guards
- `axios` - HTTP client

---

## Notes

- **Rollback plan:** Drop roles/permissions/assignments tables if rollback needed
- **Migration strategy:** Add role_assignments table, backfill from existing user.role field
- **Breaking change:** Existing users without roles default to "viewer" role
- **Future enhancement:** Support dynamic permission inheritance via permission groups

---

*Plan created: 2026-06-10*
*Phase: 18.01 - RBAC Implementation*
