//! Decision evaluation + CRUD endpoints (Phase 33).
//!
//! Provides:
//! - `POST /v1/decisions/{id}/evaluate` for decision service (§10.4) with multi-output,
//!   optional inference trace emission, and per-decision Prometheus metrics.
//! - Full decision lifecycle management: CRUD (`GET`, `POST`, `PUT`, `DELETE` on `/v1/decisions`),
//!   import via `POST /v1/decisions/import`, export via `GET /v1/decisions/{id}/export`.
//! - Hot-reload via `POST /v1/decisions/reload`.

use actix_web::{web, HttpResponse};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use engine::data_types::Value;
use engine::dmn::evaluator::{DecisionEvaluator, EvaluateRequest};
use engine::dmn::model::{Decision, DefinitionsChild, DmnModel};
use engine_state::DmnDocument;
use engine_state::FileSystemDmnStore;
// Import the DmnPersistence trait so its methods are in scope for FileSystemDmnStore.
use engine_state::DmnPersistence;

// Re-export DmnMetricsSnapshot for use in handlers.
pub use crate::v1::dmn_metrics::DmnMetricsSnapshot;

// =========================================================================
// Request/Response types
// =========================================================================

/// Request body for decision evaluation.
#[derive(Debug, Deserialize)]
pub struct EvaluateDecisionRequest {
    /// Input values (keyed by input name).
    pub inputs: HashMap<String, Value>,
    /// Whether to emit an inference trace (default: false).
    #[serde(default)]
    pub trace: bool,
}

/// Response body for decision evaluation.
#[derive(Debug, Serialize)]
pub struct EvaluateDecisionResponse {
    /// Decision ID that was evaluated.
    pub decision_id: String,
    /// Output values produced by the evaluation.
    pub outputs: HashMap<String, Value>,
    /// Number of rules that fired (for hit-policy diagnostics).
    pub rules_fired: u64,
    /// Evaluation duration in milliseconds.
    pub duration_ms: f64,
    /// Optional inference trace (only if `trace: true` on the request).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace: Option<TraceResponse>,
}

/// Inference trace response (subset of full trace for API exposure).
#[derive(Debug, Serialize)]
pub struct TraceResponse {
    /// Decision ID being evaluated.
    pub decision_id: String,
    /// Number of events in the trace.
    pub event_count: usize,
    /// Total duration in milliseconds.
    pub duration_ms: f64,
}

/// Request body for reloading a DMN model (hot-reload).
#[derive(Debug, Deserialize)]
pub struct ReloadDmnRequest {
    /// The new DMN XML content.
    pub dmn_content: String,
}

/// Request body for creating a new decision.
#[derive(Debug, Deserialize)]
pub struct CreateDecisionRequest {
    /// Human-readable name for the decision.
    pub name: String,
    /// Raw DMN XML content (string representation of the .dmn file).
    pub content: String,
    /// Optional explicit ID — if not provided, a UUID is generated.
    #[serde(default)]
    pub id: Option<String>,
}

/// Request body for updating an existing decision.
#[derive(Debug, Deserialize)]
pub struct UpdateDecisionRequest {
    /// Updated raw DMN XML content.
    pub content: String,
    /// Updated metadata (optional).
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
}

/// Pagination query parameters for list endpoints.
#[derive(Debug, Deserialize)]
pub struct ListQuery {
    #[serde(default = "default_page")]
    pub page: u32,
    #[serde(default = "default_per_page")]
    pub per_page: u32,
}

fn default_page() -> u32 {
    1
}
fn default_per_page() -> u32 {
    20
}

// =========================================================================
// Helper: extract decisions from Definitions children
// =========================================================================

#[allow(dead_code)]
fn extract_decisions(definitions: &DmnModel) -> Vec<&Decision> {
    definitions
        .definitions
        .children
        .iter()
        .filter_map(|child| match child {
            DefinitionsChild::Decision(d) => Some(d),
            _ => None,
        })
        .collect()
}

// =========================================================================
// Handlers
// =========================================================================

