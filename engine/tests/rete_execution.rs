//! Plan 23-03 Task 2: integration tests for the real direct rule evaluator on
//! `ReteEngine`. Exercises the keystone `score := score + 10` scenario, the
//! fixpoint forward-chaining loop (D-07), the once-per-rule coverage recording
//! (L-06 / Pitfall 6 regression), and the cyclic-rule termination guard.
//!
//! Filter: `cargo test -p engine --features engine-rete --test rete_execution`

use engine::engine::registry::ExecutionEngine;
use engine::engine::rete::ReteEngine;
use engine::data_types::Value;
use engine::fact::Fact;
use engine::monitoring::coverage::CoverageTracking;
use engine::rules::{Action, ComparisonOp, Condition, Expression, Rule, RuleSet};
use std::collections::HashMap;

/// Build a scorecard fact with the given integer `score` payload.
fn scorecard_fact(score: i64) -> Fact {
    let mut payload = HashMap::new();
    payload.insert("score".to_string(), Value::Integer(score));
    Fact::new("scorecard", payload)
}

/// Build the keystone rule: `r1` with condition `score == 0` and action
/// `score := score + 10`.
fn score_plus_10_rule() -> Rule {
    let mut fields = HashMap::new();
    fields.insert(
        "score".to_string(),
        Expression::Add(
            Box::new(Expression::FieldRef {
                fact_type: "scorecard".to_string(),
                field: "score".to_string(),
            }),
            Box::new(Expression::Literal(Value::Integer(10))),
        ),
    );
    Rule::builder("r1", "Increment score")
        .condition(Condition::Pattern {
            fact_type: "scorecard".to_string(),
            field: "score".to_string(),
            value: Value::Integer(0),
            op: ComparisonOp::Eq,
        })
        .action(Action::UpdateFact {
            id: "scorecard".to_string(),
            fields,
        })
        .build()
}

/// Build a single-field `UpdateFact` action with a literal value.
fn literal_update(id: &str, field: &str, value: Value) -> Action {
    let mut fields = HashMap::new();
    fields.insert(field.to_string(), Expression::Literal(value));
    Action::UpdateFact {
        id: id.to_string(),
        fields,
    }
}

/// Every test in this file calls `execute()`, which records into the
/// process-global `CoverageTracking` singleton. To keep the coverage
/// regression assertions truthful (Pitfall 6), all tests are marked
/// `#[serial_test::serial]` so no two run concurrently, and each clears
/// the singleton at start.
fn reset_coverage() {
    CoverageTracking.clear();
}

// =====================================================================
// Keystone scenario: score := score + 10
// =====================================================================

/// The keystone: a rule with a matching Pattern condition and a computed
/// `UpdateFact` action yields `rules_fired >= 1` and `get_facts()` showing
/// `score == 10` (ROADMAP success criterion #1).
#[serial_test::serial]
#[test]
fn fires_on_matching_fact() {
    reset_coverage();
    let mut ruleset = RuleSet::new();
    ruleset.add_rule(score_plus_10_rule()).unwrap();

    let mut engine = ReteEngine::new();
    engine.load_ruleset(&ruleset).unwrap();
    engine.assert_fact(scorecard_fact(0));

    let result = engine.execute();
    assert!(result.success, "execute should succeed");
    assert!(
        result.rules_fired >= 1,
        "keystone rule should fire, got rules_fired = {}",
        result.rules_fired
    );
    assert!(
        result.fired_rule_ids.contains(&"r1".to_string()),
        "fired_rule_ids should contain r1, got {:?}",
        result.fired_rule_ids
    );

    let facts = engine.get_facts();
    let scorecard = facts
        .iter()
        .find(|f| f.fact_type_id == "scorecard")
        .expect("scorecard fact should exist after execute");
    assert_eq!(
        scorecard.payload.get("score"),
        Some(&Value::Integer(10)),
        "score should be incremented to 10, got {:?}",
        scorecard.payload.get("score")
    );
}

