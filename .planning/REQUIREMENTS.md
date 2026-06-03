# Requirements: rRules - Business Rules Management System

**Defined:** 2026-05-22  
**Updated:** 2026-06-03  
**Core Value:** Users can define complex business decisions as reusable rules, deploy them instantly, and monitor their impact — all without modifying application code.

## Milestone v2.0: Testing Framework

This milestone establishes a comprehensive testing infrastructure for the rule engine, enabling reliable development and verification of business rules.

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

### TEST-09: Gherkin Feature File Support (Deferred to v2.1)

**Category:** BDD Integration  
**Priority:** P3  
**Description:** Write tests in human-readable Given/When/Then syntax.

**Acceptance Criteria:**
- Parser accepts `.feature` files with Gherkin syntax
- Steps map to Rust test functions (#[given], #[when], #[then])
- Tests execute via cucumber-rs or compatible runner
- Integration with cargo test

---

### TEST-10: BDD Test Runner Integration (Deferred to v2.1)

**Category:** BDD Integration  
**Priority:** P3  
**Description:** Run Gherkin tests as part of regular test suite.

**Acceptance Criteria:**
- `cargo test` discovers and runs feature files
- Results reported alongside Rust unit tests
- CI/CD integration for feature file testing
- Failure reporting with step context

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

### Governance & Access Control - COMPLETED

- [ ] **AUTH-01**: Role-Based Access Control (RBAC) with predefined roles - Stub only
- [ ] **AUTH-02**: Maker-checker approval workflow for rule changes - Stub only
- [ ] **AUTH-03**: Rule ownership assignment and stewardship - Stub only
- [ ] **GOV-01**: Full audit trail of all rule modifications - Stub only
- [ ] **GOV-02**: Rule version control with branching and merging - Stub only

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

### Milestone v2.0 (Testing Framework)

| Requirement | Phase | Status |
|-------------|-------|--------|
| TEST-01, TEST-02, TEST-03, TEST-04, TEST-05 | Phase 7: Testing Infrastructure Foundation | Planned |
| TEST-06, TEST-07, TEST-08 | Phase 8: Advanced Testing Features | Planned |
| TEST-09, TEST-10 | Phase 9: BDD/Gherkin Integration (v2.1) | Deferred |

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
- v2 requirements mapped: 10 testing framework requirements

---

*Requirements defined: 2026-05-22*
*Last updated: 2026-06-03 after v2.0 milestone start*
