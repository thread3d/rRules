//! Tenant types for multi-tenancy support.

use chrono::{DateTime, Utc};

/// Represents a tenant (organization/workspace) in the system.
#[derive(Debug, Clone)]
pub struct Tenant {
    /// Unique identifier for the tenant.
    pub id: String,
    /// Human-readable display name.
    pub name: String,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
}

impl Tenant {
    /// Create a new tenant with the given name and auto-generated ID.
    pub fn new(name: impl Into<String>) -> Self {
        use uuid::Uuid;
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            created_at: Utc::now(),
        }
    }

    /// Create a tenant with an explicit ID (e.g., from deserialization).
    pub fn with_id(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            created_at: Utc::now(),
        }
    }
}

/// Request body for creating a tenant.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct CreateTenantRequest {
    /// Human-readable display name (required).
    pub name: String,
}

/// Response shape for tenant data.
#[derive(Debug, Clone, serde::Serialize)]
pub struct TenantResponse {
    pub id: String,
    pub name: String,
    pub created_at: DateTime<Utc>,
}

impl From<&Tenant> for TenantResponse {
    fn from(tenant: &Tenant) -> Self {
        Self {
            id: tenant.id.clone(),
            name: tenant.name.clone(),
            created_at: tenant.created_at,
        }
    }
}

/// Paginated list response.
#[derive(Debug, Clone, serde::Serialize)]
pub struct TenantListResponse {
    pub total_count: usize,
    pub tenants: Vec<TenantResponse>,
}
