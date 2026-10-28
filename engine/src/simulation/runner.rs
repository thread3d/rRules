//! Simulation runner: executes What-If analyses against an engine instance.
//!
//! The runner takes a base `Engine` (with rules already loaded) and one or more
//! proposed rule changes, applies each change to an isolated clone of the engine,
//! runs both sides against identical test fact sets, and produces a structured
//! diff report showing which rules fired / did not fire / produced different
//! outputs.

use crate::data_types::Value;
use crate::environment::Engine;
use crate::fact::Fact;

use super::types::*;

/// Errors that can occur during simulation execution.
#[derive(Debug, thiserror::Error)]
pub enum SimulationRunnerError {
    #[error("Failed to deserialize condition: {0}")]
    InvalidCondition(String),

    #[error("Failed to deserialize action: {0}")]
    InvalidAction(String),

    #[error("Rule not found: {0}")]
    RuleNotFound(String),

    #[error("Invalid action index {index} for rule '{rule_id}' (has {count} actions)")]
    InvalidActionIndex {
        rule_id: String,
        index: usize,
        count: usize,
    },

    #[error("Fact set is empty: at least one fact must be provided per test case")]
    EmptyFactSet,

    #[error("Simulation failed: {0}")]
    General(String),
}

/// Result type for simulation runner operations.
pub type SimulationRunnerResult<T> = Result<T, SimulationRunnerError>;

/// The core What-If simulation engine.
///
/// Given a base `Engine` and one or more proposed rule changes, the runner:
/// 1. Executes the **base** engine against each fact set (baseline).
/// 2. For each proposed change, clones the base engine, applies the change,
///    and executes it against all fact sets (variant).
/// 3. Compares results between base and each variant to produce a diff report.
pub struct SimulationRunner;

impl SimulationRunner {
    /// Run a complete simulation: compare base execution against modified variants.
    pub fn run(base_engine: &Engine, request: &SimulationRequest) -> SimulationResult {
        let start_time = std::time::Instant::now();

        // Validate input early so we fail fast before doing any work.
        if request.fact_sets.is_empty() {
            return SimulationResult {
                name: request.name.clone(),
                test_cases: Vec::new(),
                total_execution_time_ms: 0.0,
            };
        }

        let variant_plans: Vec<VariantPlan> = request
            .proposed_changes
            .iter()
            .enumerate()
            .map(|(i, change)| VariantPlan {
                label: format_change_label(i, change),
                change: change.clone(),
            })
            .collect();

        let mut test_cases = Vec::with_capacity(request.fact_sets.len());

        for (fact_set_idx, fact_set) in request.fact_sets.iter().enumerate() {
            if fact_set.is_empty() {
                continue; // Skip empty fact sets.
            }

            let mut variants: Vec<VariantResult> = Vec::with_capacity(variant_plans.len() + 1);
            let mut differences: Vec<ChangeReport> = Vec::new();

            // Run base execution (no changes).
            match SimulationRunner::execute_variant(base_engine, fact_set) {
                Ok(result) => {
                    variants.push(result);
                }
                Err(err) => {
                    variants.push(VariantResult {
                        variant_label: "base".to_string(),
                        rules_fired: Vec::new(),
                        execution_time_ms: 0.0,
                        output: None,
                        success: false,
                        error: Some(err.to_string()),
                    });

                    differences.push(ChangeReport::ChangeApplicationError {
                        variant_label: "base".to_string(),
                        error: err.to_string(),
                    });
                }
            }

            // Run each variant with proposed changes.
            for plan in &variant_plans {
                match SimulationRunner::execute_variant_with_change(
                    base_engine,
                    fact_set,
                    &plan.change,
                ) {
                    Ok(result) => {
                        variants.push(result);
                    }
                    Err(err) => {
                        variants.push(VariantResult {
                            variant_label: plan.label.clone(),
                            rules_fired: Vec::new(),
                            execution_time_ms: 0.0,
                            output: None,
                            success: false,
                            error: Some(err.to_string()),
                        });

                        differences.push(ChangeReport::ChangeApplicationError {
                            variant_label: plan.label.clone(),
                            error: err.to_string(),
                        });
                    }
                }
            }

            // Compare each variant against the base (first element).
            if let Some(base_result) = variants.first() {
                for variant in variants.iter().skip(1) {
                    differences.extend(compute_differences(
                        &base_result.rules_fired,
                        &variant.rules_fired,
                        &variant.variant_label,
                        base_result.output.as_ref(),
                        variant.output.as_ref(),
                    ));
                }
            }

            test_cases.push(TestCaseResult {
                fact_set_index: fact_set_idx,
                variants,
                differences,
            });
        }

        let total_time = start_time.elapsed().as_secs_f64() * 1000.0;

        SimulationResult {
            name: request.name.clone(),
            test_cases,
            total_execution_time_ms: total_time,
        }
    }

