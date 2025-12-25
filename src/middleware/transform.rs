//! Request/Response transformation middleware.
//!
//! Allows adding, removing, or modifying headers on requests and responses.

use std::sync::Arc;
use axum::{extract::{Request, State}, middleware::Next, response::Response};
use http::HeaderValue;

use crate::{errors::AppError, middleware::get_route_config, state::AppState};

/// Transformation middleware layer
pub async fn layer(
    State(state): State<Arc<AppState>>,
    mut req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let path = req.uri().path().to_string();
    let route = get_route_config(&state, &path).await;

    // Apply request transformations
    if let Some(route_config) = &route {
        if let Some(transform) = &route_config.transform {
            // Add request headers
            for (key, value) in &transform.request_headers.add {
                if let Ok(header_value) = HeaderValue::from_str(value) {
                    req.headers_mut().insert(
                        http::HeaderName::try_from(key.as_str()).unwrap(),
                        header_value,
                    );
                }
            }
            // Remove request headers
            for key in &transform.request_headers.remove {
                if let Ok(header_name) = http::HeaderName::try_from(key.as_str()) {
                    req.headers_mut().remove(header_name);
                }
            }
        }
    }

    let mut response = next.run(req).await;

    // Apply response transformations
    if let Some(route_config) = &route {
        if let Some(transform) = &route_config.transform {
            // Add response headers
            for (key, value) in &transform.response_headers.add {
                if let Ok(header_value) = HeaderValue::from_str(value) {
                    response.headers_mut().insert(
                        http::HeaderName::try_from(key.as_str()).unwrap(),
                        header_value,
                    );
                }
            }
            // Remove response headers
            for key in &transform.response_headers.remove {
                if let Ok(header_name) = http::HeaderName::try_from(key.as_str()) {
                    response.headers_mut().remove(header_name);
                }
            }
        }
    }

    Ok(response)
}
