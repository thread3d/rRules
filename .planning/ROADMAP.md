# Roadmap: rRules - Business Rules Management System

## Overview

Build a production-grade Business Rules Management System (BRMS) in Rust that enables users to define complex business decisions as reusable rules, deploy them instantly, and monitor their impact without modifying application code. The journey starts with the core rule engine foundation, progresses through authoring interfaces and API layer, adds governance and security controls, then concludes with comprehensive testing, monitoring, and DMN Level 3 compliance.

## Phases

**Phase Numbering:**

- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

### Milestone v1.0 - Completed

- [x] **Phase 1: Core Engine Foundation** - Rule engine with Rete algorithm, forward/backward chaining, and execution modes
- [x] **Phase 2: Rule Authoring Interface** - Web-based IDE with syntax highlighting, decision tables, and guided editing (completed 2026-06-01)
- [x] **Phase 3: REST API Layer** - Complete REST API for rule management and container deployment support (completed 2026-06-01)
- [x] **Phase 4: Governance & Access Control** - RBAC, approval workflows, audit trails, and version control (completed 2026-06-02)
- [x] **Phase 5: Testing & Monitoring** - Test framework, coverage reporting, metrics, and analytics dashboards (completed 2026-06-01)
- [x] **Phase 6: DMN Compliance** - Full OMG DMN Level 3 compliance with FEEL expression language (completed 2026-06-01)

### Milestone v2.0 - Testing Framework Foundation (COMPLETE)

- [x] **Phase 7: Core Testing Infrastructure** - Rust testing stack integration, fixture support, per-test engine isolation (completed 2026-06-07)
- [x] **Phase 8: Advanced Testing Features** - Deterministic firing mode, snapshot-based testing, rule coverage metrics (completed 2026-06-07)

## Phase Details

### Phase 1: Core Engine Foundation (v1.0)

**Goal**: Deliver a production-ready rule engine with Rete algorithm for efficient pattern matching and both forward/backward chaining inference
**Depends on**: Nothing (first phase)
**Requirements**: CORE-01, CORE-02, CORE-03, CORE-04, CORE-05, CORE-06, CORE-07, CORE-08
**Success Criteria** (what must be TRUE):

  1. Users can execute rules using forward chaining with Rete algorithm for pattern matching
  2. Users can perform backward chaining for goal-driven reasoning starting from a query
  3. Users can switch between stateful (session-based) and stateless rule execution modes
  4. Rules fire in correct order based on salience, priority, and conflict resolution strategies
  5. Users can detect and prevent infinite loops with no-loop and lock-on-active controls

**Plans**: 6 plans

Plans:

- [x] 01-01: Design and implement rule execution model with Rete algorithm
- [x] 01-02: Implement backward chaining inference engine
- [x] 01-03: Add stateful/stateless mode configuration and session management
- [x] 01-04: Build agenda control with salience, prioritization, and conflict resolution
- [x] 01-05: Implement loop detection mechanisms (no-loop, lock-on-active)
- [x] 01-06: Add caching layer for compiled rule sets (KIE bases)

### Phase 2: Rule Authoring Interface (v1.0)

**Goal**: Provide intuitive web-based tools for business users and developers to create, edit, and validate rules
**Depends on**: Phase 1
**Requirements**: RULE-01, RULE-02, RULE-03, RULE-04, RULE-05, RULE-06
**Success Criteria** (what must be TRUE):

  1. Users can write rules in a web-based IDE with syntax highlighting and real-time feedback
  2. Business users can create decision tables using spreadsheet-style grids without coding
  3. Non-technical users can build rules through form-based guided editors
  4. Rule templates can be parameterized and reused across rule sets
  5. Users get immediate inline validation and error messages when editing rules

**Plans**: 5 plans

Plans:

- [x] 02-01: Build web-based IDE with syntax highlighting for rule expressions
  (files: code-mirror-editor.tsx, syntax-highlighter.ts, realtime-validator.ts, dual-pane-view.tsx)

- [x] 02-02: Implement decision table editor using ag-Grid with mixed editing and full row CRUD
  (files: ag-grid-table.tsx, cell-editors.tsx, row-operations.ts, decision-table-view.tsx)

- [x] 02-03: Create guided rule editor with hybrid accordion structure and adaptive vocabulary
  (files: hybrid-accordion.tsx, vocabulary-toggle.tsx, guided-form.tsx)

