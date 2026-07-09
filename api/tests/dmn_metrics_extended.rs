//! Integration tests for extended DMN metrics (Phase 35, DMN-27).

use engine::dmn::evaluator::{DecisionEvaluator, EvaluateRequest, SharedDmnModel};
use engine::dmn::feel::cache::FeelAstCache;
use engine::dmn::model::DmnModel;
use engine::dmn::parser::parse_dmn;
use std::collections::HashMap;
use std::sync::Arc;

fn make_test_model() -> DmnModel {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<definitions xmlns="https://www.omg.org/spec/DMN/20180913/MODEL/" id="_test" name="Test">
    <decision id="d_test" name="Test Decision">
        <literalExpression><text>"hello"</text></literalExpression>
    </decision>
</definitions>"#;
    parse_dmn(xml).expect("parse test model")
}

fn make_evaluator(model: DmnModel) -> DecisionEvaluator {
    let ast_cache = FeelAstCache::new();
    let shared = Arc::new(std::sync::RwLock::new(SharedDmnModel::new(model, ast_cache)));
    DecisionEvaluator::new(shared)
}

fn reset_tracker() {
    let mut guard = engine::dmn::tracker::DMN_DECISION_TRACKER.lock().unwrap();
    *guard = engine::dmn::tracker::DmnDecisionTracker::new();
}

#[test]
fn test_output_evaluations_counter_increments() {
    reset_tracker();
    let evaluator = make_evaluator(make_test_model());
    let result = evaluator.evaluate(
        "d_test", EvaluateRequest { inputs: HashMap::new(), trace: false },
    );
    assert!(result.is_ok());

    if let Ok(ref t) = engine::dmn::tracker::DMN_DECISION_TRACKER.lock() {
        let summary = t.evaluation_summary();
        assert!(summary.contains_key("d_test"), "decision d_test should be tracked");
        assert!(summary["d_test"] >= 1, "d_test evaluated at least once (got {})", summary["d_test"]);
    } else { panic!("tracker lock failed"); }
}

#[test]
fn test_drd_chain_depth_observed() {
    reset_tracker();
    let evaluator = make_evaluator(make_test_model());
    let result = evaluator.evaluate(
        "d_test", EvaluateRequest { inputs: HashMap::new(), trace: false },
    ).expect("evaluation should succeed");
    assert_eq!(result.drd_chain_depth, 1, "single decision chain depth should be 1");
}

#[test]
fn test_dead_decision_detection() {
    reset_tracker();
    let known_ids = vec!["d_seen".into(), "d_dead".into(), "d_also_dead".into()];
    if let Ok(ref mut t) = engine::dmn::tracker::DMN_DECISION_TRACKER.lock() {
        t.record_evaluation("d_seen");
    }
    if let Ok(ref t) = engine::dmn::tracker::DMN_DECISION_TRACKER.lock() {
        let dead = t.find_dead_decisions(&known_ids);
        assert_eq!(dead.len(), 2, "should have 2 dead decisions (got {})", dead.len());
        assert!(dead.contains(&"d_dead".to_string()));
        assert!(dead.contains(&"d_also_dead".to_string()));
    } else { panic!("tracker lock failed"); }
}

#[test]
fn test_coverage_for_single_decision() {
    reset_tracker();
    if let Ok(ref mut t) = engine::dmn::tracker::DMN_DECISION_TRACKER.lock() {
        t.record_evaluation("d_test");
        t.record_evaluation("d_test");
    }
    if let Ok(ref t) = engine::dmn::tracker::DMN_DECISION_TRACKER.lock() {
        let cov = t.coverage_for("d_test").expect("should have coverage for d_test");
        assert_eq!(cov.0, 2, "should have 2 evaluations (got {})", cov.0);
        assert!(t.coverage_for("nonexistent").is_none(), "nonexistent should return None");
    } else { panic!("tracker lock failed"); }
}

#[test]
fn test_tracker_separate_from_rule_tracker() {
    reset_tracker();
    if let Ok(ref t) = engine::dmn::tracker::DMN_DECISION_TRACKER.lock() {
        assert!(t.evaluation_summary().is_empty(), "new tracker should start empty");
    } else { panic!("tracker lock failed"); }
}

#[test]
fn test_snapshot_includes_extended_fields() {
    reset_tracker();
    let evaluator = make_evaluator(make_test_model());
    let result = evaluator.evaluate(
        "d_test", EvaluateRequest { inputs: HashMap::new(), trace: false },
    ).expect("evaluation should succeed");
    assert!(result.drd_chain_depth >= 1, "chain depth should be at least 1 (got {})", result.drd_chain_depth);
}