    /// Execute a single variant (base or modified) against one fact set.
    fn execute_variant(
        base_engine: &Engine,
        fact_set: &[TestFact],
    ) -> SimulationRunnerResult<VariantResult> {
        let start_time = std::time::Instant::now();

        // Clone the engine for this variant.
        let mut variant_engine = base_engine.clone();

        Self::load_facts(&mut variant_engine, fact_set)?;
        let (rules_fired, output, success, error) = Self::execute(&mut variant_engine);

        let duration_ms = start_time.elapsed().as_secs_f64() * 1000.0;

        Ok(VariantResult {
            variant_label: "base".to_string(),
            rules_fired,
            execution_time_ms: duration_ms,
            output,
            success,
            error,
        })
    }

    /// Insert a fact set into a variant engine's working memory.
    ///
    /// This used to build a `WorkingMemory` and a `Fact` per test fact and drop both, so a "what-if"
    /// variant ran against an engine that had never seen the facts.
    fn load_facts(engine: &mut Engine, fact_set: &[TestFact]) -> SimulationRunnerResult<()> {
        for test_fact in fact_set {
            let fields: serde_json::Map<String, serde_json::Value> =
                match serde_json::from_value(test_fact.fields.clone()) {
                    Ok(m) => m,
                    Err(_) => serde_json::Map::new(), // Treat invalid JSON as empty.
                };

            let mut payload = std::collections::HashMap::with_capacity(fields.len());
            for (key, value) in fields {
                payload.insert(key, json_to_engine_value(&value));
            }

            engine
                .insert_fact_for_simulation(Fact::new(test_fact.id.clone(), payload))
                .map_err(SimulationRunnerError::General)?;
        }
        Ok(())
    }

    /// Run a variant engine to its fixpoint and report what happened.
    ///
    /// The result is no longer discarded: the fired rule ids come from the same evaluator the engine
    /// runs, and the facts the run produced become the variant's `output`, so two variants that behave
    /// differently produce different answers instead of two empty successes.
    fn execute(
        engine: &mut Engine,
    ) -> (Vec<String>, Option<serde_json::Value>, bool, Option<String>) {
        let report = crate::forward_chaining::run(engine);
        let output = if report.facts_added.is_empty() && report.facts_retracted.is_empty() {
            None
        } else {
            Some(serde_json::json!({
                "facts_added": report
                    .facts_added
                    .iter()
                    .map(|f| f.id.to_string())
                    .collect::<Vec<_>>(),
                "facts_retracted": report.facts_retracted,
            }))
        };
        let error = if report.errors.is_empty() {
            None
        } else {
            Some(report.errors.join("; "))
        };
        let success = error.is_none();
        (report.fired_rules, output, success, error)
    }

    /// Execute a variant with proposed changes applied against one fact set.
    fn execute_variant_with_change(
        base_engine: &Engine,
        fact_set: &[TestFact],
        change: &RuleChange,
    ) -> SimulationRunnerResult<VariantResult> {
        let start_time = std::time::Instant::now();

        // Clone the engine for this variant and apply the proposed change to *its* rules, so the
        // variant differs from the base by exactly the change under test.
        let mut variant_engine = base_engine.clone();
        Self::apply_change(&mut variant_engine, change)?;

        Self::load_facts(&mut variant_engine, fact_set)?;
        let (rules_fired, output, success, error) = Self::execute(&mut variant_engine);

        let duration_ms = start_time.elapsed().as_secs_f64() * 1000.0;

        Ok(VariantResult {
            variant_label: format_change_label(0, change),
            rules_fired,
            execution_time_ms: duration_ms,
            output,
            success,
            error,
        })
    }

