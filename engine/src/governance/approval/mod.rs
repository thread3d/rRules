//! Approval workflow module for Phase 18 Enterprise Governance
//!
//! This module implements the maker-checker pattern with:
//! - State machine for approval requests
//! - Multi-tier sequential approvers
//! - Time-based escalation

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Current state of an approval request
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum ApprovalState {
    /// Initial draft state (before submission)
    Draft,
    /// Submitted and awaiting review
    Submitted { submitted_at: DateTime<Utc> },
    /// Currently being reviewed in a specific tier
    InReview { started_at: DateTime<Utc>, current_tier: i32 },
    /// Approved by all required reviewers
    Approved { approved_by: Vec<String>, completed_at: DateTime<Utc> },
    /// Rejected by a reviewer
    Rejected { rejected_by: String, reason: Option<String>, completed_at: DateTime<Utc> },
    /// Escalated to next tier due to time limit
    Escalated { escalated_to_tier: i32, escalated_at: DateTime<Utc> },
}

/// Configuration for a single approval tier
#[derive(Clone, Debug)]
pub struct TierConfig {
    pub reviewers: Vec<String>,
    pub required_approvals: i32,
}

impl TierConfig {
    /// Create a new tier configuration
    pub fn new(reviewers: Vec<String>, required_approvals: i32) -> Self {
        Self {
            reviewers,
            required_approvals,
        }
    }
}

/// A complete approval workflow definition
#[derive(Clone, Debug)]
pub struct ApprovalWorkflow {
    pub id: Uuid,
    pub name: String,
    pub tiers: Vec<TierConfig>,
    pub auto_escape_hours: i32,
}

impl ApprovalWorkflow {
    /// Create a new approval workflow with default single-tier configuration
    pub fn new_single_tier(name: &str, reviewer: &str) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.to_string(),
            tiers: vec![TierConfig::new(vec![reviewer.to_string()], 1)],
            auto_escape_hours: 48,
        }
    }

    /// Add a new tier to the workflow
    pub fn add_tier(&mut self, tier: TierConfig) {
        self.tiers.push(tier);
    }
}

/// An approval request instance with its current state
#[derive(Clone, Debug)]
pub struct ApprovalRequest {
    pub id: Uuid,
    pub workflow_id: Uuid,
    pub rule_id: Uuid,
    pub maker_id: String,
    pub state: ApprovalState,
    pub reviewers: Vec<String>,
    pub required_approvals: i32,
    pub approved_by: Vec<String>,
}

impl ApprovalRequest {
    /// Create a new approval request in draft state
    pub fn new_draft(workflow_id: Uuid, rule_id: Uuid, maker_id: &str) -> Self {
        Self {
            id: Uuid::new_v4(),
            workflow_id,
            rule_id,
            maker_id: maker_id.to_string(),
            state: ApprovalState::Draft,
            reviewers: Vec::new(),
            required_approvals: 1,
            approved_by: Vec::new(),
        }
    }

    /// Check if this request can be approved (has enough approvals)
    pub fn can_be_approved(&self) -> bool {
        self.approved_by.len() as i32 >= self.required_approvals
    }

    /// Get the current tier index (0-based)
    pub fn current_tier_index(&self) -> Option<usize> {
        match &self.state {
            ApprovalState::InReview { current_tier, .. } => {
                if *current_tier > 0 && *current_tier as usize <= self.reviewers.len() {
                    Some(*current_tier as usize - 1)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Check if the given user can review this request
    pub fn can_review(&self, reviewer_id: &str) -> bool {
        match &self.state {
            ApprovalState::InReview { current_tier, .. } => {
                let tier_index = (*current_tier as usize).checked_sub(1);
                if tier_index.is_none() || tier_index.unwrap() >= self.reviewers.len() {
                    return false;
                }
                self.reviewers[tier_index.unwrap()] == reviewer_id
            }
            _ => false,
        }
    }
}

/// Error types for approval operations
#[derive(Debug)]
pub enum ApprovalError {
    InvalidTransition { from: ApprovalState, to: String },
    AlreadyApproved,
    AlreadyRejected,
    NotInReview,
    NotAuthorized { user_id: String, action: String },
    WorkflowNotFound,
    RuleNotFound,
}

impl std::fmt::Display for ApprovalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApprovalError::InvalidTransition { from, to } => {
                write!(f, "Invalid state transition from {:?} to {}", from, to)
            }
            ApprovalError::AlreadyApproved => write!(f, "Approval already approved"),
            ApprovalError::AlreadyRejected => write!(f, "Approval already rejected"),
            ApprovalError::NotInReview => write!(f, "Approval is not in review state"),
            ApprovalError::NotAuthorized { user_id, action } => {
                write!(f, "User {} is not authorized to {}", user_id, action)
            }
            ApprovalError::WorkflowNotFound => write!(f, "Workflow not found"),
            ApprovalError::RuleNotFound => write!(f, "Rule not found"),
        }
    }
}

impl std::error::Error for ApprovalError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_approval_request_creation() {
        let request = ApprovalRequest::new_draft(Uuid::new_v4(), Uuid::new_v4(), "user1");

        assert!(matches!(request.state, ApprovalState::Draft));
        assert_eq!(request.maker_id, "user1");
        assert_eq!(request.approved_by.len(), 0);
    }

    #[test]
    fn test_workflow_creation() {
        let workflow = ApprovalWorkflow::new_single_tier("Default", "reviewer1");

        assert_eq!(workflow.name, "Default");
        assert_eq!(workflow.tiers.len(), 1);
        assert_eq!(workflow.auto_escape_hours, 48);
    }

    #[test]
    fn test_can_review() {
        let mut request = ApprovalRequest::new_draft(Uuid::new_v4(), Uuid::new_v4(), "maker");
        request.reviewers = vec!["reviewer1".to_string(), "reviewer2".to_string()];
        request.required_approvals = 2;

        // Set to in_review state
        let now = Utc::now();
        request.state = ApprovalState::InReview {
            started_at: now,
            current_tier: 1,
        };

        assert!(request.can_review("reviewer1"));
        assert!(!request.can_review("reviewer2")); // different tier
        assert!(!request.can_review("maker"));
    }

    #[test]
    fn test_can_be_approved() {
        let mut request = ApprovalRequest::new_draft(Uuid::new_v4(), Uuid::new_v4(), "user1");
        request.required_approvals = 2;

        // Not approved yet
        assert!(!request.can_be_approved());

        // Add one approval
        request.approved_by.push("reviewer1".to_string());
        assert!(!request.can_be_approved());

        // Add second approval - can now approve
        request.approved_by.push("reviewer2".to_string());
        assert!(request.can_be_approved());
    }

    #[test]
    fn test_current_tier_index() {
        let mut request = ApprovalRequest::new_draft(Uuid::new_v4(), Uuid::new_v4(), "user1");
        request.reviewers = vec!["reviewer1".to_string()];

        // Not in review
        assert!(request.current_tier_index().is_none());

        // Set to tier 1
        request.state = ApprovalState::InReview {
            started_at: Utc::now(),
            current_tier: 1,
        };
        assert_eq!(request.current_tier_index(), Some(0));
    }
}
