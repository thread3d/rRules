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
use std::time::Instant;

use crate::engine::registry::{TraceData, TraceEvent};
use crate::fact::Fact;
use crate::monitoring::coverage::CoverageTracking;
use crate::monitoring::detection::RuleExecutionTracker;
use crate::rules::{Action, Rule};

/// Run the direct rule evaluator (D-04) over `rules` against `working_facts`.
///
/// Returns `(fired_set, evaluated_set, trace)` — the set of rule ids that
/// fired, the set of rule ids that were evaluated (fired or not), and an
/// optional ordered `TraceData` payload. Records
/// `CoverageTracking::record_rule_eval` once per evaluated rule and
/// `record_rule_fire` once per fired rule AFTER the fixpoint loop (L-06 /
/// Pitfall 6). The caller wraps `fired_set` into `EngineResult` (populating
/// `fired_rule_ids`), attaches `trace` to `EngineResult.trace`, and performs
/// any engine-specific metrics bookkeeping.
///
/// Phase 28 (D-01 / EXEC-04, C1 site 1): when `collect_trace` is true, the
/// loop emits one `TraceEvent` per fired rule IN FIRING ORDER (the order the
/// fixpoint loop fires them — NOT the HashSet iteration order of
/// `fired_set`, which is nondeterministic per Critical Finding 3). When
/// `collect_trace` is false, NO trace work happens — no `Vec`, no `String`,
/// no `chrono` call — so SC-3 latency neutrality holds for the trace-off
/// path. The trace is ADDITIVE to `fired_rule_ids` (existing consumers are
/// unchanged).
///
/// The `RRULES_MAX_CHAIN_ITERATIONS` env var (default 1000) caps the
/// fixpoint loop (D-07). Each rule fires at most once per call (the
/// `fired_set` guard bounds the loop at `|rules|` passes; the cap is the
/// ultimate safety net for pathological rule sets).
pub fn direct_execute(
    rules: &[Rule],
    working_facts: &mut HashMap<String, Fact>,
    collect_trace: bool,
) -> (HashSet<String>, HashSet<String>, Option<TraceData>) {
    direct_execute_with_limit(rules, working_facts, collect_trace, None)
}

