# rRules - Business Rules Management System

## Project Overview

rRules is a production-grade Business Rules Management System (BRMS) built in Rust. It enables users to define complex business decisions as reusable rules, deploy them instantly, and monitor their impact without modifying application code.

**Core Value:** Users can define complex business decisions as reusable rules, deploy them instantly, and monitor their impact — all without modifying application code.

## Current Phase

Milestone v4.0 (Production Platform) — **shipped & tagged** 2026-07-02 (Phases 17–22; phase 22 verified PASS, 9/9 requirements MET). Git tags `v1.0`–`v4.0` all exist.

**Next Step:** v4.0 is closed and tagged. Run `/gsd:new-milestone` to plan the next milestone. Candidate thesis (proposed): **v5.0 Live Execution** — wire the `/v1/execute` API (currently a stub returning `rules_fired: 0`, `api/src/v1/execution.rs:307`) to real engine invocation, auto-populate `RuleExecutionTracker`/`CoverageTracking`, and feed the v4.0 monitoring surfaces (Prometheus, dashboard, dead-rule, coverage) with live data.

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

## Phase Roadmap (6 Phases)

1. **Core Engine Foundation** - Rule engine with Rete algorithm, forward/backward chaining
2. **Rule Authoring Interface** - Web IDE, decision tables, guided editors
3. **REST API Layer** - Complete REST API and container deployment
4. **Governance & Access Control** - RBAC, approval workflows, audit trails
5. **Testing & Monitoring** - Test framework, metrics dashboards
6. **DMN Compliance** - Full OMG DMN Level 3 compliance

## Next Steps

1. Run `/gsd:new-milestone` to plan the next milestone (proposed thesis: **v5.0 Live Execution** — close the `execute_rules` stub gap)
2. Optionally: add a git remote (`git remote add origin <url>`) and push tags `v1.0`–`v4.0` — currently local-only (no remote configured)
