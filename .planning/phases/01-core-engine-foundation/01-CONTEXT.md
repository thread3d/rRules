# Phase 1: Core Engine Foundation - Context

**Gathered:** 2026-05-23
**Status:** Ready for planning

## Phase Boundary

Deliver a production-ready rule engine with Rete algorithm for efficient pattern matching and both forward/backward chaining inference. This includes the core data structures, execution modes (stateful/stateless), session management, rule lifecycle, and caching infrastructure.

## Implementation Decisions

### Architecture Strategy
- **D-01:** Hybrid module organization — Domain layers at top level with feature subdirectories (e.g., rete/arena/, rete/matcher/) to balance structure and flexibility
- **D-02:** Hybrid data structures — Typed internally but support dynamic casting and reflection for rule expressions; structured Rust structs with serde serialization as the base, with dynamic key-value capabilities for business users
- **D-03:** Hybrid Rete implementation — Start with a crate like `rete-rs` or `drools-rs` for faster MVP iteration, plan to replace with custom implementation if performance demands it

### Forward vs Backward Chaining
- **D-04:** Automatic selection — Engine detects the best inference mode based on rule structure and query type rather than requiring explicit API choice
- **D-05:** Fully mixed execution — Users can mix forward and backward chaining in a single execution; complex rules can trigger both modes dynamically
- **D-06:** Backward chaining primary use case is interactive debugging — Trace why rules fired or didn't fire by exploring the inference path

### Execution Modes
- **D-07:** No single default — Users must explicitly choose stateful session-based vs stateless execution based on their scenario; API should make both modes equally discoverable
- **D-08:** Explicit lifecycle management — Sessions are created/updated/destroyed explicitly via API calls rather than tied to request context or user ID
- **D-09:** Long-lived sessions (days to weeks) — Expected lifetime supports persistent working memory for ongoing rule applications, with explicit cleanup APIs

### Rule Lifecycle Management
- **D-10:** Hybrid approach — API for programmatic rule definition, file-based DSL for declarative rules stored in version control; both paths converge to the same compiled representation
- **D-11:** Syntax + semantic validation — Parse tree validation plus type checking and dependency resolution before compilation; errors caught early with clear messages
- **D-12:** Hot reload support — KIE base (compiled rule cache) updates dynamically when rules change without requiring service restart

### Claude's Discretion
- Specific Rust crates for Rete algorithm selection left to researcher/planner
- Exact API endpoint design (REST vs gRPC) deferred to Phase 3 API layer discussion
- Detailed session timeout and cleanup strategies can be fleshed out during implementation

## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project-Level
- `.planning/PROJECT.md` — Core value, constraints (Rust backend, React frontend), key decisions (hybrid Git+DB storage)
- `.planning/REQUIREMENTS.md` — All 39 v1 requirements; Phase 1 covers CORE-01 through CORE-08

### Roadmap
- `.planning/ROADMAP.md` — Phase details including success criteria and plan breakdown

### Phase-Specific
- `.planning/phases/01-core-engine-foundation/01-CONTEXT.md` — This file; implementation decisions for this phase

## Existing Code Insights

### Reusable Assets
(None yet — first phase, no existing codebase)

### Established Patterns
(None yet — first phase, establishing patterns)

### Integration Points
(Will be defined during implementation — expected to integrate with future Phase 2 web IDE and Phase 3 REST API)

## Specific Ideas

- Hybrid approach suggested across multiple areas (architecture, data structures, Rete implementation) indicates preference for flexibility over dogmatic single-strategy approaches
- Interactive debugging focus for backward chaining suggests emphasis on observability and traceability features
- Long-lived session requirement with explicit lifecycle implies need for durable storage backend (PostgreSQL per architecture)
- Hot reload support will require careful design to ensure atomic updates without dropping rule executions in progress

## Deferred Ideas

### Reviewed Todos (not folded)
(None — no todos were cross-referenced or reviewed)

### General
(None — discussion stayed within phase scope)

---

*Phase: 1-Core Engine Foundation*
*Context gathered: 2026-05-23*