/// [`direct_execute`] with an explicit fixpoint cap.
///
/// `max_iterations` overrides the `RRULES_MAX_CHAIN_ITERATIONS` environment
/// variable for this execution, which is how an engine created through
/// `EngineBuilder::max_iterations` (or `set_config("max_iterations", …)`) makes
/// its setting take effect instead of being silently ignored.
pub fn direct_execute_with_limit(
    rules: &[Rule],
    working_facts: &mut HashMap<String, Fact>,
    collect_trace: bool,
    max_iterations: Option<usize>,
) -> (HashSet<String>, HashSet<String>, Option<TraceData>) {
    // D-07: fixpoint cap — explicit setting, then env var, then default 1000.
    let max_iter = max_iterations
        .unwrap_or_else(|| {
            std::env::var("RRULES_MAX_CHAIN_ITERATIONS")
                .ok()
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(1000)
        })
        .max(1);

    let mut fired_set: HashSet<String> = HashSet::new();
    let mut evaluated_set: HashSet<String> = HashSet::new();

    // Phase 28 (D-01): trace state — only meaningfully used when
    // `collect_trace` is true. `trace_start` is captured once at loop entry
    // (cheap `Instant::now()`); events are pushed in firing order inside
    // the loop. When `collect_trace` is false, no `TraceEvent` is ever
    // pushed and no `chrono` call is made (SC-3 latency neutrality).
    let trace_start = if collect_trace {
        Some(Instant::now())
    } else {
        None
    };
    let mut trace_events: Vec<TraceEvent> = Vec::new();
    let mut facts_matched: usize = 0;

    // Fixpoint loop: repeat passes until no rule newly fires in a full
    // pass (D-07). Each rule fires at most once per execute() call
    // (fired_set guard) — this bounds the loop at |rules| passes and is
    // the once-per-rule semantic that makes coverage recording truthful
    // (L-06 / Pitfall 6). The max_iter cap is the ultimate safety net
    // for pathological rule sets.
    // Salience decides the order rules are considered in each pass: higher
    // salience first, ties in load order (a stable sort, so the same rule set
    // always fires in the same order). Without this the agenda would simply be
    // the rule vector, and `Rule::salience` would be a field nobody reads.
    let mut firing_order: Vec<usize> = (0..rules.len()).collect();
    firing_order.sort_by(|a, b| rules[*b].salience.cmp(&rules[*a].salience));

    for _pass in 0..max_iter {
        let mut newly_fired_this_pass = false;
        // Agenda groups locked by a `lock_on_active` rule that fired in this
        // pass. A locked group's remaining rules wait for the next pass.
        let mut locked_groups: HashSet<String> = HashSet::new();
        // Index-based iteration so we can mutate `working_facts` while
        // reading `rules` without conflicting borrows.
        for &rule_index in &firing_order {
            let rule = &rules[rule_index];
            let rule_id = rule.id.clone();
            if fired_set.contains(&rule_id) {
                continue;
            }
            if let Some(group) = rule.agenda_group.as_ref() {
                if locked_groups.contains(group) {
                    continue;
                }
            }
            // Evaluate the condition against the current working memory.
            let facts: Vec<&Fact> = working_facts.values().collect();
            evaluated_set.insert(rule_id.clone());
            if rule.condition.evaluate(&facts) {
                // Phase 28 (D-01): emit a TraceEvent at the moment the rule
                // fires — IN FIRING ORDER. Guarded by `collect_trace` so the
                // off path skips the chrono call + String allocation entirely
                // (SC-3). `facts.len()` is the honest count of facts in
                // working memory that the condition matched against (D-01
                // discretion — a light, honest count).
                if collect_trace {
                    facts_matched += facts.len();
                    trace_events.push(TraceEvent {
                        rule_id: rule_id.clone(),
                        timestamp: chrono::Utc::now().to_rfc3339(),
                        event_type: "rule_fired",
                    });
                }
                // Apply actions. Clone the actions Vec so the immutable
                // borrow of `rules` ends before we mutably borrow
                // `working_facts`.
                let actions = rule.actions.clone();
                for action in &actions {
                    apply_action(action, working_facts);
                }
                // `lock_on_active` closes the rule's agenda group for the rest
                // of this pass: the group's other rules are considered next
                // pass, after this rule's actions have been applied.
                if rule.lock_on_active {
                    if let Some(group) = rule.agenda_group.as_ref() {
                        locked_groups.insert(group.clone());
                    }
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
    //
    // Plan 26-02: also record into the `RuleExecutionTracker` singleton
    // (D-03, L-02) at the same call sites so the dead-rules panel and
    // metadata surface (last_fired_at, rule_name) stay populated. The
    // id→name lookup is built once before the loops (Pitfall 4 —
    // evaluated_set/fired_set are HashSet<String> of ids only; Rule.id
    // and Rule.name are pub at rules.rs:18,20).
    let id_to_name: HashMap<&String, &String> = rules.iter().map(|r| (&r.id, &r.name)).collect();
    for id in &evaluated_set {
        CoverageTracking::record_rule_eval(id);
        RuleExecutionTracker::record_evaluation(id, id_to_name[id]);
    }
    for id in &fired_set {
        CoverageTracking::record_rule_fire(id);
        RuleExecutionTracker::record_fire(id, id_to_name[id]);
    }

    // Phase 28 (D-01): build the TraceData payload only when `collect_trace`
    // is true AND at least one rule fired. When `collect_trace` is false,
    // `trace_events` is empty and `trace_start` is None — we return `None`
    // so `EngineResult.trace` stays `None` (SC-3 latency neutrality).
    let trace = if collect_trace && !trace_events.is_empty() {
        let total_duration_ms = trace_start
            .map(|s| s.elapsed().as_secs_f64() * 1000.0)
            .unwrap_or(0.0);
        Some(TraceData {
            events: trace_events,
            total_duration_ms,
            facts_matched,
        })
    } else {
        None
    };

    (fired_set, evaluated_set, trace)
}

/// The working-memory entry for a fact type, chosen deterministically.
///
/// `working_facts` is a `HashMap`, so "the first fact of this type" is
/// hash-order dependent and an action could target a different fact between two
/// runs. Ordering by fact id makes the choice stable: the same working memory
/// always resolves to the same fact.
fn first_of_type(working_facts: &HashMap<String, Fact>, fact_type: &str) -> Option<String> {
    working_facts
        .iter()
        .filter(|(_, fact)| fact.fact_type_id == fact_type)
        .map(|(key, _)| key.clone())
        .min()
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
                first_of_type(working_facts, id)
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
            } else if let Some(key) = first_of_type(working_facts, id) {
                working_facts.remove(&key);
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
