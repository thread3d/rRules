//! Correlation ID middleware for request tracing.
//!
//! Generates a UUID per request, injects it into the tracing context,
//! and includes it in all log lines automatically via the JSON subscriber format.

use actix_web::body::MessageBody;
use actix_web::dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::Error;
use actix_web::HttpMessage;
use futures_util::future::{ok, LocalBoxFuture, Ready};
use uuid::Uuid;

/// Middleware that generates a correlation ID per request and injects it into tracing.
pub struct CorrelationId;

impl<S, B> Transform<S, ServiceRequest> for CorrelationId
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = CorrelationIdMiddleware<S, B>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(CorrelationIdMiddleware {
            service,
            _body: std::marker::PhantomData,
        })
    }
}

pub struct CorrelationIdMiddleware<S, B> {
    service: S,
    _body: std::marker::PhantomData<B>,
}

impl<S, B> Service<ServiceRequest> for CorrelationIdMiddleware<S, B>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    #[allow(clippy::type_complexity)]
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;
    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let request_id = Uuid::new_v4().to_string();

        // Inject correlation ID into tracing context so it appears in all log lines.
        let span = tracing::info_span!("request", request_id = %request_id);
        let _guard = span.enter();

        req.extensions_mut().insert(request_id.clone());

        let fut = self.service.call(req);

        Box::pin(async move {
            let res = fut.await?;
            Ok(res)
        })
    }
}
