# Phase 33: Decision Evaluator + DRD Resolution + REST API + Metrics - Research

**Researched:** 2026-07-09
**Domain:** DMN decision service evaluation, REST API integration, observability
**Confidence:** HIGH

## Summary

Phase 33 delivers the **decision evaluation service layer** that connects Phases 29–32's DMN parser/IR, FEEL evaluator, and decision-table engine to a REST API with full observability. The MVP implements §10.4 multi-output decision-service support via `POST /v1/decisions/{id}/evaluate`, with DRD chaining deferred but the infrastructure in place.

**Primary recommendation:** Extend the existing `engine::dmn::evaluator` module (already 80% complete) to add:
1. Multi-output evaluation for `DecisionService` nodes (iterate `output_decisions`)
2. DRD topological resolution with cycle detection (`required_decisions` → recursive evaluate)
3. Wire the existing handlers into `main.rs` and inject `SharedDmnModel`
4. Add metrics instrumentation to the evaluator's hot path

The phase is **infrastructure** — no user-facing behavior beyond API contracts. Phase 34 wires these contracts to the frontend editor.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Decision evaluation (§10.4) | `engine::dmn::evaluator` | — | Stateless, side-effect-free, three-valued; separate from Rete engines |
| DRD resolution | `engine::dmn::evaluator` (within evaluator) | — | Resolves `required_decisions` edges before evaluation |
| REST API (`POST /v1/decisions/{id}/evaluate`) | `api::v1::decisions` handlers | `web::Data<DecisionEvaluator>` | Actix-web routing, follows v5.0 patterns |
| Prometheus metrics | `api::v1::dmn_metrics` (dedicated registry) | — | Separate from main engine registry (Pitfall 24-A) |
| Inference trace | `engine::dmn::evaluator::trace` | — | Phase 28 pattern, opt-in via request field |
| Hot-reload (`SharedDmnModel`) | `api::v1::decisions::reload_dmn` | `engine_state` (file watcher) | Mirrors Phase 25's `SharedRuleSet` pattern |

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `actix-web` | 4.x | REST framework | Existing project standard (v5.0) |
| `prometheus` | 0.13 | Metrics | Dedicated registry pattern (Phase 24) |
| `serde_json` | 1.x | Serialization | Standard for API request/response |
| `thiserror` | 1.x | Error types | Existing project standard |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `tokio::sync::broadcast` | — | Metrics WebSocket broadcast | Phase 24 pattern, real-time updates |
| `dashmap` | — | Concurrent maps (if needed) | SessionManager uses it; not needed here |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Dedicated `Registry::new()` for DMN metrics | Process-global `default_registry()` | **Rejected:** Pitfall 24-A — `register()` panics on duplicate names across crates |
| Recursive DRD resolution (callee chains) | Flat caller chaining | MVP: flat is simpler; recursive deferred to Phase 35 (DMN-27) |

**Installation:**
```bash
# No new dependencies — all existing
```

## Package Legitimacy Audit

> **Required** whenever this phase installs external packages. Run the Package Legitimacy Gate protocol before completing this section.

| Package | Registry | Age | Downloads | Source Repo | Verdict | Disposition |
|---------|----------|-----|-----------|-------------|---------|-------------|
| (none) | — | — | — | — | — | No new packages required |

**Packages removed due to [SLOP] verdict:** none
**Packages flagged as suspicious [SUS]:** none

*No new dependencies required — phase uses existing actix-web, prometheus, serde_json.*

## Architecture Patterns

### System Architecture Diagram

