//! API-level error handling with proper HTTP status codes

use actix_web::{HttpResponse, ResponseError};
use std::fmt;

/// Custom error type for API endpoints with appropriate HTTP status codes
#[derive(Debug)]
pub enum ApiError {
    /// 400 Bad Request - invalid client input
    BadRequest(String),
    /// 401 Unauthorized - authentication required
    Unauthorized(String),
    /// 403 Forbidden - insufficient permissions
    Forbidden(String),
    /// 404 Not Found - resource doesn't exist
    NotFound(String),
    /// 409 Conflict - resource already exists or state conflict
    Conflict(String),
    /// 500 Internal Server Error - unexpected server failure
    InternalError(String),
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApiError::BadRequest(msg) => write!(f, "Bad Request: {}", msg),
            ApiError::Unauthorized(msg) => write!(f, "Unauthorized: {}", msg),
            ApiError::Forbidden(msg) => write!(f, "Forbidden: {}", msg),
            ApiError::NotFound(msg) => write!(f, "Not Found: {}", msg),
            ApiError::Conflict(msg) => write!(f, "Conflict: {}", msg),
            ApiError::InternalError(msg) => write!(f, "Internal Server Error: {}", msg),
        }
    }
}

impl std::error::Error for ApiError {}

impl ResponseError for ApiError {
    fn error_response(&self) -> HttpResponse {
        match self {
            ApiError::BadRequest(msg) => HttpResponse::BadRequest().json(serde_json::json!({
                "error": msg,
                "status_code": 400
            })),
            ApiError::Unauthorized(msg) => HttpResponse::Unauthorized().json(serde_json::json!({
                "error": msg,
                "status_code": 401
            })),
            ApiError::Forbidden(msg) => HttpResponse::Forbidden().json(serde_json::json!({
                "error": msg,
                "status_code": 403
            })),
            ApiError::NotFound(msg) => HttpResponse::NotFound().json(serde_json::json!({
                "error": msg,
                "status_code": 404
            })),
            ApiError::Conflict(msg) => HttpResponse::Conflict().json(serde_json::json!({
                "error": msg,
                "status_code": 409
            })),
            ApiError::InternalError(msg) => HttpResponse::InternalServerError().json(serde_json::json!({
                "error": msg,
                "status_code": 500
            })),
        }
    }

    fn status_code(&self) -> actix_web::http::StatusCode {
        match self {
            ApiError::BadRequest(_) => actix_web::http::StatusCode::BAD_REQUEST,
            ApiError::Unauthorized(_) => actix_web::http::StatusCode::UNAUTHORIZED,
            ApiError::Forbidden(_) => actix_web::http::StatusCode::FORBIDDEN,
            ApiError::NotFound(_) => actix_web::http::StatusCode::NOT_FOUND,
            ApiError::Conflict(_) => actix_web::http::StatusCode::CONFLICT,
            ApiError::InternalError(_) => actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

/// Convert a Mutex guard error to an InternalError
impl From<std::sync::PoisonError<std::sync::MutexGuard<'_, std::collections::HashMap<uuid::Uuid, crate::v1::rules::StoredRule>>>> for ApiError {
    fn from(_err: std::sync::PoisonError<std::sync::MutexGuard<'_, std::collections::HashMap<uuid::Uuid, crate::v1::rules::StoredRule>>>) -> Self {
        ApiError::InternalError("Resource temporarily unavailable".to_string())
    }
}

/// Helper to convert HashMap get errors to NotFound
pub fn map_to_not_found<T>(opt: Option<&T>, resource_name: &str) -> Result<&T, ApiError> {
    opt.ok_or_else(|| ApiError::NotFound(format!("{} not found", resource_name)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        assert_eq!(format!("{}", ApiError::BadRequest("test".to_string())), "Bad Request: test");
        assert_eq!(format!("{}", ApiError::NotFound("not found".to_string())), "Not Found: not found");
        assert_eq!(format!("{}", ApiError::Conflict("conflict".to_string())), "Conflict: conflict");
    }

    #[test]
    fn test_error_response() {
        let err = ApiError::BadRequest("bad input".to_string());
        let resp = err.error_response();
        assert_eq!(resp.status(), actix_web::http::StatusCode::BAD_REQUEST);

        let err = ApiError::NotFound("missing".to_string());
        let resp = err.error_response();
        assert_eq!(resp.status(), actix_web::http::StatusCode::NOT_FOUND);

        let err = ApiError::Conflict("duplicate".to_string());
        let resp = err.error_response();
        assert_eq!(resp.status(), actix_web::http::StatusCode::CONFLICT);
    }

    #[test]
    fn test_map_to_not_found() {
        let val = "value";
        assert!(map_to_not_found(Some(&val), "item").is_ok());
        assert!(map_to_not_found(None::<&str>, "item").is_err());
    }
}
