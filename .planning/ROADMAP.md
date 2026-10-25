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
<details>
<summary>✅ v6.0 DMN Level 3 Compliance (Phases 29–35) — SHIPPED 2026-07-10</summary>

- [x] Phase 29: DMN 1.5 Model + XML Parser + Canonical IR (3/3 plans) — completed 2026-07-06
- [x] Phase 30: FEEL Value Types + Type Lattice + Temporal Domain (4/4 plans) — completed 2026-07-07
- [x] Phase 31: FEEL Parser + Evaluator + Built-in Function Library (9/9 plans) — completed 2026-07-08
- [x] Phase 32: Decision-Table Engine + Hit Policies (2/2 plans) — completed 2026-07-09
- [x] Phase 33: Decision Evaluator + DRD Resolution + REST API + Metrics (3/3 plans) — completed 2026-07-09
- [x] Phase 34: Frontend Wiring + Hit-Policy Model Refactor (4 plans) — completed 2026-07-10
- [x] Phase 35: Conformance Verification + Round-trip + Monitoring Close-out (4 plans; conformance gate GO) — completed 2026-07-10

</details>

> **v6.0 phase detail archived:** See `.planning/milestones/v6.0-ROADMAP.md` for full per-phase goals, deliverables, pitfalls, and success criteria.

<details>
<summary>✅ v7.0 Performance + Production Readiness (Phases 36–39) — SHIPPED 2026-07-15</summary>

- [x] Phase 36: Benchmarks + Memory Profiling Gates — Criterion DMN benchmark suite + frozen memory baseline + regression gate script (completed 2026-07-13)
- [x] Phase 37: Parser/Evaluator Optimization + AST Cache Warming — zero-copy DMN XML loader, per-thread FEEL AST cache (completed 2026-07-14)
- [x] Phase 38: Testing Expansion: Fuzzing + Property-Based Tests — cargo-fuzz targets (FEEL parser, decision tables) + proptest invariants (completed 2026-07-14)
- [x] Phase 39: DRD Parallelization + Topological Sort — rayon level-by-level evaluation of independent decisions (completed 2026-07-15)

</details>

> **v7.0 phase detail archived:** See `.planning/milestones/v7.0-ROADMAP.md` for full per-phase goals, deliverables, pitfalls, and success criteria.

<details>
<summary>✅ v8.0 Close TCK Evaluator Gaps (Phases 40–43) — SHIPPED 2026-07-15</summary>

- [x] Phase 40: Numeric Built-in Functions — abs() duration, number() 3-arg, named-param resolution, edge-case tests; all numeric TCK vectors pass (3/3 plans)
- [x] Phase 41: String + List Built-in Functions — sort() comparator lambda, split/concatenate/filter/remove edge cases (1/1 plan)
- [x] Phase 42: Context + Relational + Temporal Built-in Functions — pre-existing implementation verified against TCK; no changes needed
- [x] Phase 43: TCK Conformance Verification + Close-out — 51/51 vectors pass; milestone audit `passed`

</details>

> **v8.0 phase detail archived:** See `.planning/milestones/v8.0-ROADMAP.md` for full per-phase goals, success criteria, scope locks, and outcome. Milestone audit: `.planning/milestones/v8.0-MILESTONE-AUDIT.md`.

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
| 34. Frontend Wiring + Hit-Policy Model Refactor | v6.0 | 4 plans | Complete | 2026-07-10 |
| 35. Conformance Verification + Round-trip + Monitoring Close-out | v6.0 | 4 plans | Complete | 2026-07-10 |
| 36. Benchmarks + Memory Profiling Gates | v7.0 | — | Complete | 2026-07-13 |
| 37. Parser/Evaluator Optimization + AST Cache Warming | v7.0 | — | Complete | 2026-07-14 |
| 38. Testing Expansion: Fuzzing + Property-Based Tests | v7.0 | — | Complete | 2026-07-14 |
| 39. DRD Parallelization + Topological Sort | v7.0 | — | Complete | 2026-07-15 |
| 40. Numeric Built-in Functions | v8.0 | 3/3 plans | Complete | 2026-07-15 |
| 41. String + List Built-in Functions | v8.0 | 1/1 plan | Complete | 2026-07-15 |
| 42. Context + Relational + Temporal Built-in Functions | v8.0 | pre-existing verified | Complete | 2026-07-15 |
| 43. TCK Conformance Verification + Close-out | v8.0 | audit passed | Complete | 2026-07-15 |

---
*Roadmap last updated: 2026-07-15 — Milestone v8.0 COMPLETE (51/51 TCK evaluator vectors pass; zero regression); audit passed.*