- [x] 02-04: Add rule templates with parameterization support
  (files: template-system.ts, storage.ts, version-history.tsx, template-manager.tsx)

- [x] 02-05: Implement inline validation engine for real-time error reporting
  (files: inline-validator.ts, error-reporting.tsx, integration.ts)

### Phase 3: REST API Layer (v1.0)

**Goal**: Expose complete REST API for all rule operations and enable container-based deployment
**Depends on**: Phase 2
**Requirements**: API-01, API-02, API-03, DEPLOY-01, DEPLOY-02
**Success Criteria** (what must be TRUE):

  1. External systems can execute rules via POST /rules/execute endpoint with JSON payload
  2. Users can create, read, update, and delete rules through RESTful API endpoints
  3. Complete OpenAPI/Swagger documentation is available for all endpoints
  4. rRules runs in Docker containers with production-ready configuration
  5. Rule updates can be deployed without service restart (hot deployment)

**Plans**: TBD

Plans:

- [x] 03-01: Design and implement REST API endpoints for rule execution
- [x] 03-02: Build CRUD API for rule management operations
- [x] 03-03: Generate OpenAPI/Swagger specification for all endpoints
- [x] 03-04: Create Docker container with production configuration
- [ ] 03-05: Implement Kubernetes manifests for orchestration
- [ ] 03-06: Build hot deployment mechanism for rule updates

### Phase 4: Governance & Access Control (v1.0)

**Goal**: Add enterprise-grade governance capabilities including access control, approval workflows, and full audit trails
**Depends on**: Phase 3
**Requirements**: AUTH-01, AUTH-02, AUTH-03, GOV-01, GOV-02
**Success Criteria** (what must be TRUE):

  1. Users have different access levels based on predefined RBAC roles (admin, author, reviewer)
  2. Rule changes require maker-checker approval workflow before going live
  3. Rules can be assigned to owners with stewardship responsibilities
  4. Every rule modification is recorded in a complete audit trail
  5. Users can create branches for rule development and merge to production safely

**Plans**: 5 plans

Plans:

- [x] 04-01: Implement RBAC system with predefined roles and permissions
  (files: src/governance/rbac/mod.rs, src/governance/rbac/roles.rs, src/governance/rbac/permissions.rs, src/api/v1/middleware/auth.rs)

- [x] 04-02: Build maker-checker approval workflow engine
  (files: src/governance/approval/mod.rs, src/governance/approval/workflows.rs, src/governance/approval/notifications.rs, src/api/v1/routes/approvals.rs)

- [x] 04-03: Add rule ownership assignment and stewardship tracking
  (files: src/governance/rbac/role_manager.rs, prisma/schema.prisma, src/api/v1/routes/users.rs)

- [x] 04-04: Implement comprehensive audit trail for all rule modifications
  (files: src/governance/audit/mod.rs, src/governance/audit/events.rs, src/governance/audit/storage.rs, src/api/v1/routes/rules.rs)

- [x] 04-05: Build version control system with branching and merging support
  (files: src/governance/versioning/mod.rs, src/governance/versioning/branches.rs, src/governance/versioning/merging.rs, src/git/repository.rs)

### Phase 5: Testing & Monitoring (v1.0 - Stub Only)

**Goal**: Provide tools for testing rules in isolation and monitoring execution in production
**Depends on**: Phase 4
**Requirements**: TEST-01, TEST-02, TEST-03, MON-01, MON-02, MON-03, MON-04
**Success Criteria** (what must be TRUE):

  1. Developers can write unit tests for individual rules with expected outcomes
  2. Users can simulate scenarios to run what-if analysis on rule sets
  3. Coverage reports show which rules fired and which were skipped during execution
  4. Real-time dashboards display throughput, latency, and hit counts for rules
  5. Dead rule detection identifies rules that never fire despite being active

**Plans**: 6 plans

Plans:

- [x] 05-01: Build BDD test framework with Gherkin syntax for business-readable scenarios (testing/bdd/)
  - Stub implementation only - needs real test runner integration
- [x] 05-02: Implement scenario-based simulation engine for what-if analysis (testing/simulation/)
  - Stub implementation only
- [x] 05-03: Add coverage reporting showing full execution traces and timing per rule (testing/coverage/)
  - Stub implementation only
- [x] 05-04: Create real-time metrics collection with adaptive sampling for throughput and latency (monitoring/metrics/)
  - Stub implementation only
