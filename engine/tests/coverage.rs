//! Plan 23-04 Task 2: Wave 0 coverage regression scaffold (Pitfall 6).
//!
//! Establishes the once-per-rule recording contract: after one `execute()`
//! that fires rule `r1`, `CoverageTracking::get_rule_state("r1").fire_count`
//! MUST be `1` — not `> 1` as the old per-nagram inner-loop recording
//! produced (Pitfall 6 / L-06). The direct evaluator (Plan 23-03) collects
//! fired ids in a `HashSet` and calls `record_rule_fire` once per rule AFTER
//! the fixpoint loop; this test guards against any future regression that
//! re-introduces inner-loop recording.
//!
//! This scaffold is the engine-level half of the MON-05 success criterion
//! (the HTTP-level half lives in `api/tests/execute_integration.rs`).
//!
//! Filter: `cargo test -p engine --features engine-rete --test coverage`

use engine::data_types::Value;
use engine::engine::registry::ExecutionEngine;
use engine::engine::rete::ReteEngine;
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
/// `score := score + 10` (uses the Plan 23-02 `Expression` API).
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

/// After one `execute()` that fires rule `r1` once, `CoverageTracking` records
/// `fire_count == 1` — the Pitfall 6 regression contract. Serial-gated because
/// `CoverageTracking` is a process-wide singleton; concurrent tests touching it
/// would race and mask a double-count regression.
#[serial_test::serial]
#[test]
fn record_once_per_rule() {
    CoverageTracking.clear();

    let mut ruleset = RuleSet::new();
    ruleset.add_rule(score_plus_10_rule()).unwrap();

    let mut engine = ReteEngine::new();
    engine.load_ruleset(&ruleset).unwrap();
    engine.assert_fact(scorecard_fact(0));

    let result = engine.execute();
    assert!(
        result.rules_fired >= 1,
        "keystone rule should fire at least once, got rules_fired = {}",
        result.rules_fired
    );

    let tracker = CoverageTracking;
    let state = tracker
        .get_rule_state("r1")
        .expect("r1 should be tracked after firing");
    assert!(state.fired, "r1 should be marked fired in CoverageTracking");
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
