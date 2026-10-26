//! Plan 32-02 Task 3: Integration tests for advanced hit policies and COLLECT aggregators.
//!
//! Verifies PRIORITY, OUTPUT ORDER, RULE ORDER, COLLECT with SUM/MIN/MAX/COUNT aggregators,
//! empty-match semantics, and load-time validation errors.
//! Filter: `cargo test -p engine --test hit_policy_advanced`

use engine::data_types::Value;
use engine::dmn::decision_table::{validate_decision_table, DecisionTableEvaluator};
use engine::dmn::feel::eval::Scope;
use engine::dmn::model::{DefinitionsChild, DmnModel};
use engine::dmn::parser::parse_dmn_with_cache;

const DMN_HIT_POLICY_TEST: &str = include_str!("fixtures/dmn/hit_policy_test.dmn");

/// Extract the DecisionTable from a parsed model by decision id.
fn extract_table<'a>(
    model: &'a DmnModel,
    dec_id: &str,
) -> Option<&'a engine::dmn::model::DecisionTable> {
    for child in &model.definitions.children {
        if let DefinitionsChild::Decision(dec) = child {
            if dec.id == dec_id && dec.decision_table.is_some() {
                return dec.decision_table.as_ref();
            }
        }
    }
    None
}

/// Build a scope with `score` bound to the given integer value.
fn score_scope(score: i64) -> Scope {
    let mut s = Scope::empty();
    s.bind(
        "score".into(),
        Value::Number(rust_decimal::Decimal::from(score)),
        vec![],
    );
    s
}

/// Test 1: PRIORITY selects the output that appears **first** in the output's
/// allowed values.
///
/// DMN 1.5 §8.2.12 ranks the enumeration from highest to lowest priority, so the
/// earliest entry wins: with `["Low", "Medium", "High"]`, a match producing "Low"
/// beats one producing "High". This assertion used to expect "High" because the
/// implementation compared positions backwards; OMG DMN TCK `0007-simpletable-P2`
/// settles the direction (`"Approved", "Declined"` → "Approved").
#[test]
fn test_priority_selects_first_allowed() {
    let (model, cache) = parse_dmn_with_cache(DMN_HIT_POLICY_TEST).expect("fixture parses");

    if let Some(dt) = extract_table(&model, "_dec_priority") {
        let scope = score_scope(85);
        let result = DecisionTableEvaluator::evaluate(dt, &scope, &cache).unwrap();
        assert_eq!(result, Value::String("Low".into()));
    } else {
        panic!("expected _dec_priority decision in fixture");
    }
}

/// Test 2: OUTPUT ORDER returns all matches sorted by allowedValues enumeration.
#[test]
fn test_output_order_sorted_by_allowed() {
    let (model, cache) = parse_dmn_with_cache(DMN_HIT_POLICY_TEST).expect("fixture parses");

    if let Some(dt) = extract_table(&model, "_dec_output_order") {
        let scope = score_scope(85);
        let result = DecisionTableEvaluator::evaluate(dt, &scope, &cache).unwrap();
        // Should return array ["Low", "High"] sorted by enumeration order.
        match &result {
            Value::Array(arr) => {
                assert_eq!(arr.len(), 2, "expected 2 elements in output array");
                if let (Value::String(s0), Value::String(s1)) = (&arr[0], &arr[1]) {
                    assert_eq!(*s0, "Low", "first element should be 'Low'");
                    assert_eq!(*s1, "High", "second element should be 'High'");
                } else {
                    panic!("expected string values in array, got {:?}", result);
                }
            }
            other => panic!("expected array, got {:?}", other),
        }
    } else {
        panic!("expected _dec_output_order decision in fixture");
    }
}

/// Test 3: RULE ORDER returns all matches in document order.
#[test]
fn test_rule_order_document_order() {
    let (model, cache) = parse_dmn_with_cache(DMN_HIT_POLICY_TEST).expect("fixture parses");

    if let Some(dt) = extract_table(&model, "_dec_rule_order") {
        let scope = score_scope(85);
        let result = DecisionTableEvaluator::evaluate(dt, &scope, &cache).unwrap();
        // Should return array ["Low", "High"] in document order.
        match &result {
            Value::Array(arr) => {
                assert_eq!(arr.len(), 2, "expected 2 elements in output array");
                if let (Value::String(s0), Value::String(s1)) = (&arr[0], &arr[1]) {
                    assert_eq!(*s0, "Low", "first element should be 'Low'");
                    assert_eq!(*s1, "High", "second element should be 'High'");
                } else {
                    panic!("expected string values in array, got {:?}", result);
                }
            }
            other => panic!("expected array, got {:?}", other),
        }
    } else {
        panic!("expected _dec_rule_order decision in fixture");
    }
}

