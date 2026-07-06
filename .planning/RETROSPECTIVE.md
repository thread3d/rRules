# Retrospective

Living retrospective. A new section is appended per milestone at ship time.

## Milestone: v4.0 — Production Platform

**Shipped:** 2026-07-02
**Phases:** 6 (17–22) | **Plans:** ~12 | **Timeline:** 22 days, 89 commits

### What Was Built

- Production React/TypeScript frontend — CodeMirror rule IDE, decision-table editor (all DMN hit policies), guided editors, rule-management dashboard
- Enterprise governance backend — RBAC (admin/author/reviewer/viewer) with JWT auth, maker-checker approval workflow, immutable audit trail with CSV/JSON export, Git-backed branching/merging with conflict resolution
- Monitoring & metrics — Prometheus `/metrics`, dashboard API, dead-rule detection, coverage reporting
- Complete management API — rules/categories/tags CRUD, approvals, audit, branches
- Deployment & CI/CD — multi-stage Alpine Docker, Helm chart (dev/staging/prod), hot reload with async job tracking
- Frontend-backend integration closure (Phase 22) — aligned frontend to `/v1`, JWT-protected routes + AuthProvider, mounted orphaned routes, real WebSocket metrics streaming, audit/approval/branch UI wired to backend

### What Worked

- **Phase 22 wiring-first closure pattern:** A single dedicated phase that wired orphaned UI to the substantially-complete backend (configuration + composition + stub-replacement) closed 9 partial requirements in one verified pass — cheaper than re-building.
- **Goal-backward verification:** Phase 22's VERIFICATION.md cited `file:line` evidence against the live codebase rather than SUMMARY claims — caught real integration state and produced an honest 9/9 MET verdict.
- **Single root-cause framing:** Identifying the `/api/v1` vs `/v1` baseURL mismatch as the single root cause focused the closure phase on config/proxy/auth rather than scattered rework.

### What Was Inefficient

- **Stale tracking records at close:** Phase 17's UAT sat as `partial` (pre-Phase-22) and Phases 18/19 VERIFICATION files used the unnumbered `VERIFICATION.md` form the tooling doesn't recognize — both blocked a clean `verified_closeout` until manually normalized. Verification records should be flipped to terminal status (`complete`/`passed`) and use the numbered `{phase}-VERIFICATION.md` filename at phase close, not at milestone close.
- **Roadmap drift:** ROADMAP.md accumulated duplicated/stale `## Progress` sections and inconsistent plan counts across milestones — required judgment to collapse cleanly.
- **No prior tags at ship time:** v1.0/v2.0/v3.0 were never tagged when they shipped — v4.0 was the first tag created at ship time. Backfilled retroactively on 2026-07-02: `v1.0`@8440422a9, `v2.0`@0dcfe1199, `v3.0`@e6c975b03. v3.0 is a retroactive label, not a clean release — its pluggable-engine code landed inside the v4.0 timeline (see Cross-Milestone Trends note).

### Patterns Established

- Numbered verification filename convention: `{phase}-VERIFICATION.md` with YAML frontmatter (`status`, `phase`, `verified`, `verdict`) is the form the tooling recognizes — adopt for all future phases.
- UAT terminal status is `complete` (not `passed`) for the artifact audit to clear.
- When audit finds a phase is a skeleton, a dedicated decimal/wiring closure phase (like Phase 22) is an effective inline remedy.

### Key Lessons

- Run `/gsd-verify-work` (or the verify step) and flip UAT/verification records to terminal at phase completion — deferring it to milestone close is costly.
- Keep ROADMAP.md milestone-grouped as you go (`<details>` collapse at milestone close) to avoid drift accumulating.

### Cost Observations

- Sessions: multi-session across 22 days; significant context spent on multi-track hardening/production-readiness work (2026-07-01) and the Phase 22 closure sprint.
- Notable: the heaviest single spend in this close was reverse-engineering the tooling's phase-state derivation to unblock the verified_closeout — a one-time cost now documented above as a pattern.

---

## Milestone: v5.0 — Live Execution

**Shipped:** 2026-07-06
**Phases:** 6 (23–28) | **Plans:** 20 | **Tasks:** 43 | **Timeline:** 4 days, 157 commits (2026-07-02 → 2026-07-06)

### What Was Built

- Real rule execution — `POST /v1/execute` fires rules via `EngineRegistry` (fresh `Box<dyn ExecutionEngine>` per request in `web::block`), returns truthful `rules_fired`/`rules_not_fired`/`facts_after` from post-execution working memory; the long-standing `rules_fired: 0` stub is closed.
- Fixed the v3.0 `engine::`→`crate::` E0433 import bug across all four engine modules — the pluggable-engine surface compiles for the first time; built a minimal direct rule evaluator on `ReteEngine` (mirrored onto TREAT/LEAPS/PHREAK via a shared `direct_eval` module).
- Live monitoring data — per-execution Prometheus counters + WS `MetricsSnapshot` broadcast (Phase 24); `RuleExecutionTracker` as a `LazyLock<Mutex<…>>` singleton so dead-rule detection reads live firings (Phase 26); three metric surfaces agree.
- Real reload + JobStore (Phase 25) — `SharedRuleSet` swap-don't-mutate with generation counter; async reload writes `ReloadJob` transitions; batch-execute aggregates truthfully.
- Stateful sessions (Phase 27) — `SessionManager` (DashMap, TTL/LRU/max-count/sweep/gauge shipped together).
- Frontend test-runner (Phase 28) — `execution.ts` + `TestRunner` component; engine dropdown; inference trace; `rule_filter` honored server-side (debt fix) for a true "test this rule" UX.

