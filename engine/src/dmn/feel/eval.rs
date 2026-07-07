//! FEEL evaluator — `evaluate(&FeelExpression, &Scope) -> Value`.
//!
//! Plan 31-02 owns the real implementation: the private `cmp` module
//! (`feel_equals` / `feel_compare` returning `Option<_>`, the explicit
//! `and_3vl`/`or_3vl`/`not_3vl` truth tables — Pitfall 1 mitigation), the
//! `Scope` flat-map + `deps` type, and the dispatch over
//! [`super::ast::FeelExpression`].
//!
//! **Prohibition:** never reuse Rust `==` / `Ord` / `compare_values` for FEEL
//! equality/ordering. The private `cmp` module does NOT re-export
//! `PartialEq`/`PartialOrd` — accidental `a == b` reuse at evaluator
//! call-sites becomes a "method not found" or lint error
//! (31-RESEARCH.md Pitfall 1 / Pattern 2).
//!
//! **Evaluator contract (DMN-18):** `evaluate` takes a *borrowed*
//! `&FeelExpression`, never `&str`. There is no `evaluate(src: &str, …)`
//! overload. This is the type-level contract that prevents the 4–6×
//! re-parse performance bug (a 30-cell table × 10k evals ≈ 30 parses at
//! load, not 300k). The AST cache (31-09) hands the evaluator `&*arc`.
//!
//! *(skeleton — implemented in plan 31-02)*

use serde::{Deserialize, Serialize};

/// Flat FEEL evaluation scope (CONTEXT.md user override: flat Scope +
/// explicit dependency tracking — NOT a parent/child chain).
///
/// **Skeleton** — the real `bindings` / `deps` fields land in 31-02. This
/// stub exists so that [`super::types::FeelFunction`] can reference
/// `Option<Arc<Scope>>` for the closure captured-environment field
/// (Blocker-1 carrier-shape fix). 31-02 replaces this with the full
/// struct + `get`/`bind`/`resolve_qn` methods; the `Serialize`/`Deserialize`
/// derives are retained so the `FeelFunction` serde bound continues to
/// propagate (Phase 30 D-30-01c).
///
/// 31-RESEARCH.md Pattern 3 / Scope Model + QN Resolution.
// (skeleton — implemented in plan 31-02)
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Scope;