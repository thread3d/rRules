//! In-memory user store seeded with the 4 predefined roles (D-22-01).
//!
//! Follows the existing `init_store()` pattern (`Arc<Mutex<HashMap<...>>>`).
//! Postgres is deliberately NOT introduced (deferred to v5.0). Passwords are
//! plaintext for the demo (D-22-01 permits); production hashing is deferred.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// A seeded demo user. `password` is plaintext (D-22-01 demo only).
#[derive(Debug, Clone)]
pub struct UserRecord {
    pub id: String,
    pub email: String,
    pub display_name: String,
    pub role: String,
    pub permissions: Vec<String>,
    pub password: String,
}

/// Shared user store keyed by user id (== username for the demo users).
pub type UserStore = Arc<Mutex<HashMap<String, UserRecord>>>;

/// Initialize the in-memory user store with one demo user per role:
/// `admin`, `author`, `reviewer`, `viewer`. The demo password is the same as
/// the username (e.g. `admin`/`admin`) — plaintext compare per D-22-01.
pub fn init_store() -> UserStore {
    let mut map = HashMap::new();
    for (username, role) in [
        ("admin", "admin"),
        ("author", "author"),
        ("reviewer", "reviewer"),
        ("viewer", "viewer"),
    ] {
        let record = UserRecord {
            id: username.to_string(),
            email: format!("{}@rrules.dev", username),
            display_name: username.to_string(),
            role: role.to_string(),
            permissions: permissions_for(role),
            // Plaintext demo password == username (D-22-01). DO NOT do this in
            // production — v5.0 migrates to argon2/bcrypt.
            password: username.to_string(),
        };
        map.insert(username.to_string(), record);
    }
    Arc::new(Mutex::new(map))
}

/// Resolve a user by login identifier (username OR email). Lets the frontend
/// send either field in `AuthCredentials` (`{email, password}`) while the plan
/// behavior tests use `{username, password}`.
pub fn find_user(store: &UserStore, identifier: &str) -> Option<UserRecord> {
    let users = store.lock().ok()?;
    if let Some(u) = users.get(identifier) {
        return Some(u.clone());
    }
    users
        .values()
        .find(|u| u.email == identifier)
        .cloned()
}

/// Permissions per role, mirroring the frontend `Permission` union
/// (`frontend/src/types/auth.ts`). Granular per-route enforcement is deferred
/// to v5.0 (D-22-02); these are surfaced so the frontend `usePermission` can
/// gate the UI end-to-end.
fn permissions_for(role: &str) -> Vec<String> {
    match role {
        "admin" => vec![
            "rule:read", "rule:create", "rule:update", "rule:delete",
            "approval:read", "approval:create", "approval:approve", "approval:reject",
            "metrics:read", "settings:read", "settings:update",
        ],
        "reviewer" => vec![
            "rule:read", "rule:create", "rule:update",
            "approval:read", "approval:approve", "approval:reject",
            "metrics:read", "settings:read",
        ],
        "author" => vec![
            "rule:read", "rule:create", "rule:update",
            "approval:read", "metrics:read", "settings:read",
        ],
        // viewer (and any unknown role): read-only baseline.
        _ => vec!["rule:read", "approval:read", "metrics:read", "settings:read"],
    }
    .into_iter()
    .map(String::from)
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_seeds_four_roles() {
        let store = init_store();
        let users = store.lock().unwrap();
        for role in ["admin", "author", "reviewer", "viewer"] {
            assert!(users.contains_key(role), "missing seed user {role}");
            assert_eq!(users[role].role, role);
            assert_eq!(users[role].password, role, "demo password == username");
            assert!(!users[role].permissions.is_empty());
        }
    }

    #[test]
    fn find_user_resolves_by_username_and_email() {
        let store = init_store();
        assert!(find_user(&store, "admin").is_some());
        assert!(find_user(&store, "admin@rrules.dev").is_some());
        assert!(find_user(&store, "nobody").is_none());
    }
}