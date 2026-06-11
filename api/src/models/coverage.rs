//! Coverage reporting models for API responses

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Request parameters for coverage reports
#[derive(Debug, Clone, Deserialize)]
pub struct CoverageReportRequest {
    /// Optional rule ID to filter report
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule_id: Option<Uuid>,

    /// Start timestamp for time-range filtering
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_time: Option<String>,

    /// End timestamp for time-range filtering
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_time: Option<String>,

    /// Limit on number of history entries returned
    #[serde(default)]
    pub limit: usize,
}

/// Response containing coverage report data
#[derive(Debug, Clone, Serialize)]
pub struct CoverageReportResponse {
    /// List of coverage entries for all tracked rules
    pub rules: Vec<CoverageEntry>,

    /// Total rules tracked in the reporting period
    pub total_rules: usize,

    /// Rules that were evaluated at least once
    pub evaluated_rules: usize,

    /// Rules that fired (achieved their action)
    pub fired_rules: usize,

    /// Overall coverage percentage
    pub coverage_percent: f64,
}

/// Single rule's coverage data entry
#[derive(Debug, Clone, Serialize)]
pub struct CoverageEntry {
    /// Rule ID
    pub rule_id: String,

    /// Rule name for display
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule_name: Option<String>,

    /// Whether the rule was evaluated (conditions matched)
    pub evaluated: bool,

    /// Whether the rule fired (actions executed)
    pub fired: bool,

    /// Timestamp of last evaluation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_evaluated_at: Option<String>,

    /// Timestamp of last fire
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_fired_at: Option<String>,

    /// Number of times evaluated (if tracked)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evaluation_count: Option<usize>,

    /// Number of times fired (if tracked)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fire_count: Option<usize>,
}

/// Request for historical coverage trends
#[derive(Debug, Clone, Deserialize)]
pub struct CoverageTrendRequest {
    /// Time range start (ISO 8601 string)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<String>,

    /// Time range end (ISO 8601 string)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<String>,

    /// Granularity: "hourly", "daily", or "manual"
    #[serde(default)]
    pub granularity: String,
}

/// Historical trend data point
#[derive(Debug, Clone, Serialize)]
pub struct CoverageTrendPoint {
    /// Timestamp for this data point
    pub timestamp: String,

    /// Total rules at this time
    pub total_rules: usize,

    /// Rules evaluated at this time
    pub evaluated_rules: usize,

    /// Rules fired at this time
    pub fired_rules: usize,

    /// Coverage percentage
    pub coverage_percent: f64,
}

/// Response containing trend data
#[derive(Debug, Clone, Serialize)]
pub struct CoverageTrendResponse {
    /// List of trend data points
    pub trends: Vec<CoverageTrendPoint>,

    /// Time range covered
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_time: Option<String>,

    /// Time range covered
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_time: Option<String>,

    /// Granularity of data points
    pub granularity: String,
}

/// Snapshot request for immediate coverage state
#[derive(Debug, Clone, Deserialize)]
pub struct CoverageSnapshotRequest {
    /// Include detailed per-rule breakdown
    #[serde(default)]
    pub detail: bool,
}

/// Historical coverage snapshot (stored periodically)
#[derive(Debug, Clone, Serialize)]
pub struct CoverageSnapshot {
    /// Snapshot identifier
    pub id: i32,

    /// When the snapshot was taken
    pub snapshot_time: String,

    /// Aggregated metrics
    pub total_rules: usize,
    pub evaluated_rules: usize,
    pub fired_rules: usize,
    pub overall_coverage_percent: f64,

    /// Per-engine breakdown (if captured)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub engine_breakdown: Option<serde_json::Value>,
}

/// Daily rollup for efficient trend queries
#[derive(Debug, Clone, Serialize)]
pub struct CoverageDailyRollup {
    /// Date of the rollup
    pub date: String,

    /// Aggregated metrics
    pub total_rules: usize,
    pub evaluated_rules: usize,
    pub fired_rules: usize,
    pub overall_coverage_percent: f64,

    /// Execution statistics
    pub total_evaluations: u64,
    pub total_fires: u64,
    pub avg_execution_duration_ms: Option<f64>,
}

/// Request to record a coverage event
#[derive(Debug, Clone, Deserialize)]
pub struct CoverageEventRequest {
    /// Rule ID being evaluated/fired
    pub rule_id: Uuid,

    /// Rule name for display purposes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule_name: Option<String>,

    /// Whether the rule was evaluated
    #[serde(default)]
    pub evaluated: bool,

    /// Whether the rule fired
    #[serde(default)]
    pub fired: bool,

    /// Optional execution context
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,

    /// Engine type if known
    #[serde(skip_serializing_if = "Option::is_none")]
    pub engine_type: Option<String>,
}

/// Response confirming coverage event was recorded
#[derive(Debug, Clone, Serialize)]
pub struct CoverageEventResponse {
    /// ID of the recorded history entry
    pub id: i32,
}
