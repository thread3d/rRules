# Requirements: rRules - Business Rules Management System

**Defined:** 2026-05-22  
**Updated:** 2026-06-09  
**Core Value:** Users can define complex business decisions as reusable rules, deploy them instantly, and monitor their impact — all without modifying application code.

## Milestone v4.0: Production Platform (NEW)

This milestone delivers a complete production-ready platform with full web frontend for enterprise rule management.

### Research Summary

**Production BRMS Requirements (from domain research):**

- **Monitoring:** Prometheus metrics endpoint, Grafana dashboards, alerting
- **RBAC:** Maker-checker workflows, tenant isolation, role-based permissions
- **API:** RESTful endpoints with batch execution commands
- **Frontend:** Guided editors, real-time validation, version comparison

**Tech Stack Recommendations:**

- Frontend: React + TypeScript + CodeMirror + ag-Grid
- Backend: Rust with Prometheus client library
- Deployment: Docker/Kubernetes with ConfigMap/Secrets

---

## v4 Requirements - Production Platform Features

### FRONTEND-01: Complete Rule Authoring Interface

**Category:** User Experience  
**Priority:** P1  
**Description:** Full-featured web interface for rule creation, editing, and management.

**Acceptance Criteria:**

- Web-based IDE with syntax highlighting for Drools/DRL rules
- Decision table editor with all hit policies (Unique, Any, Priority, Collect, etc.)
- Guided rule editor for business users without coding experience
- Real-time validation with inline error messages
- Side-by-side version comparison for branch/merge workflow

---

### FRONTEND-02: Rule Management Dashboard

**Category:** User Experience  
**Priority:** P1  
**Description:** Central dashboard for monitoring and managing all rules.

**Acceptance Criteria:**

- List view of all rules with filtering by category, owner, status
- Quick access to edit, clone, or delete rules
- Visual indicators for rule health (zero-hit rules, conflicts)
- Bulk operations support (disable multiple rules at once)

---

### GOV-01: Complete RBAC Implementation

**Category:** Governance  
**Priority:** P1  
**Description:** Full role-based access control with predefined roles and permissions.

**Acceptance Criteria:**

- Admin: Full access to all features and settings
- Author: Create, edit, and submit rules for approval
- Reviewer: Approve or reject submitted rule changes
- Viewer: Read-only access to rules and execution results

---

### GOV-02: Maker-Checker Approval Workflow

**Category:** Governance  
**Priority:** P1  
**Description:** Enforce maker-checker pattern for all rule modifications.

**Acceptance Criteria:**

- Rule changes require submission before deployment
- Reviewers receive notifications for pending approvals
- Approved changes can be deployed to production
- Rejected changes return to author with feedback

---

### GOV-03: Full Audit Trail

**Category:** Governance  
**Priority:** P1  
**Description:** Complete audit log of all system activities.

**Acceptance Criteria:**

- Track all rule modifications with user, timestamp, and diff
- Log access events for compliance reporting
- Exportable audit reports in CSV/JSON formats
- Immutable audit storage (append-only)

---

### GOV-04: Version Control & Branching

**Category:** Governance  
**Priority:** P1  
**Description:** Full Git-backed version control with branching and merging.

**Acceptance Criteria:**

- Create branches for rule development without affecting production
- Merge branches to main with conflict resolution
- View history of all changes to rules
- Compare versions with side-by-side diff view

---

### MON-01: Prometheus Metrics Integration

**Category:** Monitoring  
**Priority:** P1  
**Description:** Export engine metrics in Prometheus format.

**Acceptance Criteria:**

- HTTP endpoint `/metrics` serves Prometheus-formatted metrics
- Track rule firing rates, match counts, memory usage
- Engine-specific metrics per algorithm (Rete, TREAT, LEAPS, PHREAK)
- Metrics include labels for rule names, categories, owners

---

### MON-02: Real-Time Monitoring Dashboard

**Category:** Monitoring  
**Priority:** P1  
**Description:** Live dashboard showing current engine state and activity.

**Acceptance Criteria:**

- Real-time rule firing visualization
- Current working memory status (fact count, memory usage)
- Active sessions and their resource consumption
- Alerting for anomalous behavior (high latency, zero-hit rules)

