//! Plan 23-06 Task 2: per-engine keystone integration tests for the three
//! non-Rete engines (Treat, Leaps, Phreak). Each test proves the engine
//! actually fires the `score := score + 10` rule (D-02) — `rules_fired >= 1`,
//! `get_facts()` shows `score == 10`, `fired_rule_ids` contains `r1`, and
//! `CoverageTracking` records `fire_count == 1` (once-per-rule, Pitfall 6).
//!
//! Together with `engine/tests/rete_execution.rs` (Plan 23-03), this gives
//! the phase four keystone tests — one per engine — so the four-engine
//! guarantee in `available_engines()` is honest, not merely compile-clean.
//!
//! Filter: `cargo test -p engine --test engine_execution_tests \
//!          --features engine-rete,engine-treat,engine-leaps,engine-phreak`

use engine::data_types::Value;
use engine::engine::leaps::LeapsEngine;
use engine::engine::phreak::PhreakEngine;
use engine::engine::registry::ExecutionEngine;
use engine::engine::treat::TreatEngine;
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
/// `score := score + 10`. Identical to the fixture in
/// `engine/tests/rete_execution.rs` so all four engines are exercised by
/// the same scenario (D-02).
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

/// Build a one-rule `RuleSet` containing the keystone rule.
fn score_plus_10_ruleset() -> RuleSet {
    let mut ruleset = RuleSet::new();
    ruleset.add_rule(score_plus_10_rule()).unwrap();
    ruleset
}

/// Every test in this file calls `execute()`, which records into the
/// process-global `CoverageTracking` singleton. To keep the once-per-rule
/// coverage assertions truthful (Pitfall 6), all tests are marked
/// `#[serial_test::serial]` so no two run concurrently, and each clears the
/// singleton at start. (Same isolation contract as `rete_execution.rs`.)
fn reset_coverage() {
    CoverageTracking.clear();
}

/// Assert the keystone scenario holds for a freshly-constructed engine of
/// any type: load the `score := score + 10` ruleset, assert a scorecard
/// fact with `score == 0`, execute, then verify `rules_fired >= 1`,
/// `fired_rule_ids` contains `r1`, `get_facts()` shows `score == 10`, and
/// `CoverageTracking` recorded `fire_count == 1` for `r1` (Pitfall 6).
fn assert_keystone_fires<E: ExecutionEngine + 'static>(mut engine: E, engine_label: &str) {
    let ruleset = score_plus_10_ruleset();
    engine
        .load_ruleset(&ruleset)
        .unwrap_or_else(|e| panic!("{} load_ruleset should succeed, got: {}", engine_label, e));

    // Insert the scorecard fact. All three engines expose `assert_fact` as
    // an inherent method; the trait's `insert_fact` delegates to it.
    engine
        .insert_fact(scorecard_fact(0))
        .unwrap_or_else(|e| panic!("{} insert_fact should succeed, got: {}", engine_label, e));

    let result = engine.execute();
    assert!(result.success, "{}: execute should succeed", engine_label);
    assert!(
        result.rules_fired >= 1,
        "{}: keystone rule should fire, got rules_fired = {}",
        engine_label,
        result.rules_fired
    );
    assert!(
        result.fired_rule_ids.contains(&"r1".to_string()),
        "{}: fired_rule_ids should contain r1, got {:?}",
        engine_label,
        result.fired_rule_ids
    );

    let facts = engine.get_facts();
    let scorecard = facts
        .iter()
        .find(|f| f.fact_type_id == "scorecard")
        .unwrap_or_else(|| {
            panic!(
                "{}: scorecard fact should exist after execute",
                engine_label
            )
        });
    assert_eq!(
        scorecard.payload.get("score"),
        Some(&Value::Integer(10)),
        "{}: score should be incremented to 10, got {:?}",
        engine_label,
        scorecard.payload.get("score")
    );

    // Pitfall 6 regression: coverage records once per fired rule.
    let tracker = CoverageTracking;
    let state = tracker
        .get_rule_state("r1")
        .unwrap_or_else(|| panic!("{}: r1 should be tracked after firing", engine_label));
    assert!(
        state.fired,
        "{}: r1 should be marked fired in CoverageTracking",
        engine_label
    );
    assert_eq!(
        state.fire_count, 1,
        "{}: Pitfall 6 regression — fire_count should be 1 (once-per-rule), got {}",
        engine_label, state.fire_count
    );
    assert!(
        state.evaluated,
        "{}: r1 should be marked evaluated in CoverageTracking",
        engine_label
    );
}

// =====================================================================
// TreatEngine keystone (D-02)
// =====================================================================

#[serial_test::serial]
#[test]
fn treat_fires_score_plus_ten() {
    reset_coverage();
    let engine = TreatEngine::new();
    assert_keystone_fires(engine, "treat");
}

// =====================================================================
// LeapsEngine keystone (D-02)
// =====================================================================

#[serial_test::serial]
#[test]
fn leaps_fires_score_plus_ten() {
    reset_coverage();
    let engine = LeapsEngine::new();
    assert_keystone_fires(engine, "leaps");
}

// =====================================================================
// PhreakEngine keystone (D-02)
// =====================================================================

#[serial_test::serial]
#[test]
fn phreak_fires_score_plus_ten() {
    reset_coverage();
    let engine = PhreakEngine::new();
    assert_keystone_fires(engine, "phreak");
}
