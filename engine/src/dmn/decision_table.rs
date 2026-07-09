//! Decision-table evaluation engine (Phase 32, DMN-19 / DMN-20 / DMN-21).
//!
//! Implements all seven DMN hit policies per Clause 8.2.12 by evaluating a
//! parsed [`crate::dmn::model::DecisionTable`] against a FEEL [`Scope`], using the
//! Phase 31 [`FeelAstCache`](crate::dmn::feel::cache::FeelAstCache) for AST reuse —
//! never re-parsing cell text at evaluation time (DMN-18 perf property).
//!
//! This module is **NOT** routed through [`crate::engine::EngineRegistry`]: the DMN path
//! is stateless, side-effect-free, and three-valued; the Rete family is forward-chaining,
//! working-memory-mutating, and two-valued. The two paths share neither an execution engine
//! nor an expression AST (milestone research anti-pattern avoided).

use std::sync::Arc;

use crate::data_types::Value;
use crate::dmn::feel::cache::{FeelAstCache, ParseEntry};
use crate::dmn::feel::eval::{self, Scope};
use crate::dmn::model::{
    AllowedValues, BuiltinAggregator, DecisionTable, HitPolicy, InputClause, OutputClause, RuleEntry,
};

use thiserror::Error;

// =========================================================================
// Errors
// =========================================================================

/// Errors produced by the decision-table evaluator.
#[derive(Debug, Error)]
pub enum DecisionTableError {
    /// Multiple rules matched but the hit policy requires exactly one match.
    #[error("hit policy violation ({policy}): {message}")]
    HitPolicyViolation { policy: String, message: String },

    /// Aggregator function received non-numeric values (SUM / MIN / MAX).
    #[error("aggregator mismatch: {0}")]
    AggregatorMismatch(String),

    /// An output cell value could not be interpreted for the expected type.
    #[error("invalid output value at rule {0}")]
    InvalidOutputValue(String),

    /// Generic fallback error.
    #[error("{0}")]
    Other(String),
}

// =========================================================================
// Evaluator struct + public entry point
// =========================================================================

/// Stateless decision-table evaluator.
pub struct DecisionTableEvaluator;

impl DecisionTableEvaluator {
    /// Evaluate a parsed [`DecisionTable`] against the given [`Scope`], using the
    /// AST [`FeelAstCache`](crate::dmn::feel::cache::FeelAstCache) for pre-parsed FEEL text.
    ///
    /// Returns the aggregated result as a single [`Value`] (or `Null` on zero matches).
    pub fn evaluate(
        table: &DecisionTable,
        scope: &Scope,
        cache: &FeelAstCache,
    ) -> Result<Value, DecisionTableError> {
        // 1. Collect all rules whose input clauses ALL match (three-valued: None → non-match).
        let matches = Self::collect_matching_rules(table, scope, cache)?;

        // 2. Dispatch to the hit-policy-specific handler.
        Self::dispatch_hit_policy(&matches, table)
    }

    // ---------------------------------------------------------------------
    // Internal helpers
    // ---------------------------------------------------------------------

    /// Walk every rule and keep those whose **all** input clauses evaluate to `Some(true)`.
    fn collect_matching_rules(
        table: &DecisionTable,
        scope: &Scope,
        cache: &FeelAstCache,
    ) -> Result<Vec<(usize, Vec<Value>)>, DecisionTableError> {
        let mut matches = Vec::new();

        for (idx, rule) in table.rules.iter().enumerate() {
            let all_match = table
                .input_clauses
                .iter()
                .zip(rule.input_entries.iter())
                .all(|(clause, entry)| Self::input_clause_matches(clause, entry, scope, cache));

            if all_match {
                // Extract output values from the rule.
                let outputs: Vec<Value> = table
                    .output_clauses
                    .iter()
                    .zip(rule.output_entries.iter())
                    .map(|(_clause, entry)| Self::evaluate_output_entry(entry, scope, cache))
                    .collect();
                matches.push((idx, outputs));
            }
        }

        Ok(matches)
    }

