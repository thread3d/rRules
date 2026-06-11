//! Role-Based Access Control (RBAC) module for Phase 18 Enterprise Governance
//!
//! This module provides role-based authorization with:
//! - Predefined roles: Admin, Author, Reviewer, Viewer
//! - Permission-based access control
//! - JWT middleware integration

use serde::{Deserialize, Serialize};

/// Predefined roles in the system
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum Role {
    /// Full access to all features (system administrator)
    Admin,
    /// Create/edit rules, submit for approval
    Author,
    /// Approve/reject rule changes
    Reviewer,
    /// Read-only access
    Viewer,
}

impl Role {
    /// Returns the role name as a string
    pub fn name(&self) -> &'static str {
        match self {
            Role::Admin => "admin",
            Role::Author => "author",
            Role::Reviewer => "reviewer",
            Role::Viewer => "viewer",
        }
    }

    /// Returns a description of the role's capabilities
    pub fn description(&self) -> &'static str {
        match self {
            Role::Admin => "Full access to all features including system configuration",
            Role::Author => "Create and edit rules, submit for approval",
            Role::Reviewer => "Review and approve/reject rule changes",
            Role::Viewer => "Read-only access to rules and audit logs",
        }
    }

    /// Returns the permission scope string for this role
    pub fn scope(&self) -> &'static str {
        match self {
            Role::Admin => "*",
            Role::Author => "author",
            Role::Reviewer => "reviewer",
            Role::Viewer => "viewer",
        }
    }
}

/// Available permissions in the system
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Permission {
    pub resource: String,
    pub action: String,
    pub scope: String,
}

impl Permission {
    /// Create a new permission
    pub fn new(resource: &str, action: &str, scope: &str) -> Self {
        Self {
            resource: resource.to_string(),
            action: action.to_string(),
            scope: scope.to_string(),
        }
    }

    /// Check if this permission grants access to the given resource/action/scope
    pub fn grants(&self, resource: &str, action: &str, scope: &str) -> bool {
        // Resource must match exactly or use wildcard
        let resource_match = self.resource == "*" || self.resource == resource;

        // Action must match exactly or use wildcard
        let action_match = self.action == "*" || self.action == action;

        // Scope must match exactly, use wildcard, or this is admin (which has all scopes)
        let scope_match = self.scope == "*"
            || self.scope == scope
            || self.scope == "admin"; // Admin gets all scopes

        resource_match && action_match && scope_match
    }
}

/// A user's role and permission claims
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct UserClaims {
    pub sub: String,          // User ID
    pub roles: Vec<String>,   // Role names
    pub permissions: Vec<Permission>,
}

impl UserClaims {
    /// Create new claims for a user with given role
    pub fn with_role(user_id: &str, role: Role) -> Self {
        let (permissions, roles) = match role {
            Role::Admin => (
                vec![
                    Permission::new("*", "*", "*"),
                ],
                vec![role.name().to_string()],
            ),
            Role::Author => (
                vec![
                    Permission::new("rule", "read", "*"),
                    Permission::new("rule", "create", "*"),
                    Permission::new("rule", "update", "*"),
                    Permission::new("approval", "submit", "*"),
                ],
                vec![role.name().to_string()],
            ),
            Role::Reviewer => (
                vec![
                    Permission::new("rule", "read", "*"),
                    Permission::new("approval", "read", "*"),
                    Permission::new("approval", "review", "*"),
                    Permission::new("audit", "read", "*"),
                ],
                vec![role.name().to_string()],
            ),
            Role::Viewer => (
                vec![
                    Permission::new("rule", "read", "*"),
                    Permission::new("audit", "read", "*"),
                ],
                vec![role.name().to_string()],
            ),
        };

        Self {
            sub: user_id.to_string(),
            roles,
            permissions,
        }
    }

    /// Check if the user has a specific permission
    pub fn has_permission(&self, resource: &str, action: &str, scope: &str) -> bool {
        // Admin has all permissions
        if self.roles.contains(&"admin".to_string()) {
            return true;
        }

        for perm in &self.permissions {
            if perm.grants(resource, action, scope) {
                return true;
            }
        }

        false
    }

    /// Check if user has a specific role
    pub fn has_role(&self, role: &str) -> bool {
        self.roles.contains(&role.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_admin_has_all_permissions() {
        let claims = UserClaims::with_role("user1", Role::Admin);

        assert!(claims.has_permission("rule", "create", "*"));
        assert!(claims.has_permission("approval", "approve", "*"));
        assert!(claims.has_permission("audit", "read", "*"));
    }

    #[test]
    fn test_author_can_create_rules() {
        let claims = UserClaims::with_role("user1", Role::Author);

        assert!(claims.has_permission("rule", "create", "*"));
        assert!(claims.has_permission("rule", "read", "*"));
        assert!(!claims.has_permission("approval", "approve", "*"));
    }

    #[test]
    fn test_reviewer_can_approve() {
        let claims = UserClaims::with_role("user1", Role::Reviewer);

        assert!(claims.has_permission("approval", "review", "*"));
        assert!(claims.has_permission("audit", "read", "*"));
        assert!(!claims.has_permission("rule", "create", "*"));
    }

    #[test]
    fn test_viewer_is_readonly() {
        let claims = UserClaims::with_role("user1", Role::Viewer);

        assert!(claims.has_permission("rule", "read", "*"));
        assert!(claims.has_permission("audit", "read", "*"));
        assert!(!claims.has_permission("rule", "create", "*"));
        assert!(!claims.has_permission("approval", "submit", "*"));
    }

    #[test]
    fn test_role_names() {
        assert_eq!(Role::Admin.name(), "admin");
        assert_eq!(Role::Author.name(), "author");
        assert_eq!(Role::Reviewer.name(), "reviewer");
        assert_eq!(Role::Viewer.name(), "viewer");
    }
}