### What Worked

- **Audit → debt-fix → re-audit closeout loop:** The milestone audit surfaced 2 cross-phase WARNINGs (`rule_filter` dead input; `/metrics` HTML/JSON `active_sessions` hardcoded) and classified the milestone `tech_debt`. Fixing both with targeted regression tests (4 new tests; cargo 118/0, vitest 5/5) before tagging produced an honest `passed` re-audit and a clean verified closeout — debt was addressed at the right time (before the tag point-of-no-return), not deferred past it.
- **Wave 0 RED→GREEN (Nyquist) per phase:** Each phase landed RED test scaffolds encoding its ROADMAP success criteria before implementation, then flipped them GREEN — the keystone scenarios (score+10, dead-rule exclusion, session accumulation) carried the verification, not prose.
- **Singleton-with-`#[serial]`-and-`clear()` pattern:** `CoverageTracking` and `RuleExecutionTracker` as `LazyLock<Mutex<…>>` singletons with `clear()` for test isolation gave live cross-request state without flaky global-mutation tests.
- **`Cow<'a, RuleSet>` + `Option<web::Data<…>>` for backward-compatible debt fixes:** Zero-clone on the hot path while honoring `rule_filter`; `Option` extractor kept minimal test apps working without forcing every test to install a `SessionManager`.

### What Was Inefficient

- **`init.manager` projection field drift:** The complete-milestone workflow's jq readiness filter assumed `phase_complete`/`verification_status` fields, but this gsd-tools build exposes `disk_status` — so `all_phases_verified` computed `false` (false negative) and had to be re-derived from `disk_status=="complete"` + matching plan/summary counts. A `verified_closeout` was still achievable, but only after inspecting the raw projection. The workflow's jq filter should match the projection's actual field names.
- **`models/execution.rs` legacy duplicate:** A second `ExecuteRequest` struct (with `rule_filter`) sat fully dead in the codebase, discovered only during the WARNING-1 fix. Should have been removed when the live `api::v1::execution::ExecuteRequest` became canonical — left as a cleanup item.
- **SUMMARY frontmatter gaps:** Phases 24/25/28 shipped with empty `requirements_completed` frontmatter, forcing manual 3-source cross-referencing in the audit. The per-phase transition should populate frontmatter at close, not leave it for the milestone audit to backfill.

### Patterns Established

- **Audit-gated closeout:** A milestone audit with explicit `tech_debt`/`gaps_found`/`passed` routing — and a debt-fix → re-audit loop before tagging — is now the closeout discipline. Don't tag over a known contract mismatch.
- **`Option<web::Data<T>>` extractor for optional production dependencies:** Lets a handler read live state when the full app stack is installed (production) while degrading cleanly in minimal test apps — the backward-compatible way to wire a singleton into an existing handler without breaking tests.
- **`Cow` for filter/derive helpers on shared hot-path data:** `Borrowed` for the common no-op case, `Owned` only when actually transforming — the shape for zero-clone conditional processing of `Arc<RwLock<…>>`-guarded shared data.

### Key Lessons

- The milestone audit is where cross-phase contract mismatches (a frontend claim the backend doesn't honor; inconsistent metric surfaces) surface — run it before tagging, and treat `tech_debt` as "fix the v5.0-era items, then re-audit," not "tag over it."
- Verify against the live projection's field names, not the workflow template's assumed ones — a `false` readiness result that contradicts the ROADMAP/audit evidence is a projection mismatch, not a blocker.

### Cost Observations

- 4 days, 157 commits for 6 phases / 20 plans / 43 tasks — the heaviest phases were 23 (build + wire the evaluator) and 27 (SessionManager with all four OOM controls).
- Notable: the audit's 2 cross-phase WARNINGs were both concentrated in the integration layer (frontend↔backend contract; metric-surface consistency) — exactly where a per-phase verification can't see. The milestone audit earned its keep there.

---

## Cross-Milestone Trends

| Milestone | Phases | Days | Commits | Closeout | Tagged |
|-----------|--------|------|---------|----------|-------|
| v1.0 | 6 | — | — | (retro) | yes (backfilled @8440422a9, 2026-06-01) |
| v2.0 | 3 | — | — | (retro) | yes (backfilled @0dcfe1199, 2026-06-08) |
| v3.0 | 1 | — | — | (retro) | yes (backfilled @e6c975b03, 2026-06-11 — retroactive label¹) |
| v4.0 | 6 | 22 | 89 | verified | yes (@ee4683314a, 2026-07-02) |
| v5.0 | 6 | 4 | 157 | verified (audit → debt-fix → re-audit `passed`) | yes (2026-07-06) |

¹ v3.0 was never closed as a separate git milestone — STATE.md transitions directly from v2.0/complete to v4.0, and there is no Phase-10 or v3.0 milestone-completion commit. The pluggable-engine code (TREAT/LEAPS/PHREAK/selector/registry, ~2084 LOC) first landed inside the v4.0 timeline at commit `e6c975b03` (a Phase-21 deployment commit). The `v3.0` tag is a backfilled historical ref to where that deliverable materialized, not a clean release commit.

---
*Last updated: 2026-07-06 after v5.0 milestone (Live Execution — audit → debt-fix → re-audit `passed` closeout)*