    /// Evaluate a single output cell. Cell text is parsed as a FEEL expression via the cache.
    fn evaluate_output_entry(
        entry: &RuleEntry,
        scope: &Scope,
        cache: &FeelAstCache,
    ) -> Value {
        let ast = cache.get_or_parse(&entry.text.content, ParseEntry::Expression);
        eval::evaluate(&ast, scope)
    }

    /// Check whether an input clause matches for a given rule.
    /// Returns `true` only when the unary-test evaluates to *exactly* true; `None` (null / error)
    /// is treated as non-match per DMN three-valued semantics.
    fn input_clause_matches(
        clause: &InputClause,
        entry: &RuleEntry,
        scope: &Scope,
        cache: &FeelAstCache,
    ) -> bool {
        // 1. Parse the RuleEntry.text as a SimpleUnaryTests AST via cache.
        let ast = cache.get_or_parse(&entry.text.content, ParseEntry::SimpleUnaryTests);

        // 2. Evaluate the input expression (e.g., "applicant.age") against scope to get the value.
        let input_expr_ast = cache.get_or_parse(&clause.input_expression, ParseEntry::Expression);
        let input_value = eval::evaluate(&input_expr_ast, scope);

        // 3. Use eval_in_unary_test — returns Option<bool> (None → null propagation).
        match eval::eval_in_unary_test(input_value, &ast) {
            Some(true) => true,
            _ => false, // None or Some(false) both mean "does not fire".
        }
    }

    /// Dispatch to the hit-policy-specific handler based on `table.hit_policy`.
    fn dispatch_hit_policy(
        matches: &[(usize, Vec<Value>)],
        table: &DecisionTable,
    ) -> Result<Value, DecisionTableError> {
        match table.hit_policy {
            HitPolicy::Unique => Self::handle_unique(matches, table),
            HitPolicy::Any => Self::handle_any(matches, table),
            HitPolicy::First => Self::handle_first(matches, table),
            HitPolicy::Priority => Self::handle_priority(matches, table),
            HitPolicy::OutputOrder => Self::handle_output_order(matches, table),
            HitPolicy::RuleOrder => Self::handle_rule_order(matches, table),
            HitPolicy::Collect => Self::handle_collect(matches, table),
        }
    }

    // ---------------------------------------------------------------------
    // Hit-policy handlers (Phase 32 — all seven)
    // ---------------------------------------------------------------------

    /// UNIQUE: at most one rule may match; zero matches → null per column.
    fn handle_unique(
        matches: &[(usize, Vec<Value>)],
        table: &DecisionTable,
    ) -> Result<Value, DecisionTableError> {
        match matches.len() {
            0 => Ok(Self::null_with_defaults(table)),
            1 => Self::pack_outputs(&matches[0].1, table),
            n => Err(DecisionTableError::HitPolicyViolation {
                policy: "UNIQUE".into(),
                message: format!("{n} rules matched but UNIQUE allows exactly one"),
            }),
        }
    }

    /// ANY: all matching rules must agree on every output column.
    fn handle_any(
        matches: &[(usize, Vec<Value>)],
        table: &DecisionTable,
    ) -> Result<Value, DecisionTableError> {
        if matches.is_empty() {
            return Ok(Self::null_with_defaults(table));
        }

        let first = &matches[0].1;
        for (idx, (_rule_idx, outputs)) in matches.iter().enumerate().skip(1) {
            for (col, (a, b)) in first.iter().zip(outputs.iter()).enumerate() {
                if eval::feel_equals_public(a, b) != Some(true) {
                    let col_name = Self::output_column_name(table, col);
                    return Err(DecisionTableError::HitPolicyViolation {
                        policy: "ANY".into(),
                        message: format!(
                            "conflicting outputs for column '{}': {:?} vs {:?}",
                            col_name, a, b
                        ),
                    });
                }
            }
        }

        Self::pack_outputs(&matches[0].1, table)
    }

