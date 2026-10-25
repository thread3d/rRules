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

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, Weak};

use dashmap::DashMap;

use super::ast::FeelExpression;
use super::parser::{
    parse_expression_with_cache, parse_simple_unary_tests_with_cache, LexerTokenCache,
};

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

/// Default bounded capacity for the AST cache (PERF-05).
///
/// The cache is per-model-scoped — one per `parse_dmn_with_cache` call and
/// one per `DecisionEvaluator` — so it is naturally bounded by a single
/// model's distinct FEEL cells. This cap is a safety backstop for the case
/// where a single cache instance is reused across many models.
const DEFAULT_MAX_CACHE_ENTRIES: usize = 10_000;

// =====================================================================
// PERF-06: per-thread L1 AST cache
// =====================================================================

/// Default bounded capacity for the per-thread L1 cache (PERF-06).
///
/// The L1 is small and per-thread: it shadows the shared `DashMap` to avoid
/// shard-lock acquisition on the hot read path. 1024 distinct cells is ample
/// for a single request's working set; the FIFO bound caps per-thread memory.
const DEFAULT_L1_CACHE_ENTRIES: usize = 1024;

/// Per-thread L1 FEEL-AST cache (PERF-06).
///
/// FEEL text→AST is a **pure function** of `(entry-point kind, trimmed
/// content)`: the same text always yields the same AST, regardless of which
/// model or `FeelAstCache` instance issued the parse. That purity lets a
/// single thread-local cache shadow *every* `FeelAstCache` on the calling
/// thread and stay correct across model reloads — an L1 hit returns the SAME
/// `Arc` the shared `DashMap` owns, with **no shard-lock acquisition** on the
/// hot path (the dominant cost under high concurrency, where many threads
/// contend on the shared map's shards).
///
/// Entries are `Weak<FeelExpression>`: the shared `DashMap` (or a caller's
/// held `Arc`) is the authority that keeps an AST alive. When every strong
/// ref drops — e.g. a model is dropped or its cache evicts the key — the L1
/// weak simply fails to `upgrade` and the next call falls through to the
/// shared map. This bounds per-thread memory: the L1 never keeps an AST
/// alive past its real owners (no leak on long-lived worker threads such as
/// the rayon pool used by PERF-09 parallel DRD evaluation).
///
/// **Correctness across reloads:** because the parse is pure, a stale Weak
/// can only ever resolve to an AST that is bit-identical to what a fresh
/// parse would produce — so even if a reload reused a live L1 entry, the
/// result is correct. Dead Weaks (authority dropped) are purged lazily on
/// the next lookup of the same key.
struct L1AstCache {
    map: HashMap<String, Weak<FeelExpression>>,
    /// FIFO insertion order for bounded eviction.
    order: VecDeque<String>,
    max_entries: usize,
}

impl L1AstCache {
    fn new() -> Self {
        Self {
            map: HashMap::new(),
            order: VecDeque::new(),
            max_entries: DEFAULT_L1_CACHE_ENTRIES,
        }
    }
}

thread_local! {
    /// The global per-thread L1. Shared across all `FeelAstCache` instances on
    /// a thread (safe because the parse is pure). Each rayon/tokio worker
    /// thread accumulates its own warm L1 across `evaluate` calls.
    static L1_FEEL_AST: RefCell<L1AstCache> = RefCell::new(L1AstCache::new());
}

/// L1 lookup. Returns a strong `Arc` on a live hit. A dead Weak (authority
/// dropped the strong) is purged and `None` returned so the caller falls
/// through to the shared `DashMap`.
fn l1_get(key: &str) -> Option<Arc<FeelExpression>> {
    L1_FEEL_AST.with(|cell| {
        let mut c = cell.borrow_mut();
        // Clone the Weak out first so the map borrow ends before a possible
        // `remove` (avoids a double-borrow on `c.map`).
        let weak = c.map.get(key).map(Weak::clone)?;
        match weak.upgrade() {
            Some(strong) => Some(strong),
            None => {
                // Stale: the shared authority dropped the strong ref. Reclaim
                // the slot rather than leaving a dead entry to age out via FIFO.
                c.map.remove(key);
                None
            }
        }
    })
}

