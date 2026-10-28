//! Goal-driven backward chaining for interactive debugging.
//!
//! This module implements the execute_backward function that starts from a query
//! and traces the reasoning path to supporting facts.

use crate::environment::{Engine, Session};

/// Result of a backward chaining inference.
pub struct InferenceTrace {
    /// Whether the query succeeded
    pub success: bool,
    /// Full reasoning path showing how conclusion was reached
    pub path: Vec<InferenceStep>,
    /// Facts that matched at each step
    pub fact_matches: Vec<String>,
}

impl InferenceTrace {
    pub fn new(success: bool) -> Self {
        Self {
            success,
            path: Vec::new(),
            fact_matches: Vec::new(),
        }
    }

    pub fn with_step(mut self, step: InferenceStep) -> Self {
        self.path.push(step);
        self
    }

    /// Generates a human-readable explanation of the reasoning.
    pub fn explain(&self) -> String {
        let mut lines = vec!["Inference Trace:".to_string()];

        for (i, step) in self.path.iter().enumerate() {
            lines.push(format!("  {}. {}", i + 1, step.description));
        }

        if !self.fact_matches.is_empty() {
            lines.push("\nMatching Facts:".to_string());
            for fact in &self.fact_matches {
                lines.push(format!("  - {}", fact));
            }
        }

        lines.join("\n")
    }
}

/// A single step in the inference trace.
#[derive(Debug, Clone)]
pub struct InferenceStep {
    /// Step description
    pub description: String,
    /// Rule that was applied (if any)
    pub rule_id: Option<String>,
    /// Type of step
    pub step_type: StepType,
}

/// Types of inference steps.
#[derive(Debug, Clone, PartialEq)]
pub enum StepType {
    /// Starting from a query/goal
    Query,
    /// Applying a rule's condition
    RuleApplication,
    /// Sub-goals created from rule conditions
    SubGoal,
    /// Fact match that satisfied a condition
    FactMatch,
    /// Backtracking due to failed sub-goal
    Backtrack,
}

/// Executes backward chaining from a goal.
///
/// The goal names a field of a fact type — `"scorecard.score"` — and the search answers *how* that
/// field could be produced: which rule writes it, whether that rule's conditions are satisfied by the
/// facts in working memory, and, where one is not, which rules could satisfy it in turn. The result is
/// the path a reader would draw on a whiteboard, and `explain()` prints it.
///
/// This is a real search, not a stub: it reads the engine's rules and working memory, and the answer
/// changes when they do. What it does **not** do is unify variables or bind facts across rules as a
/// logic engine would; conditions are matched against the facts that exist, one level of
/// "who could supply this" at a time. `specs/rule-engine.md` states the limit.
///
/// This is the main entry point for backward rule execution as exported from lib.rs.
pub fn execute_backward(engine: &Engine, _session: &Session, goal: &str) -> InferenceTrace {
    let mut trace = InferenceTrace::new(false).with_step(InferenceStep {
        description: format!("Query: {goal}"),
        rule_id: None,
        step_type: StepType::Query,
    });

    let Some((fact_type, field)) = goal.split_once('.') else {
        trace.path.push(InferenceStep {
            description: format!(
                "The goal `{goal}` does not name a field (`<fact_type>.<field>`), so nothing can be \
                 searched for"
            ),
            rule_id: None,
            step_type: StepType::Backtrack,
        });
        return trace;
    };

    let succeeded = search(engine, fact_type.trim(), field.trim(), &mut trace, 0);
    trace.success = succeeded;
    if !succeeded {
        trace.path.push(InferenceStep {
            description: format!(
                "No rule can produce `{}` from the facts in working memory",
                goal.trim()
            ),
            rule_id: None,
            step_type: StepType::Backtrack,
        });
    }
    trace
}

/// How deep the "who could supply this" recursion goes. A rule set whose chain is longer than this is
/// reported as unexplored rather than searched for ever.
const MAX_DEPTH: usize = 8;

