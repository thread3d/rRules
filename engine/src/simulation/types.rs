//! Simulation request and response types for What-If analysis.
//!
//! These types define the contract between the simulation runner (engine-side)
//! and API consumers (HTTP clients). All fields use `serde_json::Value` for
//! rule conditions and actions so that arbitrary JSON payloads can be validated
//! against the engine's internal `Condition` / `Action` enums at execution time.

use serde::{Deserialize, Serialize};

/// Top-level request body for `POST /v1/simulation/run`.
///
/// A simulation compares a **base** rule set (the current production state)
/// against one or more **modified** copies where `proposed_changes` have been
/// applied. Each test case is then executed against both sides and the results
/// are diffed to produce `ChangeReport`s.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationRequest {
    /// Free-form name for this simulation (used in responses).
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
}

/// A single modification that can be applied to the rule set during simulation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RuleChange {
    /// Add a new condition to an existing rule's LHS.
    AddCondition {
        rule_id: String,
        /// JSON-encoded `engine::rules::Condition` value.
        condition: serde_json::Value,
    },

    /// Remove the named field from a rule's pattern-based condition. The exact
    /// semantics depend on how the engine evaluates conditions; this is a
    /// best-effort structural modification of the AST.
    RemoveCondition {
        rule_id: String,
        field: String,
    },

    /// Replace an action at a given index with a new one (JSON-encoded).
    ModifyAction {
        rule_id: String,
        action_index: usize,
        /// JSON-encoded `engine::rules::Action` value.
        new_action: serde_json::Value,
    },

    /// Remove the entire rule from the working set so it cannot fire.
    RemoveRule { rule_id: String },

    /// Add a brand-new rule to the engine (JSON-encoded as a full `Rule`).
    AddRule {
        /// JSON-encoded `engine::rules::Rule` value.
        rule: serde_json::Value,
    },

    /// Disable a rule without removing it, so its conditions are still present
    /// but will never fire. Useful when you want to compare "rule disabled" vs
    /// "rule removed".
    DisableRule { rule_id: String },
}

/// A single fact used as input for one test case within a simulation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestFact {
    /// Logical name / type identifier for the fact (e.g. `"Customer"`, `"Order"`).
    pub id: String,

    /// Key-value payload describing the fact's attributes.
    #[serde(default)]
    pub fields: serde_json::Value,
}

/// Snapshot of working memory facts used to seed a simulation run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkingMemorySnapshot {
    /// Facts in the snapshot. Each entry has an `id` and a JSON payload.
    #[serde(default)]
    pub facts: Vec<SnapshotFact>,
}

/// A single fact within a working memory snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotFact {
    /// Stable identifier for the fact.
    pub id: String,

    /// Logical type / category of the fact (e.g. `"Customer"`, `"Order"`).
    #[serde(rename = "type")]
    pub fact_type: String,

    /// Key-value payload describing this fact's attributes.
    #[serde(default)]
    pub fields: serde_json::Value,
}

/// Aggregated result of a simulation across all test cases and variants.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationResult {
    /// The `name` field from the original request.
    pub name: String,

    /// One entry per `(fact_set_index, variant)` pair. The first entry (index 0)
    /// is always the **base** execution with no changes applied; subsequent
    /// entries correspond to each `proposed_changes` entry in order.
    #[serde(default)]
    pub test_cases: Vec<TestCaseResult>,

    /// Total wall-clock time spent running all executions (milliseconds).
    #[serde(default)]
    pub total_execution_time_ms: f64,
}

/// Per-test-case comparison result between base and modified engine states.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCaseResult {
    /// Zero-based index of the fact set this result corresponds to within the
    /// original `fact_sets` array.
    pub fact_set_index: usize,

    /// Execution results for each variant (base + one per proposed change).
    #[serde(default)]
    pub variants: Vec<VariantResult>,

    /// Summary of differences across all variants that fired differently from
    /// the base execution. Empty when no rules changed behavior.
    #[serde(default)]
    pub differences: Vec<ChangeReport>,
}

/// Execution outcome for a single variant (base or modified engine).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariantResult {
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

