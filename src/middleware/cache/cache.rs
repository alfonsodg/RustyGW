use std::{sync::Arc, time::{Duration, Instant}};

use axum::{body::Body, extract::{State}, middleware::Next, response::Response};
use http::Request;
use http_body_util::BodyExt;

use crate::{constants::cache as cache_constants, errors::AppError, middleware::get_route_config, state::{AppState, CachedResponse}, utils::{logging::log_cache_operation, parse_duration}};

/// Sanitize cache key to prevent cache poisoning attacks
fn sanitize_cache_key(uri: &str) -> String {
    // Remove potentially dangerous characters and normalize the key
    let sanitized = uri
        .chars()
        .map(|c| match c {
            '/' => '_',  // Replace slashes with underscores
            '?' => '_',  // Replace query parameters
            '#' => '_',  // Replace fragments
            ' ' => '_',   // Replace spaces
            '\t' => '_',  // Replace tabs
            '\n' => '_',  // Replace newlines
            '\r' => '_',  // Replace carriage returns
            '&' => '_',   // Replace ampersands
            '=' => '_',   // Replace equals signs
            '+' => '_',   // Replace plus signs
            '%' => '_',   // Replace percent signs (potential encoding attacks)
            '<' | '>' | '"' | '\'' | '\\' | '|' | ';' | ':' | ',' | '.' | '[' | ']' | '{' | '}' | '(' | ')' => '_',
            _ if c.is_control() => '_',  // Replace control characters
            _ => c,  // Keep safe characters
        })
        .collect::<String>();
    
    // Limit length to prevent excessive memory usage
    if sanitized.len() > cache_constants::MAX_KEY_LENGTH {
        format!("{}_truncated", &sanitized[..cache_constants::TRUNCATED_KEY_LENGTH])
    } else {
        sanitized
    }
}

pub async fn layer(
    State(state): State<Arc<AppState>>,
    req: Request<Body>,
    next: Next
) -> Result<Response, AppError> {
 
    let route = get_route_config(&state, req.uri().path()).await;

    let cache_config = match route.and_then(|r| r.cache.as_ref().cloned()) {
        Some(c) => c,
        None => return Ok(next.run(req).await),
    };

    if !req.method().is_safe() {
        return Ok(next.run(req).await);
    }

    let cache_key = sanitize_cache_key(&req.uri().to_string());
    let ttl = parse_duration(&cache_config.ttl).unwrap_or_else(|_| Duration::MAX);


    //1. check if a valid response is already in the cache.
    if let Some(cached_response) = state.cache.get(&cache_key).await {
        if cached_response.inserted_at.elapsed() < ttl {
            log_cache_operation("get", &cache_key, true, Some(ttl.as_secs()));
            let mut builder = Response::builder().status(cached_response.status);
            *builder.headers_mut().unwrap() = cached_response.headers.clone();
            return Ok(builder.body(Body::from(cached_response.body.clone())).unwrap());
        } else {
            log_cache_operation("expired", &cache_key, false, None);
            state.cache.invalidate(&cache_key).await;
        }
        
    }

    log_cache_operation("get", &cache_key, false, None);

    // 2. If not in cache, call the next middleware (and eventually the proxy handler).
    let response = next.run(req).await;

    if response.status().is_success() {
        let (parts, body) = response.into_parts();
        let bytes = body.collect().await.map_err(|_| AppError::InternalServerError)?.to_bytes();

        let cached_response = Arc::new(CachedResponse {
            status: parts.status,
            headers: parts.headers.clone(),
            body: bytes.clone(),
            inserted_at: Instant::now(),
        });

        state.cache.insert(cache_key, cached_response).await;

        return Ok(Response::from_parts(parts, Body::from(bytes)));

    }

    Ok(response)

}

    