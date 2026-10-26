//! The HTTP application, assembled from [\`ApiState\`].
//!
//! Keeping this out of \`main\` is what makes the routes testable in-process: a test builds
//! the same \`App\` the server serves and drives it with \`actix_web::test\`, with no socket,
//! no port to collide with, and no separate process to wait for.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, RwLock};

use actix_files as fs;
use actix_web::dev::{ServiceFactory, ServiceRequest, ServiceResponse};
use actix_web::{web, App};
use engine::dmn::evaluator::DecisionEvaluator;
use engine::engine::{EngineRegistry, EngineSelector};
use engine::governance::versioning::VersionControlRepo;
use tokio::sync::Mutex as AsyncMutex;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;
use uuid::Uuid;

use crate::auth::store::UserStore;
use crate::metrics::{self, ExecutionCounters, MetricsTx};
use crate::openapi::ApiDoc;
use crate::v1;
use crate::v1::approvals::StoredApproval;
use crate::v1::audit::StoredAuditEvent;
use crate::v1::categories::StoredCategory;
use crate::v1::dmn_metrics::DmnMetricsTx;
use crate::v1::execution::{get_job_status, list_jobs, EngineState, JobStore, SharedRuleSet};
use crate::v1::routes::monitoring::configure_monitoring;
use crate::v1::rules::StoredRule;
use crate::v1::sessions::SessionManager;
use crate::v1::tags::StoredTag;
use crate::v1::tenants::TenantStore;
use crate::{auth, health, middleware, spa, websocket};

/// Everything the application needs, cloned into each worker.
#[derive(Clone)]
pub struct ApiState {
    pub engine_state: web::Data<EngineState>,
    pub registry: web::Data<EngineRegistry>,
    pub selector: web::Data<EngineSelector>,
    pub ruleset: web::Data<Arc<RwLock<SharedRuleSet>>>,
    pub metrics_tx: MetricsTx,
    pub counters: web::Data<ExecutionCounters>,
    pub session_manager: Arc<SessionManager>,
    pub dmn_evaluator: Arc<DecisionEvaluator>,
    pub dmn_metrics_tx: DmnMetricsTx,
    pub rule_store: Arc<Mutex<HashMap<(String, Uuid), StoredRule>>>,
    pub category_store: Arc<Mutex<HashMap<Uuid, StoredCategory>>>,
    pub tag_store: Arc<Mutex<HashMap<String, StoredTag>>>,
    pub rule_tag_store: Arc<Mutex<HashSet<(Uuid, String)>>>,
    pub tenant_store: TenantStore,
    pub approval_store: Arc<Mutex<HashMap<Uuid, StoredApproval>>>,
    pub audit_store: Arc<Mutex<HashMap<Uuid, StoredAuditEvent>>>,
    pub branch_store: web::Data<AsyncMutex<VersionControlRepo>>,
    pub job_store: JobStore,
    pub user_store: UserStore,
    /// Where the built interface lives; served for every client-side route.
    pub frontend_dist: String,
    /// Origins the browser may call from. Pinned, never a wildcard.
    pub cors_allowed_origins: Vec<String>,
}

/// Where the server keeps things. Everything that differs between a deployment and a test is here,
/// so the rest of the assembly has one shape.
pub struct ApiPaths {
    pub rules_dir: std::path::PathBuf,
    pub dmn_dir: std::path::PathBuf,
    pub frontend_dist: String,
    pub cors_allowed_origins: Vec<String>,
}