    /// Apply one proposed change to a variant engine's rules.
    fn apply_change(engine: &mut Engine, change: &RuleChange) -> SimulationRunnerResult<()> {
        let rules = engine.rules_mut();
        match change {
            RuleChange::AddCondition { rule_id, condition } => {
                let parsed: crate::rules::Condition = serde_json::from_value(condition.clone())
                    .map_err(|e| {
                        SimulationRunnerError::InvalidCondition(format!("{rule_id}: {e}"))
                    })?;
                let rule = rules
                    .iter_mut()
                    .find(|r| r.name == *rule_id || r.id == *rule_id)
                    .ok_or_else(|| SimulationRunnerError::RuleNotFound(rule_id.clone()))?;
                // Conjoining with the existing condition is what "add a condition" means: the rule
                // fires only when both hold.
                rule.condition = crate::rules::Condition::And(vec![rule.condition.clone(), parsed]);
                Ok(())
            }
            RuleChange::RemoveCondition { rule_id, field } => {
                let rule = rules
                    .iter_mut()
                    .find(|r| r.name == *rule_id || r.id == *rule_id)
                    .ok_or_else(|| SimulationRunnerError::RuleNotFound(rule_id.clone()))?;
                rule.condition.remove_field(field);
                Ok(())
            }
            RuleChange::ModifyAction {
                rule_id,
                action_index,
                new_action,
            } => {
                let parsed: crate::rules::Action = serde_json::from_value(new_action.clone())
                    .map_err(|e| SimulationRunnerError::InvalidAction(format!("{rule_id}: {e}")))?;
                let rule = rules
                    .iter_mut()
                    .find(|r| r.name == *rule_id || r.id == *rule_id)
                    .ok_or_else(|| SimulationRunnerError::RuleNotFound(rule_id.clone()))?;
                let count = rule.actions.len();
                let slot = rule.actions.get_mut(*action_index).ok_or_else(|| {
                    SimulationRunnerError::InvalidActionIndex {
                        rule_id: rule_id.clone(),
                        index: *action_index,
                        count,
                    }
                })?;
                *slot = parsed;
                Ok(())
            }
            RuleChange::RemoveRule { rule_id } => {
                let before = rules.len();
                rules.retain(|r| r.name != *rule_id && r.id != *rule_id);
                if rules.len() == before {
                    return Err(SimulationRunnerError::RuleNotFound(rule_id.clone()));
                }
                Ok(())
            }
            RuleChange::AddRule { rule } => {
                let parsed: crate::rules::Rule = serde_json::from_value(rule.clone())
                    .map_err(|e| SimulationRunnerError::General(format!("invalid rule: {e}")))?;
                engine
                    .add_rule(parsed)
                    .map_err(SimulationRunnerError::General)
            }
            RuleChange::DisableRule { rule_id } => {
                let rule = engine
                    .rules_mut()
                    .iter_mut()
                    .find(|r| r.name == *rule_id || r.id == *rule_id)
                    .ok_or_else(|| SimulationRunnerError::RuleNotFound(rule_id.clone()))?;
                // A disabled rule is one that can never fire; `False` is exactly that, and it keeps the
                // rule visible in the variant's rule set so "disabled" and "removed" stay different.
                rule.condition = crate::rules::Condition::False;
                Ok(())
            }
        }
    }
}

/// Variant plan for one simulation variant (base or modified).
struct VariantPlan {
    label: String,
    change: RuleChange,
}