---

### MON-03: Dead Rule Detection

**Category:** Monitoring  
**Priority:** P2  
**Description:** Identify rules that never fire despite being active.

**Acceptance Criteria:**

- Daily analysis of rule execution logs
- Report rules with zero hits in configurable time window
- Optional notification to rule owners
- Dashboard widget showing top dead rules by category

---

### MON-04: Rule Coverage Reporting

**Category:** Monitoring  
**Priority:** P2  
**Description:** Track which rules execute during test runs and production.

**Acceptance Criteria:**

- Coverage data per test run with pass/fail status
- Historical coverage trends over time
- Exportable reports for compliance audits

---

### API-04: Complete Management API

**Category:** API Layer  
**Priority:** P1  
**Description:** REST API endpoints for all engine management operations.

**Acceptance Criteria:**

- CRUD endpoints for rules, categories, and tags
- Engine management endpoints (select, reload, stats)
- Approval workflow endpoints (submit, review, approve, reject)
- Batch execution endpoint for performance-critical scenarios

---

### DEPLOY-03: Docker/Kubernetes Support

**Category:** Deployment  
**Priority:** P1  
**Description:** Production-ready container deployment.

**Acceptance Criteria:**

- Multi-stage Dockerfile with layer optimization
- Kubernetes manifests (Deployment, Service, ConfigMap)
- Environment variable-based configuration
- Health and readiness probes for K8s

---

### DEPLOY-04: Hot Reload Support

**Category:** Deployment  
**Priority:** P2  
**Description:** Update rules without service restart.

**Acceptance Criteria:**

- Detect file changes in watched directories
- Rebuild and reload rule sets atomically
- No dropped executions during reload
- Graceful degradation if rebuild fails

---

## Milestone v3.0: Multiple Rule Engine Implementations (In Progress)

This milestone implements multiple Rete-family rule engine algorithms with a pluggable architecture allowing runtime selection.

### Research Summary

**Algorithm Selection Criteria:**

1. **Rete**: Foundational algorithm, well-documented, party tokens for pattern matching
2. **TREAT**: Optimized left-memory management, join node caching for efficiency
3. **LEAPS**: Aggregation node support for SUM/COUNT/MAX/MIN operations
4. **PHREAK**: Incremental truth maintenance with justification tracking

**Architecture Patterns:**

- Trait-based engine interface for pluggability
- Factory pattern for engine registration and instantiation
- Configuration-driven selection (YAML) + API endpoint override
- Metrics collection via unified instrumentation trait

---

## v3 Requirements - Pluggable Engine Architecture

### ENGINE-01: Pluggable Engine Architecture

**Category:** Core Engine  
**Priority:** P1  
**Description:** Rule engine architecture supporting multiple Rete-family algorithms with runtime selection.

**Acceptance Criteria:**

- Engine trait defines core methods: `execute`, `insert_fact`, `remove_fact`, `run`
- EngineRegistry supports dynamic registration by algorithm name
- Configuration file specifies engine to use (e.g., `engine = "rete"`)

---

### ENGINE-02: Native Rete Algorithm

**Category:** Core Engine  
**Priority:** P1  
**Description:** Full Rete algorithm implementation with party tokens, alpha/beta memory, and network caching.

**Acceptance Criteria:**

- Party tokens (MemoryReference, Nagrams) for efficient pattern matching
- Alpha memory network with test nodes and join nodes
- Beta memory maintained at join nodes for partial matches
- Network caching to avoid reevaluating unchanged facts

---

### ENGINE-03: TREAT Algorithm

**Category:** Core Engine  
**Priority:** P1  
**Description:** TREAT algorithm implementation with optimized left-memory management.

**Acceptance Criteria:**

- Optimized left-memory management with join node caching
- Partial matching without full Nagram creation when possible
- Equivalence with Rete results for same inputs

---

### ENGINE-04: LEAPS Algorithm

**Category:** Core Engine  
**Priority:** P1  
**Description:** LEAPS algorithm implementation with aggregation node support.

**Acceptance Criteria:**

