//! What-If simulation API endpoint for scenario-based testing.
//!
//! This module provides the `POST /v1/simulation/run` endpoint that allows users
//! to simulate rule changes and compare results against a baseline execution,
//! without affecting production state.

use actix_web::{web, HttpResponse, Responder};

use engine::simulation::runner::SimulationRunner;
use engine::simulation::types::*;

/// Request body for POST /v1/simulation/run
#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
pub struct SimulationRunRequest {
    /// Free-form name for this simulation (used in responses and logging).
    pub name: String,

    /// Optional baseline working memory snapshot. When provided the engine is
    /// pre-seeded with these facts before execution; otherwise each test case
    /// starts from an empty working memory.
    #[serde(default)]
    pub base_state: Option<WorkingMemorySnapshot>,

    /// Ordered list of rule modifications to apply on top of the base state.
    /// Each entry produces a separate "what-if" variant that is compared against
    /// the unmodified engine.
    #[serde(default)]
    pub proposed_changes: Vec<RuleChange>,

    /// Groups of facts to evaluate. Each inner vector represents one independent
    /// test scenario. Empty groups are rejected at validation time.
    #[serde(default)]
    pub fact_sets: Vec<Vec<TestFact>>,

    /// The rules to simulate, in the same JSON form `POST /v1/execute` accepts (`Rule[]`, a rule
    /// document, or a `{"rules": […]}` envelope).
    ///
    /// The endpoint used to build a fresh engine with **no rules**, so every variant reported zero
    /// fired rules and no differences — a confident answer that simulated nothing. A what-if run has
    /// to be given what it is a what-if of.
    #[serde(default)]
    pub rules: serde_json::Value,
}

/// Response body for POST /v1/simulation/run
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct SimulationRunResponse {
    /// The `name` field from the original request.
    pub name: String,

    /// One entry per `(fact_set_index, variant)` pair. The first entry (index 0)
    /// is always the **base** execution with no changes applied; subsequent
    /// entries correspond to each `proposed_changes` entry in order.
    #[serde(default)]
    pub test_cases: Vec<SimulationTestCaseResponse>,

    /// Total wall-clock time spent running all executions (milliseconds).
    #[serde(default)]
    pub total_execution_time_ms: f64,

    /// Summary of how many variants were tested and how many produced differences.
    pub summary: SimulationSummary,
}

/// Per-test-case response for simulation results.
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct SimulationTestCaseResponse {
    /// Zero-based index of the fact set this result corresponds to within the
    /// original `fact_sets` array.
    pub fact_set_index: usize,

    /// Execution results for each variant (base + one per proposed change).
    #[serde(default)]
    pub variants: Vec<SimulationVariantResponse>,

    /// Summary of differences across all variants that fired differently from
    /// the base execution. Empty when no rules changed behavior.
    #[serde(default)]
    pub differences: Vec<String>,
}

/// Execution outcome for a single variant (base or modified engine).
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct SimulationVariantResponse {
    /// Description of this variant: `"base"` for the unmodified state, otherwise
    /// the `RuleChange` description (e.g. `"AddCondition(rule=discount)"`).
    pub variant_label: String,

    /// Rules that fired during execution, in firing order.
    #[serde(default)]
    pub rules_fired: Vec<String>,

    /// Wall-clock time spent executing this variant (milliseconds).
    pub execution_time_ms: f64,

    /// Engine output / side-effects produced by the execution. May be `null` if
    /// the engine produced no structured output.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<serde_json::Value>,

    /// Whether this variant's execution completed successfully.
    #[serde(default)]
    pub success: bool,

    /// Error message if `success` is false.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Summary statistics for a simulation run.
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct SimulationSummary {
    /// Total number of variants tested (base + one per proposed change).
    pub total_variants: usize,

    /// Number of fact sets evaluated.
    pub total_fact_sets: usize,

    /// Number of test cases that produced at least one behavioral difference.
    pub affected_test_cases: usize,

    /// Total number of distinct rules whose behavior changed across all variants.
    pub rules_affected: usize,
}

/// Execute a What-If simulation - POST /v1/simulation/run
///
/// This endpoint creates an isolated copy of the engine state, applies proposed
/// rule changes to it, and runs both sides against identical test fact sets.
/// Results are compared to produce a structured diff report showing which rules
/// fired/didn't fire and what facts changed.
#[utoipa::path(
    post,
    path = "/v1/simulation",
    tag = "simulation",
    request_body = SimulationRunRequest,
    responses(
        (status = 200, description = "Simulation completed", body = SimulationRunResponse),
        (status = 400, description = "Invalid request: fact_sets must contain at least one fact group"),
    ),
)]
pub async fn run_simulation(request: web::Json<SimulationRunRequest>) -> impl Responder {
    // Validate input early.
    if request.fact_sets.is_empty() {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "fact_sets must contain at least one fact group",
            "status_code": 400
        }));
    }

    // Build the base engine from a fresh builder (isolated, no shared state) and load the rules the
    // caller wants to simulate. An empty `rules` value means an empty rule set, which is a legitimate
    // (if uninformative) request rather than an error.
    let mut base_engine = engine::environment::EngineBuilder::new().build();
    if !request.rules.is_null() {
        if let Err(error) = base_engine.load_rules(&request.rules.to_string()) {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": format!("rules were rejected: {error}"),
                "status_code": 400
            }));
        }
    }

    // Convert request to internal simulation types.
    let sim_request = SimulationRequest {
        name: request.name.clone(),
        base_state: request.base_state.clone(),
        proposed_changes: request.proposed_changes.clone(),
        fact_sets: request
            .fact_sets
            .iter()
            .map(|facts| {
                facts
                    .iter()
                    .map(|f| TestFact {
                        id: f.id.clone(),
                        // The fact's *payload*, not the fact. This serialised the whole `TestFact`
                        // (`{id, fields}`), so every simulated fact carried keys named `id` and
                        // `fields` and none of its own attributes — invisible while the runner threw
                        // the facts away, fatal once it inserts them.
                        fields: f.fields.clone(),
                    })
                    .collect()
            })
            .collect(),
    };

    // Run the simulation.
    let result = SimulationRunner::run(&base_engine, &sim_request);

    // Convert internal types to API response types.
    let test_cases: Vec<SimulationTestCaseResponse> = result
        .test_cases
        .into_iter()
        .map(|tc| {
            let variants: Vec<SimulationVariantResponse> = tc
                .variants
                .into_iter()
                .map(|v| SimulationVariantResponse {
                    variant_label: v.variant_label,
                    rules_fired: v.rules_fired,
                    execution_time_ms: v.execution_time_ms,
                    output: v.output,
                    success: v.success,
                    error: v.error,
                })
                .collect();

            let differences: Vec<String> = tc
                .differences
                .into_iter()
                .map(|d| format_change_report_string(&d))
                .collect();

            SimulationTestCaseResponse {
                fact_set_index: tc.fact_set_index,
                variants,
                differences,
            }
        })
        .collect();

    // Calculate summary statistics.
    let affected_test_cases = test_cases
        .iter()
        .filter(|tc| !tc.differences.is_empty())
        .count();
    let mut rules_affected = std::collections::HashSet::new();
    for tc in &test_cases {
        for diff in &tc.differences {
            if let Some(rule_id) = extract_rule_id_from_diff(diff) {
                rules_affected.insert(rule_id);
            }
        }
    }

    let summary = SimulationSummary {
        total_variants: 1 + request.proposed_changes.len(),
        total_fact_sets: request.fact_sets.len(),
        affected_test_cases,
        rules_affected: rules_affected.len(),
    };

    HttpResponse::Ok().json(SimulationRunResponse {
        name: result.name,
        test_cases,
        total_execution_time_ms: result.total_execution_time_ms,
        summary,
    })
}

