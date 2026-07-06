# Phase 23: Execute evaluator + wiring - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-07-02
**Phase:** 23-execute-evaluator-wiring
**Areas discussed:** Non-Rete engines, RuleSet source, Evaluator scope, Action-expression depth, Forward-chaining semantics, facts_after contract (defaulted)

---

## Non-Rete engines

| Option | Description | Selected |
|--------|-------------|----------|
| Register 4, stub 3 | Register all 4 factories (truthful available_engines()); implement the direct-evaluator only on ReteEngine; treat/leaps/phreak → HTTP 400 "not supported for execution in this release". Smallest honest surface; matches what success criteria test. (Research Open Q#2 recommendation.) | |
| Implement all 4 | Implement the direct-evaluator on all four engines (treat/leaps/phreak also actually fire). engine: works for all 4 in Phase 23, but ~4× the implementation + test burden. | ✓ |
| Register Rete only | Register only Rete (available_engines() = ["rete"]); add the other 3 in a later phase. Smallest, but EngineSelector::with_defaults() already advertises all 4 names — a truthful mismatch. | |

**User's choice:** Implement all 4
**Notes:** User chose to take on the fuller build so all four engines actually fire, not just Rete.

### Test depth for the non-Rete engines

| Option | Description | Selected |
|--------|-------------|----------|
| Test all 4 equally | Each of the 4 engines gets its own score+10 integration test proving it actually fires (4 tests, same fixture). Strongest guarantee that "implement all 4" is honest, not just compile-clean. | ✓ |
| Rete full + others smoke | Rete gets the full integration suite (success criteria #1-4); treat/leaps/phreak get one shared smoke test cycling the same fixture through all three engine names. Faster; trusts the shared evaluator logic. | |
| You decide | Let the researcher/planner judge the test split based on how much the 4 evaluators end up sharing. | |

**User's choice:** Test all 4 equally
**Notes:** Each engine gets its own score+10 integration test.

---

## RuleSet source

| Option | Description | Selected |
|--------|-------------|----------|
| Load from disk now (live) | At startup, load real rules from RULE_STORAGE_PATH into a shared read-only Arc<RwLock<RuleSet>> (built once). After Phase 23, /v1/execute runs against the user's actual authored rules. Reload-swap lands in Phase 25 (this snapshot is the scaffold Phase 25 swaps). | ✓ |
| Test-fixture only | Server runs no rules in Phase 23; the integration test constructs a RuleSet in-test via RuleSet::add_rule. /v1/execute against a running server fires nothing until Phase 25 wires disk loading + sharing. Smaller Phase 23, but doesn't deliver a live system end-to-end. | |
| Disk + fallback seed | Load from disk now AND keep a minimal built-in fallback ruleset (the score+10 rule) seeded when the rules directory is empty — so success criterion #1 passes even on a fresh install with no authored rules. | |

**User's choice:** Load from disk now (live)
**Notes:** Shared read-only Arc<RwLock<RuleSet>> built once at startup; this is the scaffold Phase 25 swaps.

### Empty/missing rules directory at startup

| Option | Description | Selected |
|--------|-------------|----------|
| Warn and continue | Start with an empty RuleSet and log a warning. Server comes up; /v1/execute works but fires nothing until rules are authored. Consistent with the live-system choice. | ✓ |
| Fail-fast at startup | Refuse to start if the rules directory is missing or empty. Surfaces misconfiguration early, but a fresh install with no authored rules cannot boot. | |
| You decide | Let the planner judge warn-and-continue vs fail-fast based on how RULE_STORAGE_PATH is already used. | |

**User's choice:** Warn and continue
**Notes:** Start with an empty RuleSet + log a warning; server comes up.

---

## Evaluator scope

| Option | Description | Selected |
|--------|-------------|----------|
| Full rewrite in P23 | Build the full direct-interpreter rewrite of ReteEngine execute_rules in Phase 23 — evaluate conditions, apply actions, record coverage once per fired rule. Meets success criterion #1 now. Keeps the v5.0 vertical slice honest. Matches roadmap. | ✓ |
| Descope rewrite to P25 | Descope the rewrite to Phase 25. Ship Phase 23 as pure registry wiring with execute_rules still a placeholder (rules_fired: 0 from a real engine). Would require revising success criterion #1 to Phase 25. | |
| Split: minimal now, rest P25 | Phase 23 ships wiring + a minimal single-rule-path evaluator (enough for success criterion #1 with the score+10 fixture only); general multi-rule Pattern matching deferred to Phase 25. | |

**User's choice:** Full rewrite in P23
**Notes:** Confirms the keystone scope; matches the roadmap's build (not wire-only) correction.

### Condition matching richness

| Option | Description | Selected |
|--------|-------------|----------|
| Full ops and value types | Implement the full ComparisonOp enum across Integer, Float, String, Boolean. Real authored rules with any comparator work honestly. Recommended, since live-from-disk rules will hit this. | ✓ |
| Equality-only | Equality-only on Integer, Float, String — enough for the score+10 fixture and simple rules. Other comparators silently misfire on authored rules until a later phase. | |
| You decide | Let the researcher/planner decide the operator set based on what the score+10 fixture and a representative sample of authored rules require. | |

**User's choice:** Full ops and value types
**Notes:** Research correction — Condition::evaluate today only handles Eq on Integer/Float/String; everything else falls to false. "Full ops" is a real build, not a tweak.

---

## Action-expression depth (research correction)

| Option | Description | Selected |
|--------|-------------|----------|
| Minimal arithmetic | Computed values limited to + - * / over numeric fact fields and literals, assignable in UpdateFact. Meets score := score + 10 and counter rules. Smallest honest build meeting success criterion #1. | |
| Full expression language | Arithmetic, string concatenation, comparisons, nested expressions, plus wiring the existing CallFunction action. Matches the v1.0 RULE-05 promise so all real authored rules work honestly on the live path. Substantially more build. | ✓ |
| You decide | Let the researcher/planner pick the minimal expression set meeting the fixture plus a sample of real authored rules, documenting what is deferred. | |

**User's choice:** Full expression language
**Notes:** RESEARCH CORRECTION — discovered during discussion: the engine has no Expression type today; Action::UpdateFact only accepts literal Value. So `score := score + 10` is not representable, and success criterion #1 cannot pass without building a structured Expression evaluator. This is a real scope addition the research under-stated. AskUserQuestion tool repeatedly failed for this question; the user answered "b" in plain text after Claude presented A/B/C with a recommendation.

---

## Forward-chaining semantics

| Option | Description | Selected |
|--------|-------------|----------|
| Single pass | Each applicable rule fires once; later rules in the pass see earlier mutations, but no rule re-fires. Meets the score+10 fixture. No cycle risk. Defer fixpoint chaining to a later phase. | |
| Fixpoint with cap | Iterate to a fixpoint with a max-iterations safety cap (config knob): true forward chaining on the live path so rule A can enable rule B. Does not re-implement no-loop/lock-on-active (v2.0 territory) — just the cap. More work. | ✓ |
| You decide | Let the planner pick single-pass vs fixpoint based on whether any Phase 23 test exercises multi-rule chaining. | |

**User's choice:** Fixpoint with cap
**Notes:** Claude recommended single-pass (success criteria don't require chaining; live path is brand-new so single-pass isn't a regression; fixpoint pulls in v2.0 cycle-guard semantics), but the user chose the fuller fixpoint option to make the live path chain honestly, consistent with their posture across the discussion. AskUserQuestion tool failed for this question; user answered "b" in plain text.

---

## facts_after contract (defaulted — no full discussion)

| Option | Description | Selected |
|--------|-------------|----------|
| Keep existing shape (default) | Serialize facts_after from get_facts() via reverse engine_value_to_json, grouped by fact_type_id, into the existing { fact_type: json_value } map. Frontend-compatible (Phase 28 consumes it); avoids unplanned frontend shape changes. | ✓ |

**User's choice:** (defaulted — Claude recommended locking the default; user did not object)
**Notes:** No real fork worth user time — the obvious answer is frontend-compatible. Recorded as D-08 in CONTEXT.md; revisit only if the planner finds the existing shape cannot carry post-execution facts faithfully.

---

## Claude's Discretion

The following technical implementation details were explicitly left to the researcher/planner (not user-facing):
- **EngineSelector sharing model** — set default engine once at startup (mutably), use `&self` `available_engines()` validation per request; avoid `Mutex` unless per-request mutation is needed. (Research Open Q#3.)
- **`rules_not_fired` exposure mechanism** — extend `EngineResult` with `fired_rule_ids: Vec<String>` (research recommendation) vs. a new trait method. Planner to confirm. (Research Open Q#5.)
- **`Condition::evaluate` Pattern internals** — how Pattern { fact_type, field, value } matches working_facts. Part of the rewrite. (Research Open Q#6.)
- **Fixpoint cap default value + env var name** — pick a sensible default (~1000) and config knob name. (D-07.)
- **How the four evaluators share code** — shared helper/trait method vs. mirror per-engine. Architectural choice.

## Deferred Ideas

- Full Rete alpha/beta network propagation / `RuleSet → ReteNetwork` compiler — future enhancement behind the same `load_ruleset`/`get_facts` contract.
- `inference_trace` population — EXEC-04 / Phase 28.
- RuleSet reload-then-swap — Phase 25.
- `no-loop` / `lock-on-active` cycle-control semantics — v2.0 testing-framework territory (Phase 23 uses only a max-iterations cap).
- `RuleExecutionTracker` singleton-ization + dead-rule live data — Phase 26.
- Live Prometheus + WebSocket counter broadcast — Phase 24.

(No scope-creep suggestions from the user — these are the phase's own downstream milestones, listed to mark the boundaries.)