- Aggregation nodes for SUM, COUNT, MAX, MIN operations
- Efficient partial state updates when base facts change
- Equivalence with Rete results for same inputs

---

### ENGINE-05: PHREAK Algorithm

**Category:** Core Engine  
**Priority:** P2  
**Description:** PHREAK algorithm implementation with incremental truth maintenance.

**Acceptance Criteria:**

- Incremental truth maintenance with justifications
- Dependency chains tracked for fact derivation
- Equivalence with Rete results for same inputs

---

### ENGINE-06: Runtime Engine Selection

**Category:** Core Engine  
**Priority:** P1  
**Description:** Users can select engine via configuration or API endpoint.

**Acceptance Criteria:**

- Configuration file specifies engine name
- HTTP API endpoint `/engine/select` allows runtime switching
- Active sessions support graceful engine migration (or error if incompatible)

---

### METRIC-01: Engine Metrics Collection

**Category:** Monitoring  
**Priority:** P2  
**Description:** Track execution characteristics per algorithm.

**Acceptance Criteria:**

- Rule firing rate measured per engine
- Memory usage tracked for alpha/beta memory structures
- Network traversal counts recorded (test node visits, join activations)
- Metrics exportable in Prometheus format

---

### METRIC-02: Algorithm Comparison Dashboard

**Category:** Monitoring  
**Priority:** P3  
**Description:** Web dashboard displays algorithm performance characteristics.

**Acceptance Criteria:**

- Real-time engine metrics displayed
- Historical comparison of algorithm performance available
- Exportable reports for benchmarking

---

## Milestone v2.0: Testing Framework (Completed)

This milestone established a comprehensive testing infrastructure for the rule engine, enabling reliable development and verification of business rules.

### Research Summary

**Stack Recommendations:**

- **Core Testing:** criterion (benchmarking), mockall (mocking), serial_test (parallel isolation)
- **BDD/Gherkin:** cucumber-rs (official Cucumber implementation), gherkin v15 parser
- **Integration:** assert_cmd (CLI tests), wiremock (HTTP mocks), tempfile (cleanup)

**Critical Pitfalls to Address:**

1. **Infinite Loop Detection** — Implement max 1000 firings limit per test scenario
2. **State Isolation** — Use per-test engine instances with unique session IDs  
3. **Non-Deterministic Firing** — Implement deterministic conflict resolution (sort by name as tiebreaker)
4. **Test Data Cleanup** — Use RAII patterns for fact lifecycle management

---

## v2 Requirements - Testing Framework

### TEST-01: Working Memory Fixture Support

**Category:** Core Testing  
**Priority:** P1  
**Description:** Users can create and manage test fixtures with reproducible working memory state.

**Acceptance Criteria:**

- Tests can create Fact collections as fixtures
- Each test gets a fresh engine instance with isolated session ID
- Facts can be inserted via `WorkingMemory::insert_fact()`
- Facts can be retracted via `WorkingMemory::retract()`

---

### TEST-02: Rule-by-Rule Unit Tests

**Category:** Core Testing  
**Priority:** P1  
**Description:** Developers can test individual rules in isolation with known facts.

**Acceptance Criteria:**

- Test framework recognizes `#[test]` macros for rule tests
- Rules can be compiled and executed individually
- Assertions can verify expected actions execute
- Test output shows pass/fail per rule

---

### TEST-03: Forward Chaining Verification

**Category:** Core Testing  
**Priority:** P1  
**Description:** Tests can verify that asserted facts trigger expected subsequent firings in correct order.

**Acceptance Criteria:**

- Tests can track activation sequence during forward chaining
- Salience ordering is preserved during execution
- Firing traces are captured and verifiable
- Tests fail with clear error if chain doesn't match expectation

---

### TEST-04: Backward Chaining Trace Capture

**Category:** Core Testing  
**Priority:** P1  
**Description:** For query-based inference, tests capture the reasoning path for debugging.

**Acceptance Criteria:**

- InferenceTrace structure is exposed via public API
- Tests can inspect backward chaining paths
- Missing facts are tracked and reportable
- Trace includes rule applications and fact matches

---

### TEST-05: Deterministic Firing Mode