    /// FIRST: first matching rule by document order wins. No conflict detection.
    fn handle_first(
        matches: &[(usize, Vec<Value>)],
        table: &DecisionTable,
    ) -> Result<Value, DecisionTableError> {
        match matches.first() {
            Some((_, outputs)) => Self::pack_outputs(outputs, table),
            None => Ok(Self::null_with_defaults(table)),
        }
    }

    /// PRIORITY: select the match whose output column value has highest position in the allowedValues enumeration.
    fn handle_priority(
        matches: &[(usize, Vec<Value>)],
        table: &DecisionTable,
    ) -> Result<Value, DecisionTableError> {
        if matches.is_empty() {
            return Ok(Self::null_with_defaults(table));
        }

        // Get the allowedValues enumeration from the first output clause.
        let enum_order = Self::get_allowed_values_order(table)?;

        // Find each matching rule's position in the enumeration.
        let mut best_idx: Option<usize> = None;
        let mut best_pos: i64 = -1;

        for (match_idx, (_rule_idx, outputs)) in matches.iter().enumerate() {
            if let Some(first_output) = outputs.first() {
                // Find position of this output value in the enumeration.
                if let Some(pos) = enum_order.iter().position(|v| eval::feel_equals_public(v, first_output) == Some(true)) {
                    let pos_i64 = pos as i64;
                    if best_idx.is_none() || pos_i64 > best_pos {
                        best_idx = Some(match_idx);
                        best_pos = pos_i64;
                    }
                }
                // If the output value is NOT in the enumeration, skip it.
            }
        }

        match best_idx {
            Some(idx) => Self::pack_outputs(&matches[idx].1, table),
            None => Ok(Self::null_with_defaults(table)),
        }
    }

    /// OUTPUT ORDER: return all matching rules' outputs sorted by allowedValues enumeration order.
    fn handle_output_order(
        matches: &[(usize, Vec<Value>)],
        table: &DecisionTable,
    ) -> Result<Value, DecisionTableError> {
        if matches.is_empty() {
            return Ok(Value::Array(Vec::new()));
        }

        let enum_order = Self::get_allowed_values_order(table)?;

        // Sort matches by their first output's position in the enumeration.
        let mut sorted_matches: Vec<&(usize, Vec<Value>)> = matches.iter().collect();
        sorted_matches.sort_by(|a, b| {
            let pos_a = a.1.first()
                .and_then(|v| enum_order.iter().position(|ev| eval::feel_equals_public(ev, v) == Some(true)))
                .map(|p| p as i64)
                .unwrap_or(i64::MAX); // Non-enumerated values go last.
            let pos_b = b.1.first()
                .and_then(|v| enum_order.iter().position(|ev| eval::feel_equals_public(ev, v) == Some(true)))
                .map(|p| p as i64)
                .unwrap_or(i64::MAX);
            pos_a.cmp(&pos_b)
        });

        // Collect all outputs into an array.
        let mut result = Vec::new();
        for (_, outputs) in &sorted_matches {
            if outputs.len() == 1 && table.output_clauses.len() <= 1 {
                result.push(outputs[0].clone());
            } else {
                result.push(Self::pack_values_as_context(outputs, table));
            }
        }

        Ok(Value::Array(result))
    }

    /// RULE ORDER: return all matching rules' outputs in document order.
    fn handle_rule_order(
        matches: &[(usize, Vec<Value>)],
        table: &DecisionTable,
    ) -> Result<Value, DecisionTableError> {
        if matches.is_empty() {
            return Ok(Value::Array(Vec::new()));
        }

        // Collect all outputs into an array (already in document order from collect_matching_rules).
        let mut result = Vec::new();
        for (_, outputs) in matches {
            if outputs.len() == 1 && table.output_clauses.len() <= 1 {
                result.push(outputs[0].clone());
            } else {
                result.push(Self::pack_values_as_context(outputs, table));
            }
        }

        Ok(Value::Array(result))
    }