/// `get_facts()` after `execute()` reflects the action-mutated working memory,
/// not the request input (Pitfall 11).
#[serial_test::serial]
#[test]
fn get_facts_after_execute_shows_incremented_score() {
    reset_coverage();
    let mut ruleset = RuleSet::new();
    ruleset.add_rule(score_plus_10_rule()).unwrap();

    let mut engine = ReteEngine::new();
    engine.load_ruleset(&ruleset).unwrap();
    engine.assert_fact(scorecard_fact(0));

    let _ = engine.execute();

    let facts = engine.get_facts();
    assert_eq!(facts.len(), 1, "one scorecard fact should be present");
    assert_eq!(
        facts[0].payload.get("score"),
        Some(&Value::Integer(10)),
        "facts_after score should be 10 (incremented), not 0 (echoed)"
    );
}

// =====================================================================
// Non-matching condition
// =====================================================================

/// A rule whose condition does not match any fact yields `rules_fired == 0`
/// and does not appear in `fired_rule_ids`.
#[serial_test::serial]
#[test]
fn non_matching_condition_fires_zero() {
    reset_coverage();
    let mut ruleset = RuleSet::new();
    ruleset.add_rule(score_plus_10_rule()).unwrap();

    let mut engine = ReteEngine::new();
    engine.load_ruleset(&ruleset).unwrap();
    // score=10 does not match the `score == 0` condition.
    engine.assert_fact(scorecard_fact(10));

    let result = engine.execute();
    assert_eq!(result.rules_fired, 0, "non-matching rule should not fire");
    assert!(result.fired_rule_ids.is_empty());
}

// =====================================================================
// Fixpoint forward chaining (D-07)
// =====================================================================

/// A two-rule chain where rule A's action enables rule B's condition fires
/// both within a single `execute()` call (fixpoint iteration).
#[serial_test::serial]
#[test]
fn fixpoint_chain_fires_both_rules() {
    reset_coverage();
    let a = Rule::builder("a", "A: 0 -> 10")
        .condition(Condition::Pattern {
            fact_type: "scorecard".to_string(),
            field: "score".to_string(),
            value: Value::Integer(0),
            op: ComparisonOp::Eq,
        })
        .action(literal_update("scorecard", "score", Value::Integer(10)))
        .build();
    let b = Rule::builder("b", "B: 10 -> 20")
        .condition(Condition::Pattern {
            fact_type: "scorecard".to_string(),
            field: "score".to_string(),
            value: Value::Integer(10),
            op: ComparisonOp::Eq,
        })
        .action(literal_update("scorecard", "score", Value::Integer(20)))
        .build();

    let mut ruleset = RuleSet::new();
    ruleset.add_rule(a).unwrap();
    ruleset.add_rule(b).unwrap();

    let mut engine = ReteEngine::new();
    engine.load_ruleset(&ruleset).unwrap();
    engine.assert_fact(scorecard_fact(0));

    let result = engine.execute();
    assert_eq!(
        result.rules_fired, 2,
        "both chain rules should fire via fixpoint iteration"
    );
    assert!(result.fired_rule_ids.contains(&"a".to_string()));
    assert!(result.fired_rule_ids.contains(&"b".to_string()));

    let facts = engine.get_facts();
    assert_eq!(
        facts[0].payload.get("score"),
        Some(&Value::Integer(20)),
        "chain should drive score to 20"
    );
}

// =====================================================================
// Cyclic rule set termination (D-07 cycle guard)
// =====================================================================

