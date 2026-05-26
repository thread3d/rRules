---
phase: 04-governance-access-control
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - src/governance/rbac/mod.rs
  - src/governance/rbac/roles.rs
  - src/governance/rbac/permissions.rs
  - src/api/v1/middleware/auth.rs
autonomous: true
requirements:
  - AUTH-01
user_setup: []
must_haves:
  truths:
    - "Users have different access levels based on predefined RBAC roles (admin, author, reviewer)"
    - "Role assignments are stored with user records in PostgreSQL"
    - "Permissions can override role-based access for specific cases"
  artifacts:
    - path: "src/governance/rbac/roles.rs"
      provides: "RBAC role definitions (Admin, Author, Reviewer) with methods"
      exports: ["Role", "RoleType", "all_roles"]
    - path: "src/governance/rbac/permissions.rs"
      provides: "Fine-grained permission enums and check functions"
      exports: ["Permission", "has_permission", "permission_matrix"]
    - path: "src/governance/rbac/mod.rs"
      provides: "RBAC module entry point with re-exports"
      exports: ["RoleManager", "check_access", "get_user_roles"]
  key_links:
    - from: "src/api/v1/middleware/auth.rs"
      to: "src/governance/rbac/roles.rs"
      via: "role verification on request"
      pattern: "Role::from_str|Role::has_permission"
---

<objective>
Implement RBAC system with predefined roles and permissions

Purpose: Establish the foundation for access control throughout rRules. The hybrid RBAC approach (roles + fine-grained permissions) enables both broad role-based access patterns and specific permission overrides for edge cases.

Output:
- Role definitions (Admin, Author, Reviewer)
- Permission system for granular access
- Integration with session-based authentication
</objective>

<execution_context>
@$HOME/.claude/get-shit-done/workflows/execute-plan.md
@$HOME/.claude/get-shhit-done/templates/summary.md
</execution_context>

<context>
@.planning/PROJECT.md
@.planning/ROADMAP.md
@.planning/phases/04-governance-access-control/04-CONTEXT.md

# Phase 3 API Integration (D-04 from CONTEXT.md)

From .planning/phases/03-rest-api-layer/03-CONTEXT.md:
- Session-based authentication - Cookie-based sessions tied to user login
- RESTful CRUD endpoints: POST /rules, GET /rules, PUT /rules/{id}, DELETE /rules/{id}
- Versioned API at /v1/

# Phase 4 RBAC Decisions (D-04 from CONTEXT.md)

## Role Structure:
- `admin`: Full system access (all CRUD + approval + configuration)
- `author`: Create/edit rules, submit for approval; cannot approve own changes
- `reviewer`: Read rules and approve submissions; cannot create/edit directly

## Permission Matrix:
| Permission | Admin | Author | Reviewer |
|-----------|-------|--------|----------|
| RULE_CREATE | ✓ | ✓ | ✗ |
| RULE_READ | ✓ | ✓ | ✓ |
| RULE_UPDATE | ✓ | ✓ | ✗ |
| RULE_DELETE | ✓ | ✓ | ✗ |
| APPROVE_RULE | ✓ | ✗ | ✓ |
| CONFIG_ACCESS | ✓ | ✗ | ✗ |

## Hybrid Access Check Pattern:
```rust
// Step 1: Get user's roles from session
let roles = RoleManager::get_user_roles(&user_id)?;

// Step 2: Check base permission against any role
let has_base = roles.iter().any(|r| r.has_permission(permission));

// Step 3: Check for explicit override permissions (if any)
let has_override = permission_manager::check_overrides(&user_id, &permission)?;

// Result
let allowed = has_base || has_override;
```
</context>

<tasks>

<task type="auto">
  <name>Task 01: Define RBAC role system with Admin/Author/Reviewer types</name>
  <files>src/governance/rbac/mod.rs, src/governance/rbac/roles.rs</files>
  <action>Create the RBAC module structure with three predefined roles:
  
1. Create `src/governance/rbac/mod.rs` - Module entry point that exports RoleManager and role-related functions
  
2. Create `src/governance/rbac/roles.rs` with:
   - `RoleType` enum: Admin, Author, Reviewer
   - `Role` struct: Contains RoleType plus metadata (name, description, created_at)
   - `all_roles()` function: Returns vec of all role definitions
   - `from_str(&str) -> Option<Role>` for parsing from session/cookie
   - `has_permission(&self, permission: Permission) -> bool` method per D-04
   
