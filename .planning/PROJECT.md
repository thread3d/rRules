# rRules - Business Rules Management System

## What This Is

rRules is a comprehensive Business Rules Management System (BRMS) built in Rust, designed for production use. It provides a complete platform for defining, deploying, and managing business decision logic - from rule authoring through execution, governance, and monitoring.

## Core Value

**The ONE thing that matters most:** Users can define complex business decisions as reusable rules, deploy them instantly, and monitor their impact - all without modifying application code.

## Requirements

### Validated

(None yet — ship to validate)

### Active

- [ ] **CORE-01**: Rule engine with forward and backward chaining inference
- [ ] **CORE-02**: Rete algorithm implementation for efficient pattern matching
- [ ] **AUTH-01**: Role-Based Access Control (RBAC) for rule management
- [ ] **AUTH-02**: Maker-checker approval workflows for rule changes
- [ ] **RULE-01**: Web-based IDE for rule authoring with syntax highlighting
- [ ] **RULE-02**: Decision table editor for business users
- [ ] **API-01**: REST API for rule execution and management
- [ ] **DEPLOY-01**: Container/microservices deployment support (Docker, Kubernetes)
- [ ] **GOV-01**: Full audit trail of all rule changes
- [ ] **GOV-02**: Rule version control with branching and merging
- [ ] **MON-01**: Real-time rule execution metrics and dashboards
- [ ] **DMN-01**: Full DMN Level 3 compliance

### Out of Scope

- Mobile native apps — Web-first, responsive UI for all user types
- Embedded in-process SDK — Focus on standalone service/deployment modes
- Legacy system migration tools — New implementation focus only

## Context

rRules targets organizations needing to automate and govern complex business decisions: financial services (risk scoring, compliance), insurance (underwriting, claims), healthcare (clinical pathways), and any domain with evolving decision logic.

Key technical context:
- Rust backend for performance, safety, and concurrency
- React + TypeScript frontend for the web IDE
- Hybrid Git + PostgreSQL storage (Git for versioning rules as code, DB for metadata)
- Multi-mode deployment: cloud-native (Kubernetes) or on-premises

## Constraints

- **Tech Stack**: Rust for backend, React/TypeScript for frontend — ensures memory safety and type correctness
- **DMN Compliance**: Full OMG DMN Level 3 standard adherence — critical for enterprise adoption
- **Production Ready**: Must support high availability, monitoring, and governance requirements from day one

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Rust as primary language | Memory safety, performance, concurrency without GC pauses | ✓ Good |
| Hybrid Git + DB storage | Git provides natural versioning for rules as code; DB handles runtime metadata and queries | ✓ Good |
| Web IDE first | Browser-native access for all users (business analysts, developers, admins) | — Pending |
| DMN Level 3 compliance | Enterprise standard for decision modeling; ensures interoperability | — Pending |

---
*Last updated: 2026-05-22 after initialization*

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
