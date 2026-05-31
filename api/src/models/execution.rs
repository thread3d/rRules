//! Execution request/response models

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Request for rule execution
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExecuteRequest {
    /// Optional session ID for stateful execution
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    
    /// Facts for stateless execution (ignored if session_id is provided)
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub facts: HashMap<String, serde_json::Value>,
    
    /// Optional rule filter to execute only specific rules
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule_filter: Option<RuleFilter>,
}

/// Filter for selecting which rules to execute
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RuleFilter {
    /// Execute only rules matching this name pattern
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name_pattern: Option<String>,
    
    /// Execute only rules in these categories
    #[serde(skip_serializing_if = "Option::is_none")]
    pub categories: Option<Vec<String>>,
}

/// Response from rule execution
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExecuteResponse {
    /// Unique ID for this execution
    pub execution_id: String,
    
    /// Number of rules that fired
    pub rules_fired: usize,
    
    /// List of rules that were evaluated but didn't fire
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub rules_not_fired: Vec<String>,
    
    /// Facts after execution (with updated values)
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub facts_after: HashMap<String, serde_json::Value>,
    
    /// Inference trace for debugging
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inference_trace: Option<InferenceTrace>,
}

/// Complete inference trace with full detail
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct InferenceTrace {
    /// Trace entries in execution order
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub entries: Vec<InferenceTraceEntry>,
    
    /// Summary statistics
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<TraceSummary>,
}

/// Single trace entry for debugging
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct InferenceTraceEntry {
    /// Timestamp of the event
    pub timestamp: String,
    
    /// Event type (rule_fired, rule_evaluated, etc.)
    pub event_type: String,
    
    /// Rule name if applicable
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule_name: Option<String>,
    
    /// Additional context-specific data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// Summary statistics for inference trace
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TraceSummary {
    /// Total execution time in milliseconds
    pub total_duration_ms: f64,
    
    /// Number of facts matched
    pub facts_matched: usize,
}
