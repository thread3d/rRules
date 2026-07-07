//! FEEL AST cache — `DashMap<String, Arc<FeelExpression>>`.
//!
//! Plan 31-09 owns the real implementation: the cache struct keyed by a
//! canonical hash of the cell source (post-CDATA-strip, post-trim) AND the
//! entry-point kind (`Expression` vs `SimpleUnaryTests` — the same text
//! can parse differently under the two entry points, 31-RESEARCH.md
//! Pitfall 8); the IR-loader pre-parse walk (Phase 29
//! `InputClause.input_expression`, `RuleEntry.text`,
//! `OutputClause.default_output_entry`, `AllowedValues::UnaryTests`); and
//! the type-level `evaluate(&FeelExpression, &Scope)` contract that
//! prevents the #1 DMN performance bug (4–6× re-parse throughput loss).
//!
//! **Performance property:** a 30-cell table evaluated 10k times does
//! ~30 parses (at load), not 300k (31-RESEARCH.md headline property).
//!
//! *(skeleton — implemented in plan 31-09)*