/// Does any rule write `field`, and can its conditions be satisfied?
fn search(
    engine: &Engine,
    fact_type: &str,
    field: &str,
    trace: &mut InferenceTrace,
    depth: usize,
) -> bool {
    if depth > MAX_DEPTH {
        trace.path.push(InferenceStep {
            description: format!("Depth limit reached while looking for `{fact_type}.{field}`"),
            rule_id: None,
            step_type: StepType::Backtrack,
        });
        return false;
    }

    // Rules whose actions write the field: that is what it means for a rule to "produce" it.
    let producers: Vec<&crate::rules::Rule> = engine
        .rules()
        .iter()
        .filter(|rule| writes_field(rule, field))
        .collect();

    if producers.is_empty() {
        return false;
    }

    for rule in producers {
        trace.path.push(InferenceStep {
            description: format!(
                "Rule `{}` writes `{}` via {}",
                rule.name,
                field,
                describe_conditions(&rule.condition)
            ),
            rule_id: Some(rule.id.clone()),
            step_type: StepType::RuleApplication,
        });

        // A rule's conditions name the fact types *they* read, which need not be the type in the
        // goal: the supplying rule behind `line.amount` may read an `order`. Evaluating against the
        // sub-goal's own type made every chain look unsatisfiable one link in.
        let facts = engine.working_memory().get_all_facts();
        if rule.condition.evaluate(&facts) {
            for fact in facts.iter().filter(|fact| {
                fact.fact_type_id == fact_type || rule_reads_type(rule, &fact.fact_type_id)
            }) {
                trace
                    .fact_matches
                    .push(describe_fact(&fact.fact_type_id, fact));
            }
            trace.path.push(InferenceStep {
                description: format!("`{}` is satisfiable in working memory", rule.name),
                rule_id: Some(rule.id.clone()),
                step_type: StepType::FactMatch,
            });
            return true;
        }

        // Not satisfied: say which sub-conditions are missing, and recurse into each one that another
        // rule could produce.
        let mut unresolved = Vec::new();
        collect_unmet_patterns(&rule.condition, &facts, &mut unresolved);
        let mut all_supplied = !unresolved.is_empty();
        for (needed_type, needed_field) in unresolved {
            trace.path.push(InferenceStep {
                description: format!("Sub-goal: {needed_type}.{needed_field}"),
                rule_id: Some(rule.id.clone()),
                step_type: StepType::SubGoal,
            });
            if !search(engine, &needed_type, &needed_field, trace, depth + 1) {
                all_supplied = false;
            }
        }

        if all_supplied {
            trace.path.push(InferenceStep {
                description: format!("`{}` can be satisfied through its sub-goals", rule.name),
                rule_id: Some(rule.id.clone()),
                step_type: StepType::RuleApplication,
            });
            return true;
        }

        trace.path.push(InferenceStep {
            description: format!("Rule `{}` cannot be satisfied; trying the next", rule.name),
            rule_id: Some(rule.id.clone()),
            step_type: StepType::Backtrack,
        });
    }

    false
}

/// Does this rule's actions write `field`?
fn writes_field(rule: &crate::rules::Rule, field: &str) -> bool {
    rule.actions.iter().any(|action| match action {
        crate::rules::Action::UpdateFact { fields, .. } => fields.contains_key(field),
        // An inserted fact can also carry the field.
        crate::rules::Action::InsertFact { fields, .. } => fields.contains_key(field),
        _ => false,
    })
}

/// The `(fact_type, field)` pairs a condition needs but the facts do not provide.
fn collect_unmet_patterns(
    condition: &crate::rules::Condition,
    facts: &[&crate::fact::Fact],
    unmet: &mut Vec<(String, String)>,
) {
    use crate::rules::Condition;
    match condition {
        Condition::Pattern {
            fact_type, field, ..
        } => {
            // A pattern is unmet when no fact of that type carries the field at all.
            let provided = facts
                .iter()
                .any(|fact| fact.fact_type_id == *fact_type && fact.payload.contains_key(field));
            if !provided {
                unmet.push((fact_type.clone(), field.clone()));
            }
        }
        Condition::And(conditions) | Condition::Or(conditions) => {
            for condition in conditions {
                collect_unmet_patterns(condition, facts, unmet);
            }
        }
        Condition::Not(inner) => collect_unmet_patterns(inner, facts, unmet),
        _ => {}
    }
}

/// Does this rule's condition read facts of `fact_type`?
fn rule_reads_type(rule: &crate::rules::Rule, fact_type: &str) -> bool {
    fn reads(condition: &crate::rules::Condition, fact_type: &str) -> bool {
        use crate::rules::Condition;
        match condition {
            Condition::Pattern {
                fact_type: wanted, ..
            } => wanted == fact_type,
            Condition::And(conditions) | Condition::Or(conditions) => conditions
                .iter()
                .any(|condition| reads(condition, fact_type)),
            Condition::Not(inner) => reads(inner, fact_type),
            _ => false,
        }
    }
    reads(&rule.condition, fact_type)
}