/// Record a freshly resolved `Arc` in the L1 as a `Weak`. No-op if the key is
/// already present (keeps FIFO order stable on repeat inserts). FIFO-evicts
/// the oldest entry when the per-thread bound is reached.
fn l1_insert(key: String, ast: &Arc<FeelExpression>) {
    L1_FEEL_AST.with(|cell| {
        let mut c = cell.borrow_mut();
        if c.map.contains_key(&key) {
            return;
        }
        if c.map.len() >= c.max_entries {
            if let Some(old) = c.order.pop_front() {
                c.map.remove(&old);
            }
        }
        c.map.insert(key.clone(), Arc::downgrade(ast));
        c.order.push_back(key);
    })
}

/// Drop every L1 entry on the calling thread. Used by [`FeelAstCache::clear`]
/// for hygiene and by tests for isolation (the Weak design already makes
/// cross-test contamination impossible — strong refs die with each test's
/// cache — but this makes the intent explicit).
pub fn clear_l1() {
    L1_FEEL_AST.with(|cell| {
        let mut c = cell.borrow_mut();
        c.map.clear();
        c.order.clear();
    });
}

/// Concurrent FEEL AST cache.
///
/// `DashMap<String, Arc<FeelExpression>>` — keyed by [`cell_key`], values are
/// `Arc`s so the same parsed AST can be shared across many evaluation sites
/// without cloning. The map is bounded by the number of distinct (entry-point,
/// content) pairs in an IR; a pathological `.dmn` file is bounded by Phase 29's
/// parser DoS guards (`max_tokens`, `max_depth`).
///
/// **Phase 37 / PERF-05 — lexer token cache warming:** the cache owns a
/// [`LexerTokenCache`] and feeds it into the parser via the
/// `parse_*_with_cache` entry points. The first parse of a cell lexes its
/// source and stores the token stream; subsequent parses of the same cell
/// reuse the cached tokens and skip lexing entirely. This closes the
/// "orphaned dead code" gap — the token cache is now consumed by every
/// production parse path (IR-loader warm walk + evaluator).
///
/// **PERF-05 — bounded eviction:** the map is bounded by
/// [`DEFAULT_MAX_CACHE_ENTRIES`] (overridable via [`FeelAstCache::with_capacity`]).
/// Eviction is FIFO (insertion-order), tracked by a `Mutex<VecDeque<String>>`.
/// FIFO — not strict access-order LRU — is deliberate: the hot read path
/// (`get_or_parse` hits) stays lock-free (the `Mutex` is only touched on a
/// miss/insert), and the cache is populated once at model load and read-only
/// thereafter, so access-order recency carries no practical benefit at the
/// cost of serializing reads. Eviction only ever fires when a single cache
/// is reused across many models.
#[derive(Debug)]
pub struct FeelAstCache {
    map: DashMap<String, Arc<FeelExpression>>,
    /// Lexer token cache (Phase 37, PERF-05) — warms the parser so repeated
    /// parses of the same cell skip lexing.
    tokens: LexerTokenCache,
    /// Insertion order for FIFO eviction when `map.len()` exceeds `max_entries`.
    order: Mutex<VecDeque<String>>,
    /// Bounded capacity (PERF-05 backstop).
    max_entries: usize,
}