/// Evaluate a decision — `POST /v1/decisions/{id}/evaluate`.
pub async fn evaluate_decision(
    evaluator: web::Data<Arc<DecisionEvaluator>>,
    path: web::Path<String>,
    body: web::Json<EvaluateDecisionRequest>,
    dmn_metrics_tx: web::Data<crate::v1::dmn_metrics::DmnMetricsTx>,
) -> HttpResponse {
    let decision_id = path.into_inner();

    // Instrument: increment request counter BEFORE evaluation.
    crate::v1::dmn_metrics::METRIC_EVALUATIONS_TOTAL
        .with_label_values(&[&decision_id])
        .inc();

    // Build the evaluation request.
    let request = EvaluateRequest {
        inputs: body.inputs.clone(),
        trace: body.trace,
    };

    // Execute the evaluation.
    match evaluator.evaluate(&decision_id, request) {
        Ok(result) => {
            // Instrument: record duration after successful evaluate.
            let hit_policy_str = result
                .outputs
                .keys()
                .next()
                .map(|_| "UNIQUE") // Default label when no table context available
                .unwrap_or("None");

            crate::v1::dmn_metrics::METRIC_DECISION_DURATION_SECONDS
                .with_label_values(&[&decision_id, hit_policy_str])
                .observe(result.duration_ms / 1000.0);

            // Publish to the WebSocket channel. `send` fails only when no client is subscribed,
            // which is the ordinary case for a headless deployment and not an error.
            let _ = dmn_metrics_tx.send(crate::v1::dmn_metrics::DmnMetricsSnapshot {
                decision_id: decision_id.clone(),
                rules_fired: result.rules_fired,
                duration_ms: result.duration_ms,
                timestamp: chrono::Utc::now().to_rfc3339(),
            });

            let trace_response = result.trace.as_ref().map(|t| TraceResponse {
                decision_id: t.decision_id.clone(),
                event_count: t.events.len(),
                duration_ms: t.duration_ms(),
            });

            HttpResponse::Ok().json(EvaluateDecisionResponse {
                decision_id,
                outputs: result.outputs,
                rules_fired: result.rules_fired,
                duration_ms: result.duration_ms,
                trace: trace_response,
            })
        }
        Err(e) => match e {
            engine::dmn::evaluator::DecisionEvalError::NotFound(msg) => {
                HttpResponse::NotFound().json(serde_json::json!({ "error": msg }))
            }
            // Map HitPolicyViolation to 400 with DMN_HIT_POLICY_VIOLATION error code.
            engine::dmn::evaluator::DecisionEvalError::TableError(
                engine::dmn::decision_table::DecisionTableError::HitPolicyViolation {
                    policy,
                    message,
                },
            ) => {
                crate::v1::dmn_metrics::METRIC_HIT_POLICY_VIOLATIONS_TOTAL
                    .with_label_values(&[&decision_id, &policy])
                    .inc();
                HttpResponse::BadRequest().json(serde_json::json!({
                    "error_code": "DMN_HIT_POLICY_VIOLATION",
                    "policy": policy,
                    "message": message,
                }))
            }
            _ => HttpResponse::InternalServerError()
                .json(serde_json::json!({ "error": format!("{}", e) })),
        },
    }
}

/// List decisions — `GET /v1/decisions`.
pub async fn list_decisions(
    persistence: web::Data<FileSystemDmnStore>,
    query: web::Query<ListQuery>,
) -> HttpResponse {
    let page = query.page as usize;
    let per_page = query.per_page as usize;
    let offset = (page.saturating_sub(1)) * per_page;

    // Dereference Arc to access the underlying FileSystemDmnStore methods.
    let store: &FileSystemDmnStore = &persistence;
    match store.list().await {
        Ok(summaries) => {
            let total = summaries.len();
            let paginated: Vec<_> = summaries.into_iter().skip(offset).take(per_page).collect();

            HttpResponse::Ok().json(serde_json::json!({
                "decisions": paginated,
                "total": total,
                "page": page,
                "per_page": per_page,
                "pages": (total + per_page - 1) / per_page.max(1),
            }))
        }
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("failed to list decisions: {}", e)
        })),
    }
}

/// Get a specific decision — `GET /v1/decisions/{id}`.
pub async fn get_decision(
    persistence: web::Data<FileSystemDmnStore>,
    path: web::Path<String>,
) -> HttpResponse {
    let decision_id = path.into_inner();

    match persistence.get(&decision_id).await {
        Ok(doc) => HttpResponse::Ok().json(serde_json::json!({
            "id": doc.id,
            "name": doc.name,
            "content_length": doc.content.len(),
            "metadata": doc.metadata,
            "created_at": doc.created_at.to_rfc3339(),
            "updated_at": doc.updated_at.to_rfc3339(),
        })),
        Err(engine_state::PersistenceError::NotFound(_)) => {
            HttpResponse::NotFound().json(serde_json::json!({
                "error": format!("decision '{}' not found", decision_id)
            }))
        }
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("failed to get decision: {}", e)
        })),
    }
}

