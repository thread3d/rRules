# Roadmap: rRules - Business Rules Management System

## Overview

A production-grade Business Rules Management System (BRMS) in Rust: define complex business decisions as reusable rules, deploy them instantly, and monitor their impact without modifying application code. The roadmap progressed from core engine → authoring interface → REST API → governance → testing → DMN compliance → pluggable engines → production platform → live execution → DMN Level 3 compliance.

> **Milestone archives:** Full per-phase detail for completed milestones is preserved in `.planning/milestones/v{X.Y}-ROADMAP.md`.

## Milestones

- ✅ **v1.0 MVP** — Phases 1–6 (shipped 2026-06-07) — Core engine, authoring, REST API, governance, testing, DMN
- ✅ **v2.0 Testing Framework** — Phases 7–9 (shipped 2026-06-07) — Testing infrastructure, advanced features, enhancements
- ✅ **v3.0 Pluggable Engines** — Phase 10 (shipped 2026-06-xx) — Rete/TREAT/LEAPS/PHREAK with runtime selection
- ✅ **v4.0 Production Platform** — Phases 17–22 (shipped 2026-07-02) — Full frontend, governance, monitoring, management API, deployment
- ✅ **v5.0 Live Execution** — Phases 23–28 (shipped 2026-07-06) — Real rule evaluator + `/v1/execute` wiring; auto-populated trackers + metrics so v4.0 monitoring surfaces read live data; stateful sessions + frontend test-runner. Full per-phase detail archived in `milestones/v5.0-ROADMAP.md`.
- ✅ **v6.0 DMN Level 3 Compliance** — Phases 29–35 (shipped 2026-07-10) — Real DMN parser, FEEL evaluator, decision-table engine with all hit policies, REST API + metrics, frontend wiring; closes stubbed Phase 6 gap. [Archive](milestones/v6.0-ROADMAP.md)
- ✅ **v7.0 Performance + Production Readiness** — Phases 36–39 (shipped 2026-07-15) — Benchmarks + memory profiling gates, parser/evaluator optimization with AST cache warming, DRD parallelization with topological sort, testing expansion with fuzzing and property-based tests. [Archive](milestones/v7.0-ROADMAP.md)
- 🚧 **v8.0 Close TCK Evaluator Gaps** — Phases 40–43 (in progress) — Implement missing FEEL built-in functions to raise TCK evaluator coverage from 21/51 to 51/51 vectors. Numeric, string, list, context, relational, and temporal built-in function families, followed by conformance verification and milestone close-out.

## Phases

<details>
<summary>✅ v1.0 MVP (Phases 1–6) — SHIPPED 2026-06-07</summary>

- [x] Phase 1: Core Engine Foundation (6/6 plans) — completed 2026-05-23
- [x] Phase 2: Rule Authoring Interface (5/5 plans) — completed 2026-06-01
- [x] Phase 3: REST API Layer (6/6 plans) — completed 2026-06-07
- [x] Phase 4: Governance & Access Control (5/5 plans) — completed 2026-06-02
- [x] Phase 5: Testing & Monitoring (6/6 plans, stub) — completed 2026-06-01
- [x] Phase 6: DMN Compliance (3/3 plans) — completed 2026-06-01

</details>

<details>
<summary>✅ v2.0 Testing Framework (Phases 7–9) — SHIPPED 2026-06-07</summary>

- [x] Phase 7: Core Testing Infrastructure (6/6 plans) — completed 2026-06-07
- [x] Phase 8: Advanced Testing Features (5/5 plans) — completed 2026-06-07
- [x] Phase 9: Testing Framework Enhancements (4/5 plans; 1 deferred) — completed 2026-06-07

</details>

<details>
<summary>✅ v3.0 Pluggable Engines (Phase 10) — SHIPPED</summary>

- [x] Phase 10: Pluggable Engine Architecture (Rete/TREAT/LEAPS/PHREAK + runtime selection) — completed; dashboard deferred to v4.0

</details>

<details>
<summary>✅ v4.0 Production Platform (Phases 17–22) — SHIPPED 2026-07-02</summary>

- [x] Phase 17: Production Frontend Architecture (React/TS IDE, decision tables, dashboard)
- [x] Phase 18: Enterprise Governance (RBAC + JWT, approval workflow, audit trail, Git versioning)
- [x] Phase 19: Monitoring & Metrics (Prometheus /metrics, dashboards, dead-rule detection, coverage)
- [x] Phase 20: Complete Management API (rules/categories/tags/approvals/audit/branches)
- [x] Phase 21: Deployment & CI/CD (Alpine Docker, Helm chart, hot reload)
- [x] Phase 22: Close v4.0 frontend integration gaps — wire orphaned UI to backend (9/9 requirements MET, verified PASS)

