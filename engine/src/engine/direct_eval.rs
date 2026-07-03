//! Shared direct rule evaluator (D-04) for the non-Rete engines.
//!
//! Plan 23-06 mirrors the Plan 23-03 `ReteEngine::execute_rules` direct
//! interpreter onto `TreatEngine`, `LeapsEngine`, and `PhreakEngine`. To
//! avoid 3x duplication of the evaluate-and-apply loop, the engine-agnostic
//! core is factored into this module. `ReteEngine` keeps its own inlined
//! implementation (the canonical reference from Plan 23-03, with its own
//! metrics integration); the other three engines delegate here.
//!
//! The loop is engine-agnostic because every engine stores working memory
//! as `HashMap<String, Fact>` (keyed by `fact.id.to_string()`) and loads
//! rules into a `Vec<Rule>` via `load_ruleset`. The helper:
//!
//! 1. Reads `RRULES_MAX_CHAIN_ITERATIONS` (default 1000) as the fixpoint
//!    cap (D-07 — the cycle guard so cyclic rule sets terminate without
//!    panicking).
//! 2. Iterates `rules` in a fixpoint loop, evaluating each `rule.condition`
//!    against `working_facts` via `Condition::evaluate`. Each rule fires at
//!    most once per call (`fired_set` guard) — this bounds the loop at
//!    `|rules|` passes and is the once-per-rule semantic that makes
//!    coverage truthful (L-06 / Pitfall 6).
//! 3. Applies matching `rule.actions` via `apply_action`, evaluating each
//!    `UpdateFact` field's `Expression` against a working-memory snapshot.
//! 4. After the loop, records `CoverageTracking::record_rule_eval` once per
//!    evaluated rule and `record_rule_fire` once per fired rule (Pitfall 6
//!    fix — never inside an inner loop).
//! 5. Returns `(fired_set, evaluated_set)` so the caller can populate
//!    `EngineResult.fired_rule_ids` and record engine-specific metrics.
//!
//! The placeholder per-nagram counting loops and per-nagram
//! `record_rule_fire` double-count bugs in the pre-23-06 Treat/Leaps/Phreak
//! `execute_rules` are removed — this helper owns coverage recording.

use std::collections::{HashMap, HashSet};

use crate::fact::Fact;
use crate::monitoring::coverage::CoverageTracking;
use crate::rules::{Action, Rule};

/// Run the direct rule evaluator (D-04) over `rules` against `working_facts`.
///
/// Returns `(fired_set, evaluated_set)` — the set of rule ids that fired and
/// the set of rule ids that were evaluated (fired or not). Records
/// `CoverageTracking::record_rule_eval` once per evaluated rule and
/// `record_rule_fire` once per fired rule AFTER the fixpoint loop (L-06 /
/// Pitfall 6). The caller wraps `fired_set` into `EngineResult` (populating
/// `fired_rule_ids`) and performs any engine-specific metrics bookkeeping.
///
/// The `RRULES_MAX_CHAIN_ITERATIONS` env var (default 1000) caps the
/// fixpoint loop (D-07). Each rule fires at most once per call (the
/// `fired_set` guard bounds the loop at `|rules|` passes; the cap is the
/// ultimate safety net for pathological rule sets).
pub fn direct_execute(
    rules: &[Rule],
    working_facts: &mut HashMap<String, Fact>,
) -> (HashSet<String>, HashSet<String>) {
    // D-07: fixpoint cap — configurable via env var, default 1000.
    let max_iter = std::env::var("RRULES_MAX_CHAIN_ITERATIONS")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(1000);

    let mut fired_set: HashSet<String> = HashSet::new();
    let mut evaluated_set: HashSet<String> = HashSet::new();

    // Fixpoint loop: repeat passes until no rule newly fires in a full
    // pass (D-07). Each rule fires at most once per execute() call
    // (fired_set guard) — this bounds the loop at |rules| passes and is
    // the once-per-rule semantic that makes coverage recording truthful
    // (L-06 / Pitfall 6). The max_iter cap is the ultimate safety net
    // for pathological rule sets.
    for _pass in 0..max_iter {
        let mut newly_fired_this_pass = false;
        // Index-based iteration so we can mutate `working_facts` while
        // reading `rules` without conflicting borrows.
        for rule_idx in 0..rules.len() {
            let rule_id = rules[rule_idx].id.clone();
            if fired_set.contains(&rule_id) {
                continue;
            }
            // Evaluate the condition against the current working memory.
            let facts: Vec<&Fact> = working_facts.values().collect();
            evaluated_set.insert(rule_id.clone());
            if rules[rule_idx].condition.evaluate(&facts) {
                // Apply actions. Clone the actions Vec so the immutable
                // borrow of `rules` ends before we mutably borrow
                // `working_facts`.
                let actions = rules[rule_idx].actions.clone();
                for action in &actions {
                    apply_action(action, working_facts);
                }
                fired_set.insert(rule_id);
                newly_fired_this_pass = true;
            }
        }
        if !newly_fired_this_pass {
            break;
        }
    }

    // L-06 / Pitfall 6: record coverage ONCE per rule OUTSIDE the inner
    // loop. `record_rule_eval` once per evaluated rule (fired or not);
    // `record_rule_fire` once per fired rule. This replaces the old
    // per-nagram double-count at the placeholder sites in each engine.
    for id in &evaluated_set {
        CoverageTracking::record_rule_eval(id);
    }
    for id in &fired_set {
        CoverageTracking::record_rule_fire(id);
    }

    (fired_set, evaluated_set)
}