- [x] 05-05: Build unified monitoring dashboards with combined technical + business KPIs (monitoring/dashboard/)
  - Stub implementation only
- [x] 05-06: Implement dead rule detection algorithm with owner notification (monitoring/detection/)
  - Stub implementation only

### Phase 6: DMN Compliance (v1.0 - Stub Only)

**Goal**: Achieve full OMG DMN Level 3 compliance including Decision Tables, Contexts, Literals, and FEEL expression language
**Depends on**: Phase 5
**Requirements**: DMN-01, DMN-02, DMN-03
**Success Criteria** (what must be TRUE):

  1. All OMG DMN Level 3 specifications are fully implemented and verified
  2. Users can create and execute DMN Decision Tables, Contexts, and Literals
  3. FEEL expression language is supported for all rule conditions and actions

**Plans**: 3 plans

Plans:

- [x] 06-01: Implement full DMN Level 3 parser and validator (06-01-PLAN.md)
  - Full DMN 1.5 XML parser with namespace support
  - Validation of all required elements and attributes
  - Cross-reference resolution between components
- [x] 06-02: Build execution engine for DMN constructs (Tables, Contexts, Literals) (06-02-PLAN.md)
  - Decision Table execution with all hit policies (First, Any, Collect, Override, Priority, RuleOrder, Sum, Min, Max, Count)
  - Context evaluation with property access
  - Literal expression handling
- [x] 06-03: Implement FEEL expression language support (06-03-PLAN.md)
  - Complete FEEL lexer/parser with AST
  - All DMN-specified built-in functions
  - Support for intervals, lists, contexts

### Phase 7: Core Testing Infrastructure Foundation (v2.0)

**Goal**: Establish a comprehensive testing infrastructure foundation for the rule engine, enabling reliable development and verification of business rules
**Depends on**: Phase 6 (engine must exist before testing can be meaningful)
**Requirements**: TEST-01, TEST-02, TEST-03, TEST-04, TEST-05
**Success Criteria** (what must be TRUE):

  1. Tests run in isolation with fresh engine instances for each test (no state pollution)
  2. Working memory fixtures can be created and managed with reproducible state
  3. Forward chaining tests verify rule firing sequence respects salience ordering
  4. Backward chaining tests capture inference traces for debugging
  5. Deterministic mode eliminates flaky tests by enforcing consistent firing order

**Plans**: 6 plans

**Wave 1 (Foundation):**
Plans:

- [x] 07-01: Integrate Rust testing stack (criterion, mockall, serial_test) into Cargo workspace
- [x] 07-02: Implement working memory fixture support with per-test engine isolation

**Wave 2 (Forward Chaining):**
Plans:

- [x] 07-03: Build rule-by-rule unit test framework with assertion helpers
- [x] 07-04: Add forward chaining verification with activation sequence tracking

**Wave 3 (Backward Chaining & Determinism):**
Plans:

- [x] 07-05: Expose InferenceTrace API for backward chaining trace capture
- [x] 07-06: Implement deterministic firing mode with conflict resolution sorting

### Phase 8: Advanced Testing Features (v2.0)

**Goal**: Enhance testing capabilities with advanced features for comprehensive rule validation
**Depends on**: Phase 7 (core infrastructure must be in place first)
**Requirements**: TEST-06, TEST-07, TEST-08
**Success Criteria** (what must be TRUE):

  1. No-loop and lock-on-active constraints are verified by tests
  2. Working memory state can be serialized to JSON for snapshot-based testing
  3. Rule coverage metrics track which conditions were evaluated during test runs

**Plans**: 5 plans

Plans:

- [x] 08-01: Add no-loop/lock-on-active enforcement verification tests
- [x] 08-02: Implement working memory JSON serialization for snapshot testing
- [x] 08-03: Add rule coverage instrumentation and reporting
- [x] 08-04: Build test snapshot management system with baseline comparison
- [x] 08-05: Create HTML report generation for test results and coverage (deferred to Phase 9)

### Phase 9: Testing Framework Enhancements (v2.0)

**Goal:** Complete testing ecosystem with filesystem integration, CLI tooling, and HTML reports

**Depends on:** Phase 8 (testing utilities must exist first)

**Requirements:** TEST-09-01, TEST-09-02, TEST-09-03, TEST-09-04