/// Create a new decision — `POST /v1/decisions`.
pub async fn create_decision(
    persistence: web::Data<FileSystemDmnStore>,
    body: web::Json<CreateDecisionRequest>,
) -> HttpResponse {
    let id = body
        .id
        .clone()
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let now = chrono::Utc::now();

    let doc = DmnDocument {
        id: id.clone(),
        name: body.name.clone(),
        content: body.content.as_bytes().to_vec(),
        metadata: serde_json::json!({}),
        created_at: now,
        updated_at: now,
    };

    match persistence.create(doc).await {
        Ok(created) => HttpResponse::Created().json(serde_json::json!({
            "id": created.id,
            "name": created.name,
            "content_length": created.content.len(),
            "metadata": created.metadata,
            "created_at": created.created_at.to_rfc3339(),
            "updated_at": created.updated_at.to_rfc3339(),
        })),
        Err(engine_state::PersistenceError::Conflict(msg)) => {
            HttpResponse::Conflict().json(serde_json::json!({
                "error_code": "DMN_CONFLICT",
                "message": msg,
            }))
        }
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("failed to create decision: {}", e)
        })),
    }
}

/// Update an existing decision — `PUT /v1/decisions/{id}`.
pub async fn update_decision(
    persistence: web::Data<FileSystemDmnStore>,
    path: web::Path<String>,
    body: web::Json<UpdateDecisionRequest>,
) -> HttpResponse {
    let id = path.into_inner();

    match persistence.get(&id).await {
        Ok(existing) => {
            let updated_content = body.content.as_bytes().to_vec();
            let metadata = body.metadata.clone().unwrap_or(existing.metadata);

            let doc = DmnDocument {
                id: existing.id,
                name: existing.name,
                content: updated_content,
                metadata,
                created_at: existing.created_at,
                updated_at: chrono::Utc::now(),
            };

            match persistence.update(&id, doc).await {
                Ok(updated) => HttpResponse::Ok().json(serde_json::json!({
                    "id": updated.id,
                    "name": updated.name,
                    "content_length": updated.content.len(),
                    "metadata": updated.metadata,
                    "created_at": updated.created_at.to_rfc3339(),
                    "updated_at": updated.updated_at.to_rfc3339(),
                })),
                Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": format!("failed to update decision: {}", e)
                })),
            }
        }
        Err(engine_state::PersistenceError::NotFound(_)) => {
            HttpResponse::NotFound().json(serde_json::json!({
                "error": format!("decision '{}' not found", id)
            }))
        }
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("failed to get existing decision: {}", e)
        })),
    }
}

/// Delete a decision — `DELETE /v1/decisions/{id}`.
pub async fn delete_decision(
    persistence: web::Data<FileSystemDmnStore>,
    path: web::Path<String>,
) -> HttpResponse {
    let id = path.into_inner();

    match persistence.delete(&id).await {
        Ok(()) => HttpResponse::NoContent().finish(),
        Err(engine_state::PersistenceError::NotFound(_)) => {
            HttpResponse::NotFound().json(serde_json::json!({
                "error": format!("decision '{}' not found", id)
            }))
        }
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("failed to delete decision: {}", e)
        })),
    }
}

/// Import a DMN file — `POST /v1/decisions/import`.
pub async fn import_dmn(
    persistence: web::Data<FileSystemDmnStore>,
    body: web::Json<ImportDmnRequest>,
) -> HttpResponse {
    let filename = format!("{}.dmn", &body.id);

    // Check for conflicts.
    match persistence.load_file(&filename).await {
        Ok(_) => {
            return HttpResponse::Conflict().json(serde_json::json!({
                "error_code": "DMN_CONFLICT",
                "message": format!("decision with id '{}' already exists", body.id),
            }));
        }
        Err(engine_state::PersistenceError::NotFound(_)) => {
            // No conflict — proceed.
        }
        Err(e) => {
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("failed to check existing file: {}", e)
            }))
        }
    }

    match persistence.save_file(&filename, &body.content).await {
        Ok(()) => HttpResponse::Created().json(serde_json::json!({
            "status": "imported",
            "id": body.id,
            "message": format!("DMN file '{}' imported successfully", filename),
        })),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("failed to import DMN: {}", e)
        })),
    }
}

