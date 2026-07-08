//! Integration test: TCK FEEL test harness scaffold (DMN-18, Phase 31-09).
//!
//! # SCAFFOLD — full OMG DMN TCK passage deferred to Phase 35.
//!
//! This file loads a small vendored subset of FEEL expression test vectors from
//! `engine/tests/fixtures/tck/feel/` and exercises them against the real parser
//! + evaluator (`parse_expression` → `evaluate`). The harness is structured so
//! Phase 35 only needs to expand the vendored vector set (add more JSON files or
//! cases to existing ones).
//!
//! ## Provenance
//!
//! The test vectors under `engine/tests/fixtures/tck/feel/` are hand-curated
//! from the OMG DMN TCK FEEL grammar suite. They mirror the structure of the
//! Phase 30 numeric TCK (`tck/0100-arithmetic/`) but cover FEEL expression
//! grammar: arithmetic on decimals, three-valued logic operators, comparison,
//! boxed expressions (list/context), string/null literals, and parenthesized
//! expressions.
//!
//! ## Scope covered by this scaffold
//!
//! - `parse_expression` + `evaluate(&FeelExpression, &Scope)` pipeline
//! - Numeric equality via `feel_numeric_eq` (rust_decimal::Decimal)
//! - Boolean comparison for three-valued-logic operators
//! - Structural comparison for list/context/null/string values
//!
//! ## Out-of-scope vectors (deferred to Phase 35)
//!
//! - Temporal expressions (`@"2020-01-01"`)
//! - Duration arithmetic
//! - Function invocation / built-in functions beyond the numeric subset
//! - Quantified expressions (`some`, `every`)
//! - For-comprehensions
//! - Instance-of checks
//! - Path navigation / filter expressions
//! - Negative test cases (expressions expected to return Err or null)

use std::fs;
use std::path::Path;
use std::str::FromStr;

use rust_decimal::Decimal;
use engine::data_types::Value;
use engine::dmn::feel::eval::{evaluate, Scope};
use engine::dmn::feel::numeric::feel_numeric_eq;
use engine::dmn::feel::parser::parse_expression;

/// Root directory of the vendored TCK subset (relative to `engine/tests/`).
const TCK_ROOT: &str = "tests/fixtures/tck/feel";

// =====================================================================
// Data model for test vectors
// =====================================================================

/// A parsed TCK FEEL test case.
#[derive(Debug, Clone)]
struct TckFeelCase {
    id: String,
    category: String,
    expression: String,
    expected_type: ExpectedType,
}

#[derive(Debug, Clone)]
enum ExpectedType {
    Number(String),       // decimal string representation
    Boolean(bool),
    String(String),
    Null,
    Array(usize),         // expected length
    Object(Vec<String>),  // expected keys (order not guaranteed)
}

// =====================================================================
// Loader: reads JSON test-vector files from the vendored subset
// =====================================================================

/// Load all TCK FEEL test cases from `TCK_ROOT`.
///
/// Reads every `.json` file in the directory, parses each as a JSON object with
/// a `"cases"` array. Returns all cases across all files concatenated.
pub fn load_tck_subset() -> Vec<TckFeelCase> {
    let root = Path::new(TCK_ROOT);
    if !root.is_dir() {
        eprintln!("TCK feel root not found at {TCK_ROOT} — returning empty subset");
        return Vec::new();
    }

    let mut cases = Vec::new();

    for entry in fs::read_dir(root).expect("read TCK feel dir").flatten() {
        let path = entry.path();
        if path.extension().map_or(false, |ext| ext == "json") {
            cases.extend(load_tck_from_file(&path));
        }
    }

    eprintln!("TCK FEEL scaffold: loaded {} test cases from {}", cases.len(), TCK_ROOT);
    cases
}

fn load_tck_from_file(path: &Path) -> Vec<TckFeelCase> {
    let text = fs::read_to_string(path).unwrap_or_else(|e| {
        panic!("failed to read TCK file {}: {e}", path.display())
    });
    let value: serde_json::Value = serde_json::from_str(&text).unwrap_or_else(|e| {
        panic!("JSON parse error in {}: {e}", path.display())
    });

    let cases_array = value.get("cases").expect("expected 'cases' array");
    let mut out = Vec::new();

    if let Some(cases_arr) = cases_array.as_array() {
        for case_val in cases_arr {
            let id = case_val["id"].as_str().unwrap_or("").to_string();
            let category = case_val["category"].as_str().unwrap_or("").to_string();
            let expression = case_val["expression"].as_str().unwrap_or("").to_string();

            let expected_type: ExpectedType = if let Some(v) = case_val.get("expected_value") {
                match case_val["expected_type"].as_str() {
                    Some("number") => {
                        ExpectedType::Number(v.as_str().unwrap_or("").to_string())
                    }
                    Some("boolean") => {
                        ExpectedType::Boolean(v.as_bool().unwrap_or(false))
                    }
                    Some("string") => {
                        ExpectedType::String(v.as_str().unwrap_or("").to_string())
                    }
                    Some("null") => ExpectedType::Null,
                    _ => continue,
                }
            } else if let Some(len) = case_val.get("expected_length") {
                ExpectedType::Array(len.as_u64().unwrap_or(0) as usize)
            } else if let Some(keys) = case_val.get("keys") {
                let k: Vec<String> = keys
                    .as_array()
                    .expect("keys must be array")
                    .iter()
                    .filter_map(|v: &serde_json::Value| v.as_str().map(String::from))
                    .collect();
                ExpectedType::Object(k)
            } else {
                continue; // skip cases with no expected value specification
            };

            out.push(TckFeelCase {
                id,
                category,
                expression,
                expected_type,
            });
        }
    }

    out
}