**Category:** Core Testing  
**Priority:** P1  
**Description:** Eliminate flaky tests by enforcing deterministic rule firing order.

**Acceptance Criteria:**

- Tests can enable deterministic mode (cfg(test))
- Conflict resolution sorts equal-salience rules by name
- Same input always produces same output sequence
- Test logs include actual firing order for debugging

---

### TEST-06: No-Loop/Lock-on-Active Verification

**Category:** Advanced Testing  
**Priority:** P2  
**Description:** Tests verify that no_loop and lock_on_active flags prevent infinite loops.

**Acceptance Criteria:**

- Rules with `no_loop=true` don't re-fire after actions execute
- Rules with `lock_on_active=true` don't trigger when already active
- Tests can assert these constraints are enforced
- Clear error messages if violation detected

---

### TEST-07: Snapshot-Based Testing

**Category:** Advanced Testing  
**Priority:** P2  
**Description:** Save working memory state as JSON for reproducible tests.

**Acceptance Criteria:**

- WorkingMemory can serialize to JSON format
- Test snapshots can be loaded as baseline
- Tests compare against snapshot or allow updates
- Snapshots stored in `tests/snapshots/` directory

---

### TEST-08: Rule Coverage Metrics

**Category:** Advanced Testing  
**Priority:** P3  
**Description:** Track which conditions were evaluated during test runs.

**Acceptance Criteria:**

- Coverage instrumentation points at rule conditions
- Execution traces record condition evaluations
- Reports show coverage percentage per rule
- HTML report generation supported

---

## v1 Requirements (Completed)

Requirements for initial release. Each maps to roadmap phases.

### Core Engine (BRE) - COMPLETED

- [x] **CORE-01**: Rule engine with forward chaining inference using Rete algorithm
- [x] **CORE-02**: Rule engine with backward chaining inference for goal-driven reasoning  
- [x] **CORE-03**: Support for both stateful and stateless rule execution modes
- [x] **CORE-04**: Rule salience/prioritization and agenda control
- [x] **CORE-05**: Conflict resolution strategies for rule firing order
- [x] **CORE-06**: Event-driven rule triggering capabilities
- [x] **CORE-07**: Loop detection and control (no-loop, lock-on-active)
- [x] **CORE-08**: Caching of compiled rule sets (KIE bases)

### Rule Authoring & Modeling - COMPLETED

- [x] **RULE-01**: Web-based IDE for rule authoring with syntax highlighting
- [x] **RULE-02**: Decision table editor (spreadsheet-style grids)
- [x] **RULE-03**: Guided rule editor (form-based UI for business users)
- [x] **RULE-04**: Rule templates with parameterization for reuse
- [x] **RULE-05**: Expression language support (MVEL-like syntax)
- [x] **RULE-06**: Inline rule validation and error reporting

### REST API & Integration - COMPLETED

- [x] **API-01**: REST API endpoint for rule execution (POST /rules/execute)
- [x] **API-02**: REST API for rule management (CRUD operations on rules)
- [x] **API-03**: OpenAPI/Swagger specification for all endpoints
- [ ] **API-04**: Container deployment support (Docker image) - Partial
- [ ] **API-05**: Hot deployment of rule updates without restart

### Deployment & Runtime

- [x] **DEPLOY-01**: Docker container with production-ready configuration
- [x] **DEPLOY-02**: Kubernetes manifests for orchestration
- [ ] **DEPLOY-03**: Multi-environment support (dev, test, staging, prod)

### Governance & Access Control - COMPLETED (Stub Only)

- [ ] **AUTH-01**: Role-Based Access Control (RBAC) with predefined roles - Stub only
- [ ] **AUTH-02**: Maker-checker approval workflow for rule changes - Stub only
- [ ] **AUTH-03**: Rule ownership assignment and stewardship - Stub only
- [x] **GOV-01**: Full audit trail of all rule modifications - Stub only
- [x] **GOV-02**: Rule version control with branching and merging - Stub only

### Testing & Validation - COMPLETED (Stub Only)

- [x] **TEST-01**: Unit testing framework for individual rules - Stub implementation
- [x] **TEST-02**: Scenario-based simulation (what-if analysis) - Stub implementation
- [x] **TEST-03**: Coverage reporting (which rules fired, which were skipped) - Stub implementation

