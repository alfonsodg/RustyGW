use std::{sync::Arc, time::{Duration, Instant}};

use axum::{body::Body, extract::{State}, middleware::Next, response::Response};
use http::Request;
use http_body_util::BodyExt;
use tracing::info;

use crate::{errors::AppError, middleware::rate_limiter::rate_limit::parse_duration, state::{AppState, CachedResponse}};

pub async fn layer(
    State(state): State<Arc<AppState>>,
    req: Request<Body>,
    next: Next
) -> Result<Response, AppError> {
 
    let config_guard = state.config.read().await;

    let route = config_guard.find_route_for_path(req.uri().path());

    let cache_config = match route.and_then(|r| r.cache.clone()) {
        Some(c) => c,
        None => return Ok(next.run(req).await),
    };

    if !req.method().is_safe() {
        return Ok(next.run(req).await);
    }

    let cache_key = req.uri().to_string();
    let ttl = parse_duration(&cache_config.ttl).unwrap_or(Duration::MAX); // item will be explicitly removed by cache algo


    //1. check if a valid response is already in the cache.
    if let Some(cached_response) = state.cache.get(&cache_key).await {
        if cached_response.inserted_at.elapsed() < ttl {
            info!(key = %cache_key, "Cache HIT");
            let mut builder = Response::builder().status(cached_response.status);
            *builder.headers_mut().unwrap() = cached_response.headers.clone();
            return Ok(builder.body(Body::from(cached_response.body.clone())).unwrap());
        } else {
            info!(key = %cache_key, "Cache STALE (expired)");
            state.cache.invalidate(&cache_key).await;
        }
        
    }

    info!(key = %cache_key, "Cache MISS");

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

    