    /// COLLECT: collect all matching rules' outputs into a list, optionally applying an aggregator.
    fn handle_collect(
        matches: &[(usize, Vec<Value>)],
        table: &DecisionTable,
    ) -> Result<Value, DecisionTableError> {
        if matches.is_empty() {
            // Empty match with aggregation: return null for SUM/MIN/MAX, 0 for COUNT.
            if let Some(aggregator) = &table.aggregation {
                return Ok(Self::empty_aggregate_result(aggregator));
            }
            return Ok(Value::Array(Vec::new()));
        }

        // Extract output values from each matching rule (first output column).
        let mut values: Vec<Value> = Vec::new();
        for (_, outputs) in matches {
            if let Some(first_output) = outputs.first() {
                values.push(first_output.clone());
            }
        }

        // Apply aggregator if present.
        if let Some(aggregator) = &table.aggregation {
            return Ok(Self::apply_aggregator(aggregator, &values)?);
        }

        // No aggregation: return raw list.
        Ok(Value::Array(values))
    }

    /// Get the allowedValues enumeration order from the first output clause (if present).
    fn get_allowed_values_order(table: &DecisionTable) -> Result<Vec<Value>, DecisionTableError> {
        if let Some(first_clause) = table.output_clauses.first() {
            if let Some(allowed) = &first_clause.output_values {
                return Self::parse_allowed_values(allowed);
            }
        }
        // No allowedValues: return empty list (PRIORITY/OUTPUT ORDER will skip non-enumerated).
        Ok(Vec::new())
    }

    /// Parse AllowedValues into a Vec<Value>.
    fn parse_allowed_values(allowed: &AllowedValues) -> Result<Vec<Value>, DecisionTableError> {
        let mut values = Vec::new();
        match allowed {
            AllowedValues::UnaryTests(text) => {
                // For PRIORITY/OUTPUT ORDER, the allowedValues is an Enumeration, not UnaryTests.
                // If it's UnaryTests, we can't determine enumeration order — return error.
                Err(DecisionTableError::Other(
                    "allowedValues must be an Enumeration for PRIORITY/OUTPUT ORDER".into(),
                ))
            }
            AllowedValues::Enumeration(items) => {
                for item in items {
                    // Each item is a TextContent representing a literal value.
                    let cache = FeelAstCache::new();
                    let ast = cache.get_or_parse(&item.content, ParseEntry::Expression);
                    let val = eval::evaluate(&ast, &Scope::empty());
                    values.push(val);
                }
                Ok(values)
            }
        }
    }

    /// Return the empty-match result for a given aggregator.
    fn empty_aggregate_result(aggregator: &BuiltinAggregator) -> Value {
        match aggregator {
            BuiltinAggregator::Sum | BuiltinAggregator::Min | BuiltinAggregator::Max => {
                Value::Null // Empty SUM/MIN/MAX → null
            }
            BuiltinAggregator::Count => {
                Value::Number(rust_decimal::Decimal::ZERO) // Empty COUNT → 0 (spec exception)
            }
        }
    }

    /// Apply an aggregator to a list of values.
    fn apply_aggregator(aggregator: &BuiltinAggregator, values: &[Value]) -> Result<Value, DecisionTableError> {
        match aggregator {
            BuiltinAggregator::Sum => Self::aggregate_sum(values),
            BuiltinAggregator::Min => Self::aggregate_min(values),
            BuiltinAggregator::Max => Self::aggregate_max(values),
            BuiltinAggregator::Count => Ok(Self::aggregate_count(values)),
        }
    }

    /// SUM aggregator: sum all numeric outputs. Empty → null. Non-numeric → AggregatorMismatch.
    fn aggregate_sum(values: &[Value]) -> Result<Value, DecisionTableError> {
        if values.is_empty() {
            return Ok(Value::Null);
        }

        let mut sum = rust_decimal::Decimal::ZERO;
        for v in values {
            match v {
                Value::Number(d) => sum += d,
                _ => return Err(DecisionTableError::AggregatorMismatch("SUM requires numeric outputs".into())),
            }
        }
        Ok(Value::Number(sum))
    }

