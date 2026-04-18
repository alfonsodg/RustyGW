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
    let matched = config_guard.match_route_with_params(&request_path);

    let (route, params) = match matched {
        Some((route, params)) => (route, params),
        None => return Err(AppError::RouteNotFound),
    };

    let destination_path = request_path.strip_prefix(&route.path).unwrap_or(&request_path);
    // For parameterized routes, use the full request path as remainder is empty
    let destination_path = if params.is_empty() { destination_path } else { "" };

    let destinations = route.all_destinations();
    let healthy = state.health_checker.filter_healthy(&destinations);
    let idx = match state.load_balancer.next_index(healthy.len(), &route.load_balance) {
        Some(idx) => idx,
        None => {
            tracing::warn!(route = %route.name, "No healthy backends available");
            return Err(AppError::ServiceUnavailable);
        }
    };

    // Apply path rewrite if configured
    let final_path = route.transform.as_ref()
        .and_then(|t| t.rewrite_path.as_ref())
        .map(|rewrite| rewrite.replace("{path}", destination_path))
        .unwrap_or_else(|| destination_path.to_string());

    let destination_url = {
        let mut url = format!("{}{}", healthy[idx], final_path);
        for (key, value) in &params {
            url = url.replace(&format!("{{{}}}", key), value);
        }
        url
    };

    let route_timeout = route.timeout.as_ref()
        .map(|t| crate::features::health_check::parse_duration(t));

    info!(destination = %destination_url, strategy = ?route.load_balance, "Forwarding request to backend");

    // Apply request header transformations
    if let Some(transform) = &route.transform {
        for key in &transform.remove_request_headers {
            headers.remove(key.as_str());
        }
        for (key, value) in &transform.request_headers {
            if let Ok(v) = HeaderValue::from_str(value) {
                headers.insert(http::header::HeaderName::from_bytes(key.as_bytes()).unwrap_or(http::header::HeaderName::from_static("x-invalid")), v);
            }
        }
    }

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

    let client = if route.tls_skip_verify {
        &state.http_client_insecure
    } else {
        &state.http_client
    };

    let mut last_err = None;
    for attempt in 0..max_attempts {
        let mut req_builder = client
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

        match client.execute(request).await {
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
                // Apply response header transformations
                if let Some(transform) = &route.transform {
                    for key in &transform.remove_response_headers {
                        response.headers_mut().remove(key.as_str());
                    }
                    for (key, value) in &transform.response_headers {
                        if let (Ok(k), Ok(v)) = (http::header::HeaderName::from_bytes(key.as_bytes()), HeaderValue::from_str(value)) {
                            response.headers_mut().insert(k, v);
                        }
                    }
                }
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
