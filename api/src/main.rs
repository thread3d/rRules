//! Main entry point for rRules REST API server

use actix_web::{web, HttpServer};

// The modules live in the library (`lib.rs`); the binary is only process wiring.
use api::{auth, metrics, v1};

// Import execution endpoints for hot reload job tracking
use api::v1::execution::SharedRuleSet;
// Phase 27 (SESS-01): stateful session manager + background sweep task.
use api::v1::sessions::{read_session_config, run_session_sweep, SessionManager};

// Phase 23 (D-03 / L-07): real engine registry + selector + shared ruleset
// wired into the execute path. `EngineRegistry` carries all four factories
// (Plan 23-01); `EngineSelector` has its default set once at startup
// (Pitfall 7); the shared `Arc<RwLock<SharedRuleSet>>` is built from
// `RULE_STORAGE_PATH` and injected read-only (D-03a — empty dir logs a
// warning and continues with an empty ruleset rather than crashing).
// Phase 25 (Plan 25-02 / R1 / D-05): `build_ruleset_from_dir` now lives in
// `api::v1::execution` as a pub fn and is the single source of truth for
// both startup (here) and reload (`reload_and_swap`) — it parses `.rules`
// / `.drl` JSON files into `Rule` objects via `serde_json`.
use engine::dmn::evaluator::DecisionEvaluator;
use engine::engine::{EngineRegistry, EngineSelector};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // `RRULES_BIND_ADDRESS` and `PORT` decide where the server listens. The
    // default is loopback, which is right for a laptop and wrong for a container:
    // an image sets `RRULES_BIND_ADDRESS=0.0.0.0` so the port is reachable.
    let bind_address = {
        let host = std::env::var("RRULES_BIND_ADDRESS").unwrap_or_else(|_| "127.0.0.1".to_string());
        let port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());
        format!("{host}:{port}")
    };

    println!("rRules REST API Server v9.0.26");
    println!("Starting on http://{bind_address}");

    // Create engine state for hot reload functionality
    let engine_state = web::Data::new(v1::execution::EngineState::new());

    // Phase 23 (D-01 / L-07): build the shared engine registry, selector, and
    // read-only ruleset snapshot. The registry carries all four default
    // factories; the selector has its default engine set ONCE at startup
    // (mutably, before wrapping in `web::Data` — Pitfall 7: `select_engine`
    // takes `&mut self`, so per-request validation uses `available_engines()`
    // (`&self`) in `execute_single`, NOT `select_engine`).
    let mut registry = EngineRegistry::new();
    registry.register_default_engines();
    let registry = web::Data::new(registry);

    let mut selector = EngineSelector::with_defaults();
    let default_engine =
        std::env::var("RRULES_DEFAULT_ENGINE").unwrap_or_else(|_| "rete".to_string());
    if selector.select_engine(&default_engine).is_err() {
        tracing::warn!(
            engine = %default_engine,
            "Invalid RRULES_DEFAULT_ENGINE, falling back to rete"
        );
        let _ = selector.select_engine("rete");
    }
    let selector = web::Data::new(selector);

    // L-07 config: validate `RRULES_MAX_CHAIN_ITERATIONS` is parseable at
    // startup (warn on failure). The ReteEngine evaluator reads the env var
    // directly per Plan 23-03, so main.rs only validates — no shared state
    // passed to handlers for this knob.
    if let Ok(raw) = std::env::var("RRULES_MAX_CHAIN_ITERATIONS") {
        if raw.parse::<usize>().is_err() {
            tracing::warn!(
                value = %raw,
                "RRULES_MAX_CHAIN_ITERATIONS not a valid usize, evaluator will fall back to 1000"
            );
        }
    }

    // D-03: shared read-only `Arc<RwLock<SharedRuleSet>>` built once at
    // startup. Phase 25 (Plan 25-01 / D-02 / L-01): the shared payload is now
    // `SharedRuleSet { ruleset, generation: 0 }` so the execute path reads
    // the ruleset and generation atomically under one read lock (L-04 / L-05).
    // The generation is bumped on each successful reload (Plan 25-02's
    // `reload_and_swap`). The `Arc<RwLock<...>>` pattern is preserved — no
    // `ArcSwap` (L-01).
    let rules_dir = std::env::var("RULE_STORAGE_PATH").unwrap_or_else(|_| "./rules".to_string());
    let ruleset = SharedRuleSet::new(v1::execution::build_ruleset_from_dir(&rules_dir));
    let ruleset = web::Data::new(std::sync::Arc::new(std::sync::RwLock::new(ruleset)));

    // Phase 33 (Plan 33-01 / D-33-01): shared DMN model snapshot built once at
    // startup. Read `DMN_STORAGE_PATH` with fallback `"./dmn"`. If the directory
    // exists and contains at least one `.dmn` file, load via `parse_dmn_with_cache()`;
    // otherwise construct an empty SharedDmnModel (do NOT crash — model can be loaded
    // later via hot-reload). This mirrors Phase 25's SharedRuleSet pattern.
    let dmn_storage_path =
        std::env::var("DMN_STORAGE_PATH").unwrap_or_else(|_| "./dmn".to_string());
    // Every `.dmn` file in the directory, in sorted order, merged into one model. The old code
    // parsed `read_dir`'s first entry, so a deployment with two decision files evaluated against
    // an arbitrary one of them and a restart could switch.
    let dmn_models = std::path::Path::new(&dmn_storage_path).is_dir();
    let shared_dmn_model = if dmn_models {
        api::dmn_loader::load_models(std::path::Path::new(&dmn_storage_path))
    } else {
        tracing::info!(path = %dmn_storage_path, "no DMN storage directory; using an empty DMN model");
        engine::dmn::evaluator::SharedDmnModel::new(
            engine::dmn::model::DmnModel::default(),
            engine::dmn::feel::cache::FeelAstCache::default(),
        )
    };

    // Phase 33: create the DecisionEvaluator holding the shared snapshot.
    // Wrapped in `Arc` because actix-web's `web::Data<T>` requires `T: Clone`,
    // and `DecisionEvaluator` is not `Clone` (it holds an `Arc<RwLock<>>` internally).
    let dmn_evaluator = std::sync::Arc::new(DecisionEvaluator::new(std::sync::Arc::new(
        std::sync::RwLock::new(shared_dmn_model),
    )));

    // Phase 33 (Plan 33-01 Task 3): create the DMN metrics broadcast channel
    // for real-time WebSocket delivery of DmnMetricsSnapshot events. Capacity 16
    // matches the existing engine metrics channel pattern. The receiver is moved
    // into a background broadcaster task; the sender is cloned and injected as
    // `web::Data` so handlers can push snapshots after each evaluation.
    let (dmn_metrics_tx, dmn_metrics_rx) = v1::dmn_metrics::create_dmn_metrics_channel(16);
    v1::dmn_metrics::spawn_dmn_metrics_broadcaster(dmn_metrics_rx);

    // Create metrics broadcast channel for real-time updates (for future WebSocket use)
    let (metrics_tx, _) = metrics::create_metrics_channel(16);

    // Phase 24 (Plan 24-02, Deliverable 1 wiring): construct the per-request
    // ExecutionCounters once at startup and inject as web::Data. The counters
    // hold a dedicated Prometheus Registry (Pitfall 24-A) and the three
    // labeled metrics for POST /v1/execute. Inert in Plan 24-02 — no handler
    // reads them yet; Plan 24-03 wires execute_single and metrics_endpoint to
    // increment/render them.
    let counters = web::Data::new(metrics::ExecutionCounters::new());

    // Phase 27 (SESS-01 / D-02 / D-03 / D-08 / D-12): construct the shared
    // SessionManager once at startup. The `rrules_sessions_active` IntGauge
    // registers on the Phase-24 dedicated Prometheus Registry (NOT
    // default_registry — Pitfall 24-A: register() panics on duplicate names).
    // The manager is wrapped in Arc and shared across actix workers via
    // web::Data; a clone is moved into the background sweep task (D-10).
    let session_config = read_session_config();
    let sessions_active =
        prometheus::IntGauge::new("rrules_sessions_active", "Active stateful session count")
            .expect("construct rrules_sessions_active gauge");
    counters
        .registry
        .register(Box::new(sessions_active.clone()))
        .expect("register rrules_sessions_active on dedicated Registry");
    let session_manager = std::sync::Arc::new(SessionManager::new(session_config, sessions_active));
    let sweep_manager = session_manager.clone();

    // Initialize in-memory stores
    let rule_store = v1::rules::init_store();
    let category_store = v1::categories::init_store();
    let (tag_store, rule_tag_store) = v1::tags::init_stores();
    // Multi-tenancy: in-memory tenant registry (organisation/workspace CRUD).
    let tenant_store = v1::tenants::init_store();
    let approval_store = v1::approvals::init_store();
    let audit_store = v1::audit::init_store();
    let job_store = v1::execution::init_job_store();
    // Phase 22: in-memory user store seeded with 4 demo roles (D-22-01).
    let user_store = auth::init_store();

    // Get rules directory from environment and initialize git-backed version control.
    let rules_dir = std::env::var("RULE_STORAGE_PATH").unwrap_or_else(|_| "./rules".to_string());
    std::fs::create_dir_all(&rules_dir)?;

    let vcs_config = engine::governance::versioning::VcsConfig::new(&format!("{}/.git", rules_dir));
    let repo_path = vcs_config.repo_path.clone();
    let branch_store: web::Data<
        tokio::sync::Mutex<engine::governance::versioning::VersionControlRepo>,
    > = {
        let repo = engine::governance::versioning::VersionControlRepo::open_or_init(
            &repo_path, vcs_config,
        )
        .map_err(|e| std::io::Error::other(format!("Failed to init VCS: {}", e)))?;
        web::Data::new(tokio::sync::Mutex::from(repo))
    };

    tracing::info!(vcs_repo = %branch_store.lock().await.repo().path().display(), "Initialized git-backed version control");

    // Get frontend dist path from environment or use default
    let frontend_dist =
        std::env::var("FRONTEND_DIST").unwrap_or_else(|_| "../frontend/dist".to_string());

    println!("Frontend dist path: {}", frontend_dist);

    // Start file watcher in background
    let watch_rules = std::env::var("WATCH_RULES_DIR")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(true);

    if watch_rules {
        println!("Starting file watcher for rules directory: {}", rules_dir);
        tokio::spawn(v1::execution::start_file_watcher(rules_dir));
    }

    // Phase 27 (D-10): spawn the background session sweep task alongside
    // start_file_watcher. Loops on tokio::time::interval(sweep_interval) and
    // calls SessionManager::sweep_expired (evicts by TTL/max-lifetime/LRU +
    // updates the gauge). Lives for the process lifetime (no explicit
    // CancellationToken — matches start_file_watcher).
    println!(
        "Starting session sweep task (interval: {:?})",
        session_manager.sweep_interval
    );
    tokio::spawn(run_session_sweep(sweep_manager));

    // CORS allowed origins (D-22-05). Default: the vite dev origin
    // (http://localhost:5173). Override with `CORS_ALLOWED_ORIGINS` (comma-
    // separated) to support a separate API host in prod, e.g.
    // "https://app.rrules.example.com". The Phase 21 single-container deploy
    // is same-origin and does not need CORS, but D-22-05 keeps VITE_API_URL
    // configurable-absolute — CORS makes that mode real (RESEARCH Pitfall 2).
    // NEVER combine `allow_any_origin()` with `allow_credentials(true)` (T-22-05).

    let cors_allowed_origins: Vec<String> = std::env::var("CORS_ALLOWED_ORIGINS")
        .unwrap_or_else(|_| "http://localhost:5173".to_string())
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    println!("CORS allowed origins: {:?}", cors_allowed_origins);

    let state = api::app::ApiState {
        engine_state,
        registry,
        selector,
        ruleset,
        metrics_tx,
        counters,
        session_manager,
        dmn_evaluator,
        dmn_metrics_tx,
        rule_store,
        category_store,
        tag_store,
        rule_tag_store,
        tenant_store,
        approval_store,
        audit_store,
        branch_store,
        job_store,
        user_store,
        frontend_dist,
        cors_allowed_origins,
    };
    let server = HttpServer::new(move || api::app::build_app(&state)).bind(&bind_address)?;

    println!("Server running at http://{bind_address}");
    println!("Metrics endpoint available at /metrics");
    server.run().await
}