impl ApiPaths {
    /// The configuration a running server uses, from the environment.
    pub fn from_environment() -> Self {
        let rules_dir =
            std::env::var("RULE_STORAGE_PATH").unwrap_or_else(|_| "./rules".to_string());
        let dmn_dir = std::env::var("DMN_STORAGE_PATH").unwrap_or_else(|_| "./dmn".to_string());
        let frontend_dist =
            std::env::var("FRONTEND_DIST").unwrap_or_else(|_| "../frontend/dist".to_string());
        let cors_allowed_origins = std::env::var("CORS_ALLOWED_ORIGINS")
            .map(|raw| {
                raw.split(',')
                    .map(str::trim)
                    .filter(|origin| !origin.is_empty())
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();
        Self {
            rules_dir: rules_dir.into(),
            dmn_dir: dmn_dir.into(),
            frontend_dist,
            cors_allowed_origins,
        }
    }
}

/// Assemble the state the application runs on.
///
/// This is the single place the state is built. `main` calls it with
/// [`ApiPaths::from_environment`] and tests call it with a temporary directory, so a test cannot be
/// exercising a differently wired server than a deployment does — which had already happened once,
/// when a test registry had no engines registered.
pub fn from_paths(paths: ApiPaths) -> std::io::Result<ApiState> {
    let rules_dir = paths.rules_dir.display().to_string();
    std::fs::create_dir_all(&paths.rules_dir)?;

    let engine_state = web::Data::new(v1::execution::EngineState::new());

    let mut registry = EngineRegistry::new();
    registry.register_default_engines();
    let registry = web::Data::new(registry);

    let mut selector = EngineSelector::with_defaults();
    let default_engine =
        std::env::var("RRULES_DEFAULT_ENGINE").unwrap_or_else(|_| "rete".to_string());
    if selector.select_engine(&default_engine).is_err() {
        tracing::warn!(engine = %default_engine, "invalid RRULES_DEFAULT_ENGINE, falling back to rete");
        let _ = selector.select_engine("rete");
    }
    let selector = web::Data::new(selector);

    if let Ok(raw) = std::env::var("RRULES_MAX_CHAIN_ITERATIONS") {
        if raw.parse::<usize>().is_err() {
            tracing::warn!(value = %raw, "RRULES_MAX_CHAIN_ITERATIONS is not a number; ignoring it");
        }
    }

    let ruleset = SharedRuleSet::new(v1::execution::build_ruleset_from_dir(&rules_dir));
    let ruleset = web::Data::new(Arc::new(RwLock::new(ruleset)));

    let shared_dmn_model = crate::dmn_loader::load_models(&paths.dmn_dir);
    let dmn_evaluator = Arc::new(DecisionEvaluator::new(Arc::new(RwLock::new(
        shared_dmn_model,
    ))));

    let (dmn_metrics_tx, _) = v1::dmn_metrics::create_dmn_metrics_channel(16);
    let (metrics_tx, _) = metrics::create_metrics_channel(16);
    let counters = web::Data::new(ExecutionCounters::new());

    let session_config = v1::sessions::read_session_config();
    let sessions_active =
        prometheus::IntGauge::new("rrules_sessions_active", "Active stateful session count")
            .expect("construct rrules_sessions_active gauge");
    counters
        .registry
        .register(Box::new(sessions_active.clone()))
        .expect("register rrules_sessions_active on the dedicated registry");
    let session_manager = Arc::new(SessionManager::new(session_config, sessions_active));

    let (tag_store, rule_tag_store) = v1::tags::init_stores();

    let vcs_config = engine::governance::versioning::VcsConfig::new(&format!("{rules_dir}/.git"));
    let repo_path = vcs_config.repo_path.clone();
    let repo = VersionControlRepo::open_or_init(&repo_path, vcs_config)
        .map_err(|error| std::io::Error::other(format!("Failed to init VCS: {error}")))?;

    Ok(ApiState {
        engine_state,
        registry,
        selector,
        ruleset,
        metrics_tx,
        counters,
        session_manager,
        dmn_evaluator,
        dmn_metrics_tx,
        rule_store: v1::rules::init_store(),
        category_store: v1::categories::init_store(),
        tag_store,
        rule_tag_store,
        tenant_store: v1::tenants::init_store(),
        approval_store: v1::approvals::init_store(),
        audit_store: v1::audit::init_store(),
        branch_store: web::Data::new(AsyncMutex::new(repo)),
        job_store: v1::execution::init_job_store(),
        user_store: auth::init_store(),
        frontend_dist: paths.frontend_dist,
        cors_allowed_origins: paths.cors_allowed_origins,
    })
}

/// The state a running server uses: [`ApiPaths::from_environment`], then [`from_paths`].
pub fn from_environment() -> std::io::Result<ApiState> {
    from_paths(ApiPaths::from_environment())
}

/// Build the application. Called once per worker by the server, and directly by tests.
pub fn build_app(
    state: &ApiState,
) -> App<
    impl ServiceFactory<
        ServiceRequest,
        Config = (),
        Response = ServiceResponse<actix_web::body::EitherBody<actix_web::body::BoxBody>>,
        Error = actix_web::Error,
        InitError = (),
    >,
> {
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
        for origin in &state.cors_allowed_origins {
            c = c.allowed_origin(origin.as_str());
        }
        c
    };
    App::new()
        .wrap(cors)
        .app_data(state.engine_state.clone())
        // Phase 23 (D-03 / L-07): real engine wiring for POST /v1/execute.
        .app_data(state.registry.clone())
        .app_data(state.selector.clone())
        .app_data(state.ruleset.clone())
        .app_data(web::Data::new(state.metrics_tx.clone()))
        // Phase 24 (Plan 24-02): inject ExecutionCounters as web::Data
        // alongside metrics_tx. Inert until Plan 24-03 wires the handlers.
        .app_data(state.counters.clone())
        // Phase 27 (D-12): inject the shared SessionManager as web::Data
        // so execute_rules / dispose_session / reset_session can extract it.
        .app_data(web::Data::new(state.session_manager.clone()))
        // Phase 33 (Plan 33-01): inject the DecisionEvaluator as web::Data so
        // handlers can evaluate decisions and hot-reload the shared model.
        .app_data(web::Data::new(state.dmn_evaluator.clone()))
        // Phase 33 (Task 3): inject the DMN metrics broadcast sender so handlers
        // can push DmnMetricsSnapshot events after each evaluation (WS delivery).
        .app_data(web::Data::new(state.dmn_metrics_tx.clone()))
        // Phase 33 (Task 5): inject the FileSystemDmnStore for decision CRUD.
        .app_data(web::Data::new(engine_state::FileSystemDmnStore::default()))
        .app_data(web::Data::new(state.rule_store.clone()))
        .app_data(web::Data::new(state.category_store.clone()))
        .app_data(web::Data::new(state.tag_store.clone()))
        .app_data(web::Data::new(state.rule_tag_store.clone()))
        .app_data(web::Data::new(state.tenant_store.clone()))
        .app_data(web::Data::new(state.approval_store.clone()))
        .app_data(web::Data::new(state.audit_store.clone()))
        .app_data(state.branch_store.clone())
        .app_data(web::Data::new(state.job_store.clone()))
        .app_data(web::Data::new(state.user_store.clone()))
        // Health check endpoints (must be before /v1 to avoid prefix conflict)
        .configure(health::configure_health)
        // OpenAPI spec + Swagger UI (served at root level, outside /v1)
        .service(
            SwaggerUi::new("/swagger-ui/{_:.*}").url("/api-docs/openapi.json", ApiDoc::openapi()),
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
        .app_data(web::Data::new(spa::load(&state.frontend_dist)))
        .configure(metrics::configure_metrics)
        // Serve the frontend's real files (JS, CSS, images) from the build
        // directory, last so every API route takes precedence.
        .service(
            fs::Files::new("/", &state.frontend_dist)
                .index_file("index.html")
                .show_files_listing(),
        )
        // Anything that is not a file and not an API route is a client-side
        // route: serve the application shell and let the router decide. The
        // interface is a single-page app, so without this every page except
        // the landing page answered 404 on a reload or a bookmark, before the
        // router had a chance to run.
        .default_service(web::get().to(spa::serve_index))
}

impl ApiState {
    /// A fully wired application state rooted in `root`, for tests and generated verification.
    ///
    /// A thin wrapper over [`from_paths`]: it chooses where the paths point and nothing else, so a
    /// test cannot run against a differently assembled server. (It once could: the test state had no
    /// engines registered while the real one had four.)
    pub fn for_testing(root: &std::path::Path) -> Self {
        let rules_dir = root.join("rules");
        let dmn_dir = root.join("dmn");
        std::fs::create_dir_all(&rules_dir).expect("create the rules directory");
        std::fs::create_dir_all(&dmn_dir).expect("create the DMN directory");
        from_paths(ApiPaths {
            rules_dir,
            dmn_dir,
            frontend_dist: root.join("dist").display().to_string(),
            cors_allowed_origins: Vec::new(),
        })
        .expect("assemble the test state")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::http::StatusCode;
    use actix_web::test;

    /// The application can be built and driven without a socket: the same `App` the
    /// server serves, exercised with `actix_web::test`. This is what the integration
    /// suites could not do — they had to bind a port and race whatever else was running.
    #[actix_web::test]
    async fn serves_health_without_a_socket() {
        let dir = std::env::temp_dir().join(format!("rrules-app-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let state = ApiState::for_testing(&dir);
        let app = test::init_service(build_app(&state)).await;

        let request = test::TestRequest::get().uri("/health").to_request();
        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::OK);
    }

    /// An unauthenticated call to a protected route is refused by the middleware, and the
    /// refusal comes from the same stack the server runs — not from a mock.
    #[actix_web::test]
    async fn refuses_rules_without_a_token() {
        let dir = std::env::temp_dir().join(format!("rrules-app-test-auth-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let state = ApiState::for_testing(&dir);
        let app = test::init_service(build_app(&state)).await;

        let request = test::TestRequest::get().uri("/v1/rules").to_request();
        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    /// Logging in and then listing rules works end to end inside one process.
    #[actix_web::test]
    async fn login_then_list_rules() {
        let dir =
            std::env::temp_dir().join(format!("rrules-app-test-login-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let state = ApiState::for_testing(&dir);
        let app = test::init_service(build_app(&state)).await;

        let login = test::TestRequest::post()
            .uri("/v1/auth/login")
            .set_json(serde_json::json!({"email": "admin", "password": "admin"}))
            .to_request();
        let response = test::call_service(&app, login).await;
        assert_eq!(response.status(), StatusCode::OK, "login should succeed");
        let body: serde_json::Value = test::read_body_json(response).await;
        let token = body["token"].as_str().expect("a token").to_string();

        let list = test::TestRequest::get()
            .uri("/v1/rules")
            .insert_header(("Authorization", format!("Bearer {token}")))
            .to_request();
        let response = test::call_service(&app, list).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body: serde_json::Value = test::read_body_json(response).await;
        assert_eq!(body["total_count"], 0);
        assert_eq!(body["page_size"], 50, "an absent page size means 50, not 1");
    }
}

#[cfg(test)]
mod audit_tests {
    //! The audit trail is a headline feature: a deployment expects `/v1/audit` to show who changed
    //! what. These tests drive the real application in-process and read the trail back through the
    //! API, so "it records" means the endpoint a user calls shows the event.

    use super::*;
    use actix_web::http::StatusCode;
    use actix_web::test;

    #[actix_web::test]
    async fn a_created_rule_appears_in_the_audit_trail() {
        let dir = std::env::temp_dir().join(format!("rrules-audit-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let state = ApiState::for_testing(&dir);
        let app = test::init_service(build_app(&state)).await;

        let login = test::TestRequest::post()
            .uri("/v1/auth/login")
            .set_json(serde_json::json!({"email": "admin", "password": "admin"}))
            .to_request();
        let body: serde_json::Value = test::call_and_read_body_json(&app, login).await;
        let token = body["token"].as_str().expect("a token").to_string();

        let create = test::TestRequest::post()
            .uri("/v1/rules")
            .insert_header(("Authorization", format!("Bearer {token}")))
            .insert_header(("User-Agent", "rrules-test/1.0"))
            .set_json(serde_json::json!({
                "name": "audited-rule",
                "dsl_body": "rule \"a\" when true then log(\"ok\")",
            }))
            .to_request();
        let response = test::call_service(&app, create).await;
        assert_eq!(response.status(), StatusCode::CREATED);

        let list = test::TestRequest::get()
            .uri("/v1/audit")
            .insert_header(("Authorization", format!("Bearer {token}")))
            .to_request();
        let response = test::call_service(&app, list).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body: serde_json::Value = test::read_body_json(response).await;

        let events = body["events"].as_array().expect("an events array");
        assert_eq!(events.len(), 1, "one create should leave one event: {body}");
        let event = &events[0];
        assert_eq!(event["action"], "create");
        assert_eq!(event["resource_type"], "rule");
        assert_eq!(event["resource_name"], "audited-rule");
        assert_eq!(event["user_agent"], "rrules-test/1.0");
    }

    #[actix_web::test]
    async fn deleting_a_rule_records_a_delete() {
        let dir = std::env::temp_dir().join(format!("rrules-audit-delete-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let state = ApiState::for_testing(&dir);
        let app = test::init_service(build_app(&state)).await;

        let login = test::TestRequest::post()
            .uri("/v1/auth/login")
            .set_json(serde_json::json!({"email": "admin", "password": "admin"}))
            .to_request();
        let body: serde_json::Value = test::call_and_read_body_json(&app, login).await;
        let token = body["token"].as_str().expect("a token").to_string();

        let create = test::TestRequest::post()
            .uri("/v1/rules")
            .insert_header(("Authorization", format!("Bearer {token}")))
            .set_json(serde_json::json!({
                "name": "short-lived",
                "dsl_body": "rule \"s\" when true then log(\"ok\")",
            }))
            .to_request();
        let created: serde_json::Value = test::call_and_read_body_json(&app, create).await;
        let rule_id = created["id"].as_str().expect("a rule id").to_string();

        let delete = test::TestRequest::delete()
            .uri(&format!("/v1/rules/{rule_id}"))
            .insert_header(("Authorization", format!("Bearer {token}")))
            .to_request();
        let response = test::call_service(&app, delete).await;
        assert_eq!(response.status(), StatusCode::OK);

        let list = test::TestRequest::get()
            .uri("/v1/audit")
            .insert_header(("Authorization", format!("Bearer {token}")))
            .to_request();
        let body: serde_json::Value = test::call_and_read_body_json(&app, list).await;
        let actions: Vec<&str> = body["events"]
            .as_array()
            .expect("events")
            .iter()
            .filter_map(|event| event["action"].as_str())
            .collect();
        assert!(
            actions.contains(&"delete"),
            "the archive should be audited, saw {actions:?}"
        );
    }
}

#[cfg(test)]
mod assembly_tests {
    //! The state a test builds and the state a server builds come from one function. These assert
    //! the properties that drifted when they were two: the test state once had no engines registered
    //! while the server had four.

    use super::*;

    fn temp_root(tag: &str) -> std::path::PathBuf {
        let root =
            std::env::temp_dir().join(format!("rrules-assembly-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&root).expect("temp dir");
        root
    }

    #[actix_web::test]
    async fn the_test_state_matches_what_the_server_registers() {
        let state = ApiState::for_testing(&temp_root("engines"));
        let names = state.registry.available_engines();
        for expected in ["rete", "treat", "leaps", "phreak"] {
            assert!(
                names.contains(&expected),
                "the assembled state should offer {expected}; it offers {names:?}"
            );
        }
    }

    #[actix_web::test]
    async fn the_test_state_loads_rules_and_models_from_its_paths() {
        let root = temp_root("loads");
        let rules_dir = root.join("rules");
        let dmn_dir = root.join("dmn");
        std::fs::create_dir_all(&rules_dir).expect("rules dir");
        std::fs::create_dir_all(&dmn_dir).expect("dmn dir");
        std::fs::write(
            dmn_dir.join("model.dmn"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<definitions xmlns="https://www.omg.org/spec/DMN/20180913/MODEL/" id="m" name="m" namespace="http://example.com/m">
  <decision id="assembled_decision" name="Assembled"><literalExpression><text>"ok"</text></literalExpression></decision>
</definitions>"#,
        )
        .expect("write model");

        let state = from_paths(ApiPaths {
            rules_dir,
            dmn_dir,
            frontend_dist: root.join("dist").display().to_string(),
            cors_allowed_origins: vec!["http://localhost:3000".to_string()],
        })
        .expect("assemble");

        // The DMN model in the directory is the one the evaluator holds.
        let model = state.dmn_evaluator.shared();
        let guard = model.read().expect("model guard");
        let decisions: Vec<&str> = guard
            .model
            .definitions
            .children
            .iter()
            .filter_map(|child| match child {
                engine::dmn::model::DefinitionsChild::Decision(decision) => {
                    Some(decision.id.as_str())
                }
                _ => None,
            })
            .collect();
        assert_eq!(decisions, vec!["assembled_decision"]);

        assert_eq!(
            state.cors_allowed_origins,
            vec!["http://localhost:3000".to_string()]
        );
    }
}
