//! Rule execution API endpoints - stub implementations

use actix_web::{HttpResponse, Responder};

/// Execute rules - POST /v1/rules/execute
pub async fn execute_rules() -> impl Responder {
    HttpResponse::NotImplemented().json(serde_json::json!({
        "error": "not implemented"
    }))
}

/// Execute with session - POST /v1/rules/execute/session  
pub async fn execute_with_session() -> impl Responder {
    HttpResponse::NotImplemented().json(serde_json::json!({
        "error": "not implemented"
    }))
}