/// Apply a single `Action` to the engine's working memory (D-04). Mirrors
/// `WorkingMemory::insert_fact`/`update_fact`/`retract_fact` (fact.rs) but
/// operates on the engine's `HashMap<String, Fact>` keyed by
/// `fact.id.to_string()`.
///
/// For `Action::UpdateFact { id, fields }`: the target fact is located by
/// exact `FactId` key first, then by the first fact whose `fact_type_id`
/// matches `id` (the keystone scenario uses `id: "scorecard"`, a fact type).
/// Each field's `Expression` is evaluated against the current working memory
/// snapshot and written into the fact's payload. `Action::CallFunction` and
/// `Action::LogMessage` are no-ops in Phase 23 (function dispatch and
/// logging are out of scope for the direct evaluator).
pub fn apply_action(action: &Action, working_facts: &mut HashMap<String, Fact>) {
    match action {
        Action::UpdateFact { id, fields } => {
            // Snapshot the facts for Expression evaluation (FieldRef reads).
            // Cloning is O(facts) and acceptable per action application.
            let facts_snapshot: Vec<Fact> = working_facts.values().cloned().collect();
            let facts_refs: Vec<&Fact> = facts_snapshot.iter().collect();

            // Locate the target fact: exact FactId key first, else first
            // fact of the named fact_type_id.
            let key_opt = if working_facts.contains_key(id) {
                Some(id.clone())
            } else {
                working_facts
                    .iter()
                    .find(|(_, f)| f.fact_type_id == *id)
                    .map(|(k, _)| k.clone())
            };
            let key = match key_opt {
                Some(k) => k,
                None => return,
            };
            let fact = match working_facts.get_mut(&key) {
                Some(f) => f,
                None => return,
            };
            for (field, expr) in fields {
                let value = expr.evaluate(&facts_refs);
                fact.payload.insert(field.clone(), value);
            }
        }
        Action::InsertFact { fact_type, fields } => {
            let fact = Fact::new(fact_type.clone(), fields.clone());
            working_facts.insert(fact.id.to_string(), fact);
        }
        Action::RetractFact { id } => {
            if working_facts.contains_key(id) {
                working_facts.remove(id);
            } else {
                let key_opt = working_facts
                    .iter()
                    .find(|(_, f)| f.fact_type_id == *id)
                    .map(|(k, _)| k.clone());
                if let Some(k) = key_opt {
                    working_facts.remove(&k);
                }
            }
        }
        Action::CallFunction { .. } => {
            // No-op in Phase 23 — function dispatch is out of scope (D-06).
        }
        Action::LogMessage { .. } => {
            // No-op — logging is not wired in the direct evaluator.
        }
    }
}