/// A cyclic rule set (rule's action re-enables its own condition) terminates
/// without panicking. The once-per-rule `fired_set` guard bounds the loop at
/// one fire per rule; the `RRULES_MAX_CHAIN_ITERATIONS` cap is the ultimate
/// safety net. The test sets a low cap and asserts the call returns.
#[serial_test::serial]
#[test]
fn cyclic_rule_set_terminates_without_panic() {
    reset_coverage();
    // Low cap so a runaway loop would finish quickly either way; the
    // once-per-rule guard terminates the loop well before the cap.
    // SAFETY on env var: tests run in a single process; we set a unique
    // value here and the evaluator reads it once per execute() call.
    std::env::set_var("RRULES_MAX_CHAIN_ITERATIONS", "10");

    let mut fields = HashMap::new();
    fields.insert(
        "score".to_string(),
        Expression::Add(
            Box::new(Expression::FieldRef {
                fact_type: "scorecard".to_string(),
                field: "score".to_string(),
            }),
            Box::new(Expression::Literal(Value::Integer(1))),
        ),
    );
    let cyclic = Rule::builder("cyc", "Cyclic increment")
        .condition(Condition::True)
        .action(Action::UpdateFact {
            id: "scorecard".to_string(),
            fields,
        })
        .build();

    let mut ruleset = RuleSet::new();
    ruleset.add_rule(cyclic).unwrap();

    let mut engine = ReteEngine::new();
    engine.load_ruleset(&ruleset).unwrap();
    engine.assert_fact(scorecard_fact(0));

    // Must return (terminate) — no panic, no infinite loop.
    let result = engine.execute();
    assert!(result.success, "cyclic execute should succeed (terminate)");
    // The once-per-rule guard fires `cyc` exactly once.
    assert_eq!(
        result.rules_fired, 1,
        "once-per-rule guard should fire cyc exactly once"
    );

    std::env::remove_var("RRULES_MAX_CHAIN_ITERATIONS");
}

// =====================================================================
// Coverage regression: once-per-rule (Pitfall 6)
// =====================================================================

/// After one `execute()` that fires rule `r1`, `CoverageTracking` records
/// `fire_count == 1` (not `> 1` from the old per-nagram double-count —
/// Pitfall 6). Serial-gated because `CoverageTracking` is a process-wide
/// singleton.
#[serial_test::serial]
#[test]
fn coverage_records_fire_count_one_for_single_fire() {
    reset_coverage();

    let mut ruleset = RuleSet::new();
    ruleset.add_rule(score_plus_10_rule()).unwrap();

    let mut engine = ReteEngine::new();
    engine.load_ruleset(&ruleset).unwrap();
    engine.assert_fact(scorecard_fact(0));

    let result = engine.execute();
    assert!(result.rules_fired >= 1);

    let tracker = CoverageTracking;
    let state = tracker
        .get_rule_state("r1")
        .expect("r1 should be tracked after firing");
    assert!(
        state.fired,
        "r1 should be marked fired in CoverageTracking"
    );
    assert_eq!(
        state.fire_count, 1,
        "Pitfall 6 regression: fire_count should be 1 (once-per-rule), got {}",
        state.fire_count
    );
    assert!(
        state.evaluated,
        "r1 should be marked evaluated in CoverageTracking"
    );
}

/// A non-matching rule is evaluated but not fired: `evaluated == true`,
/// `fired == false`, `fire_count == 0`.
#[serial_test::serial]
#[test]
fn coverage_records_eval_but_not_fire_for_non_matching_rule() {
    reset_coverage();

    let mut ruleset = RuleSet::new();
    ruleset.add_rule(score_plus_10_rule()).unwrap();

    let mut engine = ReteEngine::new();
    engine.load_ruleset(&ruleset).unwrap();
    // score=10 does not match `score == 0`.
    engine.assert_fact(scorecard_fact(10));

    let _ = engine.execute();

    let tracker = CoverageTracking;
    let state = tracker
        .get_rule_state("r1")
        .expect("r1 should be tracked after evaluation");
    assert!(state.evaluated, "r1 should be evaluated");
    assert!(!state.fired, "r1 should not be fired");
    assert_eq!(state.fire_count, 0);
}