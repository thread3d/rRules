//! Rule CRUD request/response models

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Request to create a new rule
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CreateRuleRequest {
    /// Unique identifier for the rule
    pub name: String,
    
    /// Human-readable description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    
    /// Rule category for organization
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    
    /// Rule owner (email or user ID)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    
    /// DSL body of the rule
    pub dsl_body: String,
    
    /// Tags for filtering/searching
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub tags: Vec<String>,
}

/// Response with created rule details
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RuleResponse {
    /// Unique identifier (UUID)
    pub id: String,
    
    /// Rule name
    pub name: String,
    
    /// Description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    
    /// Category
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    
    /// Owner
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    
    /// Rule status (draft, active, deprecated)
    pub status: String,
    
    /// Version number
    pub version: u32,
    
    /// Tags
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub tags: Vec<String>,
    
    /// When the rule was created
    pub created_at: String,
    
    /// When the rule was last updated
    pub updated_at: String,
}

/// Request to update an existing rule
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UpdateRuleRequest {
    /// New description (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    
    /// New category (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    
    /// New owner (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    
    /// Updated DSL body (optional - only if rule should be modified)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dsl_body: Option<String>,
    
    /// New tags (replaces all existing)
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub tags: Vec<String>,
    
    /// New status (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

/// Paginated list of rules
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RuleListResponse {
    /// Total number of rules matching filters
    pub total_count: usize,
    
    /// Number of rules in this page
    pub page_size: usize,
    
    /// Current page number (1-indexed)
    pub page_number: u32,
    
    /// List of rules on this page
    pub rules: Vec<RuleResponse>,
}

/// Request for paginated rule listing
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RuleListRequest {
    /// Page number (1-indexed), default 1
    #[serde(default)]
    pub page: u32,
    
    /// Items per page, default 50
    #[serde(default)]
    pub page_size: u32,
    
    /// Filter by category
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    
    /// Filter by status (draft, active, deprecated)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

/// Delete rule response
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DeleteRuleResponse {
    /// ID of the deleted rule
    pub id: String,
    
    /// Name of the deleted rule
    pub name: String,
    
    /// Timestamp of deletion
    pub deleted_at: String,
}

/// Undelete rule request/response
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UndeleteRuleResponse {
    /// ID of the restored rule
    pub id: String,
    
    /// New status after restoration
    pub new_status: String,
}
