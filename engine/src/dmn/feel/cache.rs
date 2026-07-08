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

use std::sync::Arc;

use dashmap::DashMap;

use super::ast::FeelExpression;
use super::parser::{parse_expression, parse_simple_unary_tests};

/// Entry-point kind for `get_or_parse`. The same FEEL text can produce a
/// different AST under the two entry points (Pitfall 8 of 31-RESEARCH.md) —
/// e.g. `"x"` is a name reference in Expression context but a positive unary
/// test on the name `x` in SimpleUnaryTests context. The cache key MUST
/// include this tag to avoid cross-contamination.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseEntry {
    /// A FEEL expression (`<inputExpression>`, `<defaultOutputEntry>`, etc.).
    Expression,
    /// A FEEL simple unary test (rule 15 — `RuleEntry.text` for input/output
    /// annotations).
    SimpleUnaryTests,
}

/// Canonical cache key: `(entry-point kind, trimmed content)`.
///
/// The `.trim()` normalizes leading/trailing whitespace so two cells that differ
/// only in indentation share a cache entry. Content is post-CDATA-strip (the
/// caller strips CDATA before passing `src` — this is the contract between
/// `FeelAstCache::get_or_parse` and its IR-loader callers). The entry-point
/// kind is part of the key to prevent Pitfall 8 cross-contamination.
pub fn cell_key(src: &str, entry: ParseEntry) -> String {
    let trimmed = src.trim();
    format!("{:?}:{}", entry, trimmed)
}

/// Concurrent FEEL AST cache.
///
/// `DashMap<String, Arc<FeelExpression>>` — keyed by [`cell_key`], values are
/// `Arc`s so the same parsed AST can be shared across many evaluation sites
/// without cloning. The map is bounded by the number of distinct (entry-point,
/// content) pairs in an IR; a pathological `.dmn` file is bounded by Phase 29's
/// parser DoS guards (`max_tokens`, `max_depth`).
#[derive(Debug)]
pub struct FeelAstCache {
    map: DashMap<String, Arc<FeelExpression>>,
}

impl FeelAstCache {
    /// Construct an empty cache.
    pub fn new() -> Self {
        Self {
            map: DashMap::new(),
        }
    }

    /// Look up or parse `src` under the given entry-point kind. Returns the
    /// cached (or freshly parsed) `Arc<FeelExpression>`. The same key always
    /// returns the SAME `Arc` — pointer identity is preserved via `DashMap`'s
    /// entry API and `Arc::clone()`.
    ///
    /// **Contract:** `src` must be post-CDATA-strip. CDATA flag round-trips at
    /// the IR-loader level, not here (the cache key uses only content +
    /// entry-point kind — two cells with identical text but different CDATA
    /// flags share a cache entry, which is correct: FEEL parsing ignores the
    /// CDATA wrapper).
    pub fn get_or_parse(&self, src: &str, entry: ParseEntry) -> Arc<FeelExpression> {
        let key = cell_key(src, entry);
        self.map
            .entry(key)
            .or_insert_with(|| match entry {
                ParseEntry::Expression => Arc::new(parse_expression(src)),
                ParseEntry::SimpleUnaryTests => Arc::new(parse_simple_unary_tests(src)),
            })
            .clone()
    }

    /// Number of distinct (entry-point, content) pairs currently cached.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// True iff the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

impl Default for FeelAstCache {
    fn default() -> Self {
        Self::new()
    }
}

// =====================================================================
// Tests
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Same (entry, content) pair returns the SAME Arc — pointer identity via
    /// `Arc::as_ptr`. This is the parse-once-per-key guarantee.
    #[test]
    fn get_or_parse_returns_same_arc_for_same_key() {
        let cache = FeelAstCache::new();

        // Parse "1 + 2" twice under Expression — must yield pointer-identical Arcs.
        let a = cache.get_or_parse("1 + 2", ParseEntry::Expression);
        let b = cache.get_or_parse("1 + 2", ParseEntry::Expression);

        assert!(Arc::as_ptr(&a) == Arc::as_ptr(&b), "same key must yield same Arc");
        // strong_count should be >= 2 (two clones of the same inner pointer,
        // plus any transient references from DashMap internals during the call).
        assert!(
            Arc::strong_count(&a) >= 2,
            "shared Arc must have count >= 2"
        );

        // Cache len: only one distinct key.
        assert_eq!(cache.len(), 1);
    }

