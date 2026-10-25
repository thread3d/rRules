# rRules - Business Rules Management System

## Project Overview

rRules is a production-grade Business Rules Management System (BRMS) built in Rust. It enables users to define complex business decisions as reusable rules, deploy them instantly, and monitor their impact without modifying application code.

**Core Value:** Users can define complex business decisions as reusable rules, deploy them instantly, and monitor their impact — all without modifying application code.

## Current Phase

Milestone v8.0 (Close TCK Evaluator Gaps) is **complete and tagged** (`v8.0`), and the project is
now past its milestone work: it has a specification and independent verification.

**Verified state (2026-10-25, measured — not asserted):**

- `specs/` holds the specification: `requirements.md` (35 normative requirements with stable ids)
  plus implementation-derived subsystem specs, each with `path:line` citations and an explicit
  `Known gaps` section.
- [`CodeWitness`](https://github.com/codewitness/codewitness) verifies it: **28 requirements
  VERIFIED, 0 FAILED, 7 UNVERIFIED** for the `engine` crate. The 7 are the HTTP API requirements,
  verified from the `api` crate where their integration suites pass.
- OMG DMN TCK, every layer recorded: **143 layer results pass, 31 fail, 0 skip** across 51 vectors.
  The failures are enumerated by class in `TCK-COVERAGE-GAPS.md`; the suite gates on that baseline
  so a regression fails the build.
- `cargo test --workspace` green, `cargo clippy --workspace -- -D warnings` clean,
  `cargo fmt --all -- --check` clean.

Earlier milestones: v1.0 MVP → v2.0 Testing Framework → v3.0 Pluggable Engines
(Rete/TREAT/LEAPS/PHREAK) → v4.0 Production Platform → v5.0 Live Execution → v6.0 DMN Level 3
Compliance → v7.0 Performance + Production Readiness → v8.0 TCK evaluator gaps.

**Next Step:** no milestone is in flight. Scope the next one from `.planning/TECH-DEBT.md`,
`TCK-COVERAGE-GAPS.md` and the `Known gaps` sections of `specs/` — candidates: DMN 1.6 / FEEL 1.6
(COLLECT `distinct`), business-knowledge-model and decision-service evaluation, real
`PostgresDmnStore` queries, and PostgreSQL-backed tenants.

## Quality gates

Run all of them before claiming anything is done:

```bash
make ci          # build, test, clippy -D warnings, fmt --check, frontend build + tests
make tck         # DMN TCK sweep, baseline-gated (143/31) with a per-layer report
make verify      # CodeWitness: verify specs/requirements.md against engine and api
```

A claim without a gate behind it is not a claim this project should make: the v8.0 "51/51" figure
was written from phase summaries and did not survive execution.

## Project Structure

| Artifact | Location |
|----------|----------|
| **Specification** | `specs/` (start at `specs/README.md`) |
| Normative requirements | `specs/requirements.md` |
| Verification configuration | `engine/codewitness.yaml`, `api/codewitness.yaml` |
| DMN TCK conformance | `TCK-COVERAGE-GAPS.md` |
| API reference | `docs/API.md` |
| User guide | `docs/user-guide.md` |
| Documentation index | `docs/README.md` |
| Historical documents | `docs/archive/` (superseded; do not treat as current) |
| Project Context | `.planning/PROJECT.md` |
| Roadmap | `.planning/ROADMAP.md` |
| State | `.planning/STATE.md` |
| Shipped milestones | `.planning/MILESTONES.md` + `.planning/milestones/` |
| Known tech debt | `.planning/TECH-DEBT.md` |
| Changelog | `CHANGELOG.md` |

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
6. **DMN Compliance** - OMG DMN 1.5 Level 3 implementation (landed in v6.0/v8.0); measured conformance is tracked in `TCK-COVERAGE-GAPS.md`, not claimed as complete

## Next Steps

1. Scope the next milestone from the candidate backlog above (no milestone currently active).
2. The repository is local-only — no git remote is configured. Add one (`git remote add origin <url>`) and push tags `v1.0`–`v8.0` when a remote exists.


