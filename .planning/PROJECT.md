# rRules - Business Rules Management System

## What This Is

rRules is a comprehensive Business Rules Management System (BRMS) built in Rust, designed for production use. It provides a complete platform for defining, deploying, and managing business decision logic - from rule authoring through execution, governance, and monitoring.

**New in this milestone:** Multiple Rete-family algorithm implementations with user-selectable execution engine.

## Core Value

**The ONE thing that matters most:** Users can define complex business decisions as reusable rules, deploy them instantly, and monitor their impact - all without modifying application code.

**Milestone v3.0 Additions:**
- Multiple rule engine implementations (Rete, TREAT, LEAPS, PHREAK) with pluggable architecture
- Runtime selection of inference algorithm via configuration or API
- Algorithm-specific metrics and performance tracking

**Milestone v4.0 Additions (In Progress):**
- Full production-ready web frontend for rule authoring and management
- Enterprise RBAC with full maker-checker approval workflows
- Real-time monitoring dashboards with Prometheus integration
- Complete REST API for all engine management operations

## Requirements

### Validated

(None yet — ship to validate)

### Active

- [ ] **ENGINE-01**: Pluggable rule engine architecture supporting multiple Rete-family algorithms
- [ ] **ENGINE-02**: Native Rete algorithm implementation with party tokens, alpha/beta memory, network caching
- [ ] **ENGINE-03**: TREAT algorithm implementation for optimized left-memory management
- [ ] **ENGINE-04**: LEAPS algorithm implementation with aggregation node support
- [ ] **ENGINE-05**: PHREAK algorithm implementation with incremental truth maintenance
- [ ] **ENGINE-06**: Runtime engine selection via configuration or API endpoint
- [ ] **METRIC-01**: Per-engine execution metrics (firing rate, memory usage, network traversal counts)
- [ ] **METRIC-02**: Algorithm comparison dashboard for performance analysis

### Out of Scope

- Mobile native apps — Web-first, responsive UI for all user types
- Embedded in-process SDK — Focus on standalone service/deployment modes
- Legacy system migration tools — New implementation focus only
- Experimental/non-standard algorithms (e.g., Multi-Rete, Incremental Rete)

## Current Milestone: v4.0 Production Platform (In Progress)

**Goal:** Deliver a complete production-ready platform with full web frontend, governance features, monitoring dashboards, and REST API for enterprise deployment.

**Target features:**
- Full React-based rule authoring interface with decision tables and guided editors
- Enterprise RBAC with maker-checker approval workflows and full audit trails
- Real-time metrics collection with Prometheus export and Grafana dashboards
- Complete REST API for all rule engine management operations
- Docker/Kubernetes deployment with hot-reload support

### Previous Milestones (v1.0, v2.0, v3.0)

**v1.0:** Core Engine Foundation, Rule Authoring Interface, REST API Layer, Governance & Access Control, Testing & Monitoring (stub), DMN Compliance (partial)

**v2.0:** Testing Framework Foundation - Comprehensive testing infrastructure with Rust testing stack, working memory fixtures, forward/backward chaining verification, snapshot-based testing, and deterministic firing mode

**v3.0:** Pluggable Engine Architecture - Multiple Rete-family algorithms (Rete, TREAT, LEAPS, PHREAK) with runtime selection and engine-specific metrics

## Context

rRules targets organizations needing to automate and govern complex business decisions: financial services (risk scoring, compliance), insurance (underwriting, claims), healthcare (clinical pathways), and any domain with evolving decision logic.

Key technical context:
- Rust backend for performance, safety, and concurrency
- React + TypeScript frontend for the web IDE
- Hybrid Git + PostgreSQL storage (Git for versioning rules as code, DB for runtime metadata)
- Multi-mode deployment: cloud-native (Kubernetes) or on-premises

**New in v3.0:** The engine layer becomes pluggable, allowing users to select different Rete-family algorithms based on their specific use case characteristics and performance requirements.

## Constraints

- **Tech Stack**: Rust for backend, React/TypeScript for frontend — ensures memory safety and type correctness
- **Production Ready**: Must support high availability, monitoring, and governance requirements from day one
- **Algorithm Correctness**: All implementations must pass identical functional test suite regardless of algorithm selected

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Rust as primary language | Memory safety, performance, concurrency without GC pauses | ✓ Good |
| Hybrid Git + DB storage | Git provides natural versioning for rules as code; DB handles runtime metadata and queries | ✓ Good |
| Web IDE first | Browser-native access for all users (business analysts, developers, admins) | ✓ Completed v1.0/v2.0 |
| Pluggable engine architecture | Different algorithms excel at different workloads; user selection enables optimization | ✓ Completed in v3.0 |
| Production frontend focus | Enterprise users need comprehensive web UI for day-to-day operations | New in v4.0 |

## Evolution

This document evolves at phase transitions and milestone boundaries.

**After each phase transition** (via `/gsd-transition`):
1. Requirements invalidated? → Move to Out of Scope with reason
2. Requirements validated? → Move to Validated with phase reference
3. New requirements emerged? → Add to Active
4. Decisions to log? → Add to Key Decisions
5. "What This Is" still accurate? → Update if drifted

**After each milestone** (via `/gsd:complete-milestone`):
1. Full review of all sections
2. Core Value check — still the right priority?
3. Audit Out of Scope — reasons still valid?
4. Update Context with current state

---
*Last updated: 2026-06-09 after v4.0 milestone start*