/// Convert a ChangeReport enum variant to a human-readable string.
fn format_change_report_string(report: &ChangeReport) -> String {
    match report {
        ChangeReport::RuleFiredBeforeButNotAfter {
            rule_id,
            variant_label,
        } => {
            format!(
                "Rule '{}' fired before but not after change '{}'",
                rule_id, variant_label
            )
        }
        ChangeReport::RuleFiredAfterButNotBefore {
            rule_id,
            variant_label,
        } => {
            format!(
                "Rule '{}' fires after change '{variant_label}' but not before",
                rule_id
            )
        }
        ChangeReport::DifferentResultsForRule {
            rule_id,
            variant_label,
            ..
        } => {
            format!(
                "Rule '{}' produces different results with change '{}'",
                rule_id, variant_label
            )
        }
        ChangeReport::ChangeApplicationError {
            variant_label,
            error,
        } => {
            format!("Change '{}' failed: {}", variant_label, error)
        }
    }
}

/// Extract the rule ID from a formatted difference string.
fn extract_rule_id_from_diff(diff_str: &str) -> Option<String> {
    // Parse patterns like "Rule 'r1' fired before but not after..."
    if let Some(start) = diff_str.find('\'') {
        if let Some(end) = diff_str[start + 1..].find('\'') {
            return Some(diff_str[start + 1..start + 1 + end].to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_change_report_string() {
        let report = ChangeReport::RuleFiredBeforeButNotAfter {
            rule_id: "discount-rule".to_string(),
            variant_label: "v1".to_string(),
        };

        let formatted = format_change_report_string(&report);
        assert!(formatted.contains("discount-rule"));
        assert!(formatted.contains("fired before but not after"));
    }

    #[test]
    fn test_extract_rule_id_from_diff() {
        let diff_str = "Rule 'rule-1' fired before but not after change 'v1'";
        let extracted = extract_rule_id_from_diff(diff_str);
        assert_eq!(extracted, Some("rule-1".to_string()));

        let no_quote = "No quotes here";
        assert_eq!(extract_rule_id_from_diff(no_quote), None);
    }

    #[test]
    fn test_simulation_request_deserialization() {
        let json = r#"{
            "name": "test-sim",
            "proposed_changes": [],
            "fact_sets": [
                [{"id": "f1", "fields": {"tier": "Gold"}}]
            ]
        }"#;

        let request: SimulationRunRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.name, "test-sim");
        assert_eq!(request.proposed_changes.len(), 0);
        assert_eq!(request.fact_sets[0].len(), 1);
    }

    #[test]
    fn test_simulation_response_serialization() {
        let response = SimulationRunResponse {
            name: "test".to_string(),
            test_cases: vec![SimulationTestCaseResponse {
                fact_set_index: 0,
                variants: vec![],
                differences: vec![],
            }],
            total_execution_time_ms: 1.5,
            summary: SimulationSummary {
                total_variants: 1,
                total_fact_sets: 1,
                affected_test_cases: 0,
                rules_affected: 0,
            },
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"name\":\"test\""));
    }
}
