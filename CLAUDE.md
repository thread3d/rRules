# rRules - Business Rules Management System

## Project Overview

rRules is a production-grade Business Rules Management System (BRMS) built in Rust. It enables users to define complex business decisions as reusable rules, deploy them instantly, and monitor their impact without modifying application code.

**Core Value:** Users can define complex business decisions as reusable rules, deploy them instantly, and monitor their impact — all without modifying application code.

## Current Phase

Milestone v4.0 (Production Platform) — **complete** (3/3 phases; phase 22 verified PASS, 9/9 requirements MET).

**Next Step:** The v4.0 milestone is fully delivered. Run `/gsd:ship` to ship the milestone, or `/gsd:new-milestone` to plan the next milestone.

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

1. Run `/gsd:ship` to ship the completed v4.0 milestone
2. Alternatively: `/gsd:new-milestone` to plan the next milestone
