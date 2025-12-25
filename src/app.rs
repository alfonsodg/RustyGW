use std::{sync::Arc};

use anyhow::Error;
use axum::{extract::{Request}, middleware::{from_fn, from_fn_with_state}, routing::{any, get}, Router};
use http::StatusCode;
use axum_client_ip::{ClientIpSource};
use tower_http::{trace::TraceLayer};
use uuid::Uuid;

use crate::{middleware::{auth::auth::layer as auth_layer, cache::cache::layer as cache_layer, circuit_breaker::circuit_breaker::layer as circuit_breaker_layer, rate_limiter::rate_limit::layer as ratelimiter_layer, request_id::request_id::layer as request_id_layer}, proxy::proxy_handler, state::AppState, utils::metric_handler::metrics_handler};

pub const REQUEST_ID_HEADER: &str = "x-request-id";

pub fn create_app(state: Arc<AppState>) -> Result<Router,Error> {
    let proxy_router = Router::new()
        .route("/{*path}", any(proxy_handler))
        .route_layer(from_fn_with_state(state.clone(), circuit_breaker_layer))
        .route_layer(from_fn_with_state(state.clone(), cache_layer))
        .route_layer(
            from_fn_with_state(state.clone(), ratelimiter_layer)
        ) 
        .route_layer(from_fn_with_state(state.clone(),auth_layer));

    let prometheus_router = Router::new()
        .route("/metrics", get(metrics_handler));

    let router = Router::new()
        .route("/health", get(|| async { (StatusCode::OK, "OK") }))
        .merge(proxy_router)
        .merge(prometheus_router)
        .with_state(state)
        .layer(ClientIpSource::ConnectInfo.into_extension());
 
    Ok(router
        .layer(
        TraceLayer::new_for_http().make_span_with(|request:&Request<_>| {
            let uuid = Uuid::new_v4().to_string();
            let request_id = request
                    .headers()
                    .get(REQUEST_ID_HEADER)
                    .and_then(|value| value.to_str().ok())
                    .unwrap_or(uuid.as_str());

            tracing::error_span!(
                    "request",
                    id = %request_id,
                    method = %request.method(),
                    uri = %request.uri(),
            )
        })   
        )
        .layer(from_fn(request_id_layer))
    )
}
