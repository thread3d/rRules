//! Authentication utilities for API v1 routes

use actix_web::{dev::ServiceRequest, Error};

/// Check if a request has a valid Bearer token in the Authorization header
pub fn requires_auth(req: &ServiceRequest) -> Result<(), Error> {
    let auth_header = req.headers().get("Authorization");

    match auth_header {
        Some(header) => {
            let auth_str = header.to_str().map_err(|_| {
                actix_web::error::ErrorUnauthorized("Invalid authorization header format")
            })?;

            if !auth_str.starts_with("Bearer ") {
                return Err(actix_web::error::ErrorUnauthorized(
                    "Missing or invalid authorization token",
                ));
            }

            Ok(())
        }
        None => Err(actix_web::error::ErrorUnauthorized(
            "Missing or invalid authorization token",
        )),
    }
}