fn describe_conditions(condition: &crate::rules::Condition) -> String {
    use crate::rules::Condition;
    match condition {
        Condition::True => "an always-true condition".to_string(),
        Condition::False => "an always-false condition".to_string(),
        Condition::Pattern {
            fact_type,
            field,
            value,
            op,
        } => format!("{fact_type}.{field} {op:?} {value:?}"),
        Condition::And(conditions) => format!("all of [{}]", describe_all(conditions)),
        Condition::Or(conditions) => format!("any of [{}]", describe_all(conditions)),
        Condition::Not(inner) => format!("not ({})", describe_conditions(inner)),
        other => format!("{other:?}"),
    }
}

fn describe_all(conditions: &[crate::rules::Condition]) -> String {
    conditions
        .iter()
        .map(describe_conditions)
        .collect::<Vec<_>>()
        .join(", ")
}

fn describe_fact(fact_type: &str, fact: &crate::fact::Fact) -> String {
    let mut fields: Vec<String> = fact
        .payload
        .iter()
        .map(|(name, value)| format!("{name} = {value:?}"))
        .collect();
    fields.sort();
    format!("{fact_type} {{ {} }}", fields.join(", "))
}

/// Interactive trace explorer for debugging.
pub struct TraceExplorer<'a> {
    trace: &'a InferenceTrace,
    /// Current position in the trace
    position: usize,
    /// Maximum depth to explore
    max_depth: usize,
}

impl<'a> TraceExplorer<'a> {
    pub fn new(trace: &'a InferenceTrace) -> Self {
        Self {
            trace,
            position: 0,
            max_depth: 10, // Default limit
        }
    }

    /// Moves to the next step in the trace.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Option<&InferenceStep> {
        if self.position < self.trace.path.len() && self.position < self.max_depth {
            let step = &self.trace.path[self.position];
            self.position += 1;
            Some(step)
        } else {
            None
        }
    }

    /// Moves back to the previous step.
    pub fn prev(&mut self) -> Option<&InferenceStep> {
        if self.position > 0 {
            self.position -= 1;
            Some(&self.trace.path[self.position])
        } else {
            None
        }
    }

    /// Gets the current step without advancing.
    pub fn current(&self) -> Option<&InferenceStep> {
        if self.position == 0 && !self.trace.path.is_empty() {
            // Before any next() call, return first item
            Some(&self.trace.path[0])
        } else if self.position > 0
            && self.position <= self.trace.path.len()
            && self.position <= self.max_depth
        {
            // After next(), return the last step returned (position - 1)
            Some(&self.trace.path[self.position - 1])
        } else {
            None
        }
    }

    /// Resets to the beginning of the trace.
    pub fn reset(&mut self) {
        self.position = 0;
    }

    /// Checks if there are more steps to explore.
    pub fn has_more(&self) -> bool {
        self.position < self.trace.path.len() && self.position < self.max_depth
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inference_trace_explanation() {
        let trace = InferenceTrace::new(true)
            .with_step(InferenceStep {
                description: "Query: Is customer eligible for discount?".to_string(),
                rule_id: None,
                step_type: StepType::Query,
            })
            .with_step(InferenceStep {
                description: "Rule 'DiscountEligible' matched with customer facts".to_string(),
                rule_id: Some("discount-1".to_string()),
                step_type: StepType::RuleApplication,
            });

        let explanation = trace.explain();
        assert!(explanation.contains("Inference Trace:"));
        assert!(explanation.contains("Query: Is customer eligible for discount?"));
    }

    #[test]
    fn test_trace_explorer() {
        let mut trace = InferenceTrace::new(true);
        trace.path.push(InferenceStep {
            description: "Step 1".to_string(),
            rule_id: None,
            step_type: StepType::Query,
        });
        trace.path.push(InferenceStep {
            description: "Step 2".to_string(),
            rule_id: None,
            step_type: StepType::RuleApplication,
        });

        let mut explorer = TraceExplorer::new(&trace);

        assert!(explorer.has_more());
        assert_eq!(explorer.next().unwrap().description, "Step 1");
        assert_eq!(explorer.current().unwrap().description, "Step 1");

        assert!(explorer.has_more());
        assert_eq!(explorer.next().unwrap().description, "Step 2");
        assert!(!explorer.has_more());

        explorer.reset();
        assert!(explorer.has_more());
        assert_eq!(explorer.current().unwrap().description, "Step 1");
    }
}
