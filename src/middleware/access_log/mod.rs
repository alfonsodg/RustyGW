use axum::{extract::Request, middleware::Next, response::Response};
use std::time::Instant;
use tracing::info;

/// Structured access log middleware.
/// Logs method, path, status, duration for every request.
pub async fn layer(req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let start = Instant::now();

    let response = next.run(req).await;

    let duration = start.elapsed();
    let status = response.status().as_u16();

    info!(
        method = %method,
        path = %path,
        status = status,
        duration_ms = duration.as_millis() as u64,
        "access"
    );

    response
}
