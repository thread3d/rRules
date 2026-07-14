# rRules - Business Rules Management System

## Project Overview

rRules is a production-grade Business Rules Management System (BRMS) built in Rust. It enables users to define complex business decisions as reusable rules, deploy them instantly, and monitor their impact without modifying application code.

**Core Value:** Users can define complex business decisions as reusable rules, deploy them instantly, and monitor their impact — all without modifying application code.

## Current Phase

Milestone v5.0 (Live Execution) — **shipped & tagged** 2026-07-06 (Phases 23–28; verified closeout via audit → debt-fix → re-audit `passed`). Git tags `v1.0`–`v5.0` all exist. v5.0 closed the long-standing `/v1/execute` stub: real rule firing through `EngineRegistry`, live Prometheus metrics + WS broadcast, `RuleExecutionTracker` singleton feeding dead-rule detection, shared reloadable `RuleSet` + `JobStore`, `SessionManager` (stateful sessions), and a frontend test-runner.

**Next Step:** v5.0 is closed and tagged. Run `/gsd:new-milestone` to plan the next milestone. Candidate thesis (proposed): **v6.0 DMN Level 3 Compliance** — Phase 6 shipped as a *stub* in v1.0 (no `dmn` module exists in `engine/src` or `api/src`); deliver a real DMN 1.5 parser, FEEL expression language, and decision-table engine with hit policies to close that stated key requirement.

## Project Structure

| Artifact | Location |
|----------|----------|
| Project Context | `.planning/PROJECT.md` |
| Requirements | `.planning/REQUIREMENTS.md` |
| Roadmap | `.planning/ROADMAP.md` |
| State | `.planning/STATE.md` |
| Config | `.planning/config.json` |

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

## Phase Roadmap (6 Phases)

1. **Core Engine Foundation** - Rule engine with Rete algorithm, forward/backward chaining
2. **Rule Authoring Interface** - Web IDE, decision tables, guided editors
3. **REST API Layer** - Complete REST API and container deployment
4. **Governance & Access Control** - RBAC, approval workflows, audit trails
5. **Testing & Monitoring** - Test framework, metrics dashboards
6. **DMN Compliance** - Full OMG DMN Level 3 compliance

## Next Steps

1. Run `/gsd:new-milestone` to plan the next milestone (proposed thesis: **v6.0 DMN Level 3 Compliance** — close the stubbed Phase 6 gap; no real `dmn` module exists yet)
2. Optionally: add a git remote (`git remote add origin <url>`) and push tags `v1.0`–`v5.0` — currently local-only (no remote configured)