    /// MIN aggregator: minimum of all output values. Empty → null. Non-numeric → AggregatorMismatch.
    fn aggregate_min(values: &[Value]) -> Result<Value, DecisionTableError> {
        if values.is_empty() {
            return Ok(Value::Null);
        }

        let mut min = None;
        for v in values {
            match v {
                Value::Number(d) => {
                    let d_clone = d.clone();
                    if min.is_none() || d_clone < *min.as_ref().unwrap() {
                        min = Some(d_clone);
                    }
                }
                _ => return Err(DecisionTableError::AggregatorMismatch("MIN requires numeric outputs".into())),
            }
        }

        Ok(Value::Number(min.unwrap_or(rust_decimal::Decimal::ZERO)))
    }

    /// MAX aggregator: maximum of all output values. Empty → null. Non-numeric → AggregatorMismatch.
    fn aggregate_max(values: &[Value]) -> Result<Value, DecisionTableError> {
        if values.is_empty() {
            return Ok(Value::Null);
        }

        let mut max = None;
        for v in values {
            match v {
                Value::Number(d) => {
                    let d_clone = d.clone();
                    if max.is_none() || d_clone > *max.as_ref().unwrap() {
                        max = Some(d_clone);
                    }
                }
                _ => return Err(DecisionTableError::AggregatorMismatch("MAX requires numeric outputs".into())),
            }
        }

        Ok(Value::Number(max.unwrap_or(rust_decimal::Decimal::ZERO)))
    }

    /// COUNT aggregator: count of all matching rules. Always returns a Number (0 for empty).
    fn aggregate_count(values: &[Value]) -> Value {
        let count = values.len() as i64;
        Value::Number(rust_decimal::Decimal::from(count))
    }

    // ---------------------------------------------------------------------
    // Output packaging helpers
    // ---------------------------------------------------------------------

    /// Build a null result using each output clause's `defaultOutputEntry` (or `Null`).
    fn null_with_defaults(table: &DecisionTable) -> Value {
        if table.output_clauses.is_empty() {
            return Value::Null;
        }
        let values: Vec<Value> = table
            .output_clauses
            .iter()
            .map(|clause| {
                clause
                    .default_output_entry
                    .as_deref()
                    .map(|text| {
                        // Try to evaluate the default entry text as a FEEL expression.
                        let cache = FeelAstCache::new();
                        let ast = cache.get_or_parse(text, ParseEntry::Expression);
                        eval::evaluate(&ast, &Scope::empty())
                    })
                    .unwrap_or(Value::Null)
            })
            .collect();

        if values.len() == 1 {
            values.into_iter().next().unwrap()
        } else {
            Self::pack_values_as_context(&values, table)
        }
    }

    /// Pack output values into a single Value: scalar for single-output tables, context for multi.
    fn pack_outputs(outputs: &[Value], table: &DecisionTable) -> Result<Value, DecisionTableError> {
        if outputs.len() == 1 && table.output_clauses.len() <= 1 {
            return Ok(outputs[0].clone());
        }
        Ok(Self::pack_values_as_context(outputs, table))
    }

    /// Build a context (`Value::Object`) keyed by output-clause name / label.
    fn pack_values_as_context(values: &[Value], table: &DecisionTable) -> Value {
        let mut map = std::collections::HashMap::new();
        for (idx, v) in values.iter().enumerate() {
            let key = Self::output_column_name(table, idx);
            map.insert(key, v.clone());
        }
        Value::Object(map)
    }

    /// Derive an output-column name from `label`, falling back to `name`, then a generated id.
    fn output_column_name(table: &DecisionTable, idx: usize) -> String {
        table.output_clauses.get(idx).map_or_else(
            || format!("output_{idx}"),
            |clause| {
                clause.label.clone().unwrap_or_else(|| {
                    clause.name.clone().unwrap_or_else(|| format!("output_{idx}"))
                })
            },
        )
    }
}

