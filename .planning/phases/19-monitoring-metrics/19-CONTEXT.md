# Phase 19: Monitoring & Metrics Platform - Context

**Gathered:** 2026-06-12
**Status:** Ready for planning

## Phase Boundary

Deliver production-grade monitoring and metrics capabilities for the rRules BRMS:

- **MON-01:** Prometheus-format metrics endpoint (`/metrics`) with engine-level instrumentation
- **MON-02:** Real-time dashboard showing engine state, firing rates, and active sessions via WebSocket streaming
- **MON-03:** Dead rule detection with configurable inactivity thresholds (default 24 hours)
- **MON-04:** Full execution trace coverage reporting across test runs and production

This phase builds on the stub implementations in Phase 5 to provide actual production monitoring infrastructure, integrating with the pluggable engine architecture from Phase 10.

## Implementation Decisions

### Metrics Export Format (D-19-01)
- **Decision:** Support both Prometheus text format and JSON format based on HTTP Accept header
- **Rationale:** Works with standard Prometheus scrapers while also providing structured data for dashboard APIs
- **Implementation:** Content-negotiated endpoint returns `text/plain` (Prometheus) for `Accept: text/plain` and `application/json` for `Accept: application/json`
- **Code context:** Extend existing `engine/src/metrics/prometheus.rs` with JSON export path; modify `api/src/v1/metrics.rs` to handle accept header negotiation

### Real-Time Dashboard Data Source (D-19-02)
- **Decision:** WebSocket streaming for real-time engine state updates
- **Rationale:** Lowest latency push updates, ideal for active dashboards requiring live visibility into rule execution
- **Implementation:**
  - `/v1/metrics/ws` WebSocket endpoint in Actix Web
  - `useEngineMetrics` hook (frontend/src/hooks/) with automatic reconnect
  - Fallback to HTTP polling if WebSocket unavailable (already exists in useEngineMetrics)
- **Code context:** Frontend already has `frontend/src/hooks/useEngineMetrics.ts` with WS + polling fallback; needs backend endpoint

### Dead Rule Detection Scope (D-19-03)
- **Decision:** Configurable inactivity threshold (default 24 hours), analyzing all rules
- **Rationale:** Flexibility for different deployment scenarios while providing sensible defaults
- **Implementation:**
  - Read `inactivity_threshold` from config file or env variable (e.g., `DEAD_RULE_THRESHOLD_HOURS`)
  - Default: 24 hours if not specified
  - CLI flag `/v1/monitoring/dead-rules?threshold_hours=XX` for one-off analysis
- **Code context:** Existing `engine/src/monitoring/dead_rules.rs` has `SlidingWindowConfig` with default 24h; extend to read from config

### Coverage Reporting Depth (D-19-04)
- **Decision:** Full execution trace tracking
- **Rationale:** Complete debugging capability, enables advanced analysis of rule behavior patterns
- **Implementation:**
  - Track: rule firing + condition evaluation results (true/false/skipped) + working memory changes
  - Store traces with timestamps, rule IDs, and metadata
  - API endpoint for trace export (CSV/JSON)
- **Code context:** Existing `engine/src/monitoring/coverage.rs` has basic structure; extend to capture full trace

### Claude's Discretion

**Engine instrumentation integration:**
- How to hook metrics collection into the pluggable engine architecture (Rete/TREAT/LEAPS/PHREAK)
- Recommendation: Instrument at Engine trait level in `engine/src/engine/` for uniform data across all implementations

**Dashboard UI patterns:**
- Reuse existing `frontend/src/features/rules/` structure and styling
- Add monitoring-specific components to new `frontend/src/features/monitoring/` directory

## Canonical References

### Project-Level
- `.planning/PROJECT.md` — Core value, constraints (Rust backend, React frontend), hybrid Git+DB storage
- `.planning/REQUIREMENTS.md` §MON-01 through MON-04 — Acceptance criteria for monitoring features

### Phase 5 Context (stub implementations)
- `.planning/phases/05-testing-monitoring/05-CONTEXT.md` — Previous stub implementation patterns; D-05 to D-06 decisions on test framework and metrics

### Phase 10 Context (pluggable engine architecture)
- `.planning/phases/10-pluggable-engine/10-PLAN.md` or existing CONTEXT.md — Engine trait interface for instrumentation

### Roadmap
- `.planning/ROADMAP.md` §Phase 19 — Phase goal and success criteria
- `.planning/REQUIREMENTS.md` §MON-01 through MON-04 — Full acceptance criteria

## Existing Code Insights

### Reusable Assets

**Rust Backend:**
- `engine/src/metrics/mod.rs` — `MetricsCollector` trait for engine instrumentation; extend to add per-engine tracking
- `engine/src/metrics/prometheus.rs` — Prometheus text exporter; add JSON export path with accept-header routing
- `engine/src/engine/metrics.rs` — `EngineMetrics` struct and `SimpleMetricsCollector`; foundation for phase implementation
- `engine/src/monitoring/dead_rules.rs` — Dead rule detection with sliding window config; extend to read from config file

