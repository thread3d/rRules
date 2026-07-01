//! Main entry point for rRules REST API server

use actix_files as fs;
use actix_web::{web, App, HttpServer};
use utoipa_swagger_ui::SwaggerUi;
use utoipa::OpenApi;

use crate::openapi::ApiDoc;

mod health;
mod metrics;
mod openapi;

// Phase 20 modules
pub mod v1;

// Import execution endpoints for hot reload job tracking
use crate::v1::execution::{get_job_status, list_jobs};
use crate::v1::routes::monitoring::configure_monitoring;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    println!("rRules REST API Server v0.1.0");
    println!("Starting on http://127.0.0.1:8080");

    // Create engine state for hot reload functionality
    let engine_state = web::Data::new(v1::execution::EngineState::new());

    // Create metrics broadcast channel for real-time updates (for future WebSocket use)
    let (metrics_tx, _) = metrics::create_metrics_channel(16);

    // Initialize in-memory stores
    let rule_store = v1::rules::init_store();
    let category_store = v1::categories::init_store();
    let (tag_store, rule_tag_store) = v1::tags::init_stores();
    let approval_store = v1::approvals::init_store();
    let audit_store = v1::audit::init_store();
    let job_store = v1::execution::init_job_store();

    // Get rules directory from environment and initialize git-backed version control.
    let rules_dir = std::env::var("RULE_STORAGE_PATH").unwrap_or_else(|_| "./rules".to_string());
    std::fs::create_dir_all(&rules_dir)?;

    let vcs_config = engine::governance::versioning::VcsConfig::new(
        &format!("{}/.git", rules_dir),
    );
    let repo_path = vcs_config.repo_path.clone();
    let branch_store: web::Data<tokio::sync::Mutex<engine::governance::versioning::VersionControlRepo>> = {
        let repo = engine::governance::versioning::VersionControlRepo::open_or_init(
            &repo_path,
            vcs_config,
        )
        .map_err(|e| std::io::Error::other(format!("Failed to init VCS: {}", e)))?;
        web::Data::new(tokio::sync::Mutex::from(repo))
    };

    tracing::info!(vcs_repo = %branch_store.lock().await.repo().path().display(), "Initialized git-backed version control");

    // Get frontend dist path from environment or use default
    let frontend_dist = std::env::var("FRONTEND_DIST")
        .unwrap_or_else(|_| "../frontend/dist".to_string());

    println!("Frontend dist path: {}", frontend_dist);

    // Start file watcher in background
    let watch_rules = std::env::var("WATCH_RULES_DIR")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(true);

    if watch_rules {
        println!("Starting file watcher for rules directory: {}", rules_dir);
        tokio::spawn(v1::execution::start_file_watcher(rules_dir));
    }

    let server = HttpServer::new(move || {
        App::new()
            .app_data(engine_state.clone())
            .app_data(web::Data::new(metrics_tx.clone()))
            .app_data(web::Data::new(rule_store.clone()))
            .app_data(web::Data::new(category_store.clone()))
            .app_data(web::Data::new(tag_store.clone()))
            .app_data(web::Data::new(rule_tag_store.clone()))
            .app_data(web::Data::new(approval_store.clone()))
            .app_data(web::Data::new(audit_store.clone()))
            .app_data(branch_store.clone())
            .app_data(web::Data::new(job_store.clone()))
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
                    // Execution endpoint
                    .route("/execute", web::post().to(v1::execution::execute_rules))
                    // Rules CRUD endpoints (Phase 20-01)
                    .route("/rules", web::get().to(v1::rules::get_rules))
                    .route("/rules", web::post().to(v1::rules::create_rule))
                    // Static rule sub-paths MUST be registered before /rules/{id}
                    // so actix matches the literal segment instead of binding it
                    // to the {id} wildcard.
                    .route("/rules/batch-execute", web::post().to(v1::execution::batch_execute_rules))
                    // Hot reload endpoint for updating rules without restart
                    .route("/rules/reload", web::post().to(v1::execution::reload_rules))
                    .route("/rules/{id}", web::get().to(v1::rules::get_rule))
                    .route("/rules/{id}", web::put().to(v1::rules::update_rule))
                    .route("/rules/{id}", web::patch().to(v1::rules::update_rule))
                    .route("/rules/{id}/undelete", web::post().to(v1::rules::undelete_rule))
                    // Soft delete (marks as archived)
                    .route("/rules/{id}", web::delete().to(v1::rules::delete_rule))
                    // Reload job status endpoints (Phase 21)
                    .route("/jobs/{job_id}", web::get().to(get_job_status))
                    .route("/jobs", web::get().to(list_jobs))
                    // Approval workflow endpoints (Phase 20-03)
                    .route("/rules/{id}/submit", web::post().to(v1::approvals::submit_for_approval))
                    .route("/approvals/pending", web::get().to(v1::approvals::list_pending_approvals))
                    .route("/approvals/{id}/approve", web::post().to(v1::approvals::approve_request))
                    .route("/approvals/{id}/reject", web::post().to(v1::approvals::reject_request))
                    // Category endpoints (Phase 20-02)
                    .route("/categories", web::get().to(v1::categories::list_categories))
                    .route("/categories", web::post().to(v1::categories::create_category))
                    .route("/categories/{id}", web::get().to(v1::categories::get_category))
                    .route("/categories/{id}", web::put().to(v1::categories::update_category))
                    .route("/categories/{id}", web::patch().to(v1::categories::update_category))
                    .route("/categories/{id}", web::delete().to(v1::categories::delete_category))
                    // Tag endpoints (Phase 20-02)
                    .route("/tags", web::get().to(v1::tags::list_tags))
                    .route("/tags", web::post().to(v1::tags::create_tag))
                    .route("/tags/{name}", web::get().to(v1::tags::get_tag))
                    .route("/tags/{name}", web::put().to(v1::tags::update_tag))
                    .route("/tags/{name}", web::delete().to(v1::tags::delete_tag))
                    .route("/rules/{id}/tags", web::post().to(v1::tags::assign_tag_to_rule))
                    .route("/rules/{id}/tags/{tag_name}", web::delete().to(v1::tags::remove_tag_from_rule))
                    // Branch endpoints (Phase 20-05)
                    .route("/branches", web::get().to(v1::branches::list_branches))
                    .route("/branches", web::post().to(v1::branches::create_branch))
                    .route("/branches/{name}", web::get().to(v1::branches::get_branch))
                    .route("/branches/{name}", web::delete().to(v1::branches::delete_branch))
                    .route("/branches/{name}/merge", web::post().to(v1::branches::merge_branch))
                    // Git log endpoint (Phase 20-05)
                    .route("/rules/{id}/git-log", web::get().to(v1::branches::get_git_log))
                    // Audit trail endpoints (Phase 20-04)
                    .route("/audit", web::get().to(v1::audit::query_audit_events))
                    .route("/audit/export", web::get().to(v1::audit::export_audit_log))
                    // Monitoring endpoints (Phase 19)
                    .configure(configure_monitoring)
                    // Simulation / What-If analysis endpoints (Phase 5)
                    .configure(v1::configure_v1_simulation),
            )
            .configure(metrics::configure_metrics)
            // Serve frontend static files at root path (last, so API routes take precedence)
            .service(
                fs::Files::new("/", &frontend_dist)
                    .index_file("index.html")
                    .show_files_listing(),
            )
    })
    .bind("127.0.0.1:8080")?;

    println!("Server running at http://127.0.0.1:8080");
    println!("Metrics endpoint available at /metrics");
    server.run().await
}