// =====================================================================
// Runner: evaluate each case and assert correctness
// =====================================================================

/// Run all vendored TCK FEEL cases. Asserts every in-scope case passes.
#[test]
fn feel_tck_subset_runs_green() {
    let cases = load_tck_subset();

    // Must have at least 5 cases to be a meaningful scaffold.
    assert!(cases.len() >= 5, "TCK subset must have at least 5 test cases, got {}", cases.len());

    let mut passed = 0usize;
    let mut failed: Vec<String> = Vec::new();

    for case in &cases {
        match run_one_case(case) {
            Ok(()) => {
                passed += 1;
            }
            Err(msg) => {
                failed.push(format!("{} ({:?}): {}", case.id, case.category, msg));
            }
        }
    }

    assert!(
        failed.is_empty(),
        "{} TCK FEEL subset case(s) failed:\n{}",
        failed.len(),
        failed.join("\n")
    );

    eprintln!("TCK FEEL scaffold: {passed} cases passed, 0 failed");
}

/// Run a single TCK FEEL case. Returns `Ok(())` if the evaluated result matches
/// the expected value; `Err(msg)` otherwise.
fn run_one_case(case: &TckFeelCase) -> Result<(), String> {
    // Parse the expression. The parser never panics (error recovery).
    let ast = parse_expression(&case.expression);

    // Evaluate against an empty scope.
    let result = evaluate(&ast, &Scope::empty());

    match &case.expected_type {
        ExpectedType::Number(expected_str) => {
            match &result {
                Value::Number(actual) => {
                    let expected_dec = Decimal::from_str(expected_str)
                        .map_err(|e| format!("expected value parse error: {e}"))?;
                    if feel_numeric_eq(&Value::Number(actual.clone()), &Value::Number(expected_dec))
                        == Some(true)
                    {
                        Ok(())
                    } else {
                        Err(format!(
                            "number mismatch: expr {:?} = {} but expected {}",
                            case.expression, actual, expected_str
                        ))
                    }
                }
                _ => Err(format!(
                    "expected Number, got {:?} for expression {:?}",
                    result, case.expression
                )),
            }
        }
        ExpectedType::Boolean(expected) => {
            match &result {
                Value::Bool(actual) if actual == expected => Ok(()),
                _ => Err(format!(
                    "expected boolean {}, got {:?} for expression {:?}",
                    expected, result, case.expression
                )),
            }
        }
        ExpectedType::String(expected) => {
            match &result {
                Value::String(actual) if actual == expected => Ok(()),
                _ => Err(format!(
                    "expected string {:?}, got {:?} for expression {:?}",
                    expected, result, case.expression
                )),
            }
        }
        ExpectedType::Null => {
            match &result {
                Value::Null => Ok(()),
                _ => Err(format!(
                    "expected null, got {:?} for expression {:?}",
                    result, case.expression
                )),
            }
        }
        ExpectedType::Array(expected_len) => {
            match &result {
                Value::Array(arr) if arr.len() == *expected_len => Ok(()),
                _ => Err(format!(
                    "expected array of length {}, got {:?} for expression {:?}",
                    expected_len, result, case.expression
                )),
            }
        }
        ExpectedType::Object(expected_keys) => {
            match &result {
                Value::Object(map) => {
                    let actual_keys: Vec<String> = map.keys().cloned().collect();
                    if actual_keys.len() == expected_keys.len()
                        && expected_keys.iter().all(|k| map.contains_key(k.as_str()))
                    {
                        Ok(())
                    } else {
                        Err(format!(
                            "expected object with keys {:?}, got {:?}",
                            expected_keys, actual_keys
                        ))
                    }
                }
                _ => Err(format!(
                    "expected object, got {:?} for expression {:?}",
                    result, case.expression
                )),
            }
        }
    }
}

// =====================================================================
// SCAFFOLD marker — full TCK passage deferred to Phase 35
// =====================================================================

/// The vendored subset is intentionally small (~15 cases). Full OMG DMN TCK
/// FEEL passage (hundreds of cases across grammar, temporal, arithmetic,
/// built-in functions) is deferred to Phase 35. This test function exists to
/// document that boundary — it always passes trivially as a marker.
#[test]
fn tck_full_passage_deferred_to_phase_35() {
    // SCAFFOLD: full TCK passage deferred to Phase 35 (CONTEXT.md).
    eprintln!("TCK FEEL scaffold: full OMG DMN TCK passage deferred to Phase 35");
}
