//! Main entry point for rRules REST API server

use actix_files as fs;
use actix_web::{web, App, HttpServer};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::openapi::ApiDoc;

mod health;
mod metrics;
mod openapi;
mod spa;

// Phase 22 modules
mod auth;
mod middleware;
mod websocket;

// Phase 20 modules
pub mod v1;

// Import execution endpoints for hot reload job tracking
use crate::v1::execution::{get_job_status, list_jobs, SharedRuleSet};
use crate::v1::routes::monitoring::configure_monitoring;
// Phase 27 (SESS-01): stateful session manager + background sweep task.
use crate::v1::sessions::{read_session_config, run_session_sweep, SessionManager};

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
use engine::dmn::evaluator::{DecisionEvaluator, SharedDmnModel};
use engine::dmn::feel::cache::FeelAstCache;
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

    println!("rRules REST API Server v9.0.4");
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
    let shared_dmn_model = if std::path::Path::new(&dmn_storage_path).is_dir() {
        let entries: Vec<_> = match std::fs::read_dir(&dmn_storage_path) {
            Ok(rd) => rd.filter_map(|e| e.ok()).collect(),
            Err(_) => vec![],
        };
        let dmn_files: Vec<_> = entries
            .iter()
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "dmn"))
            .collect();
        if !dmn_files.is_empty() {
            tracing::info!(path = %dmn_storage_path, files = dmn_files.len(), "Loading DMN model from storage");
            let first_dmn = &dmn_files[0];
            match std::fs::read_to_string(first_dmn.path()) {
                Ok(content) => match engine::dmn::parser::parse_dmn_with_cache(&content) {
                    Ok((model, ast_cache)) => SharedDmnModel::new(model, ast_cache),
                    Err(e) => {
                        tracing::warn!(error = %e, "Failed to parse first .dmn file, using empty model");
                        SharedDmnModel::new(
                            engine::dmn::model::DmnModel::default(),
                            FeelAstCache::default(),
                        )
                    }
                },
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to read first .dmn file, using empty model");
                    SharedDmnModel::new(
                        engine::dmn::model::DmnModel::default(),
                        FeelAstCache::default(),
                    )
                }
            }
        } else {
            tracing::info!(path = %dmn_storage_path, "No .dmn files found in storage path; using empty DMN model");
            SharedDmnModel::new(
                engine::dmn::model::DmnModel::default(),
                FeelAstCache::default(),
            )
        }
    } else {
        tracing::info!(path = %dmn_storage_path, "DMN storage path does not exist; using empty DMN model");
        SharedDmnModel::new(
            engine::dmn::model::DmnModel::default(),
            FeelAstCache::default(),
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

    let server = HttpServer::new(move || {
        // Build the CORS middleware per worker from the configured origins.
        // Origins are pinned (not wildcard), so the Bearer-token auth flow
        // works without `allow_credentials(true)` — credentials (cookies) are
        // not used by the JWT auth scheme.
        let cors = {
            let mut c = actix_cors::Cors::default()
                .allowed_methods(["GET", "POST", "PUT", "PATCH", "DELETE", "OPTIONS"])
                // `X-Tenant-ID` is the only tenancy key the API trusts, so a
                // browser client must be allowed to send it; without it here
                // every cross-origin request silently lands in the default
                // tenant.
                .allowed_headers(["Authorization", "Content-Type", "Accept", "X-Tenant-ID"])
                .max_age(3600);
            for origin in &cors_allowed_origins {
                c = c.allowed_origin(origin.as_str());
            }
            c
        };
        App::new()
            .wrap(cors)
            .app_data(engine_state.clone())
            // Phase 23 (D-03 / L-07): real engine wiring for POST /v1/execute.
            .app_data(registry.clone())
            .app_data(selector.clone())
            .app_data(ruleset.clone())
            .app_data(web::Data::new(metrics_tx.clone()))
            // Phase 24 (Plan 24-02): inject ExecutionCounters as web::Data
            // alongside metrics_tx. Inert until Plan 24-03 wires the handlers.
            .app_data(counters.clone())
            // Phase 27 (D-12): inject the shared SessionManager as web::Data
            // so execute_rules / dispose_session / reset_session can extract it.
            .app_data(web::Data::new(session_manager.clone()))
            // Phase 33 (Plan 33-01): inject the DecisionEvaluator as web::Data so
            // handlers can evaluate decisions and hot-reload the shared model.
            .app_data(web::Data::new(dmn_evaluator.clone()))
            // Phase 33 (Task 3): inject the DMN metrics broadcast sender so handlers
            // can push DmnMetricsSnapshot events after each evaluation (WS delivery).
            .app_data(web::Data::new(dmn_metrics_tx.clone()))
            // Phase 33 (Task 5): inject the FileSystemDmnStore for decision CRUD.
            .app_data(web::Data::new(engine_state::FileSystemDmnStore::default()))
            .app_data(web::Data::new(rule_store.clone()))
            .app_data(web::Data::new(category_store.clone()))
            .app_data(web::Data::new(tag_store.clone()))
            .app_data(web::Data::new(rule_tag_store.clone()))
            .app_data(web::Data::new(tenant_store.clone()))
            .app_data(web::Data::new(approval_store.clone()))
            .app_data(web::Data::new(audit_store.clone()))
            .app_data(branch_store.clone())
            .app_data(web::Data::new(job_store.clone()))
            .app_data(web::Data::new(user_store.clone()))
            // Health check endpoints (must be before /v1 to avoid prefix conflict)
            .configure(health::configure_health)
            // OpenAPI spec + Swagger UI (served at root level, outside /v1)
            .service(
                SwaggerUi::new("/swagger-ui/{_:.*}")
                    .url("/api-docs/openapi.json", ApiDoc::openapi()),
            )
            // API v1 endpoints
            .service(
                web::scope("/v1")
                    // Phase 22 (Task 2): wrap all `/v1/*` routes with JWT
                    // validation. The public allowlist (login, /health,
                    // /swagger-ui, /api-docs, /metrics) is bypassed INSIDE the
                    // middleware (D-22-02: authenticated-vs-public for v4.0;
                    // per-route role enforcement deferred to v5.0). T-22-01:
                    // algorithm pinned to HS256 in `decode_token`, so
                    // `alg:none`/wrong-secret tokens are rejected.
                    .wrap(actix_web::middleware::from_fn(
                        middleware::auth::requires_auth,
                    ))
                    // Phase 22 auth endpoints (D-22-01). `/v1/auth/login` is
                    // public (allowlist bypass); `/me` and `/refresh` require a
                    // valid Bearer token (enforced by the middleware wrap).
                    .route("/auth/login", web::post().to(auth::login))
                    .route("/auth/me", web::get().to(auth::me))
                    .route("/auth/refresh", web::post().to(auth::refresh))
                    // Execution endpoint
                    .route("/execute", web::post().to(v1::execution::execute_rules))
                    // Phase 28 (D-03 / FRONTEND-04): engine-selector dropdown
                    // data source. Reuses shared web::Data<EngineRegistry> +
                    // web::Data<EngineSelector>. Behind requires_auth on /v1.
                    .route("/engines", web::get().to(v1::execution::list_engines))
                    // Phase 27 (D-05): sessionful lifecycle routes — dispose
                    // and reset. Registered before any wildcard /execute/{...}
                    // route (none currently exists, but the discipline holds
                    // per PATTERNS.md line 372).
                    .route(
                        "/execute/session/{id}",
                        web::delete().to(v1::sessions::dispose_session),
                    )
                    .route(
                        "/execute/session/{id}/reset",
                        web::post().to(v1::sessions::reset_session),
                    )
                    // Rules CRUD endpoints (Phase 20-01)
                    .route("/rules", web::get().to(v1::rules::get_rules))
                    .route("/rules", web::post().to(v1::rules::create_rule))
                    // Static rule sub-paths MUST be registered before /rules/{id}
                    // so actix matches the literal segment instead of binding it
                    // to the {id} wildcard.
                    .route(
                        "/rules/batch-execute",
                        web::post().to(v1::execution::batch_execute_rules),
                    )
                    // Hot reload endpoint for updating rules without restart
                    .route("/rules/reload", web::post().to(v1::execution::reload_rules))
                    .route("/rules/{id}", web::get().to(v1::rules::get_rule))
                    .route("/rules/{id}", web::put().to(v1::rules::update_rule))
                    .route("/rules/{id}", web::patch().to(v1::rules::update_rule))
                    .route(
                        "/rules/{id}/undelete",
                        web::post().to(v1::rules::undelete_rule),
                    )
                    // Soft delete (marks as archived)
                    .route("/rules/{id}", web::delete().to(v1::rules::delete_rule))
                    // Reload job status endpoints (Phase 21)
                    .route("/jobs/{job_id}", web::get().to(get_job_status))
                    .route("/jobs", web::get().to(list_jobs))
                    // Approval workflow endpoints (Phase 20-03)
                    .route(
                        "/rules/{id}/submit",
                        web::post().to(v1::approvals::submit_for_approval),
                    )
                    .route(
                        "/approvals/pending",
                        web::get().to(v1::approvals::list_pending_approvals),
                    )
                    .route(
                        "/approvals/{id}/approve",
                        web::post().to(v1::approvals::approve_request),
                    )
                    .route(
                        "/approvals/{id}/reject",
                        web::post().to(v1::approvals::reject_request),
                    )
                    // Category endpoints (Phase 20-02)
                    .route(
                        "/categories",
                        web::get().to(v1::categories::list_categories),
                    )
                    .route(
                        "/categories",
                        web::post().to(v1::categories::create_category),
                    )
                    .route(
                        "/categories/{id}",
                        web::get().to(v1::categories::get_category),
                    )
                    .route(
                        "/categories/{id}",
                        web::put().to(v1::categories::update_category),
                    )
                    .route(
                        "/categories/{id}",
                        web::patch().to(v1::categories::update_category),
                    )
                    .route(
                        "/categories/{id}",
                        web::delete().to(v1::categories::delete_category),
                    )
                    // Tag endpoints (Phase 20-02)
                    .route("/tags", web::get().to(v1::tags::list_tags))
                    .route("/tags", web::post().to(v1::tags::create_tag))
                    .route("/tags/{name}", web::get().to(v1::tags::get_tag))
                    .route("/tags/{name}", web::put().to(v1::tags::update_tag))
                    .route("/tags/{name}", web::delete().to(v1::tags::delete_tag))
                    // Tenant (organisation/workspace) management.
                    .route("/tenants", web::get().to(v1::tenants::list_tenants))
                    .route("/tenants", web::post().to(v1::tenants::create_tenant))
                    .route("/tenants/{id}", web::get().to(v1::tenants::get_tenant))
                    .route(
                        "/tenants/{id}",
                        web::delete().to(v1::tenants::delete_tenant),
                    )
                    .route(
                        "/rules/{id}/tags",
                        web::post().to(v1::tags::assign_tag_to_rule),
                    )
                    .route(
                        "/rules/{id}/tags/{tag_name}",
                        web::delete().to(v1::tags::remove_tag_from_rule),
                    )
                    // Branch endpoints (Phase 20-05)
                    .route("/branches", web::get().to(v1::branches::list_branches))
                    .route("/branches", web::post().to(v1::branches::create_branch))
                    .route("/branches/{name}", web::get().to(v1::branches::get_branch))
                    .route(
                        "/branches/{name}",
                        web::delete().to(v1::branches::delete_branch),
                    )
                    .route(
                        "/branches/{name}/merge",
                        web::post().to(v1::branches::merge_branch),
                    )
                    // Git log endpoint (Phase 20-05)
                    .route(
                        "/rules/{id}/git-log",
                        web::get().to(v1::branches::get_git_log),
                    )
                    // Audit trail endpoints (Phase 20-04)
                    .route("/audit", web::get().to(v1::audit::query_audit_events))
                    .route("/audit/export", web::get().to(v1::audit::export_audit_log))
                    // Monitoring endpoints (Phase 19)
                    .configure(configure_monitoring)
                    // Dashboard metric routes (MON-03/MON-04) — registered inside
                    // /v1 so they resolve to /v1/metrics/dead-rules and
                    // /v1/metrics/coverage. The Prometheus /metrics endpoint
                    // stays at root (configured below).
                    .configure(metrics::configure_metrics_dashboards)
                    // Real-time metrics WebSocket (MON-02, Plan 22-04). A real
                    // WS upgrade at /v1/metrics/ws broadcasting MetricsSnapshot from
                    // `metrics_tx` (replaces the prior JSON stub — RESEARCH §
                    // Pitfall 6). Public via the auth allowlist so a browser
                    // `new WebSocket()` (which cannot send a Bearer token)
                    // can connect; the stream is read-only (threat model).
                    .configure(websocket::configure_metrics_ws)
                    // Simulation / What-If analysis endpoints (Phase 5)
                    .configure(v1::configure_v1_simulation)
                    // Phase 33 (Plan 33-01): decision evaluation + reload routes.
                    // Registered after simulation but before metrics so that the
                    // `/v1/decisions/*` paths resolve without conflict with any
                    // existing wildcard segments.
                    .configure(v1::configure_v1_decisions)
                    // Phase 33 (Task 3): DMN Prometheus text endpoint + WS broadcast.
                    // Registered after decisions so the full path is /v1/metrics/dmn
                    // and /v1/metrics/dmn/ws without conflicts with the root /metrics scope.
                    .configure(v1::configure_dmn_metrics),
            )
            .app_data(web::Data::new(spa::load(&frontend_dist)))
            .configure(metrics::configure_metrics)
            // Serve the frontend's real files (JS, CSS, images) from the build
            // directory, last so every API route takes precedence.
            .service(
                fs::Files::new("/", &frontend_dist)
                    .index_file("index.html")
                    .show_files_listing(),
            )
            // Anything that is not a file and not an API route is a client-side
            // route: serve the application shell and let the router decide. The
            // interface is a single-page app, so without this every page except
            // the landing page answered 404 on a reload or a bookmark, before the
            // router had a chance to run.
            .default_service(web::get().to(spa::serve_index))
    })
    .bind(&bind_address)?;

    println!("Server running at http://{bind_address}");
    println!("Metrics endpoint available at /metrics");
    server.run().await
}
