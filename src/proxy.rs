use axum::{body::Body, extract::{Path, State}, http::HeaderMap, response::Response, Extension};
use http::{HeaderValue, Method};
use tracing::info;
use http_body_util::BodyExt;
use bytes::Bytes;
use std::sync::Arc;
use url::Url;

use crate::{app::REQUEST_ID_HEADER, errors::AppError, state::AppState};

/// Validate URL against allowed domains to prevent SSRF attacks
fn validate_destination_url(url: &str, allowed_domains: &[String]) -> Result<(), AppError> {
    let parsed_url = Url::parse(url).map_err(|_| {
        AppError::InvalidDestination(format!("Invalid URL format: {}", url))
    })?;

    // Only allow HTTP and HTTPS protocols
    if !matches!(parsed_url.scheme(), "http" | "https") {
        return Err(AppError::InvalidDestination(format!(
            "Only HTTP and HTTPS protocols allowed, got: {}",
            parsed_url.scheme()
        )));
    }

    // Check if the host is in the allowed domains
    let host = parsed_url.host_str().ok_or_else(|| {
        AppError::InvalidDestination(format!("URL missing host: {}", url))
    })?;

    let is_allowed = allowed_domains.iter().any(|allowed| {
        host == allowed || host.ends_with(&format!(".{}", allowed))
    });

    if !is_allowed {
        return Err(AppError::InvalidDestination(format!(
            "Host '{}' not in allowed domains: {:?}",
            host, allowed_domains
        )));
    }

    Ok(())
}

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
    let matched_route = config_guard
    .find_route_for_path(&request_path);

    let route = match matched_route {
        Some(route) => route,
        None => return Err(AppError::RouteNotFound),
    };

    let destination_path = request_path.strip_prefix(&route.path).unwrap_or("");
    
    let destination_url = format!("{}{}", route.destination, destination_path);

    // Validate URL to prevent SSRF attacks
    let allowed_domains = &config_guard.security.allowed_domains;
    validate_destination_url(&destination_url, allowed_domains)
        .map_err(|e| {
            tracing::error!("SSRF protection: URL validation failed for {}: {}", destination_url, e);
            e
        })?;

    info!(destination = %destination_url, "Forwarding request to backend");
    
    headers.insert(
        REQUEST_ID_HEADER,
        HeaderValue::from_str(&request_id).unwrap(),
    );

    let body_bytes: Bytes = body.collect().await
        .map_err(|e| {
        tracing::error!("Failed to read request body: {}", e);
        AppError::InternalServerError
        })?
        .to_bytes();

    let request = state.http_client
        .request(method, &destination_url)
        .headers(headers)
        .body(body_bytes)
        .build()
        .map_err(|e|{
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

        let mut response = response_builder.body(body).unwrap();
        response.headers_mut().insert(
            REQUEST_ID_HEADER,
            HeaderValue::from_str(&request_id).unwrap(),
        );
        Ok(response)    
        

}