/// Compute differences between base and variant execution results.
fn compute_differences(
    base_rules: &[String],
    variant_rules: &[String],
    variant_label: &str,
    base_output: Option<&serde_json::Value>,
    variant_output: Option<&serde_json::Value>,
) -> Vec<ChangeReport> {
    let mut differences = Vec::new();

    // Find rules that fired in base but not in variant.
    for rule_id in base_rules {
        if !variant_rules.contains(rule_id) {
            differences.push(ChangeReport::RuleFiredBeforeButNotAfter {
                rule_id: rule_id.clone(),
                variant_label: variant_label.to_string(),
            });
        }
    }

    // Find rules that fired in variant but not in base.
    for rule_id in variant_rules {
        if !base_rules.contains(rule_id) {
            differences.push(ChangeReport::RuleFiredAfterButNotBefore {
                rule_id: rule_id.clone(),
                variant_label: variant_label.to_string(),
            });
        }
    }

    // Compare outputs if both variants produced output.
    if let (Some(base_out), Some(variant_out)) = (base_output, variant_output) {
        if base_out != variant_out {
            differences.push(ChangeReport::DifferentResultsForRule {
                rule_id: "output".to_string(),
                variant_label: variant_label.to_string(),
                before_output: base_out.clone(),
                after_output: variant_out.clone(),
            });
        }
    }

    differences
}

/// Generate a human-readable label for a proposed change.
fn format_change_label(index: usize, change: &RuleChange) -> String {
    match change {
        RuleChange::AddCondition { rule_id, .. } => {
            format!("add-condition(rule={})", rule_id)
        }
        RuleChange::RemoveCondition { rule_id, field } => {
            format!("remove-field(rule={},field={})", rule_id, field)
        }
        RuleChange::ModifyAction {
            rule_id,
            action_index,
            ..
        } => {
            format!("modify-action(rule={},index={})", rule_id, action_index)
        }
        RuleChange::RemoveRule { rule_id } => {
            format!("remove-rule(rule={})", rule_id)
        }
        RuleChange::AddRule { .. } => {
            format!("add-rule(index={})", index)
        }
        RuleChange::DisableRule { rule_id } => {
            format!("disable-rule(rule={})", rule_id)
        }
    }
}

/// Convert a JSON value to an engine Value.
fn json_to_engine_value(value: &serde_json::Value) -> Value {
    match value {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(b) => Value::Bool(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Integer(i)
            } else if let Some(f) = n.as_f64() {
                Value::Float(f)
            } else {
                Value::Null
            }
        }
        serde_json::Value::String(s) => Value::String(s.clone()),
        serde_json::Value::Array(arr) => {
            let values: Vec<Value> = arr.iter().map(json_to_engine_value).collect();
            Value::Array(values)
        }
        serde_json::Value::Object(obj) => {
            let map: std::collections::HashMap<String, Value> = obj
                .iter()
                .map(|(k, v)| (k.clone(), json_to_engine_value(v)))
                .collect();
            Value::Object(map)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_change_label() {
        assert_eq!(
            format_change_label(
                0,
                &RuleChange::AddCondition {
                    rule_id: "r1".to_string(),
                    condition: serde_json::json!({"True": null}),
                }
            ),
            "add-condition(rule=r1)"
        );

        assert_eq!(
            format_change_label(
                1,
                &RuleChange::RemoveRule {
                    rule_id: "r2".to_string(),
                }
            ),
            "remove-rule(rule=r2)"
        );

        assert_eq!(
            format_change_label(
                2,
                &RuleChange::DisableRule {
                    rule_id: "r3".to_string(),
                }
            ),
            "disable-rule(rule=r3)"
        );
    }

    #[test]
    fn test_simulation_runner_empty_fact_sets() {
        use crate::environment::EngineBuilder;

        let engine = EngineBuilder::new().build();
        let request = SimulationRequest {
            name: "test".to_string(),
            base_state: None,
            proposed_changes: vec![],
            fact_sets: vec![], // Empty.
        };

        let result = SimulationRunner::run(&engine, &request);
        assert!(result.test_cases.is_empty());
    }

    #[test]
    fn test_simulation_runner_with_base() {
        use crate::environment::EngineBuilder;

        let engine = EngineBuilder::new().build();

        let request = SimulationRequest {
            name: "base-only".to_string(),
            base_state: None,
            proposed_changes: vec![],
            fact_sets: vec![vec![TestFact {
                id: "f1".to_string(),
                fields: serde_json::json!({"tier": "Gold"}),
            }]],
        };

        let result = SimulationRunner::run(&engine, &request);
        assert_eq!(result.test_cases.len(), 1);
        // Should have one variant (base).
        assert_eq!(result.test_cases[0].variants.len(), 1);
    }
}