/// Test 4: COLLECT/SUM computes sum correctly.
#[test]
fn test_collect_sum_aggregator() {
    let (model, cache) = parse_dmn_with_cache(DMN_HIT_POLICY_TEST).expect("fixture parses");

    if let Some(dt) = extract_table(&model, "_dec_collect_sum") {
        let scope = score_scope(50); // All three rules match: >=0, >=20, >=40.
        let result = DecisionTableEvaluator::evaluate(dt, &scope, &cache).unwrap();
        assert_eq!(result, Value::Number(rust_decimal::Decimal::from(60))); // 10 + 20 + 30 = 60.
    } else {
        panic!("expected _dec_collect_sum decision in fixture");
    }
}

/// Test 5: COLLECT/empty SUM returns null.
#[test]
fn test_collect_empty_sum_returns_null() {
    let (model, cache) = parse_dmn_with_cache(DMN_HIT_POLICY_TEST).expect("fixture parses");

    if let Some(dt) = extract_table(&model, "_dec_collect_empty") {
        let scope = score_scope(0); // No rule matches (all thresholds are >=100 in empty fixture).
        let result = DecisionTableEvaluator::evaluate(dt, &scope, &cache).unwrap();
        assert_eq!(result, Value::Null);
    } else {
        panic!("expected _dec_collect_empty decision in fixture");
    }
}

/// Test 6: COLLECT/COUNT returns count as number.
#[test]
fn test_collect_count_aggregator() {
    let (model, cache) = parse_dmn_with_cache(DMN_HIT_POLICY_TEST).expect("fixture parses");

    if let Some(dt) = extract_table(&model, "_dec_collect_count") {
        let scope = score_scope(50); // All three rules match.
        let result = DecisionTableEvaluator::evaluate(dt, &scope, &cache).unwrap();
        assert_eq!(result, Value::Number(rust_decimal::Decimal::from(3)));
    } else {
        panic!("expected _dec_collect_count decision in fixture");
    }
}

/// Test 7: COLLECT/COUNT empty returns 0 (not null).
#[test]
fn test_collect_count_empty_returns_zero() {
    let (model, cache) = parse_dmn_with_cache(DMN_HIT_POLICY_TEST).expect("fixture parses");

    if let Some(dt) = extract_table(&model, "_dec_collect_count_empty") {
        let scope = score_scope(0); // No rule matches.
        let result = DecisionTableEvaluator::evaluate(dt, &scope, &cache).unwrap();
        assert_eq!(result, Value::Number(rust_decimal::Decimal::ZERO));
    } else {
        panic!("expected _dec_collect_count_empty decision in fixture");
    }
}

/// Test 8: Multi-output COLLECT rejected at load time.
#[test]
fn test_collect_multi_output_rejected_at_load() {
    let (model, _cache) = parse_dmn_with_cache(DMN_HIT_POLICY_TEST).expect("fixture parses");

    if let Some(dt) = extract_table(&model, "_dec_collect_multi") {
        // Validation should reject this table.
        let result = validate_decision_table(dt);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("COLLECT with aggregation requires exactly one output column"));
    } else {
        panic!("expected _dec_collect_multi decision in fixture");
    }
}

/// Test 9: PRIORITY without allowedValues rejected at load time.
#[test]
fn test_priority_no_allowed_values_rejected_at_load() {
    let (model, _cache) = parse_dmn_with_cache(DMN_HIT_POLICY_TEST).expect("fixture parses");

    if let Some(dt) = extract_table(&model, "_dec_priority_no_allowed") {
        // Validation should reject this table.
        let result = validate_decision_table(dt);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("PRIORITY hit policy requires allowedValues"));
    } else {
        panic!("expected _dec_priority_no_allowed decision in fixture");
    }
}
