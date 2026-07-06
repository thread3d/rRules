# Changelog

All notable changes to rRules are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/).

> Note on early tags: `v1.0`–`v3.0` were backfilled retroactively (see
> `.planning/RETROSPECTIVE.md` "Cross-Milestone Trends"). `v3.0` in particular
> is a historical ref, not a clean release commit — its pluggable-engine code
> landed inside the v4.0 timeline. Entries below begin at v4.0, the first
> milestone tagged at ship time.

## [Unreleased]

### Housekeeping
- Updated stale `CLAUDE.md` to reflect v5.0 (was a full milestone behind, still
  describing v4.0 as current with v5.0 as a "candidate thesis").
- Removed the dead `api/src/models/` module (duplicate `ExecuteRequest`/
  `ExecuteResponse`/`InferenceTraceEntry`/`RuleFilter`/`TraceSummary` plus
  duplicate `rules` DTOs and a `coverage.rs`). The canonical types live in
  `api::v1::execution` and `api::v1::rules`; nothing in the workspace imported
  `models::`. (v5.0 retrospective flagged the `execution.rs` half; the whole
  module was equally dead.)
- Deleted a stray bare git repository left in `rules/` (untracked, no content).
- Replaced stale `// pub mod auth;` / `// pub mod tenants;` TODO comments in
  `api/src/v1/mod.rs` with accurate pointers (auth is at `crate::auth` +
  `crate::middleware::auth`; tenants route module not yet wired).
- Added `.planning/TECH-DEBT.md` recording the three engine retraction
  `TODO`s (Rete/TREAT/LEAPS network cleanup) and the multi-tenancy route gap as
  known debt, intentionally deferred from this pass.

### Verification
- `cargo test --workspace`: all binaries passing, 0 failed (~353 tests).
- `cargo clippy --workspace --all-targets`: 0 errors (4 pre-existing warnings).
- `npx vitest run`: 5/5 passing.

## [v5.0] - 2026-07-06 — Live Execution

### Added
- **Real rule execution** — `POST /v1/execute` fires rules through
  `EngineRegistry` (fresh `Box<dyn ExecutionEngine>` per request in
  `web::block`) and returns truthful `rules_fired`/`rules_not_fired`/
  `facts_after` from post-execution working memory. Closes the long-standing
  `rules_fired: 0` stub.
- **Live monitoring data** — per-execution Prometheus counters + WebSocket
  `MetricsSnapshot` broadcast (Phase 24); `RuleExecutionTracker` as a
  `LazyLock<Mutex<…>>` singleton so dead-rule detection reads live firings
  (Phase 26). Three metric surfaces (Prometheus, dashboard, dead-rule) agree.
- **Real reload + JobStore** (Phase 25) — `SharedRuleSet` swap-don't-mutate with
  a generation counter; async reload writes `ReloadJob` transitions;
  batch-execute aggregates truthfully.
- **Stateful sessions** (Phase 27) — `SessionManager` (DashMap sharded
  registry) with TTL, LRU eviction, max-count, background sweep, and a gauge
  shipped together; stateful `POST /v1/execute/session/{id}` endpoints.
- **Frontend test-runner** (Phase 28) — `execution.ts` + `TestRunner` component;
  engine-selector dropdown; inference trace surfaced; `rule_filter` honored
  server-side for a true "test this rule" UX.

### Fixed
- v3.0 `engine::`→`crate::` E0433 import bug across all four engine modules —
  the pluggable-engine surface compiles for the first time.
- Minimal direct rule evaluator built on `ReteEngine` and mirrored onto
  TREAT/LEAPS/PHREAK via a shared `direct_eval` module.
- Milestone-audit cross-phase WARNINGs (frontend `rule_filter` dead input;
  `/metrics` `active_sessions` hardcoded) resolved with regression tests before
  tagging.

## [v4.0] - 2026-07-02 — Production Platform

### Added
- **Production React/TypeScript frontend** — CodeMirror rule IDE, decision-table
  editor (all DMN hit policies), guided editors, rule-management dashboard.
- **Enterprise governance backend** — RBAC (admin/author/reviewer/viewer) with
  JWT auth, maker-checker approval workflow, immutable audit trail with
  CSV/JSON export, Git-backed branching/merging with conflict resolution.
- **Monitoring & metrics** — Prometheus `/metrics`, dashboard API, dead-rule
  detection, coverage reporting.
- **Complete management API** — rules/categories/tags CRUD, approvals, audit,
  branches.
- **Deployment & CI/CD** — multi-stage Alpine Docker, Helm chart
  (dev/staging/prod), hot reload with async job tracking.
- **Frontend-backend integration closure** (Phase 22) — aligned frontend to
  `/v1`, JWT-protected routes + AuthProvider, mounted orphaned routes, real
  WebSocket metrics streaming, audit/approval/branch UI wired to backend.

<!-- Link references will be populated once a git remote is configured
     (currently local-only — see CLAUDE.md "Next Steps" #2). -->
[Unreleased]: #unreleased
[v5.0]: #v500---2026-07-06---live-execution
[v4.0]: #v400---2026-07-02---production-platform