</details>

<details>
<summary>✅ v5.0 Live Execution (Phases 23–28) — SHIPPED 2026-07-06</summary>

- [x] **Phase 23: Execute evaluator + wiring** — Fix the v3.0 engine import bug, build a minimal real rule evaluator on `ReteEngine`, and route `/v1/execute` through `EngineRegistry`; factory shared, engine instance per request (completed 2026-07-03)
- [x] **Phase 24: Execution counters + metrics broadcast** — `ExecutionCounters` + Prometheus promote + WS broadcast; dashboards go live (completed 2026-07-03)
- [x] **Phase 25: Real reload + JobStore** — Shared reloadable `RuleSet` snapshot; async reload writes `ReloadJob`; batch inherits real execution (completed 2026-07-03)
- [x] **Phase 26: RuleExecutionTracker singleton + dead-rule live data** — Singleton-ize tracker; dead-rules reads live firings; pin with e2e test (completed 2026-07-03)
- [x] **Phase 27: Stateful sessions** — `SessionManager` (DashMap, TTL, LRU, sweep); stateful `/v1/execute/session/{id}` endpoints (completed 2026-07-06)
- [x] **Phase 28: Frontend test-runner** — `execution.ts` + `TestRunner` component; engine selector dropdown; inference trace surfaced (completed 2026-07-06)

</details>

> **v5.0 phase detail archived:** See `.planning/milestones/v5.0-ROADMAP.md` for full per-phase goals, deliverables, pitfalls, and success criteria.

- ✅ **v6.0 DMN Level 3 Compliance** — Phases 29–35 (shipped 2026-07-10) — Real DMN parser, FEEL evaluator, decision-table engine with all hit policies, REST API + metrics, frontend wiring; closes stubbed Phase 6 gap. [Archive](milestones/v6.0-ROADMAP.md)
- ✅ **v7.0 Performance + Production Readiness** — Phases 36–39 (shipped 2026-07-15) — Benchmarks + memory profiling gates, parser/evaluator optimization with AST cache warming, DRD parallelization with topological sort, testing expansion with fuzzing and property-based tests. [Archive](milestones/v7.0-ROADMAP.md)
- 🚧 **v8.0 Close TCK Evaluator Gaps** — Phases 40–43 (in progress) — Implement missing FEEL built-in functions to raise TCK evaluator coverage from 21/51 to 51/51 vectors. Numeric, string, list, context, relational, and temporal built-in function families, followed by conformance verification and milestone close-out.

**Milestone Goal:** Raise TCK evaluator coverage from 21/51 to 51/51 vectors by implementing the missing FEEL built-in functions across numeric, string, list, context, relational, and temporal families, then verify conformance and close out the milestone.

**Scope locks (user decisions):**

- v8.0 is a focused built-in-function gap-closing milestone — no new DMN features, no frontend changes, no new API endpoints.
- All 30 vectors that already parse and round-trip cleanly must continue to do so (zero regression).
- Each new built-in function must handle null propagation per DMN 1.5 Section 10.3.4.
- The existing Rete rule-execution path (`/v1/execute`, `EngineRegistry`) is unchanged — zero regression.
- Phase numbering continues from 40 (v7.0 ended at Phase 39).

- [ ] **Phase 40: Numeric Built-in Functions** — Implement abs, sqrt, exp, log, even/odd, modulo, number(), median/mode/stddev, product, and arithmetic edge cases (FEEL-01..FEEL-10)
- [ ] **Phase 41: String + List Built-in Functions** — Implement split, filter, concatenate, insert/remove, sort/list contains, all/any, and singleton/list semantics (FEEL-11..FEEL-17)
- [ ] **Phase 42: Context + Relational + Temporal Built-in Functions** — Implement context construction/navigation, get value, get entries, deep equality, between, temporal built-ins, and remaining families (FEEL-18..FEEL-24)
- [ ] **Phase 43: TCK Conformance Verification + Close-out** — Full 51/51 TCK evaluator sweep, milestone audit, tag v8.0 (FEEL-25, FEEL-26)

## Phase Details

### Phase 40: Numeric Built-in Functions