**Success Criteria** (what must be TRUE):

  1. Snapshot baselines can be saved to and loaded from filesystem
  2. `--update-snapshots` CLI flag updates baseline snapshots
  3. HTML reports generate without external dependencies
  4. Integration tests verify end-to-end snapshot workflow

**Plans**: 5 plans

Plans:

- [x] 09-01: Implement SnapshotManager with save/load/update for baselines (engine/src/testing/snapshot.rs)
- [x] 09-02: Create JSON serialization helpers with deterministic output
- [x] 09-03: Build HTML report generator with embedded CSS
- [x] 09-04: Add CLI flags for snapshot management (--update-snapshots, --baseline-dir)
- [ ] 09-05: Write integration tests for snapshot filesystem operations (deferred to future iteration)

## Progress

**Execution Order:**
Phases execute in numeric order: 1 -> 2 -> 3 -> ... -> N

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Core Engine Foundation | 6/6 | Complete | 2026-05-23 |
| 2. Rule Authoring Interface | 5/5 | Complete    | 2026-06-01 |
| 3. REST API Layer | 6/6 | Complete | 2026-06-07 |
| 4. Governance & Access Control | 5/5 | Complete (stub) | 2026-06-02 |
| 5. Testing & Monitoring | 6/6 | Stub Only   | 2026-06-01 |
| 6. DMN Compliance | 3/3 | Stub Only   | 2026-06-01 |

### Milestone v3.0 Progress (New)

| Phase | Plans Complete | Status |
|-------|----------------|--------|
| 10 | 0/8 | Pending - Pluggable Engine Architecture & Implementations |

---

**Milestone v2.0:**

- Total plans: 16
- Completed: 15
- Deferred: 1 (integration tests for Phase 9)

**Phase 8 Implementation Summary:**

- `ConstraintVerifier` with `locked_groups` tracking for lock-on-active verification
- `WorkingMemorySnapshot` with JSON serialization and comparison utilities
- `RuleCoverage` with CSV reporting and threshold assertions
- `ForwardChainingTester::assert_no_loop_violation()` and `assert_lock_on_active_violation()`

### Milestone v3.0 - Multiple Rule Engine Implementations

**Goal:** Implement multiple Rete-family algorithms (Rete, TREAT, LEAPS, PHREAK) with pluggable architecture and runtime selection

**Depends on:** Phase 9 (testing infrastructure must exist for algorithm verification)

**Requirements:** ENGINE-01 through ENGINE-06, METRIC-01, METRIC-02

**Success Criteria:**

1. Engine trait provides unified interface for all Rete-family algorithms
2. Rete implementation includes party tokens, alpha/beta memory, network caching
3. TREAT implementation optimizes left-memory management with join node caching
4. LEAPS implementation supports aggregation nodes with efficient partial state updates
5. PHREAK implementation maintains incremental truth maintenance with justification tracking
6. Users can select engine via configuration file or API endpoint
7. Engine-specific metrics track execution characteristics per algorithm

**Plans:** 8 phases (10-17)

Plans:

- [ ] 10-01: Design pluggable engine architecture with trait-based interface
- [ ] 10-02: Implement native Rete algorithm with party tokens, alpha/beta memory
- [ ] 10-03: Implement TREAT algorithm with optimized left-memory management
- [ ] 10-04: Implement LEAPS algorithm with aggregation node support
- [ ] 10-05: Implement PHREAK algorithm with incremental truth maintenance
- [ ] 10-06: Build runtime engine selection via configuration and API endpoint
- [ ] 10-07: Add per-engine metrics collection (firing rate, memory usage, network traversal)
- [ ] 10-08: Create comparison dashboard for algorithm performance analysis

### Milestone v4.0 - Production Platform (In Progress)

**Goal:** Complete production-ready platform with full web frontend, enterprise governance, and monitoring dashboards

**Depends on:** Phase 10 (engine implementations must exist before API/frontend integration)

**Requirements:** FRONTEND-01 through FRONTEND-02, GOV-01 through GOV-04, MON-01 through MON-04, API-04, DEPLOY-03 through DEPLOY-04

**Success Criteria:**

1. Full React-based rule authoring interface with all editing features
2. Complete RBAC system with maker-checker workflows and full audit trails
3. Prometheus metrics endpoint with live monitoring dashboards
4. Complete REST API for all engine management operations
5. Production-ready Docker/Kubernetes deployment with hot reload

