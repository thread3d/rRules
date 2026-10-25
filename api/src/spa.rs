//! Serving the single-page application's shell.
//!
//! The interface is a SPA: `/rules`, `/governance/approvals` and the rest are routes
//! the *router* owns, not files on disk. The server therefore serves a real file
//! when one exists and the shell (`index.html`) for everything else. Without that
//! fallback every page except the landing page answered 404 on a reload or a
//! bookmark — before the router had a chance to run.
//!
//! API paths are deliberately *not* answered with HTML: an unknown `/v1/...` must
//! stay a 404, or a typo in a client would look like success.

use actix_web::{http::header, web, HttpRequest, HttpResponse};

/// Paths that belong to the API, not the interface.
const API_PREFIXES: &[&str] = &["/v1", "/api-docs", "/metrics", "/swagger-ui"];

/// Read the shell once, at startup, so serving a route costs no file I/O.
pub fn load(frontend_dist: &str) -> Option<String> {
    let index = std::path::Path::new(frontend_dist).join("index.html");
    match std::fs::read_to_string(&index) {
        Ok(html) => Some(html),
        Err(error) => {
            tracing::warn!(
                path = %index.display(),
                %error,
                "no index.html in the frontend build directory: client-side routes will 404"
            );
            None
        }
    }
}

/// Serve the cached shell for a non-API GET, or a 404 for an API path.
pub async fn serve_index(index: web::Data<Option<String>>, req: HttpRequest) -> HttpResponse {
    let path = req.path();
    if API_PREFIXES.iter().any(|prefix| path.starts_with(prefix)) {
        return HttpResponse::NotFound().json(serde_json::json!({
            "error": format!("no such endpoint: {path}")
        }));
    }
    match index.get_ref() {
        Some(html) => HttpResponse::Ok()
            .content_type(header::ContentType::html())
            .body(html.clone()),
        None => HttpResponse::NotFound()
            .content_type(header::ContentType::plaintext())
            .body("the frontend was not built: no index.html"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, App};

    const SHELL: &str = "<!doctype html><title>app shell</title>";

    #[actix_web::test]
    async fn a_client_side_route_serves_the_shell() {
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(Some(SHELL.to_string())))
                .default_service(web::get().to(serve_index)),
        )
        .await;

        let response = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/governance/approvals")
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), actix_web::http::StatusCode::OK);
        let body = test::read_body(response).await;
        assert_eq!(std::str::from_utf8(&body).unwrap(), SHELL);
    }

    #[actix_web::test]
    async fn an_unknown_api_path_stays_a_404() {
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(Some(SHELL.to_string())))
                .default_service(web::get().to(serve_index)),
        )
        .await;

        let response = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/v1/no-such-endpoint")
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), actix_web::http::StatusCode::NOT_FOUND);
    }

    #[::core::prelude::v1::test]
    fn a_missing_build_reports_no_shell() {
        assert!(load("/nonexistent/frontend/dist").is_none());
    }
}