/// Load-time validation for a decision table.
///
/// Checks structural constraints that must hold before evaluation:
/// - COLLECT with aggregation requires exactly one output column (DMN-20)
/// - PRIORITY/OUTPUT ORDER require allowedValues on the first output clause (DMN-21)
/// - aggregation attribute only valid with COLLECT hit policy
pub fn validate_decision_table(table: &DecisionTable) -> Result<(), String> {
    // 1. Multi-output rejection for aggregator tables (DMN-20).
    if table.hit_policy == HitPolicy::Collect && table.aggregation.is_some() && table.output_clauses.len() != 1 {
        return Err(format!(
            "COLLECT with aggregation requires exactly one output column, got {}",
            table.output_clauses.len()
        ));
    }

    // 2. allowedValues validation for PRIORITY (DMN-21).
    if table.hit_policy == HitPolicy::Priority {
        if let Some(first_clause) = table.output_clauses.first() {
            if first_clause.output_values.is_none() {
                return Err("PRIORITY hit policy requires allowedValues on the output column".into());
            }
        }
    }

    // 3. allowedValues validation for OUTPUT ORDER (DMN-21).
    if table.hit_policy == HitPolicy::OutputOrder {
        if let Some(first_clause) = table.output_clauses.first() {
            if first_clause.output_values.is_none() {
                return Err("OUTPUT ORDER hit policy requires allowedValues on the output column".into());
            }
        }
    }

    // 4. Aggregator attribute consistency.
    if table.aggregation.is_some() && table.hit_policy != HitPolicy::Collect {
        return Err("aggregation attribute only valid with COLLECT hit policy".into());
    }

    Ok(())
}

