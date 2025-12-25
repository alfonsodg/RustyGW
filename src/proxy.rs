//! Proxy module for forwarding requests to backend services.
//!
//! Handles request routing, load balancing, and response forwarding.

use axum::{body::Body, extract::{Path, State}, http::HeaderMap, response::Response, Extension};
use http::{HeaderValue, Method};
use tracing::info;
use http_body_util::BodyExt;
use bytes::Bytes;
use std::sync::Arc;
use url::Url;

use crate::{app::REQUEST_ID_HEADER, errors::AppError, state::AppState, utils::logging::*};

/// Selects a backend destination using round-robin load balancing.
///
/// # Arguments
/// * `route` - Route configuration with destination list
/// * `request_id` - Unique request ID for consistent hashing
fn select_destination(route: &crate::config::RouteConfig, request_id: &str) -> Result<String, AppError> {
    let destinations = &route.destinations;
    
    // For backward compatibility, if no destinations specified, use single destination
    if destinations.is_empty() && !route.destination.is_empty() {
        return Ok(route.destination.clone());
    }
    
    if destinations.is_empty() {
        return Err(AppError::InvalidDestination("No destinations configured".to_string()));
    }
    
    // Simple round-robin based on request_id hash
    if destinations.len() == 1 {
        return Ok(destinations[0].clone());
    }
    
    let hash = request_id.chars().map(|c| c as usize).sum::<usize>();
    let selected_index = hash % destinations.len();
    Ok(destinations[selected_index].clone())
}

/// Filter headers to prevent injection attacks - only allow safe headers
fn filter_safe_headers(headers: &HeaderMap) -> HeaderMap {
    let mut safe_headers = HeaderMap::new();
    
    // Whitelist of safe headers that can be propagated
    let safe_headers_list = [
        "content-type",
        "content-length", 
        "accept",
        "accept-encoding",
        "user-agent",
        "host",
        "connection",
        "cache-control",
    ];
    
    for (name, value) in headers {
        let name_str = name.as_str();
        let name_lowercase = name_str.to_lowercase();
        if safe_headers_list.contains(&name_lowercase.as_str()) {
            safe_headers.insert(name, value.clone());
        }
    }
    
    safe_headers
}

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
    
    // Use load balancing to select destination
    let selected_destination = select_destination(&route, &request_id)?;
    let destination_url = format!("{}{}", selected_destination, destination_path);

    // Validate URL to prevent SSRF attacks
    let allowed_domains = &config_guard.security.allowed_domains;
    validate_destination_url(&destination_url, allowed_domains)
        .map_err(|e| {
            log_security_event("SSRF protection failure", "gateway", 
                &format!("URL validation failed for {}: {}", destination_url, e), "high");
            e
        })?;

    info!(destination = %destination_url, "Forwarding request to backend");
    
    headers.insert(
        REQUEST_ID_HEADER,
        HeaderValue::from_str(&request_id).unwrap(),
    );

    // Filter headers to prevent injection attacks
    let safe_headers = filter_safe_headers(&headers);

    // Check request size limits to prevent DoS attacks
    let body_bytes: Bytes = body.collect().await
        .map_err(|e| {
        log_error(&e, "request_body_parsing", "body_collect_error");
        AppError::InternalServerError
        })?
        .to_bytes();

    // Validate request size against configured limits
    let max_size = config_guard.security.max_request_size;
    if body_bytes.len() > max_size {
        log_security_event("Request size exceeded", "gateway", 
            &format!("Request size {} bytes exceeds limit {} bytes", body_bytes.len(), max_size), "medium");
        return Err(AppError::InvalidDestination("Request too large".to_string()));
    }

    let request = state.http_client
        .request(method, &destination_url)
        .headers(safe_headers) // Use filtered headers only
        .body(body_bytes)
        .build()
        .map_err(|e|{
            log_error(&e, "request_building", "reqwest_build_error");
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