3. Each role's permissions:
   - Admin: All CRUD (RULE_CREATE, RULE_READ, RULE_UPDATE, RULE_DELETE), APPROVE_RULE, CONFIG_ACCESS
   - Author: RULE_CREATE, RULE_READ, RULE_UPDATE; NO APPROVE_RULE (cannot approve own changes)
   - Reviewer: RULE_READ only; can be granted APPROVE_RULE on specific rules via ownership</action>
  <verify>
    <automated>cd /home/threaded/projects/rRules && cargo test --package governance -- rbac::roles --nocapture</automated>
  </verify>
  <done>Role struct compiles with all required methods; from_str handles "admin", "author", "reviewer" case-insensitively; has_permission returns correct matrix per D-04</done>
</task>

<task type="auto">
  <name>Task 02: Implement fine-grained permission system</name>
  <files>src/governance/rbac/permissions.rs</files>
  <action>Create the permissions module with:
  
1. `Permission` enum with variants:
   - RULE_CREATE, RULE_READ, RULE_UPDATE, RULE_DELETE
   - APPROVE_RULE (for maker-checker workflow)
   - CONFIG_ACCESS (admin-only system config)
   - USER_MANAGE (add/remove users, assign roles)
   - AUDIT_READ (access audit logs)

2. `permission_matrix()` function returning HashMap<RoleType, HashSet<Permission>>

3. `has_permission(user_roles: &[Role], permission: Permission) -> bool` that:
   - Returns true if any role in user_roles has the permission
   - Handles empty roles vec as false

4. `PermissionManager` struct with methods:
   - `add_override(user_id: &str, permission: Permission)` - grant explicit permission
   - `remove_override(user_id: &str, permission: Permission)`
   - `get_overrides(user_id: &str) -> Vec<Permission>`
   
5. Override storage: Use PostgreSQL table `user_permissions` with columns (user_id, permission, granted_by, granted_at)</action>
  <verify>
    <automated>cd /home/threaded/projects/rRules && cargo test --package governance -- rbac::permissions --nocapture</automated>
  </verify>
  <done>Permission enum has all variants; permission_matrix returns correct sets per D-04; has_permission correctly aggregates across roles; PermissionManager compiles with PostgreSQL integration</done>
</task>

<task type="auto">
  <name>Task 03: Integrate RBAC into authentication middleware</name>
  <files>src/api/v1/middleware/auth.rs</files>
  <action>Update the existing auth middleware to integrate RBAC:
  
1. Load user roles from session after successful authentication
2. Store RoleManager and user's roles in request extensions
3. Add `check_access(permission: Permission)` helper that:
   - Retrieves user roles from request extensions
   - Calls has_permission() with requested permission
   - Returns 403 Forbidden if access denied, continue otherwise
   
4. Apply role-based middleware to routes:
   - Admin-only routes (CONFIG_ACCESS): /v1/config/*
   - Author/Reviewer routes (RULE_READ): /v1/rules/* (read operations)
   - Author-only routes (RULE_CREATE/RULE_UPDATE): POST/PUT /v1/rules
   - Reviewer/Admin routes (APPROVE_RULE): /v1/approvals/*
   
5. Add RBAC headers to all responses:
   - X-User-Roles: admin,author (comma-separated)
   - X-Available-Permissions: rule_create,rule_read,...</action>
  <verify>
    <automated>cd /home/threaded/projects/rRules && cargo test --package api -- middleware::auth --nocapture</automated>
  </verify>
  <done>Middleware compiles; RBAC headers present in responses; 403 returned for unauthorized access attempts per D-04 role constraints</done>
</task>

</tasks>

<threat_model>
## Trust Boundaries

| Boundary | Description |
|----------|-------------|
| Client→API (auth) | Untrusted session cookie must be validated before RBAC checks |

## STRIDE Threat Register

| Threat ID | Category | Component | Disposition | Mitigation Plan |
|-----------|----------|-----------|-------------|-----------------|
| T-04-A1 | Tampering | Session cookie | mitigate | Validate signature via existing middleware; reject modified cookies |
| T-04-A2 | Repudiation | Role assignment | mitigate | Log all role changes to audit trail (Phase 4, Plan 03) |
| T-04-A3 | Elevation | Permission override | mitigate | Require admin role for add_override(); log all override modifications |
</threat_model>

<verification>
- RBAC roles compile and pass unit tests
- Session authentication returns proper role assignments
- API middleware rejects unauthorized access with 403
- RBAC headers present in all responses
- permission_matrix matches D-04 requirements exactly
</verification>

<success_criteria>
1. Admin role has full access (all permissions)
2. Author can create/update rules but cannot approve their own changes
3. Reviewer can only read and approve rules (if assigned as owner)
4. Fine-grained permissions can override base role access
5. RBAC checks enforced on all protected API endpoints
</success_criteria>

<output>
Create `.planning/phases/04-governance-access-control/04-01-SUMMARY.md` when done
</output>