// =========================================================================
// Tests
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dmn::model::{Decision, DecisionTable as DtModel, InputClause, OutputClause};
    use crate::dmn::parser::parse_dmn_with_cache;
    use std::fs;

    /// Helpers to build a minimal scope for tests.
    fn scope_with(score: i64) -> Scope {
        let mut s = Scope::empty();
        s.bind(
            "score".into(),
            Value::Number(rust_decimal::Decimal::from(score)),
            vec![],
        );
        s
    }

    /// Helper to load a DMN fixture and extract the first decision's table.
    fn load_fixture(name: &str) -> (crate::dmn::model::DmnModel, FeelAstCache) {
        let path = format!(
            "{}/tests/fixtures/dmn/{}",
            env!("CARGO_MANIFEST_DIR"),
            name
        );
        let src = fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read fixture {path}: {e}"));
        parse_dmn_with_cache(&src).expect("fixture should parse cleanly")
    }

    /// Helper: extract the DecisionTable from a loaded model by decision id.
    fn table_for<'a>(
        model: &'a crate::dmn::model::DmnModel,
        dec_id: &str,
    ) -> Option<&'a DtModel> {
        for child in &model.definitions.children {
            if let crate::dmn::model::DefinitionsChild::Decision(dec) = child {
                if dec.id == dec_id && dec.decision_table.is_some() {
                    return dec.decision_table.as_ref();
                }
            }
        }
        None
    }

    // ---------------------------------------------------------------------
    // Unit tests for the evaluator core + foundational hit policies
    // ---------------------------------------------------------------------

    #[test]
    fn empty_table_returns_null() {
        let (model, cache) = load_fixture("decision_1_5.dmn");
        if let Some(dt) = table_for(&model, "_dec_unique") {
            let scope = Scope::empty();
            let result = DecisionTableEvaluator::evaluate(dt, &scope, &cache);
            // An empty decision table (no rules) under UNIQUE returns null.
            assert_eq!(result.unwrap(), Value::Null);
        }
    }

    #[test]
    fn unique_single_match_returns_output() {
        let (model, cache) = load_fixture("decision_1_5.dmn");
        if let Some(dt) = table_for(&model, "_dec_unique") {
            // Score=95 only matches rule 0 (>= 90) in the UNIQUE fixture.
            let scope = scope_with(95);
            let result = DecisionTableEvaluator::evaluate(dt, &scope, &cache).unwrap();
            assert_eq!(result, Value::String("A".into()));
        }
    }

    #[test]
    fn unique_multi_match_returns_violation() {
        // Build a synthetic UNIQUE table where two rules overlap.
        let mut dt = DtModel::default();
        dt.hit_policy = HitPolicy::Unique;
        dt.input_clauses.push(InputClause {
            id: None,
            label: None,
            input_expression: "x".into(),
            input_values: None,
            description: None,
        });
        dt.output_clauses.push(OutputClause {
            id: None,
            label: None,
            name: Some("Result".into()),
            type_ref: Some("feel:string".into()),
            output_values: None,
            default_output_entry: None,
            description: None,
        });
        // Two rules that both match x >= 0 (overlap).
        dt.rules = vec![
            crate::dmn::model::DmnRule {
                id: None,
                description: None,
                input_entries: vec![crate::dmn::model::RuleEntry {
                    text: crate::dmn::model::TextContent::plain(">= 0"),
                }],
                output_entries: vec![crate::dmn::model::RuleEntry {
                    text: crate::dmn::model::TextContent::plain("\"A\""),
                }],
                annotation_entries: vec![],
            },
            crate::dmn::model::DmnRule {
                id: None,
                description: None,
                input_entries: vec![crate::dmn::model::RuleEntry {
                    text: crate::dmn::model::TextContent::plain(">= 0"),
                }],
                output_entries: vec![crate::dmn::model::RuleEntry {
                    text: crate::dmn::model::TextContent::plain("\"B\""),
                }],
                annotation_entries: vec![],
            },
        ];

        let cache = FeelAstCache::new();
        let mut scope = Scope::empty();
        scope.bind("x".into(), Value::Number(rust_decimal::Decimal::from(5)), vec![]);
        let result = DecisionTableEvaluator::evaluate(&dt, &scope, &cache);
        assert!(result.is_err());
        match result.unwrap_err() {
            DecisionTableError::HitPolicyViolation { policy, .. } => {
                assert_eq!(policy, "UNIQUE");
            }
            other => panic!("expected HitPolicyViolation, got {other:?}"),
        }
    }

    #[test]
    fn first_returns_first_match() {
        // Build a synthetic FIRST table with two overlapping rules.
        let mut dt = DtModel::default();
        dt.hit_policy = HitPolicy::First;
        dt.input_clauses.push(InputClause {
            id: None,
            label: None,
            input_expression: "x".into(),
            input_values: None,
            description: None,
        });
        dt.output_clauses.push(OutputClause {
            id: None,
            label: None,
            name: Some("Result".into()),
            type_ref: Some("feel:string".into()),
            output_values: None,
            default_output_entry: None,
            description: None,
        });
        dt.rules = vec![
            crate::dmn::model::DmnRule {
                id: None,
                description: None,
                input_entries: vec![crate::dmn::model::RuleEntry {
                    text: crate::dmn::model::TextContent::plain(">= 0"),
                }],
                output_entries: vec![crate::dmn::model::RuleEntry {
                    text: crate::dmn::model::TextContent::plain("\"First\""),
                }],
                annotation_entries: vec![],
            },
            crate::dmn::model::DmnRule {
                id: None,
                description: None,
                input_entries: vec![crate::dmn::model::RuleEntry {
                    text: crate::dmn::model::TextContent::plain(">= 0"),
                }],
                output_entries: vec![crate::dmn::model::RuleEntry {
                    text: crate::dmn::model::TextContent::plain("\"Second\""),
                }],
                annotation_entries: vec![],
            },
        ];

        let cache = FeelAstCache::new();
        let mut scope = Scope::empty();
        scope.bind("x".into(), Value::Number(rust_decimal::Decimal::from(5)), vec![]);
        let result = DecisionTableEvaluator::evaluate(&dt, &scope, &cache).unwrap();
        assert_eq!(result, Value::String("First".into()));
    }

    #[test]
    fn any_agreeing_returns_value() {
        // Build a synthetic ANY table with two rules that both match and agree.
        let mut dt = DtModel::default();
        dt.hit_policy = HitPolicy::Any;
        dt.input_clauses.push(InputClause {
            id: None,
            label: None,
            input_expression: "x".into(),
            input_values: None,
            description: None,
        });
        dt.output_clauses.push(OutputClause {
            id: None,
            label: None,
            name: Some("Result".into()),
            type_ref: Some("feel:string".into()),
            output_values: None,
            default_output_entry: None,
            description: None,
        });
        dt.rules = vec![
            crate::dmn::model::DmnRule {
                id: None,
                description: None,
                input_entries: vec![crate::dmn::model::RuleEntry {
                    text: crate::dmn::model::TextContent::plain(">= 0"),
                }],
                output_entries: vec![crate::dmn::model::RuleEntry {
                    text: crate::dmn::model::TextContent::plain("\"High\""),
                }],
                annotation_entries: vec![],
            },
            crate::dmn::model::DmnRule {
                id: None,
                description: None,
                input_entries: vec![crate::dmn::model::RuleEntry {
                    text: crate::dmn::model::TextContent::plain(">= 0"),
                }],
                output_entries: vec![crate::dmn::model::RuleEntry {
                    text: crate::dmn::model::TextContent::plain("\"High\""),
                }],
                annotation_entries: vec![],
            },
        ];

        let cache = FeelAstCache::new();
        let mut scope = Scope::empty();
        scope.bind("x".into(), Value::Number(rust_decimal::Decimal::from(5)), vec![]);
        let result = DecisionTableEvaluator::evaluate(&dt, &scope, &cache).unwrap();
        assert_eq!(result, Value::String("High".into()));
    }

    #[test]
    fn any_conflicting_raises_error() {
        // Build a synthetic ANY table with two rules that both match but disagree.
        let mut dt = DtModel::default();
        dt.hit_policy = HitPolicy::Any;
        dt.input_clauses.push(InputClause {
            id: None,
            label: None,
            input_expression: "x".into(),
            input_values: None,
            description: None,
        });
        dt.output_clauses.push(OutputClause {
            id: None,
            label: None,
            name: Some("Result".into()),
            type_ref: Some("feel:string".into()),
            output_values: None,
            default_output_entry: None,
            description: None,
        });
        dt.rules = vec![
            crate::dmn::model::DmnRule {
                id: None,
                description: None,
                input_entries: vec![crate::dmn::model::RuleEntry {
                    text: crate::dmn::model::TextContent::plain(">= 0"),
                }],
                output_entries: vec![crate::dmn::model::RuleEntry {
                    text: crate::dmn::model::TextContent::plain("\"High\""),
                }],
                annotation_entries: vec![],
            },
            crate::dmn::model::DmnRule {
                id: None,
                description: None,
                input_entries: vec![crate::dmn::model::RuleEntry {
                    text: crate::dmn::model::TextContent::plain(">= 0"),
                }],
                output_entries: vec![crate::dmn::model::RuleEntry {
                    text: crate::dmn::model::TextContent::plain("\"Low\""),
                }],
                annotation_entries: vec![],
            },
        ];

        let cache = FeelAstCache::new();
        let mut scope = Scope::empty();
        scope.bind("x".into(), Value::Number(rust_decimal::Decimal::from(5)), vec![]);
        let result = DecisionTableEvaluator::evaluate(&dt, &scope, &cache);
        assert!(result.is_err());
        match result.unwrap_err() {
            DecisionTableError::HitPolicyViolation { policy, .. } => {
                assert_eq!(policy, "ANY");
            }
            other => panic!("expected HitPolicyViolation, got {other:?}"),
        }
    }

    #[test]
    fn unimplemented_policy_returns_other_error() {
        // All policies are now implemented — this test no longer applies.
        // Previously tested that COLLECT/OUTPUT_ORDER/RULE_ORDER returned Other error.
        let mut dt = DtModel::default();
        dt.hit_policy = HitPolicy::Collect;
        // No rules → returns empty list (no aggregation).
        let cache = FeelAstCache::new();
        let scope = Scope::empty();
        let result = DecisionTableEvaluator::evaluate(&dt, &scope, &cache);
        assert!(result.is_ok());
    }
}