```
┌─────────────────────────────────────────────────────────────┐
│  Client (Browser / API Consumer)                            │
│  POST /v1/decisions/{id}/evaluate                          │
└─────────────────────────────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────┐
│  api/src/v1/decisions.rs                                    │
│  • evaluate_decision() handler                              │
│  • Injected: web::Data<DecisionEvaluator>                   │
│  • Returns: EvaluateDecisionResponse (JSON)                 │
└─────────────────────────────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────┐
│  engine/src/dmn/evaluator/mod.rs                            │
│  DecisionEvaluator::evaluate(decision_id, request)          │
│                                                             │
│  Steps:                                                     │
│  1. Read SharedDmnModel (Arc<RwLock<>>)                    │
│  2. Find decision by ID in model.children                   │
│  3. If DecisionService: iterate output_decisions            │
│     └─ For each: resolve DRD (required_decisions)           │
│        ├─ Topological sort with cycle detection             │
│        └─ Recursively evaluate (deferred to Phase 35)       │
│  4. Build Scope from request.inputs                         │
│  5. Evaluate decision_table (if present)                    │
│     └─ DecisionTableEvaluator::evaluate(table, scope, cache)│
│  6. Collect outputs + trace events                          │
│  7. Increment Prometheus metrics                            │
│  8. Return EvaluateResult                                   │
└─────────────────────────────────────────────────────────────┘
                           │
              ┌────────────┼────────────┐
              ▼            ▼            ▼
     ┌──────────────┐ ┌──────────┐ ┌──────────────┐
     │ metrics.rs   │ │ trace.rs │ │ websocket.rs │
     │ (Prometheus) │ │ (trace)  │ │ (broadcast)  │
     └──────────────┘ └──────────┘ └──────────────┘
```

### Recommended Project Structure
```
engine/src/dmn/
├── evaluator/              # Phase 33: decision evaluation service layer
│   ├── mod.rs             # DecisionEvaluator, SharedDmnModel, evaluate()
│   └── trace.rs           # InferenceTrace, TraceEvent (Phase 28 pattern)
├── model.rs               # DMN IR (Phase 29) — consumed by evaluator
├── decision_table.rs      # Hit-policy evaluation (Phase 32) — consumed by evaluator
├── parser.rs              # XML → DmnModel + FeelAstCache (Phase 29)
└── feel/                  # FEEL value types, cache, eval (Phases 30–31)

api/src/v1/
├── decisions.rs           # Phase 33: REST handlers for /v1/decisions/*
├── dmn_metrics.rs         # Phase 33: dedicated Prometheus registry + metrics
└── mod.rs                 # configure_v1_decisions() wiring
```

### Pattern 1: Shared Snapshot Swap (Phase 25 pattern)
**What:** `Arc<RwLock<SharedDmnModel>>` with generation counter, mirroring `SharedRuleSet`. Hot-reload writes to a `ReloadJob` in `JobStore`, then swaps the shared snapshot.

**When to use:** Any phase that needs to reload DMN models without restart (Phase 28 / DMN-28).

**Example:**
```rust
// engine/src/dmn/evaluator/mod.rs
pub struct SharedDmnModel {
    pub model: DmnModel,
    pub ast_cache: FeelAstCache,
    generation: u64,
}

impl DecisionEvaluator {
    pub fn reload(&self, dmn_content: &str) -> Result<(), DecisionEvalError> {
        let (model, ast_cache) = crate::dmn::parser::parse_dmn_with_cache(dmn_content)?;
        let mut guard = self.shared.write().map_err(|e| {
            DecisionEvalError::Internal(format!("failed to acquire write lock: {}", e))
        })?;
        *guard = SharedDmnModel::new(model, ast_cache);
        guard.bump_generation();
        Ok(())
    }
}
```

### Pattern 2: Dedicated Prometheus Registry (Phase 24 pattern)
**What:** Separate `Registry` from the main one to avoid `register()` panics on duplicate names. Use `TextEncoder` for `/metrics` endpoint. Labels: `decision_id`, `hit_policy`.

**When to use:** Any phase that needs its own metrics namespace (Phase 33 / DMN-26).

**Example:**
```rust
// api/src/v1/dmn_metrics.rs
pub static DMN_METRICS_REGISTRY: LazyLock<Registry> = LazyLock::new(|| {
    let registry = Registry::new();
    let _ = registry.register(Box::new(METRIC_EVALUATE_REQUESTS_TOTAL.clone()));
    let _ = registry.register(Box::new(METRIC_EVALUATE_DURATION_SECONDS.clone()));
    let _ = registry.register(Box::new(METRIC_RULES_FIRED_TOTAL.clone()));
    registry
});

pub static METRIC_EVALUATE_REQUESTS_TOTAL: LazyLock<IntCounterVec> = LazyLock::new(|| {
    IntCounterVec::new(
        Opts::new("rrules_dmn_evaluations_total", "Total DMN decision evaluations"),
        &["decision_id"],
    ).expect("metrics to be valid")
});
```

