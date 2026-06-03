# rRules - Business Rules Management System

## What This Is

rRules is a comprehensive Business Rules Management System (BRMS) built in Rust, designed for production use. It provides a complete platform for defining, deploying, and managing business decision logic - from rule authoring through execution, governance, and monitoring.

## Core Value

**The ONE thing that matters most:** Users can define complex business decisions as reusable rules, deploy them instantly, and monitor their impact - all without modifying application code.

## Requirements

### Validated

(None yet — ship to validate)

### Active

- [ ] **TEST-01**: Rule-by-rule unit tests with working memory fixtures
- [ ] **TEST-02**: Forward chaining verification with salience order testing
- [ ] **TEST-03**: Backward chaining trace capture for debugging
- [ ] **TEST-04**: Deterministic firing mode to eliminate flaky tests
- [ ] **TEST-05**: No-loop/lock-on-active enforcement verification
- [ ] **TEST-06**: Snapshot-based testing with JSON baseline files

### Out of Scope

- Mobile native apps — Web-first, responsive UI for all user types
- Embedded in-process SDK — Focus on standalone service/deployment modes
- Legacy system migration tools — New implementation focus only

## Current Milestone: v2.0 Testing Framework

**Goal:** Build comprehensive testing infrastructure for the rule engine, establishing a foundation for reliable rule development and verification.

**Target features:**
- Core Rust testing stack integration (criterion, mockall, serial_test)
- Working memory fixture support with per-test engine isolation
- Rule-by-rule unit tests with assertion framework
- Forward chaining verification with salience order testing
- Backward chaining trace capture for debugging
- Deterministic firing mode to eliminate flaky tests
- No-loop/lock-on-active enforcement verification
- Snapshot-based testing with JSON baseline files

### Previous Milestones (v1.0)

**Core Engine Foundation:** Rete algorithm, forward/backward chaining, working memory pattern matching
**Rule Authoring Interface:** Web IDE with CodeMirror 6, decision table editor, guided builder
**REST API Layer:** CRUD endpoints, Docker/K8s deployment
**Governance & Access Control:** RBAC, approval workflows, audit trail, Git version control
**Testing & Monitoring (stub):** Placeholder test structure, metrics collection hooks
**DMN Compliance (partial):** Parser foundation, needs execution engine verification

## Context

rRules targets organizations needing to automate and govern complex business decisions: financial services (risk scoring, compliance), insurance (underwriting, claims), healthcare (clinical pathways), and any domain with evolving decision logic.

Key technical context:
- Rust backend for performance, safety, and concurrency
- React + TypeScript frontend for the web IDE
- Hybrid Git + PostgreSQL storage (Git for versioning rules as code, DB for metadata)
- Multi-mode deployment: cloud-native (Kubernetes) or on-premises

## Constraints

- **Tech Stack**: Rust for backend, React/TypeScript for frontend — ensures memory safety and type correctness
- **Production Ready**: Must support high availability, monitoring, and governance requirements from day one

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Rust as primary language | Memory safety, performance, concurrency without GC pauses | ✓ Good |
| Hybrid Git + DB storage | Git provides natural versioning for rules as code; DB handles runtime metadata and queries | ✓ Good |
| Web IDE first | Browser-native access for all users (business analysts, developers, admins) | — Pending |

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
*Last updated: 2026-06-03 after v2.0 milestone start*