**Frontend:**
- `frontend/src/hooks/useEngineMetrics.ts` — WebSocket + HTTP polling hook; needs backend endpoint integration
- `frontend/src/features/monitoring/dashboards/` — Dashboard components ready for implementation:
  - `EngineMetricsDashboard.tsx` — Rules firing rate visualization
  - `DeadRuleReportDashboard.tsx` — Dead rule listing
  - `RuleCoverageDashboard.tsx` — Coverage tracking display

**API Layer:**
- `api/src/v1/metrics.rs` — stub endpoints (`get_engine_metrics`, `reset_coverage`); expand to full implementation

### Established Patterns

- **Hybrid Git + PostgreSQL storage** (PROJECT.md): Use DB for runtime metadata (metrics, health records), Git for versioned rule data
- **Rust optional features** (engine/Cargo.toml): prometheus crate is `optional = true` with `--features metrics`; design to work without it
- **React hooks pattern:** `useEngineMetrics` shows WebSocket connection lifecycle; standard pattern for real-time updates
- **Actix Web routing:** `/v1/` prefix, async handlers returning `impl Responder`

### Integration Points

- Phase 10 Engine implementations — Instrument Engine trait methods to record metrics
- Phase 4 Governance RBAC — Metrics endpoint should respect viewer role permissions
- Database migrations — New tables: `engine_metrics`, `rule_health`, `coverage_traces`
- Frontend routes — Add `/monitoring` path in `frontend/src/routes/_layout.gen.ts`

## Specific Ideas

### Directory Structure

**Rust backend extensions:**
```
engine/src/
├── metrics/
│   ├── mod.rs              # MetricsCollector trait (already exists)
│   ├── prometheus.rs       # Prometheus exporter (extends to JSON)
│   └── config.rs           # NEW: Read dead rule thresholds from config
├── monitoring/
│   ├── detection.rs        # Dead rule API routes wrapper
│   ├── coverage_ext.rs     # NEW: Full trace tracking extension
│   └── health_store.rs     # NEW: RuleHealth persistence layer
api/src/v1/
└── routes/
    ├── metrics.rs          # Extend with accept-header routing
    ├── monitoring.rs       # NEW: Dashboard API endpoints
    └── dead_rules.rs       # NEW: Dead rule detection endpoints
```

**Frontend additions:**
```
frontend/src/features/monitoring/
├── dashboards/
│   ├── EngineMetricsDashboard.tsx    # Extend with real WS connection
│   ├── DeadRuleReportDashboard.tsx   # Connect to /api/v1/dead-rules
│   └── RuleCoverageDashboard.tsx     # NEW: Full trace display
├── hooks/
│   └── useEngineMetrics.ts           # Add WebSocket path config
└── routes/
    └── monitoring.tsx                # New route for monitoring page
```

### Metrics Export Implementation Pattern

```rust
// api/src/v1/metrics.rs - Extended with accept-header routing
use actix_web::{web, HttpResponse, Responder};
use engine::metrics::{MetricsCollector, EnginePrometheusMetrics};

#[derive(Deserialize)]
struct MetricsQuery {
    #[serde(default)]
    format: Option<String>, // "json" or "prometheus"
}

pub async fn get_metrics(query: web::Query<MetricsQuery>) -> impl Responder {
    let metrics = collect_engine_metrics(); // Implement this
    
    match query.format.as_deref() {
        Some("json") | None => HttpResponse::Ok()
            .content_type("application/json")
            .json(serde_json::json!({ /* JSON format */ })),
        
        Some("prometheus") => HttpResponse::Ok()
            .content_type("text/plain; charset=utf-8")
            .body(metrics.export_prometheus_text()),
        
        _ => HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Invalid format. Use 'json' or 'prometheus'"
        })),
    }
}
```

### WebSocket Endpoint Pattern

```rust
// api/src/v1/routes/monitoring.rs - NEW
use actix_web::{web, HttpResponse};
use futures_util::{Stream, StreamExt};

pub async fn metrics_ws(
    req: web::HttpRequest,
    payload: web::Payload,
) -> Result<HttpResponse, actix_web::Error> {
    // WebSocket upgrade handler
    ws::start(req, payload, |mut frame| {
        // Send initial state on connect
        let initial = get_engine_state();
        
        async move {
            while let Some(msg) = frame.next().await {
                match msg? {
                    Text(text) => {
                        if text == "subscribe" {
                            // Start streaming updates
                            frame.send(Text(serde_json::to_string(&initial)?)).await?;
                        }
                    }
                    _ => {}
                }
            }
            Ok(())
        }
    })
}
```

### Dead Rule Detection Config

```toml
# config/monitoring.toml - NEW
[dead_rules]
threshold_hours = 24  # Default threshold
enabled = true

# CLI override example:
# /api/v1/dead-rules?threshold_hours=48
```

## Deferred Ideas

### Reviewed Todos (not folded)
(None — no todos were cross-referenced for Phase 19)

### Future Phases / Milestones
- Metrics historical retention with time-series database (InfluxDB/ClickHouse) — deferred to v5.0
- Automated rule health scoring (combining dead rules, firing patterns, performance) — deferred
- Alerting thresholds configurable per rule/category — deferred to future iteration
- Real-time alert streaming via WebSocket (not just dashboards) — deferred

---

*Phase: 19-Monitoring & Metrics Platform*
*Context gathered: 2026-06-12*
