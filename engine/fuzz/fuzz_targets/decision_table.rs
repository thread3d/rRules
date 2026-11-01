//! Fuzzing target for the decision-table evaluator (Phase 38, T3).
//!
//! Drives [`DecisionTableEvaluator::evaluate`] from *fuzzed* input across all
//! seven DMN hit policies (UNIQUE, ANY, PRIORITY, FIRST, RULE ORDER, OUTPUT
//! ORDER, COLLECT) and every COLLECT aggregator (SUM, MIN, MAX, COUNT, plus the
//! no-aggregator form), so corpus exploration varies the table shape, the FEEL
//! cell text, and the input scope rather than re-running one fixed table.
//!
//! The contract under fuzz is **no panics**: the evaluator must return a
//! `Result<Value, DecisionTableError>` (Ok or Err) for any structurally
//! consistent table. A panic here is a real bug in the DMN path.
//!
//! The table is always kept *structurally consistent* — `input_entries.len()`
//! == `input_clauses.len()` and `output_entries.len() == `output_clauses.len()`
//! per rule — so the fuzzer exercises evaluation logic, not trivially
//! malformed input. FEEL cell text is drawn from a curated palette of valid
//! fragments so the AST cache parses real expressions; the palette is wide
//! enough (null, comparisons, arithmetic, string literals, bare references)
//! to cover the three-valued + numeric + string branches the evaluator
//! distinguishes.

#![no_main]

use std::sync::Arc;

use libfuzzer_sys::fuzz_target;
use rust_decimal::Decimal;

use engine::data_types::Value;
use engine::dmn::decision_table::DecisionTableEvaluator;
use engine::dmn::feel::cache::FeelAstCache;
use engine::dmn::feel::eval::Scope;
use engine::dmn::model::{
    AllowedValues, BuiltinAggregator, DecisionTable, HitPolicy, InputClause, OutputClause,
    RuleEntry, TextContent,
};

// ── Curated FEEL-fragment palettes (all valid; chosen for branch coverage) ──

/// Input-expression text for the single input clause. References scope
/// bindings so input evaluation exercises scope resolution.
const INPUT_EXPRS: &[&str] = &["x", "y", "x + y", "true"];

/// Input-entry (unary-test) cell text. `-` is match-any; the rest exercise
/// comparison, equality, null, and boolean unary tests.
const INPUT_TESTS: &[&str] = &["-", "> 0", "< 100", "50", ">= 0", "null", "< 1000", "true", "- 1"];

/// Output-entry cell text (FEEL expressions). Produces strings, numbers,
/// null, and booleans so COLLECT aggregators and PRIORITY/OUTPUT ORDER enum
/// matching each have something to operate on.
const OUTPUT_EXPRS: &[&str] = &[
    "\"a\"", "\"b\"", "\"c\"", "1", "x", "x + 1", "null", "true", "50", "x * 2",
];

/// All seven hit policies, iterated for every fuzzed table.
const ALL_POLICIES: &[HitPolicy] = &[
    HitPolicy::Unique,
    HitPolicy::Any,
    HitPolicy::Priority,
    HitPolicy::First,
    HitPolicy::RuleOrder,
    HitPolicy::OutputOrder,
    HitPolicy::Collect,
];

/// Every COLLECT aggregator plus the no-aggregator form, iterated when the
/// hit policy is COLLECT.
const ALL_AGGREGATORS: &[Option<BuiltinAggregator>] = &[
    None,
    Some(BuiltinAggregator::Sum),
    Some(BuiltinAggregator::Min),
    Some(BuiltinAggregator::Max),
    Some(BuiltinAggregator::Count),
];

// ── Byte cursor: deterministic, panic-free consumption of fuzz input ────────

/// Cursor over the fuzz byte stream. Every reader returns a default when
/// exhausted, so a short input still yields a complete, consistent table.
struct Cursor<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }

    /// Next byte, or 0 when exhausted (0 is a valid, neutral index base).
    fn u8(&mut self) -> u8 {
        let b = *self.bytes.get(self.pos).unwrap_or(&0);
        self.pos = self.pos.saturating_add(1);
        b
    }

    /// Map the next byte into `0..len` (len must be non-zero).
    fn pick<'b, T>(&mut self, slice: &'b [T]) -> &'b T {
        let len = slice.len();
        debug_assert!(len > 0);
        &slice[self.u8() as usize % len]
    }

    /// A small bounded count in `1..=max`.
    fn count(&mut self, max: usize) -> usize {
        let m = max.max(1);
        (self.u8() as usize % m) + 1
    }
}

// ── Fuzzed scope construction ───────────────────────────────────────────────

