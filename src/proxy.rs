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

    let route_timeout = route.timeout.as_ref()
        .map(|t| crate::features::health_check::parse_duration(t));

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

    let max_attempts = route.retry.as_ref().map(|r| r.count + 1).unwrap_or(1);
    let retry_on: Vec<u16> = route.retry.as_ref()
        .map(|r| if r.retry_on.is_empty() { vec![502, 503, 504] } else { r.retry_on.clone() })
        .unwrap_or_default();
    let backoff = route.retry.as_ref()
        .map(|r| crate::features::health_check::parse_duration(&r.backoff))
        .unwrap_or(std::time::Duration::from_millis(100));

    let mut last_err = None;
    for attempt in 0..max_attempts {
        let mut req_builder = state.http_client
            .request(method.clone(), &destination_url)
            .headers(headers.clone())
            .body(body_bytes.clone());

        if let Some(timeout) = route_timeout {
            req_builder = req_builder.timeout(timeout);
        }

        let request = req_builder.build().map_err(|e| {
            tracing::error!("Failed to build reqwest request: {}", e);
            AppError::InvalidDestination(destination_url.clone())
        })?;

        match state.http_client.execute(request).await {
            Ok(resp) => {
                let status = resp.status();
                if attempt + 1 < max_attempts && retry_on.contains(&status.as_u16()) {
                    tracing::warn!(attempt = attempt + 1, status = %status, "Retrying request");
                    tokio::time::sleep(backoff * (attempt + 1)).await;
                    continue;
                }
                let resp_headers = resp.headers().clone();
                let bytes = resp.bytes().await.map_err(AppError::from)?;
                let body = Body::from(bytes);

                let mut response_builder = Response::builder().status(status);
                for (name, value) in resp_headers.iter() {
                    response_builder = response_builder.header(name, value);
                }
                let mut response = response_builder.body(body)
                    .map_err(|_| AppError::InternalServerError)?;
                response.headers_mut().insert(
                    REQUEST_ID_HEADER,
                    HeaderValue::from_str(&request_id)
                        .unwrap_or_else(|_| HeaderValue::from_static("unknown")),
                );
                return Ok(response);
            }
            Err(e) => {
                if attempt + 1 < max_attempts {
                    tracing::warn!(attempt = attempt + 1, "Request failed, retrying: {}", e);
                    tokio::time::sleep(backoff * (attempt + 1)).await;
                }
                last_err = Some(e);
            }
        }
    }

    Err(last_err.map(AppError::from).unwrap_or(AppError::InternalServerError))
}
