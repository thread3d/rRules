//! Decision evaluation endpoints (Phase 33).
//!
//! Provides `POST /v1/decisions/{id}/evaluate` for decision service (§10.4) with:
//! - Multi-output support (MVP: evaluate all outputs of a single decision)
//! - Optional inference trace emission
//! - Per-decision Prometheus metrics

use actix_web::{web, HttpResponse};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use engine::data_types::Value;
use engine::dmn::evaluator::{DecisionEvaluator, EvaluateRequest};
use engine::dmn::model::{DmnModel, DefinitionsChild, Decision};

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

// =========================================================================
// Helper: extract decisions from Definitions children
// =========================================================================

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
    evaluator: web::Data<DecisionEvaluator>,
    path: web::Path<String>,
    body: web::Json<EvaluateDecisionRequest>,
) -> HttpResponse {
    let decision_id = path.into_inner();

    // Build the evaluation request.
    let request = EvaluateRequest {
        inputs: body.inputs.clone(),
        trace: body.trace,
    };

    // Execute the evaluation.
    match evaluator.evaluate(&decision_id, request) {
        Ok(result) => {
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
            _ => {
                HttpResponse::InternalServerError().json(serde_json::json!({ "error": format!("{}", e) }))
            }
        },
    }
}

/// Get a DMN model — `GET /v1/decisions`.
pub async fn list_decisions(
    evaluator: web::Data<DecisionEvaluator>,
) -> HttpResponse {
    let guard = match evaluator.shared.read() {
        Ok(guard) => guard,
        Err(e) => {
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("failed to acquire read lock: {}", e)
            }));
        }
    };

    let decisions = extract_decisions(&guard.model).iter().map(|d| {
        serde_json::json!({
            "id": d.id,
            "name": d.name.as_deref().unwrap_or(""),
            "description": d.description.as_ref().map(|dc| dc.content.clone()),
            "hit_policy": d.decision_table.as_ref()
                .map(|t| format!("{:?}", t.hit_policy))
                .unwrap_or_else(|| "None".to_string()),
        })
    }).collect::<Vec<_>>();

    HttpResponse::Ok().json(serde_json::json!({
        "decisions": decisions,
        "model_version": guard.generation(),
    }))
}

/// Get a specific decision — `GET /v1/decisions/{id}`.
pub async fn get_decision(
    evaluator: web::Data<DecisionEvaluator>,
    path: web::Path<String>,
) -> HttpResponse {
    let decision_id = path.into_inner();

    let guard = match evaluator.shared.read() {
        Ok(guard) => guard,
        Err(e) => {
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("failed to acquire read lock: {}", e)
            }));
        }
    };

    let decision = match extract_decisions(&guard.model).into_iter()
        .find(|d| d.id == decision_id) {
        Some(d) => d,
        None => {
            return HttpResponse::NotFound().json(serde_json::json!({
                "error": format!("decision '{}' not found", decision_id)
            }));
        }
    };

    let table_info = decision.decision_table.as_ref().map(|t| {
        serde_json::json!({
            "id": t.id,
            "name": t.name,
            "hit_policy": format!("{:?}", t.hit_policy),
            "input_clauses": t.input_clauses.iter().map(|ic| {
                serde_json::json!({
                    "label": ic.label,
                    "input_expression": ic.input_expression,
                })
            }).collect::<Vec<_>>(),
            "output_clauses": t.output_clauses.iter().map(|oc| {
                serde_json::json!({
                    "name": oc.name,
                    "type_ref": oc.type_ref,
                })
            }).collect::<Vec<_>>(),
        })
    });

    HttpResponse::Ok().json(serde_json::json!({
        "id": decision.id,
        "name": decision.name,
        "description": decision.description.as_ref().map(|dc| dc.content.clone()),
        "question": decision.question,
        "decision_table": table_info,
    }))
}

/// Reload a DMN model — `POST /v1/decisions/reload`.
pub async fn reload_dmn(
    evaluator: web::Data<DecisionEvaluator>,
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

// =========================================================================
// Route configuration
// =========================================================================

/// Configure `/v1/decisions` routes.
pub fn configure_v1_decisions(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/decisions")
            .route("", web::get().to(list_decisions))
            .route("/", web::get().to(list_decisions))
            .route("/{id}", web::get().to(get_decision))
            .route("/{id}/evaluate", web::post().to(evaluate_decision))
            .route("/reload", web::post().to(reload_dmn)),
    );
}
