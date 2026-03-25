use axum::{
    Extension,
    body::Body,
    extract::{Path, State},
    http::HeaderMap,
    response::Response,
};
use bytes::Bytes;
use http::{HeaderValue, Method};
use http_body_util::BodyExt;
use std::sync::Arc;
use tracing::info;

use crate::{app::REQUEST_ID_HEADER, errors::AppError, state::AppState};

#[axum::debug_handler]
pub async fn proxy_handler(
    State(state): State<Arc<AppState>>,
    Extension(request_id): Extension<Arc<String>>,
    Path(path): Path<String>,
    method: Method,
    mut headers: HeaderMap,
    body: Body,
) -> Result<Response, AppError> {
    let request_path = format!("/{}", path);
    info!("Received request for path: {}", request_path);

    let config_guard = state.config.read().await;
    let matched_route = config_guard.find_route_for_path(&request_path);

    let route = match matched_route {
        Some(route) => route,
        None => return Err(AppError::RouteNotFound),
    };

    let destination_path = request_path.strip_prefix(&route.path).unwrap_or("");

    let destinations = route.all_destinations();
    let healthy = state.health_checker.filter_healthy(&destinations);
    let idx = state.load_balancer.next_index(healthy.len(), &route.load_balance);
    let destination_url = format!("{}{}", healthy[idx], destination_path);

    info!(destination = %destination_url, strategy = ?route.load_balance, "Forwarding request to backend");

    headers.insert(
        REQUEST_ID_HEADER,
        HeaderValue::from_str(&request_id)
            .unwrap_or_else(|_| HeaderValue::from_static("unknown")),
    );

    let body_bytes: Bytes = body
        .collect()
        .await
        .map_err(|e| {
            tracing::error!("Failed to read request body: {}", e);
            AppError::InternalServerError
        })?
        .to_bytes();

    let request = state
        .http_client
        .request(method, &destination_url)
        .headers(headers)
        .body(body_bytes)
        .build()
        .map_err(|e| {
            tracing::error!("Failed to build reqwest request: {}", e);
            AppError::InvalidDestination(destination_url)
        })?;

    let response = state.http_client.execute(request).await?;

    let status = response.status();
    let headers = response.headers().clone();
    let bytes = response.bytes().await.map_err(AppError::from)?;
    let body = Body::from(bytes);

    let mut response_builder = Response::builder().status(status);
    for (name, value) in headers.iter() {
        response_builder = response_builder.header(name, value);
    }

    let mut response = response_builder.body(body)
        .map_err(|_| AppError::InternalServerError)?;
    response.headers_mut().insert(
        REQUEST_ID_HEADER,
        HeaderValue::from_str(&request_id)
            .unwrap_or_else(|_| HeaderValue::from_static("unknown")),
    );
    Ok(response)
}
