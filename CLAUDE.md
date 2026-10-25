# rRules - Business Rules Management System

## Project Overview

rRules is a production-grade Business Rules Management System (BRMS) built in Rust. It enables users to define complex business decisions as reusable rules, deploy them instantly, and monitor their impact without modifying application code.

**Core Value:** Users can define complex business decisions as reusable rules, deploy them instantly, and monitor their impact — all without modifying application code.

## Current Phase

Milestone v8.0 (Close TCK Evaluator Gaps) — **complete** as of 2026-07-15 (Phases 40–43; milestone audit `passed`). Git tags `v1.0`–`v7.0` exist; **`v8.0` is tagged at this close-out**. v8.0 raised OMG DMN TCK evaluator coverage from 21/51 to **51/51 vectors** (123/123 layer passes, zero regression) by completing the FEEL built-in function library: numeric (Phase 40), string + list (Phase 41), and verified context/relational/temporal families (Phase 42), then conformance close-out (Phase 43).

Shipped milestones: v1.0 MVP → v2.0 Testing Framework → v3.0 Pluggable Engines (Rete/TREAT/LEAPS/PHREAK) → v4.0 Production Platform (frontend + governance + monitoring + deployment) → v5.0 Live Execution (`/v1/execute`, sessions, metrics) → v6.0 DMN Level 3 Compliance (parser, FEEL, decision tables, DRD) → v7.0 Performance + Production Readiness (benchmarks, memory gates, AST cache, DRD parallelization, fuzzing) → v8.0 TCK evaluator gaps.

**Next Step:** no milestone is in flight. Scope the next one from the recorded backlog in `.planning/TECH-DEBT.md`, `TCK-COVERAGE-GAPS.md`, and the v8.0 audit — leading candidates: DMN 1.6 / FEEL 1.6 extensions (COLLECT `distinct`), and real `PostgresDmnStore` queries. (Engine retraction was completed on 2026-10-25 — it is no longer outstanding.)

## Project Structure

| Artifact | Location |
|----------|----------|
| Project Context | `.planning/PROJECT.md` |
| Requirements | `.planning/REQUIREMENTS.md` |
| Roadmap | `.planning/ROADMAP.md` |
| State | `.planning/STATE.md` |
| Config | `.planning/config.json` |
| Shipped milestones | `.planning/MILESTONES.md` + `.planning/milestones/` |
| Known tech debt | `.planning/TECH-DEBT.md` |
| DMN TCK divergences | `TCK-COVERAGE-GAPS.md` |
| API reference | `docs/API.md` |

## Tech Stack

- **Backend:** Rust (primary language)
- **Frontend:** React + TypeScript
- **Storage:** Hybrid Git + PostgreSQL
- **Deployment:** Docker, Kubernetes

## Key Requirements

- Rule engine with forward/backward chaining and Rete algorithm
- Web-based rule authoring IDE
- REST API for rule execution and management
- RBAC with maker-checker approval workflows
- Full DMN Level 3 compliance

## Compact Instructions

When compacting, preserve working state for continuation, not chat history.

Always keep:

- Current goal and acceptance criteria
- Exact files changed, created, deleted, or inspected and why
- Important hooks, functions, classes, routes, settings, commands, and config keys
- Business rules and architectural decisions
- Rejected approaches and why they were rejected
- Errors, failed tests, commands run, and fixes attempted
- Pending tasks and the exact next step

Summarize:

- Completed exploration
- Older discussion
- Repeated command output

Drop:

- Verbose logs unless they contain unresolved errors
- Duplicate explanations
- Abandoned ideas that are no longer relevant

After compaction, re-read PLAN.md or HANDOFF.md if present before continuing.

## Original v1.0 Scope (all delivered)

1. **Core Engine Foundation** - Rule engine with Rete algorithm, forward/backward chaining
2. **Rule Authoring Interface** - Web IDE, decision tables, guided editors
3. **REST API Layer** - Complete REST API and container deployment
4. **Governance & Access Control** - RBAC, approval workflows, audit trails
5. **Testing & Monitoring** - Test framework, metrics dashboards
6. **DMN Compliance** - Full OMG DMN Level 3 compliance (real implementation landed in v6.0/v8.0; 51/51 TCK evaluator vectors pass)

## Next Steps

1. Scope the next milestone from the candidate backlog above (no milestone currently active).
2. The repository is local-only — no git remote is configured. Add one (`git remote add origin <url>`) and push tags `v1.0`–`v8.0` when a remote exists.