/// Export a DMN decision — `GET /v1/decisions/{id}/export`.
#[allow(deprecated)]
pub async fn export_dmn(
    persistence: web::Data<FileSystemDmnStore>,
    path: web::Path<String>,
) -> HttpResponse {
    let id = path.into_inner();

    match persistence.get(&id).await {
        Ok(doc) => HttpResponse::Ok()
            .content_type("application/xml")
            .header(
                actix_web::http::header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{}.dmn\"", id),
            )
            .body(String::from_utf8_lossy(&doc.content).to_string()),
        Err(engine_state::PersistenceError::NotFound(_)) => {
            HttpResponse::NotFound().json(serde_json::json!({
                "error": format!("decision '{}' not found", id)
            }))
        }
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("failed to export decision: {}", e)
        })),
    }
}

/// Return parsed decision table IR as JSON — `GET /v1/decisions/{id}/ir`.
///
/// Loads the raw DMN XML, parses it via the Phase 29 parser, extracts the
/// Decision node by ID from `definitions.children`, and returns the full
/// DecisionTable IR as JSON. This is what the frontend uses to load a saved
/// decision for editing — without it, the frontend has no way to get
/// input_expression names from the backend.
pub async fn get_decision_ir(
    persistence: web::Data<FileSystemDmnStore>,
    path: web::Path<String>,
) -> HttpResponse {
    let decision_id = path.into_inner();

    match persistence.get(&decision_id).await {
        Ok(doc) => {
            let content = String::from_utf8_lossy(&doc.content).to_string();
            match engine::dmn::parser::parse_dmn(&content) {
                Ok(model) => {
                    // Find the decision node by ID in definitions.children.
                    let target_id = decision_id.clone();
                    let decision_node = model.definitions.children.iter().find_map(|child| {
                        if let DefinitionsChild::Decision(decision) = child {
                            if decision.id == target_id {
                                return Some(decision);
                            }
                        }
                        None
                    });

                    match decision_node {
                        Some(decision) => {
                            if let Some(ref table) = decision.decision_table {
                                HttpResponse::Ok().json(serde_json::json!({
                                    "decision_id": doc.id,
                                    "name": doc.name,
                                    "decision_table": table,
                                }))
                            } else {
                                HttpResponse::BadRequest().json(serde_json::json!({
                                    "error": "Decision has no decision table expression"
                                }))
                            }
                        }
                        None => HttpResponse::NotFound().json(serde_json::json!({
                            "error": format!("decision '{}' not found in model", decision_id)
                        })),
                    }
                }
                Err(e) => HttpResponse::BadRequest().json(serde_json::json!({
                    "error": format!("failed to parse DMN: {}", e)
                })),
            }
        }
        Err(engine_state::PersistenceError::NotFound(_)) => {
            HttpResponse::NotFound().json(serde_json::json!({
                "error": format!("decision '{}' not found", decision_id)
            }))
        }
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("failed to get decision: {}", e)
        })),
    }
}

/// Reload a DMN model — `POST /v1/decisions/reload`.
pub async fn reload_dmn(
    evaluator: web::Data<Arc<DecisionEvaluator>>,
    body: web::Json<ReloadDmnRequest>,
) -> HttpResponse {
    match evaluator.reload(&body.dmn_content) {
        Ok(()) => HttpResponse::Ok().json(serde_json::json!({
            "status": "reloaded",
            "message": "DMN model reloaded successfully"
        })),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("{}", e)
        })),
    }
}

/// Request body for DMN import.
#[derive(Debug, Deserialize)]
pub struct ImportDmnRequest {
    /// Decision ID extracted from the DMN file or provided by caller.
    pub id: String,
    /// Raw DMN XML content.
    pub content: Vec<u8>,
}

// =========================================================================
// Route configuration
// =========================================================================

/// Configure `/v1/decisions` routes (Phase 33).
pub fn configure_v1_decisions(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/decisions")
            // CRUD endpoints.
            .route("", web::get().to(list_decisions))
            .route("/", web::get().to(list_decisions))
            .route("", web::post().to(create_decision))
            .route("/{id}", web::get().to(get_decision))
            .route("/{id}", web::put().to(update_decision))
            .route("/{id}", web::delete().to(delete_decision))
            // Evaluate endpoint.
            .route("/{id}/evaluate", web::post().to(evaluate_decision))
            // Import/export endpoints.
            .route("/import", web::post().to(import_dmn))
            .route("/{id}/export", web::get().to(export_dmn))
            // IR endpoint — returns parsed decision table as JSON for frontend editing.
            .route("/{id}/ir", web::get().to(get_decision_ir))
            // Hot-reload endpoint.
            .route("/reload", web::post().to(reload_dmn)),
    );
}