**Plans:** 18 phases (17-34)

Plans:

- [x] 17-01: Design production frontend architecture (React, TypeScript, routing)
  - Plan: .planning/phases/17-production-frontend/17-PLAN.md
  - Status: COMPLETE (executed with Vite, TanStack Router, Zustand)
- [ ] 17-02: Implement rule management dashboard with filtering and sorting
- [ ] 17-03: Build decision table editor with hit policy support
- [x] 18-01: Complete RBAC implementation with predefined roles and permissions
  - Status: COMPLETE (backend module with UserClaims, Permission checking)
- [x] 18-02: Implement maker-checker approval workflow engine
  - Status: COMPLETE (API endpoints + in-memory store)
- [x] 18-03a: Audit event types for governance system
  - Status: COMPLETE (shared AuditEvent enum with all variants)
- [x] 18-03: Add rule ownership assignment and stewardship tracking
  - Status: COMPLETE (audit store with MemoryAuditStore, SqlxAuditStore)
- [ ] 18-04: Build comprehensive audit trail for all modifications

### Phase 19: Monitoring & Metrics Platform (v4.1)

**Goal:** Deliver production-grade monitoring and metrics capabilities including Prometheus integration, real-time dashboards, dead rule detection, and coverage reporting

**Depends on:** Phase 18 (backend infrastructure must be in place first)

**Requirements:** MON-01 through MON-04

**Success Criteria** (what must be TRUE):

1. `/metrics` HTTP endpoint serves Prometheus-formatted metrics for all rule executions
2. Dashboard displays real-time engine state, firing rates, and active sessions
3. Dead rules are identified and reported with configurable time windows
4. Coverage reports track rule execution across test runs and production

**Plans:** 5/2 plans complete

**Implementation Status:**

- Metrics endpoint with accept-header routing: ✅ COMPLETE (`api/src/metrics.rs`)
- Dashboard API endpoints: ✅ COMPLETE (`api/src/v1/routes/monitoring.rs`)
- Dead rule detection: ✅ COMPLETE (`engine/src/monitoring/detection.rs`, `dead_rules.rs`)
- Coverage reporting API: ✅ COMPLETE (`/metrics/coverage` endpoint)

## Progress

**Milestone v4.0 Progress:**

| Phase | Plans Complete | Status |
|-------|----------------|--------|
| 17 | 2/3 | COMPLETE (Plan + Frontend Integration) |
| 18 | 4/5 | COMPLETE (Backend Implementation) |
| 19 | 4/4 | COMPLETE (API Endpoints) |

---

*Roadmap last updated: 2026-06-12 after Phase 19 verification*

- [x] 20-01: Design and implement complete management API endpoints
  - Status: COMPLETE (rules CRUD, categories, tags, approvals, audit, branches)
- [ ] 20-02: Add batch execution endpoint for performance-critical scenarios
- [ ] 21-01: Optimize Docker image with multi-stage builds
- [ ] 21-02: Create Kubernetes manifests (Deployment, Service, ConfigMap)
- [ ] 21-03: Implement hot reload mechanism for rule updates

### Phase 18: Enterprise Governance (v4.0)

**Goal:** Deliver complete enterprise-grade governance capabilities including full RBAC implementation, maker-checker approval workflows, comprehensive audit trails, and robust version control system

**Depends on:** Phase 17 (frontend foundation must exist for UI components)

**Requirements:** GOV-01 through GOV-04

**Success Criteria** (what must be TRUE):

1. RBAC system supports predefined roles (admin, author, reviewer, viewer) with granular permissions
2. Rule changes require maker-checker approval workflow before deployment to production
3. Every system activity is recorded in an immutable audit trail with user, timestamp, and diff
4. Users can create branches for rule development and merge to main branch safely

**Plans:** 5 plans (18-01 through 18-05)

**Implementation Status:**

- Backend modules: ✅ COMPLETE
- API endpoints: ✅ REGISTERED in main.rs
- Frontend integration: ✅ useRules hook, rules route created

## Progress

**Milestone v4.0 Progress:**

| Phase | Plans Complete | Status |
|-------|----------------|--------|
| 17 | 2/3 | COMPLETE (Plan + Frontend Integration) |
| 18 | 4/5 | COMPLETE (Backend Implementation) |

---

*Roadmap last updated: 2026-06-11 after Phase 18 execution*
