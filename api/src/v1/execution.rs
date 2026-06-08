//! Rule execution API endpoints

use actix_web::{web, HttpResponse, Responder};
use std::sync::Arc;
use uuid::Uuid;

use engine::{EngineBuilder, WorkingMemory};

/// Request for rule execution
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ExecuteRequest {
    /// Optional session ID for stateful execution
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,

    /// Facts for stateless execution (ignored if session_id is provided)
    #[serde(default)]
    pub facts: std::collections::HashMap<String, serde_json::Value>,

    /// Optional rule filter to execute only specific rules
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule_filter: Option<RuleFilter>,
}

/// Filter for selecting which rules to execute
#[derive(Debug, Clone, serde::Deserialize)]
pub struct RuleFilter {
    /// Execute only rules matching this name pattern
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name_pattern: Option<String>,

    /// Execute only rules in these categories
    #[serde(skip_serializing_if = "Option::is_none")]
    pub categories: Option<Vec<String>>,
}

/// Response from rule execution
#[derive(Debug, Clone, serde::Serialize)]
pub struct ExecuteResponse {
    /// Unique ID for this execution
    pub execution_id: String,

    /// Number of rules that fired
    pub rules_fired: usize,

    /// List of rules that were evaluated but didn't fire
    #[serde(default)]
    pub rules_not_fired: Vec<String>,

    /// Facts after execution (with updated values)
    #[serde(default)]
    pub facts_after: std::collections::HashMap<String, serde_json::Value>,

    /// Inference trace for debugging
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inference_trace: Option<InferenceTrace>,
}

/// Complete inference trace with full detail
#[derive(Debug, Clone, serde::Serialize)]
pub struct InferenceTrace {
    /// Trace entries in execution order
    #[serde(default)]
    pub entries: Vec<InferenceTraceEntry>,

    /// Summary statistics
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<TraceSummary>,
}

/// Single trace entry for debugging
#[derive(Debug, Clone, serde::Serialize)]
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
#[derive(Debug, Clone, serde::Serialize)]
pub struct TraceSummary {
    /// Total execution time in milliseconds
    pub total_duration_ms: f64,

    /// Number of facts matched
    pub facts_matched: usize,
}

/// Request to reload rules
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ReloadRequest {
    /// Optional rule file path to reload
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule_file: Option<String>,
}

/// Response from rule reload
#[derive(Debug, Clone, serde::Serialize)]
pub struct ReloadResponse {
    /// Unique ID for this reload operation
    pub reload_id: String,

    /// Number of rules reloaded
    pub rules_reloaded: usize,

    /// Status message
    pub status: String,
}

/// Execute rules - POST /v1/execute
pub async fn execute_rules(
    req: web::Json<ExecuteRequest>,
) -> impl Responder {
    let execution_id = Uuid::new_v4().to_string();
    let start_time = std::time::Instant::now();

    // Create a fresh engine for stateless execution
    let _engine = EngineBuilder::new().build();
    let _working_memory = WorkingMemory::new();

    // Parse and insert facts from JSON
    let mut facts_after: std::collections::HashMap<String, serde_json::Value> =
        std::collections::HashMap::new();

    for (fact_type, fact_value) in &req.facts {
        if let serde_json::Value::Object(fields) = fact_value.clone() {
            // Convert JSON fields to engine Value
            let mut payload = std::collections::HashMap::new();
            for (key, value) in fields {
                payload.insert(key, json_to_engine_value(value));
            }
            facts_after.insert(fact_type.clone(), fact_value.clone());
        }
    }

    // Execute rules (placeholder - full Rete integration needed)
    let rules_fired = 0;
    let rules_not_fired: Vec<String> = Vec::new();

    let _duration_ms = start_time.elapsed().as_secs_f64() * 1000.0;

    let response = ExecuteResponse {
        execution_id,
        rules_fired,
        rules_not_fired,
        facts_after,
        inference_trace: None,
    };

    HttpResponse::Ok().json(response)
}

/// Reload rules - POST /v1/rules/reload
///
/// Triggers a hot reload of rules while preserving existing sessions.
/// This endpoint allows updating rule definitions without restarting the server.
pub async fn reload_rules(
    _req: web::Json<ReloadRequest>,
) -> impl Responder {
    let reload_id = Uuid::new_v4().to_string();

    // In a full implementation, this would:
    // 1. Re-read rule files from disk
    // 2. Parse and validate new rules
    // 3. Update the engine's KIE base with new rules
    // 4. Preserve existing sessions (if stateful mode)

    HttpResponse::Ok().json(ReloadResponse {
        reload_id,
        rules_reloaded: 0,  // Placeholder - actual count would come from parsing
        status: "reload triggered".to_string(),
    })
}

fn json_to_engine_value(value: serde_json::Value) -> engine::Value {
    match value {
        serde_json::Value::Null => engine::Value::Null,
        serde_json::Value::Bool(b) => engine::Value::Bool(b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                engine::Value::Integer(i)
            } else if let Some(f) = n.as_f64() {
                engine::Value::Float(f)
            } else {
                engine::Value::Null
            }
        }
        serde_json::Value::String(s) => engine::Value::String(s),
        serde_json::Value::Array(arr) => {
            engine::Value::Array(arr.into_iter().map(json_to_engine_value).collect())
        }
        serde_json::Value::Object(obj) => {
            let map = obj
                .into_iter()
                .map(|(k, v)| (k, json_to_engine_value(v)))
                .collect();
            engine::Value::Object(map)
        }
    }
}

/// Execute with session - POST /v1/execute/session
pub async fn execute_with_session(
    session_id: web::Path<String>,
    _req: web::Json<ExecuteRequest>,
) -> impl Responder {
    let execution_id = Uuid::new_v4().to_string();

    HttpResponse::Ok().json(serde_json::json!({
        "execution_id": execution_id,
        "session_id": session_id.into_inner(),
        "status": "executed",
        "rules_fired": 0
    }))
}