/// Build a scope binding `x` and `y` to fuzz-chosen values drawn from the
/// FEEL-relevant domain (number, integer, string, bool, null). Both names are
/// always bound so any input expression resolves to a real value.
fn fuzz_scope(cur: &mut Cursor) -> Scope {
    let mut scope = Scope::empty();
    scope.bind("x".to_string(), fuzz_value(cur), Vec::new());
    scope.bind("y".to_string(), fuzz_value(cur), Vec::new());
    scope
}

/// Pick a FEEL value from a small palette covering the numeric, string,
/// boolean, and null branches the evaluator treats distinctly.
fn fuzz_value(cur: &mut Cursor) -> Value {
    let n = cur.u8();
    match n % 6 {
        0 => Value::Number(Decimal::from((n as i64).wrapping_sub(128))),
        1 => Value::Number(Decimal::from(cur.u8() as i64)),
        2 => Value::Integer(n as i64 - 128),
        3 => Value::String(format!("s{}", n % 5)),
        4 => Value::Bool(n.is_multiple_of(2)),
        _ => Value::Null,
    }
}

// ── Fuzzed table construction ───────────────────────────────────────────────

/// Build a structurally consistent [`DecisionTable`] from the fuzz stream.
///
/// The output clause carries a `string` enumeration as its `output_values` so
/// the PRIORITY and OUTPUT ORDER branches have a valid allowed-values list to
/// rank against (outputs outside the enum exercise those policies' error
/// paths, which must not panic either).
fn fuzz_table(cur: &mut Cursor) -> DecisionTable {
    let input_expr = *cur.pick(INPUT_EXPRS);

    let input_clause = InputClause {
        id: Some("in0".to_string()),
        label: Some("x".to_string()),
        input_expression: input_expr.to_string(),
        input_values: None,
        description: None,
    };

    // Enumeration gives PRIORITY/OUTPUT ORDER a valid ranking source.
    let enum_values = vec![
        TextContent::plain("\"a\""),
        TextContent::plain("\"b\""),
        TextContent::plain("\"c\""),
    ];
    let output_clause = OutputClause {
        id: Some("out0".to_string()),
        label: Some("result".to_string()),
        name: Some("result".to_string()),
        type_ref: Some("string".to_string()),
        output_values: Some(AllowedValues::Enumeration(enum_values)),
        default_output_entry: None,
        description: None,
    };

    let n_rules = cur.count(4); // 1..=4 rules
    let mut rules = Vec::with_capacity(n_rules);
    for i in 0..n_rules {
        let in_text = *cur.pick(INPUT_TESTS);
        let out_text = *cur.pick(OUTPUT_EXPRS);
        rules.push(engine::dmn::model::DmnRule {
            id: Some(format!("r{i}")),
            description: None,
            input_entries: vec![RuleEntry {
                text: TextContent::plain(in_text),
            }],
            output_entries: vec![RuleEntry {
                text: TextContent::plain(out_text),
            }],
            annotation_entries: Vec::new(),
        });
    }

    DecisionTable {
        id: Some("fuzz_table".to_string()),
        name: Some("Fuzz Table".to_string()),
        hit_policy: HitPolicy::Unique, // overridden per-policy in the harness
        aggregation: None,
        preferred_orientation: None,
        output_label: None,
        input_clauses: vec![input_clause],
        output_clauses: vec![output_clause],
        rules,
    }
}

// ── Fuzz entry point ────────────────────────────────────────────────────────

fuzz_target!(|data: &[u8]| {
    let mut cur = Cursor::new(data);

    // One shared AST cache across all evaluations of this table, mirroring
    // the production hot path where the cache is reused per decision.
    let cache = Arc::new(FeelAstCache::new());
    // The evaluator borrows `&FeelAstCache`; hand it a reference bound to the
    // Arc's lifetime for this target invocation.
    let cache_ref: &FeelAstCache = &cache;

    let table = fuzz_table(&mut cur);
    let scope = fuzz_scope(&mut cur);

    // Evaluate the SAME fuzzed table under every hit policy. Each policy
    // dispatches to a distinct evaluator branch; any panic is a bug.
    for &policy in ALL_POLICIES {
        // COLLECT additionally iterates every aggregator (incl. none).
        let aggregators: &[Option<BuiltinAggregator>] = if matches!(policy, HitPolicy::Collect) {
            ALL_AGGREGATORS
        } else {
            std::slice::from_ref(&None)
        };

        for &agg in aggregators {
            let mut t = table.clone();
            t.hit_policy = policy;
            t.aggregation = agg;

            // The contract: never panic. Discard the result (Ok or Err).
            let _ = DecisionTableEvaluator::evaluate(&t, &scope, cache_ref);
        }
    }
});