**Goal**: All 14 numeric TCK vectors pass the evaluator layer — users can use abs, sqrt, exp, log, even/odd, modulo, number(), median/mode/stddev, and product in FEEL expressions with correct DMN 1.5 semantics.
**Depends on**: Phase 31 (FEEL evaluator core + built-in function registry), Phase 30 (FEEL value types + Decimal128 numeric domain)
**Requirements**: FEEL-01, FEEL-02, FEEL-03, FEEL-04, FEEL-05, FEEL-06, FEEL-07, FEEL-08, FEEL-09, FEEL-10
**Success Criteria** (what must be TRUE):

  1. `abs(number)` returns the absolute value for Integer and Number inputs, and propagates null — TCK vector `0050-feel-abs-function` passes.
  2. `sqrt(number)` returns the square root for positive inputs and null for negative inputs — TCK vector `0051-feel-sqrt-function` passes.
  3. `exp(number)` and `log(number)` compute correctly; `log` returns null for non-positive inputs — TCK vectors `0052-feel-exp-function` and `0053-feel-log-function` pass.
  4. `even(number)` / `odd(number)`, `modulo(dividend, divisor)`, `number(from, grouping, decimal)`, `median(list)`, `mode(list)`, `stddev(list)`, and `product(list)` all return correct results per DMN 1.5 Section 10.3.4.1 — TCK vectors `0054`–`0056`, `0058`, `0061`–`0063`, `0094` pass.
  5. Arithmetic edge cases (empty-list SUM/MIN/MAX returns null, negation edge cases) are handled correctly — TCK vectors `0100-arithmetic` and `0099-arithmetic-negation` pass.

**Plans**: TBD

### Phase 41: String + List Built-in Functions

**Goal**: All string and list TCK vectors pass the evaluator layer — users can use split, filter, concatenate, insert/remove, sort/list contains, all/any, and singleton/list semantics in FEEL expressions with correct DMN 1.5 semantics.
**Depends on**: Phase 40 (numeric built-in functions establish the built-in registry pattern)
**Requirements**: FEEL-11, FEEL-12, FEEL-13, FEEL-14, FEEL-15, FEEL-16, FEEL-17
**Success Criteria** (what must be TRUE):

  1. `split(string, delimiter)` returns a list of substrings per DMN 1.5 Section 10.3.4.2 — TCK vector `0067-feel-split-function` passes.
  2. `filter(list, function)` returns a filtered list using a FEEL lambda/function as the second argument — TCK vector `0001-filter` passes.
  3. `concatenate(list1, list2, ...)` returns a concatenated list — TCK vector `0010-concatenate` passes.
  4. `insert(list, position, element)` and `remove(list, position)` return new lists with correct element manipulation — TCK vector `0011-insert-remove` passes.
  5. `sort(list)` / `sort(list, function)` and `list contains(list, element)` return correct ordering and membership results — TCK vectors `0013-sort` and `0069-feel-list` pass.
  6. `all(list)` and `any(list)` return correct quantifier results — TCK vectors `0059-feel-all-function` and `0060-feel-any-function` pass.
  7. Singleton lists and single elements behave correctly per DMN 1.5 singleton/list semantics — TCK vector `0021-singleton-list` passes.

**Plans**: TBD

### Phase 42: Context + Relational + Temporal Built-in Functions

**Goal**: All context, relational, and temporal TCK vectors pass the evaluator layer — users can use context construction/navigation, get value, get entries, deep equality, between, and temporal built-in functions in FEEL expressions with correct DMN 1.5 semantics.
**Depends on**: Phase 41 (string/list built-in functions establish the pattern for context/temporal families)
**Requirements**: FEEL-18, FEEL-19, FEEL-20, FEEL-21, FEEL-22, FEEL-23, FEEL-24
**Success Criteria** (what must be TRUE):

  1. Context literal evaluation with proper scoping works correctly — TCK vector `0057-feel-context` passes.
  2. `get value(context, key)` returns the value for a given key in a context — TCK vector `0080-feel-getvalue-function` passes.
  3. `get entries(context)` returns a list of key-value entries from a context — TCK vector `0081-feel-getentries-function` passes.
  4. Deep equality (`=` / `!=`) works correctly across all FEEL types including context, list, and temporal types — TCK vector `0068-feel-equality` passes.
  5. `between(value, start, end)` returns true when value is in the range [start, end] — TCK vector `0071-feel-between` passes.
  6. Temporal built-in functions (`date()`, `time()`, `date and time()`, `duration()`) handle string-to-temporal parsing and temporal arithmetic correctly — temporal family TCK vectors pass.
  7. All remaining Section 10.3.4 built-in functions not covered in Phases 40-41 are implemented — all 51 TCK vectors have evaluator-layer passes.

**Plans**: TBD

### Phase 43: TCK Conformance Verification + Close-out

**Goal**: All 51 TCK vectors pass the evaluator layer, the milestone is audited, and v8.0 is tagged — users can rely on complete FEEL built-in function coverage verified against the OMG DMN TCK.
**Depends on**: Phases 40, 41, 42 (all built-in functions must be implemented before the final sweep)
**Requirements**: FEEL-25, FEEL-26
**Success Criteria** (what must be TRUE):

  1. `cargo test --test tck_sweep` reports 51/51 vectors passing all three layers (parser, round-trip, evaluator) — zero regressions in the 30 vectors that already pass parser and round-trip.
  2. Any remaining known divergences are documented with spec references in TCK-COVERAGE-GAPS.md.
  3. TCK-COVERAGE-GAPS.md is updated to reflect 51/51 evaluator coverage.
  4. VERIFICATION.md is written for all v8.0 phases (40-43) documenting what was delivered and how it was verified.
  5. Milestone audit passes and v8.0 is tagged.