### Pattern 3: Inference Trace Emission (Phase 28 pattern)
**What:** Opt-in via request field (`trace: bool`). `TraceEvent`/`TraceData` emitted at each evaluation step. Stored in-memory, served by `GET /v1/decisions/{id}/trace`.

**When to use:** Any phase that needs evaluation traceability (Phase 33 / DMN-25).

**Example:**
```rust
// engine/src/dmn/evaluator/mod.rs
pub fn evaluate(&self, decision_id: &str, request: EvaluateRequest) -> Result<EvaluateResult, DecisionEvalError> {
    let mut trace = if request.trace { Some(InferenceTrace::new(decision_id)) } else { None };

    // ... evaluation logic ...

    if let Some(ref mut t) = trace {
        t.emit_with("evaluate_table", "evaluated table", None, Vec::new(), Some(result.clone()));
    }

    Ok(EvaluateResult {
        outputs,
        rules_fired: 1,
        duration_ms: start.elapsed().as_secs_f64() * 1000.0,
        trace: trace.filter(|_| request.trace),
    })
}
```

### Anti-Patterns to Avoid
- **Hand-roll DMN parser:** Use Phase 29's `quick-xml` parser (already XXE-hardened, CDATA-handled).
- **Reuse rule-engine `Expression` AST:** DMN is stateless/three-valued; Rete is forward-chaining/two-valued. Separate paths (milestone anti-pattern avoided).
- **Register metrics on `default_registry()`:** Pitfall 24-A — use dedicated `Registry::new()` (Phase 33 / DMN-26).
- **Recursive DRD resolution in MVP:** Defer to Phase 35; MVP evaluates single decision only.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| DMN XML parsing | Custom XML parser | `engine::dmn::parser::parse_dmn_with_cache()` (Phase 29) | Namespace-aware, XXE-hardened, CDATA-handled |
| FEEL expression evaluation | Hand-roll FEEL evaluator | `engine::dmn::feel::eval::evaluate()` (Phase 31) | Full grammar rules 1–65, three-valued logic |
| Decision table hit policies | Implement each policy manually | `DecisionTableEvaluator::dispatch_hit_policy()` (Phase 32) | All 7 policies per Clause 8.2.12 |
| Prometheus metrics | Manual metric formatting | `prometheus` crate with dedicated registry | Pitfall 24-A avoidance, TextEncoder compliance |
| AST caching | Re-parse cell text on every evaluate | `FeelAstCache` (Phase 31) | Prevents 4–6× re-parse perf bug (DMN-18) |

**Key insight:** The evaluator's job is **orchestration** — it takes the parsed `DmnModel`, resolves DRD references, builds a `Scope<Value>`, calls `DecisionTableEvaluator::evaluate()`, and returns results + trace. It does NOT parse FEEL or evaluate hit policies directly.

## Runtime State Inventory

> Include this section for rename/refactor/migration phases only. Omit entirely for greenfield phases.

Phase 33 is a **greenfield infrastructure phase** — no runtime state migration required. The evaluator consumes the parsed `DmnModel` from Phase 29 and produces API responses via Phase 34's routes.

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| Stored data | None — DMN models are ephemeral (parsed on demand) | — |
| Live service config | None — no external service config for DMN evaluation | — |
| OS-registered state | None | — |
| Secrets/env vars | `RRULES_DEFAULT_ENGINE` (used by SessionManager, not DMN path) | — |
| Build artifacts | None | — |

**Nothing found in category:** All categories verified empty.

## Common Pitfalls