/// Describes a behavioral difference between base and modified executions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChangeReport {
    /// A rule fired in the base execution but does not fire with the proposed
    /// changes applied (or vice versa).
    RuleFiredBeforeButNotAfter {
        rule_id: String,
        variant_label: String,
    },

    /// A rule fires with the proposed changes but did not fire in the base
    /// execution.
    RuleFiredAfterButNotBefore {
        rule_id: String,
        variant_label: String,
    },

    /// The same rule fired in both executions but produced different outputs.
    DifferentResultsForRule {
        rule_id: String,
        variant_label: String,
        before_output: serde_json::Value,
        after_output: serde_json::Value,
    },

    /// A structural issue with the proposed change itself (e.g. unknown rule ID).
    ChangeApplicationError {
        variant_label: String,
        error: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simulation_request_roundtrip() {
        let req = SimulationRequest {
            name: "discount-test".to_string(),
            base_state: None,
            proposed_changes: vec![RuleChange::AddCondition {
                rule_id: "rule-1".to_string(),
                condition: serde_json::json!({"Pattern": {"fact_type": "Customer", "field": "tier", "value": "Gold"}}),
            }],
            fact_sets: vec![vec![TestFact {
                id: "c1".to_string(),
                fields: serde_json::json!({"name": "Alice", "tier": "Gold"}),
            }]],
        };

        let json = serde_json::to_string(&req).expect("serialize");
        let deserialized: SimulationRequest = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(deserialized.name, "discount-test");
        assert_eq!(deserialized.proposed_changes.len(), 1);
        assert_eq!(deserialized.fact_sets[0].len(), 1);
    }

    #[test]
    fn test_simulation_result_serialization() {
        let result = SimulationResult {
            name: "test".to_string(),
            test_cases: vec![TestCaseResult {
                fact_set_index: 0,
                variants: vec![],
                differences: vec![],
            }],
            total_execution_time_ms: 1.5,
        };

        let json = serde_json::to_string(&result).expect("serialize");
        assert!(json.contains("\"name\":\"test\""));
    }

    #[test]
    fn test_change_report_variants() {
        // Verify all ChangeReport variants serialize correctly.
        for report in [
            ChangeReport::RuleFiredBeforeButNotAfter {
                rule_id: "r1".to_string(),
                variant_label: "v1".to_string(),
            },
            ChangeReport::RuleFiredAfterButNotBefore {
                rule_id: "r2".to_string(),
                variant_label: "v2".to_string(),
            },
            ChangeReport::DifferentResultsForRule {
                rule_id: "r3".to_string(),
                variant_label: "v3".to_string(),
                before_output: serde_json::json!({"approved": true}),
                after_output: serde_json::json!({"approved": false}),
            },
            ChangeReport::ChangeApplicationError {
                variant_label: "v4".to_string(),
                error: "unknown rule".to_string(),
            },
        ] {
            let json = serde_json::to_string(&report).expect("serialize");
            assert!(!json.is_empty());
            // Verify we can roundtrip back.
            if let ChangeReport::RuleFiredBeforeButNotAfter { .. } = &report {
                let _: ChangeReport = serde_json::from_str(&json).unwrap();
            }
        }
    }

    #[test]
    fn test_rule_change_variants() {
        for change in [
            RuleChange::AddCondition {
                rule_id: "r1".to_string(),
                condition: serde_json::json!({"True": null}),
            },
            RuleChange::RemoveCondition {
                rule_id: "r2".to_string(),
                field: "tier".to_string(),
            },
            RuleChange::ModifyAction {
                rule_id: "r3".to_string(),
                action_index: 0,
                new_action: serde_json::json!({"LogMessage": {"level": "Info", "message": "test"}}),
            },
            RuleChange::RemoveRule {
                rule_id: "r4".to_string(),
            },
            RuleChange::AddRule {
                rule: serde_json::json!({"id": "new-1", "name": "New", "salience": 0, "no_loop": false, "lock_on_active": false, "condition": {"True": null}, "actions": []}),
            },
            RuleChange::DisableRule {
                rule_id: "r5".to_string(),
            },
        ] {
            let json = serde_json::to_string(&change).expect("serialize");
            assert!(!json.is_empty());
        }
    }
}