**Plans**: TBD

## Progress

**Execution Order:**
Phases execute in numeric order: 40 → 41 → 42 → 43.

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 1. Core Engine Foundation | v1.0 | 6/6 | Complete | 2026-05-23 |
| 2. Rule Authoring Interface | v1.0 | 5/5 | Complete | 2026-06-01 |
| 3. REST API Layer | v1.0 | 6/6 | Complete | 2026-06-07 |
| 4. Governance & Access Control | v1.0 | 5/5 | Complete | 2026-06-02 |
| 5. Testing & Monitoring | v1.0 | 6/6 | Complete (stub) | 2026-06-01 |
| 6. DMN Compliance | v1.0 | 3/3 | Complete | 2026-06-01 |
| 7. Core Testing Infrastructure | v2.0 | 6/6 | Complete | 2026-06-07 |
| 8. Advanced Testing Features | v2.0 | 5/5 | Complete | 2026-06-07 |
| 9. Testing Framework Enhancements | v2.0 | 4/5 | Complete (1 deferred) | 2026-06-07 |
| 10. Pluggable Engine Architecture | v3.0 | — | Complete | 2026-06-xx |
| 17. Production Frontend | v4.0 | — | Complete | 2026-07-02 |
| 18. Enterprise Governance | v4.0 | — | Complete | 2026-07-02 |
| 19. Monitoring & Metrics | v4.0 | — | Complete | 2026-07-02 |
| 20. Complete Management API | v4.0 | — | Complete | 2026-07-02 |
| 21. Deployment & CI/CD | v4.0 | — | Complete | 2026-07-02 |
| 22. Frontend Integration Closure | v4.0 | 6/6 | Complete (verified PASS) | 2026-07-02 |
| 23. Execute evaluator + wiring | v5.0 | 6/6 | Complete | 2026-07-03 |
| 24. Execution counters + metrics broadcast | v5.0 | 3/3 | Complete | 2026-07-03 |
| 25. Real reload + JobStore | v5.0 | 3/3 | Complete | 2026-07-03 |
| 26. RuleExecutionTracker singleton + dead-rule live data | v5.0 | 2/2 | Complete | 2026-07-03 |
| 27. Stateful sessions | v5.0 | 1/1 | Complete | 2026-07-06 |
| 28. Frontend test-runner | v5.0 | 5/5 | Complete | 2026-07-06 |
| 29. DMN 1.5 Model + XML Parser + Canonical IR | v6.0 | 3/3 | Complete | 2026-07-06 |
| 30. FEEL Value Types + Type Lattice + Temporal Domain | v6.0 | 4/4 | Complete | 2026-07-07 |
| 31. FEEL Parser + Evaluator + Built-in Function Library | v6.0 | 9/9 | Complete | 2026-07-08 |
| 32. Decision-Table Engine + Hit Policies | v6.0 | 2/2 | Complete | 2026-07-09 |
| 33. Decision Evaluator + DRD Resolution + REST API + Metrics | v6.0 | 3/3 | Complete | 2026-07-09 |
| 34. Frontend Wiring + Hit-Policy Model Refactor | v6.0 | 0/TBD | Not started | - |
| 35. Conformance Verification + Round-trip + Monitoring Close-out | v6.0 | 0/TBD | Not started | - |
| 36. Benchmarks + Memory Profiling Gates | v7.0 | — | Complete | 2026-07-13 |
| 37. Parser/Evaluator Optimization + AST Cache Warming | v7.0 | — | Complete | 2026-07-14 |
| 38. Testing Expansion: Fuzzing + Property-Based Tests | v7.0 | — | Complete | 2026-07-14 |
| 39. DRD Parallelization + Topological Sort | v7.0 | — | Complete | 2026-07-15 |
| 40. Numeric Built-in Functions | v8.0 | 0/TBD | Not started | - |
| 41. String + List Built-in Functions | v8.0 | 0/TBD | Not started | - |
| 42. Context + Relational + Temporal Built-in Functions | v8.0 | 0/TBD | Not started | - |
| 43. TCK Conformance Verification + Close-out | v8.0 | 0/TBD | Not started | - |

---
*Roadmap last updated: 2026-07-15 — v8.0 Close TCK Evaluator Gaps roadmap added (Phases 40–43); v7.0 per-phase detail archived in `milestones/v7.0-ROADMAP.md`*
