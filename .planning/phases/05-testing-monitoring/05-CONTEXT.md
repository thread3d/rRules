# Phase 5: Testing & Monitoring - Context

**Gathered:** 2026-05-24
**Status:** Ready for planning

## Phase Boundary

Provide tools for testing rules in isolation and monitoring execution in production. This includes BDD-style test framework for rule validation, scenario-based simulation engine for what-if analysis, full execution trace coverage reporting, adaptive-sampling metrics collection, and unified dashboards showing both technical and business KPIs.

## Requirements (locked by Phase 4 decisions)

Phase 1 established:
- Hybrid approach: Both API and file-based DSL for rule definition
- Full tracing capability for debugging

Phase 2 established:
- Dual-pane editor with CodeMirror 6 and ag-Grid decision tables
- Guided rule builder with adaptive vocabulary

Phase 3 established:
- REST/GraphQL hybrid API with versioning (/v1/)
- Webhook callbacks for async jobs

Phase 4 established:
- RBAC with admin, author, reviewer roles
- Multi-tier approval workflow
- Full audit trail tracking

This phase focuses on observability and testing capabilities.

## Implementation Decisions

### Test Framework Structure (D-05)
- **TF-S1:** BDD-style testing - Gherkin syntax for business-readable test scenarios
- Test format example:
  ```
  Feature: High-value customer discount
    Scenario: Customer exceeds spending threshold
      Given a customer with annual_spend = 15000
      When rules are executed
      Then rule "high_value_discount" fires
      And tag "high_priority" is added to customer
  ```

### Coverage Reporting Needs (D-05)
- **CR-N1:** Full execution trace - Complete path through rule network with timing per rule
- Trace includes:
  - Rule firing order and timing
  - Fact matching details at each node
  - Condition evaluation results
  - Action execution summary

### Metrics Collection Strategy (D-06)
- **MC-S1:** Adaptive sampling - High-sampling for errors, low-sampling for success
- Sampling logic:
  - Error events: 100% capture
  - Success events with high latency (>1s): 100% capture
  - Normal success events: 1% sample rate

### Dashboard KPIs (D-06)
- **DK-K1:** Both combined - Technical + business KPIs on unified dashboard
- Dashboard sections:
  - Technical: Throughput (RPS), latency P50/P95/P99, error rates
  - Business: Rules executed per hour, decisions made, approval times

## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project-Level
- `.planning/PROJECT.md` — Core value, constraints (Rust backend, React frontend)

### Phase 1 Context (locked decisions)
- `.planning/phases/01-core-engine-foundation/01-CONTEXT.md` — Hybrid approach

### Phase 2 Context (locked decisions)
- `.planning/phases/02-rule-authoring-interface/02-CONTEXT.md` — Dual-pane editor, ag-Grid

### Phase 3 Context (locked decisions)
- `.planning/phases/03-rest-api-layer/03-CONTEXT.md` — REST/GraphQL API with session auth

### Phase 4 Context (locked decisions)
- `.planning/phases/04-governance-access-control/04-CONTEXT.md` — RBAC, approval workflow, audit trail

### Roadmap
- `.planning/ROADMAP.md` — Phase 5 details, success criteria, and plan breakdown

## Existing Code Insights

### Reusable Assets
(None yet — initial project setup; Phases 1-4 will establish patterns)

### Established Patterns
(None yet — first phases of implementation)

### Integration Points
Phase 5 must integrate with:
- Phase 1 Engine - Test framework wraps execute_forward/execute_backward
- Phase 2 Web UI - Coverage reports display, test editor interface
- Phase 3 REST API - Metrics collection via middleware, dashboard API
- Phase 4 RBAC - Audit log access control based on roles

## Specific Ideas

### Testing Directory Structure
```
src/testing/
├── Cargo.toml
├── lib.rs
├── bdd/                 # BDD-style testing framework
│   ├── mod.rs
│   ├── parser.rs       # Gherkin syntax parsing
│   └── runner.rs       # Feature file execution
├── simulation/          # What-if analysis engine
│   ├── mod.rs
│   ├── scenario.rs     # Scenario definition and execution
│   └── results.rs      # What-if comparison output
└── coverage/            # Coverage reporting
    ├── mod.rs
    ├── tracker.rs      # Rule/network coverage tracking
    └── reporter.rs     # HTML/PDF report generation

src/monitoring/
├── Cargo.toml
├── lib.rs
├── metrics/             # Metrics collection
│   ├── mod.rs
│   ├── collector.rs    # Adaptive sampling logic
│   └── histogram.rs    # Latency tracking (P50, P95, P99)
└── dashboard/           # Monitoring dashboards
    ├── mod.rs
    ├── api.rs          # Dashboard data endpoints
    └── kpis.rs         # Technical + business KPI definitions
```

## Deferred Ideas

### Reviewed Todos (not folded)
(None — no todos were cross-referenced or reviewed)

### General
(None — discussion stayed within phase scope)

---

*Phase: 5-Testing & Monitoring*
*Context gathered: 2026-05-24*