### Monitoring & Analytics - COMPLETED (Stub Only)

- [x] **MON-01**: Real-time rule execution metrics (throughput, latency) - Stub implementation
- [x] **MON-02**: Rule hit counts and frequency analysis - Stub implementation
- [x] **MON-03**: Dead rule detection (rules that never fire) - Stub implementation
- [x] **MON-04**: Rule execution logging for audit - Stub implementation

### DMN Compliance - COMPLETED (Stub Only)

- [x] **DMN-01**: Full DMN Level 3 compliance (OMG standard)
- [x] **DMN-02**: Support for Decision Tables, Contexts, Literals
- [x] **DMN-03**: Built-in functions and FEEL expression language support

---

## Out of Scope

Explicitly excluded. Documented to prevent scope creep.

| Feature | Reason |
|---------|--------|
| Mobile native apps | Web-first, responsive UI serves all user types |
| Legacy system migration tools | New implementation focus only |
| In-process embedded SDK | Standalone service architecture prioritized |
| OAuth/social login | Email/password authentication first; integrate with enterprise auth |

---

## Traceability

Which phases cover which requirements. Updated during roadmap creation.

### Milestone v4.0 (New)

| Requirement | Phase | Status |
|-------------|-------|--------|
| FRONTEND-01, FRONTEND-02 | Phase 17: Production Frontend Architecture | Complete |
| GOV-01, GOV-02, GOV-03, GOV-04 | Phase 18: Enterprise Governance | Planned |
| MON-01, MON-02, MON-03, MON-04 | Phase 19: Monitoring & Metrics | Planned |
| API-04 | Phase 20: Complete Management API | Planned |
| DEPLOY-03, DEPLOY-04 | Phase 21: Deployment & CI/CD | Planned |

**Phase 17 Coverage:**

- FRONTEND-01: Production Frontend Architecture (syntax highlighting, decision table editor)
- FRONTEND-02: Rule Management Dashboard with filtering/sorting
- GOV-01: RBAC implementation in frontend navigation controls

### Milestone v3.0 (In Progress)

| Requirement | Phase | Status |
|-------------|-------|--------|
| ENGINE-01, ENGINE-02, ENGINE-03, ENGINE-04, ENGINE-05, ENGINE-06 | Phase 10: Pluggable Engine Architecture & Implementations | In Progress |
| METRIC-01, METRIC-02 | Phase 11: Metrics & Comparison Dashboard | Planned |

### Milestone v2.0 (Completed)

| Requirement | Phase | Status |
|-------------|-------|--------|
| TEST-01, TEST-02, TEST-03, TEST-04, TEST-05 | Phase 7: Testing Infrastructure Foundation | Complete |
| TEST-06, TEST-07, TEST-08 | Phase 8: Advanced Testing Features | Complete |

### Milestone v1.0 (Completed)

| Requirement | Phase | Status |
|-------------|-------|--------|
| CORE-01 through CORE-08 | Phase 1: Core Engine Foundation | Completed |
| RULE-01 through RULE-06 | Phase 2: Rule Authoring Interface | Completed |
| API-01, API-02, API-03, DEPLOY-01, DEPLOY-02 | Phase 3: REST API Layer | Completed |
| AUTH-01, AUTH-02, GOV-01, GOV-02 | Phase 4: Governance & Access Control | Completed (stub) |
| TEST-01 through TEST-03 | Phase 5: Testing & Monitoring | Completed (stub) |
| MON-01 through MON-04 | Phase 5: Testing & Monitoring | Completed (stub) |
| DMN-01, DMN-02, DMN-03 | Phase 6: DMN Compliance | Completed (stub) |

**Coverage:**

- v1 requirements mapped: 39 total
- v2 requirements mapped: 8 testing framework requirements  
- v3 requirements mapped: 8 pluggable engine requirements
- v4 requirements mapped: 15+ production platform requirements (including Phase 17 frontend)

---

*Requirements defined: 2026-05-22*
*Last updated: 2026-06-09 after v4.0 milestone start*
