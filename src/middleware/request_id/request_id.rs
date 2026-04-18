use std::sync::Arc;

use crate::app::REQUEST_ID_HEADER;
use axum::{
    body::Body,
    http::{HeaderValue, Request},
    middleware::Next,
    response::Response,
};
use uuid::Uuid;

pub async fn layer(mut req: Request<Body>, next: Next) -> Response {
    let id = req
        .headers()
        .get(REQUEST_ID_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(|s| s.to_string());

    let request_id = match id {
        Some(id) => id,
        None => {
            let new_id = Uuid::new_v4().to_string();
            req.headers_mut().insert(
                REQUEST_ID_HEADER,
                HeaderValue::from_str(&new_id).unwrap_or_else(|_| HeaderValue::from_static("unknown")),
            );
            new_id
        }
    };

    // Store the final request ID in the request extensions so it can be
    // accessed by other handlers, like our proxy handler.
    req.extensions_mut().insert(Arc::new(request_id));

    next.run(req).await
}