impl FeelAstCache {
    /// Construct an empty cache with the default bounded capacity.
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_MAX_CACHE_ENTRIES)
    }

    /// Construct an empty cache with a custom bounded capacity.
    pub fn with_capacity(max_entries: usize) -> Self {
        Self {
            map: DashMap::new(),
            tokens: LexerTokenCache::new(),
            order: Mutex::new(VecDeque::new()),
            max_entries,
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
    ///
    /// **PERF-05:** on a miss, the parse runs through the `*_with_cache` entry
    /// points with `Some(&self.tokens)`, so lexing is served from the
    /// [`LexerTokenCache`] and skipped on repeat parses of the same text.
    pub fn get_or_parse(&self, src: &str, entry: ParseEntry) -> Arc<FeelExpression> {
        let key = cell_key(src, entry);

        // PERF-06: per-thread L1 cache (see module docs). FEEL text→AST is a
        // pure function, so a thread_local keyed by (entry, content) is correct
        // across model reloads. A live hit returns the SAME Arc the shared
        // DashMap owns — no shard-lock acquisition, which is the dominant cost
        // under high concurrency. A dead Weak (authority dropped the strong)
        // falls through to the shared map below.
        if let Some(ast) = l1_get(&key) {
            return ast;
        }

        let ast = match self.map.entry(key.clone()) {
            dashmap::mapref::entry::Entry::Occupied(o) => o.get().clone(),
            dashmap::mapref::entry::Entry::Vacant(v) => {
                let parsed = match entry {
                    ParseEntry::Expression => {
                        Arc::new(parse_expression_with_cache(src, Some(&self.tokens)))
                    }
                    ParseEntry::SimpleUnaryTests => {
                        Arc::new(parse_simple_unary_tests_with_cache(src, Some(&self.tokens)))
                    }
                };
                v.insert(parsed.clone());
                if let Ok(mut ord) = self.order.lock() {
                    ord.push_back(key.clone());
                }
                parsed
            }
        };
        // PERF-05 backstop: evict oldest entries if over capacity. Eviction
        // only fires when a single cache is reused across many models; the
        // per-model scope means `map.len()` stays under `max_entries` in
        // normal operation.
        if self.map.len() > self.max_entries {
            self.evict_oldest();
        }

        // PERF-06: warm the per-thread L1 with a Weak so future lookups on
        // this thread hit the L1 and skip the shared DashMap shard lock. The
        // Weak ensures the L1 does not keep the AST alive past the shared
        // cache's authority (no per-thread memory leak).
        l1_insert(key, &ast);

        ast
    }

    /// FIFO eviction: drop the oldest-inserted keys until `map.len()` is back
    /// under `max_entries`. Duplicate keys in the deque (from concurrent
    /// misses) are harmless — `map.remove` is idempotent on a missing key.
    fn evict_oldest(&self) {
        let to_remove: Vec<String> = {
            let mut ord = match self.order.lock() {
                Ok(o) => o,
                Err(_) => return, // poisoned lock — skip eviction, stay bounded by next pass
            };
            let mut keys = Vec::new();
            while self.map.len() > self.max_entries {
                match ord.pop_front() {
                    Some(k) => keys.push(k),
                    None => break,
                }
            }
            keys
        };
        for k in &to_remove {
            self.map.remove(k);
        }
    }

    /// Number of distinct (entry-point, content) pairs currently cached.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// True iff the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Clear all cached ASTs and tokens (useful for test isolation or reload
    /// scenarios). Resets both the AST map and the lexer token cache.
    pub fn clear(&self) {
        self.map.clear();
        self.tokens.clear();
        if let Ok(mut ord) = self.order.lock() {
            ord.clear();
        }
        // PERF-06: also drop the per-thread L1 on the calling thread. The
        // Weak design means L1 entries pointing at this cache's strongs are
        // already dead, but clearing reclaims the slots immediately and keeps
        // the L1 from carrying stale keys into a reload.
        clear_l1();
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

        assert!(
            Arc::as_ptr(&a) == Arc::as_ptr(&b),
            "same key must yield same Arc"
        );
        // strong_count should be >= 2 (two clones of the same inner pointer,
        // plus any transient references from DashMap internals during the call).
        assert!(
            Arc::strong_count(&a) >= 2,
            "shared Arc must have count >= 2"
        );

        // Cache len: only one distinct key.
        assert_eq!(cache.len(), 1);
    }

    /// PERF-05 wiring proof: `get_or_parse` warms the owned [`LexerTokenCache`].
    /// This is the regression guard for the Phase 37 "orphaned dead code" audit
    /// finding — if someone reverts `get_or_parse` to the non-cache
    /// `parse_expression`, this test fails because the token cache stays empty.
    #[test]
    fn get_or_parse_warms_lexer_token_cache() {
        let cache = FeelAstCache::new();
        assert!(
            cache.tokens.is_empty(),
            "token cache empty before first parse"
        );

        let _ = cache.get_or_parse("1 + 2 * 3", ParseEntry::Expression);
        assert!(
            !cache.tokens.is_empty(),
            "get_or_parse must warm the LexerTokenCache (PERF-05 wiring)"
        );
        let token_entries = cache.tokens.len();
        assert_eq!(
            token_entries, 1,
            "one distinct cell → one token cache entry"
        );

        // A second, distinct cell adds a second token cache entry.
        let _ = cache.get_or_parse("x", ParseEntry::SimpleUnaryTests);
        assert_eq!(
            cache.tokens.len(),
            2,
            "second distinct cell → two token entries"
        );

        // Re-parsing the first cell must NOT grow the token cache (cache hit).
        let _ = cache.get_or_parse("1 + 2 * 3", ParseEntry::Expression);
        assert_eq!(cache.tokens.len(), 2, "repeat parse reuses cached tokens");
    }

    /// PERF-05 bounded eviction: a cache with a tiny capacity evicts the
    /// oldest entry once the cap is exceeded, keeping the AST map bounded.
    #[test]
    fn bounded_cache_evicts_oldest_when_over_capacity() {
        let cache = FeelAstCache::with_capacity(2);
        let _ = cache.get_or_parse("a", ParseEntry::Expression);
        let _ = cache.get_or_parse("b", ParseEntry::Expression);
        assert_eq!(cache.len(), 2, "at capacity");

        // Third distinct entry pushes over the cap → "a" (oldest) is evicted.
        let _ = cache.get_or_parse("c", ParseEntry::Expression);
        assert!(
            cache.len() <= 2,
            "cache bounded after eviction (got {})",
            cache.len()
        );

        // "b" and "c" must still be reachable without panic; re-parsing the
        // evicted "a" must succeed (it re-parses) and keep the cache bounded.
        let _ = cache.get_or_parse("b", ParseEntry::Expression);
        let _ = cache.get_or_parse("c", ParseEntry::Expression);
        let _ = cache.get_or_parse("a", ParseEntry::Expression);
        assert!(
            cache.len() <= 2,
            "cache stays bounded after re-parse of evicted key"
        );
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

        assert!(
            Arc::as_ptr(&expr_ast) != Arc::as_ptr(&unary_ast),
            "different entry points must yield different Arcs (Pitfall 8)"
        );
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
        assert_eq!(
            cache.len(),
            1,
            "same content shares a key regardless of CDATA"
        );
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

        assert_eq!(
            cache.len(),
            100,
            "re-requesting same keys does not grow cache"
        );
    }

    /// `trim()` normalizes whitespace — `"  x  "` and `"x"` share a key.
    #[test]
    fn trim_normalizes_whitespace_in_key() {
        let cache = FeelAstCache::new();

        let a = cache.get_or_parse("  x  ", ParseEntry::Expression);
        let b = cache.get_or_parse("x", ParseEntry::Expression);

        assert!(
            Arc::as_ptr(&a) == Arc::as_ptr(&b),
            "trimmed content shares key"
        );
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

    // =================================================================
    // PERF-06: per-thread L1 cache
    // =================================================================

    /// L1 hit returns the SAME Arc the shared DashMap owns (pointer identity),
    /// and a repeat call does NOT touch the shared map — `cache.len()` stays
    /// at 1 because the second call is served entirely from the L1.
    #[test]
    fn l1_hit_returns_same_arc_and_skips_shared_map() {
        clear_l1();
        let cache = FeelAstCache::new();

        let a = cache.get_or_parse("l1_hit_probe", ParseEntry::Expression);
        assert_eq!(cache.len(), 1, "first call populates the shared map");

        // Second call: L1 hit (Weak upgrades — `a` and the shared map both
        // hold strongs). Served from the L1, shared map untouched.
        let b = cache.get_or_parse("l1_hit_probe", ParseEntry::Expression);
        assert!(
            Arc::as_ptr(&a) == Arc::as_ptr(&b),
            "L1 hit must return the SAME Arc the shared map owns"
        );
        assert_eq!(
            cache.len(),
            1,
            "L1 hit must not grow or touch the shared map"
        );
    }

    /// Cross-instance sharing: the L1 is global per thread, and FEEL text→AST
    /// is pure, so a SECOND `FeelAstCache` instance resolves the same cell
    /// from the L1 without parsing or inserting into its own shared map.
    /// This is the cross-reload value of the thread_local design.
    #[test]
    fn l1_shares_across_cache_instances_on_same_thread() {
        clear_l1();
        let cache_a = FeelAstCache::new();
        // Hold a strong ref so the L1 Weak stays live across the instance swap.
        let a = cache_a.get_or_parse("l1_cross_instance_probe", ParseEntry::Expression);
        assert_eq!(cache_a.len(), 1);

        // A brand-new cache on the same thread. The L1 already has a live Weak
        // for this key (kept alive by `a`), so this call is an L1 HIT — it
        // returns cache_a's Arc and does NOT insert into cache_b's shared map.
        let cache_b = FeelAstCache::new();
        let b = cache_b.get_or_parse("l1_cross_instance_probe", ParseEntry::Expression);
        assert!(
            Arc::as_ptr(&a) == Arc::as_ptr(&b),
            "cross-instance L1 hit returns cache_a's Arc (pure-function sharing)"
        );
        assert_eq!(
            cache_b.len(),
            0,
            "L1 hit must not populate the new instance's shared map"
        );
        clear_l1();
    }

    /// When the shared authority drops the strong (eviction), the L1 Weak
    /// dies and the next lookup falls through to a fresh parse rather than
    /// returning a stale/dead Arc. This is the no-leak / no-stale guarantee.
    #[test]
    fn l1_dead_weak_falls_through_to_shared_map() {
        clear_l1();
        let cache = FeelAstCache::with_capacity(1);

        // "first" is parsed, L1 Weak inserted. `let _` drops the caller's
        // strong immediately, so the only strong lives in the shared map.
        let _ = cache.get_or_parse("l1_dead_first", ParseEntry::Expression);
        assert_eq!(cache.len(), 1);

        // "second" overflows the cap-1 cache → evicts "first" from the shared
        // map. The L1 Weak for "first" is now dead (no strong remains).
        let _ = cache.get_or_parse("l1_dead_second", ParseEntry::Expression);
        assert!(cache.len() <= 1, "cap-1 cache evicted the first entry");

        // Re-request "first": L1 Weak is dead → fall through → re-parse →
        // re-insert. Must succeed and return a valid Arc, not panic.
        let again = cache.get_or_parse("l1_dead_first", ParseEntry::Expression);
        assert_eq!(
            cache.len(),
            1,
            "re-parse of evicted key re-populates the shared map"
        );
        // Touch the Arc to prove it's usable after a dead-weak fallthrough.
        let _ = &*again;
        clear_l1();
    }

    /// `clear()` clears the per-thread L1 as well as the shared map: after a
    /// clear, a repeat call is a true miss (re-parse), not an L1 hit.
    #[test]
    fn clear_clears_l1_too() {
        clear_l1();
        let cache = FeelAstCache::new();
        let a = cache.get_or_parse("l1_clear_probe", ParseEntry::Expression);
        let _ = a;
        assert_eq!(cache.len(), 1);

        cache.clear();
        assert_eq!(cache.len(), 0, "shared map cleared");

        // After clear(), the L1 is empty on this thread → genuine re-parse.
        let _ = cache.get_or_parse("l1_clear_probe", ParseEntry::Expression);
        assert_eq!(
            cache.len(),
            1,
            "post-clear repeat is a real miss, not an L1 hit"
        );
        clear_l1();
    }

    /// `clear_l1` is callable and idempotent (no panic on empty or repeat).
    #[test]
    fn clear_l1_is_idempotent() {
        clear_l1();
        clear_l1(); // empty — no panic
        let cache = FeelAstCache::new();
        let _ = cache.get_or_parse("l1_idem", ParseEntry::Expression);
        clear_l1();
        clear_l1(); // populated then cleared twice — no panic
    }
}