    /// Same text but different entry-point kinds produce DIFFERENT Arcs (Pitfall 8).
    #[test]
    fn different_entry_points_yield_different_arcs() {
        let cache = FeelAstCache::new();

        // "x" is a Name in Expression context; it's also a name reference in
        // SimpleUnaryTests, but the parser treats them as distinct (the unary
        // test entry point has its own sentinel for lone `-`, etc.). The key
        // MUST differ — even if the AST happens to be structurally identical.
        let expr_ast = cache.get_or_parse("x", ParseEntry::Expression);
        let unary_ast = cache.get_or_parse("x", ParseEntry::SimpleUnaryTests);

        assert!(Arc::as_ptr(&expr_ast) != Arc::as_ptr(&unary_ast),
            "different entry points must yield different Arcs (Pitfall 8)");
        assert_eq!(cache.len(), 2, "two distinct keys");
    }

    /// Two cells with the same content but different CDATA flags hit the SAME
    /// cache entry — content is post-CDATA-strip.
    #[test]
    fn cdata_flag_does_not_affect_cache_key() {
        let cache = FeelAstCache::new();

        // The caller passes `src` after stripping CDATA; both calls below use
        // the same trimmed content (no CDATA markers). This is the documented
        // contract.
        let a = cache.get_or_parse("1 + 2", ParseEntry::Expression);
        let b = cache.get_or_parse("1 + 2", ParseEntry::Expression);

        assert!(Arc::as_ptr(&a) == Arc::as_ptr(&b));
        assert_eq!(cache.len(), 1, "same content shares a key regardless of CDATA");
    }

    /// Malformed expression returns an `Err` node — never panics (T-31-09-01
    /// mitigation: DoS via malformed .dmn).
    #[test]
    fn malformed_expression_returns_err_node() {
        let cache = FeelAstCache::new();

        // "1 + " is an incomplete binary op — the parser's error recovery
        // should emit a `FeelExpression::Err` node rather than panicking.
        let ast = cache.get_or_parse("1 + ", ParseEntry::Expression);

        // The result must be a FeelExpression (any variant). We just assert
        // no panic and that the cache grew by 1.
        assert_eq!(cache.len(), 1, "malformed expression still cached");
        // Verify it's an Err node — the parser's error-recovery contract.
        match &*ast {
            FeelExpression::Err(_, _) => {}
            other => panic!("expected Err node for malformed input, got {:?}", other),
        }
    }

    /// Cache bounded by distinct (entry-point, content) pairs: a fixed IR has
    /// a finite number of FEEL cells, so the cache cannot grow unboundedly.
    #[test]
    fn cache_bounded_by_distinct_pairs() {
        let cache = FeelAstCache::new();

        // Push 100 distinct keys — cache len must equal 100, not 10_000 or
        // anything else.
        for i in 0..100u32 {
            let _ = cache.get_or_parse(&format!("{}", i), ParseEntry::Expression);
        }

        assert_eq!(cache.len(), 100, "cache len == distinct pairs");

        // Now re-request the same 100 keys — len must remain 100.
        for i in 0..100u32 {
            let _ = cache.get_or_parse(&format!("{}", i), ParseEntry::Expression);
        }

        assert_eq!(cache.len(), 100, "re-requesting same keys does not grow cache");
    }

    /// `trim()` normalizes whitespace — `"  x  "` and `"x"` share a key.
    #[test]
    fn trim_normalizes_whitespace_in_key() {
        let cache = FeelAstCache::new();

        let a = cache.get_or_parse("  x  ", ParseEntry::Expression);
        let b = cache.get_or_parse("x", ParseEntry::Expression);

        assert!(Arc::as_ptr(&a) == Arc::as_ptr(&b), "trimmed content shares key");
        assert_eq!(cache.len(), 1);
    }

    /// `ParseEntry` derives PartialEq, Eq — required for cell_key hashing.
    #[test]
    fn parse_entry_derives() {
        let a = ParseEntry::Expression;
        let b = ParseEntry::SimpleUnaryTests;
        assert_ne!(a, b);
        assert_eq!(a.clone(), ParseEntry::Expression);

        // Debug derive — used in cell_key format string.
        let debug_a = format!("{:?}", a);
        assert!(debug_a.contains("Expression"));
    }

    /// `cell_key` produces deterministic output for the same inputs.
    #[test]
    fn cell_key_deterministic() {
        let k1 = cell_key("  foo + bar  ", ParseEntry::Expression);
        let k2 = cell_key("foo + bar", ParseEntry::Expression);
        assert_eq!(k1, k2, "trim normalizes whitespace");

        // Different entry point → different key for same content.
        let k3 = cell_key("foo + bar", ParseEntry::SimpleUnaryTests);
        assert_ne!(k1, k3, "entry-point kind is part of the key");
    }
}