### Pitfall 1: DashMap Shard Guard Held Across Await
**What goes wrong:** `web::block` called while holding a `DashMap::Ref` guard → deadlock (actix-web workers can't drop the guard).

**Why it happens:** Same pattern as SessionManager Pitfall 1 — clone the Arc out, drop the Ref guard immediately.

**How to avoid:** In any handler that reads from a DashMap and then does async work:
```rust
// CORRECT: clone the Arc, drop the Ref guard before await
if let Some(entry_arc) = self.sessions.get(&uuid).map(|r| r.clone()) {
    // Drop the Ref guard here (map closure returns)
    // Now safe to do web::block or other async work with entry_arc
}
```

**Warning signs:** Handler hangs, actix-web logs "worker blocked", Prometheus scrape times out.

### Pitfall 2: Duplicate Metric Registration (Pitfall 24-A)
**What goes wrong:** `registry.register()` panics if a metric name is already registered on the process-global registry. Phase 33 uses a **dedicated** registry (`DMN_METRICS_REGISTRY`), but handlers must NOT read from `prometheus::default_registry()`.

**Why it happens:** Copy-paste from v5.0 metrics code that used the main `ExecutionCounters::registry`.

**How to avoid:** Always use `DMN_METRICS_REGISTRY.gather()` in Phase 33 handlers, never `prometheus::default_registry()`.

**Warning signs:** Process panics on startup with "duplicate metric registration"; or silent metric loss if using default registry.

### Pitfall 3: DRD Cycle Detection Missing
**What goes wrong:** Recursive DRD resolution loops infinitely (or until stack overflow) when the decision graph has cycles.

**Why it happens:** DMN allows cyclic DRDs (e.g., mutual reference between decisions); spec requires cycle detection.

**How to avoid:** Track visited decision IDs in a `HashSet<String>`; if a decision is already being evaluated, return an error or use cached result.

**Warning signs:** Stack overflow (`thread 'main' has overflowed its stack`), or infinite loop with no progress.

### Pitfall 4: Multi-Output Aggregation Misunderstanding
**What goes wrong:** Returning only the first output instead of all outputs for a `DecisionService`.

**Why it happens:** MVP scope says "evaluate all outputs"; implementation might shortcut to single-output.

**How to avoid:** Iterate `decision_service.output_decisions`, evaluate each, collect into a map keyed by decision name.

**Warning signs:** API response has only one output when the spec says multi-output; frontend shows incomplete results.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | `cargo test` (Rust standard) |
| Config file | `Cargo.toml` [dev-dependencies] |
| Quick run command | `cargo test -p engine --lib dmn::evaluator -- --nocapture` |
| Full suite command | `cargo test -p engine --features engine-rete,engine-treat,engine-leaps,engine-phreak` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| DMN-22 | `POST /v1/decisions/{id}/evaluate` returns multi-output | integration | `cargo test -p api --test integration::decisions::evaluate_multi_output` | ❌ Wave 0 |
| DMN-23 | DRD topological resolution works | unit | `cargo test -p engine --lib dmn::evaluator::drd_resolver` | ❌ Wave 0 |
| DMN-25 | Inference trace emitted when requested | integration | `cargo test -p api --test integration::decisions::trace_emission` | ❌ Wave 0 |
| DMN-26 | Prometheus metrics increment correctly | unit | `cargo test -p engine --lib dmn::evaluator::metrics_test` | ❌ Wave 0 |
| DMN-28 | Hot-reload swaps shared model | integration | `cargo test -p api --test integration::decisions::hot_reload` | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p engine --lib dmn::evaluator -- --nocapture`
- **Per wave merge:** `cargo test -p engine --features engine-rete,engine-treat,engine-leaps,engine-phreak`
- **Phase gate:** Full suite green before `/gsd-verify-work`

### Wave 0 Gaps
- [ ] `api/tests/integration/decisions.rs` — covers DMN-22 (multi-output)
- [ ] `engine/src/dmn/evaluator/mod.rs::tests` — covers DMN-23 (DRD resolution)
- [ ] Framework install: none (existing test infrastructure sufficient)

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Single-decision evaluation only | Multi-output decision service (§10.4) | Phase 33 MVP | Aligns with OMG DMN spec; frontend can evaluate complex DRDs |
| No metrics for DMN path | Dedicated registry + 3 metrics (DMN-26) | Phase 33 | Observability parity with v5.0 rule engine |
| Trace only for rules (Phase 28) | Trace for decisions too (DMN-25) | Phase 33 | Consistent debugging surface across both paths |

**Deprecated/outdated:**
- **Single-output evaluation:** Spec requires multi-output; MVP scope locked to this.
- **Process-global metrics registry:** Pitfall 24-A avoidance requires dedicated registry.

## Assumptions Log

> List all claims tagged `[ASSUMED]` in this research. The planner and discuss-phase use this section to identify decisions that need user confirmation before execution.

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `DecisionService::output_decisions` contains the IDs of decisions to evaluate (per spec §10.4) | Architecture Patterns | Medium — could be `encapsulated_decisions` instead; verify against Phase 35 scope |
| A2 | DRD resolution is deferred to Phase 35 (recursive), MVP is single-decision only | Architecture Patterns | Low — CONTEXT.md confirms "DRD chaining deferred to a follow-up if not already in scope" |
| A3 | `SharedDmnModel` uses `Arc<RwLock<>>` pattern (not `ArcSwap`) per L-01 design constraint | Runtime State Inventory | Medium — would require different reload strategy; verify against Phase 25 implementation |

**If this table is empty:** All claims in this research were verified or cited — no user confirmation needed.

## Open Questions

1. **Where does the DMN model live at runtime?**
   - What we know: `SharedDmnModel` is constructed from `parse_dmn_with_cache()` and stored in `Arc<RwLock<>>`.
   - What's unclear: Is it loaded from a file on disk (like Phase 25's `RULE_STORAGE_PATH`), or injected via an API call?
   - Recommendation: Load from `dmn_storage_path` env var at startup (mirrors `RULE_STORAGE_PATH`); support hot-reload via `POST /v1/decisions/reload`.

2. **How are multiple decisions in a `DecisionService` evaluated?**
   - What we know: Spec §10.4 says evaluate all output decisions; CONTEXT.md says "MVP: evaluate all outputs of a single decision".
   - What's unclear: Does "all outputs" mean iterating `output_decisions`, or just returning the result of the root decision?
   - Recommendation: MVP evaluates only the root decision (single-output); Phase 35 adds multi-output iteration.

## Environment Availability

> Skip this section if the phase has no external dependencies (code/config-only changes).

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust toolchain | Build | ✓ | — | — |
| `cargo test` | Tests | ✓ | — | — |
| actix-web 4.x | REST API | ✓ (existing) | — | — |
| prometheus 0.13 | Metrics | ✓ (existing) | — | — |

**Missing dependencies with no fallback:** none
**Missing dependencies with fallback:** none

## Security Domain

> Required when `security_enforcement` is enabled (absent = enabled). Omit only if explicitly `false` in config.

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | yes | JWT middleware (`api/src/middleware/auth.rs`) — already in place for `/v1/*` routes |
| V3 Session Management | no | DMN evaluation is stateless (no sessions) |
| V4 Access Control | yes | Maker-checker approval workflows (Phase 20-03) — decisions are immutable once approved |
| V5 Input Validation | yes | `serde_json::Deserialize` + actix-web `web::Json` provides basic validation; FEEL expressions validated by Phase 31 parser |
| V6 Cryptography | no | No cryptographic operations in DMN evaluation path |

### Known Threat Patterns for {stack}

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| XML External Entity (XXE) injection | Tampering | Phase 29 parser uses `quick-xml::NsReader` with XXE-hardened config |
| Denial of Service via large DMN files | Availability | File size limits in upload handler (Phase 34); not yet implemented |
| Metric scraping information disclosure | Information Exposure | Metrics endpoint is public (like v5.0 `/metrics`), but labels are sanitized (no PII) |

## Sources

### Primary (HIGH confidence)
- `/camunda/dmn-scala` - decision service evaluation patterns, audit log structure
- `engine/src/dmn/evaluator/mod.rs` - existing DecisionEvaluator implementation (80% complete)
- `engine/src/dmn/model.rs` - DMN IR structure (Phase 29)
- `api/src/v1/decisions.rs` - existing REST handler stubs (Phase 33)

### Secondary (MEDIUM confidence)
- [OMG DMN 1.5 Specification §10.4](https://www.omg.org/spec/DMN/1.5/PDF/changebar) - decision service API contract
- QuantumBPM documentation - multi-output evaluation pattern, DRD resolution

### Tertiary (LOW confidence)
- WebSearch results for OMG DMN spec — verified against Context7 library docs

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - all libraries are existing project dependencies
- Architecture: HIGH - patterns documented in CONTEXT.md and v5.0 reference implementations
- Pitfalls: MEDIUM - inferred from SessionManager/EngineRegistry pitfalls; not yet observed in DMN path

**Research date:** 2026-07-09
**Valid until:** 2026-08-08 (30 days for stable patterns)
