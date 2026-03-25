use std::sync::Arc;

use anyhow::Error;
use axum::{
    Router,
    extract::Request,
    middleware::{from_fn, from_fn_with_state},
    routing::{any, get},
};
use axum_client_ip::ClientIpSource;
use http::{HeaderName, Method as HttpMethod, StatusCode};
use tower_http::{compression::CompressionLayer, cors::CorsLayer, trace::TraceLayer};
use uuid::Uuid;

use crate::{
    middleware::{
        auth::auth::layer as auth_layer, cache::cache::layer as cache_layer,
        circuit_breaker::circuit_breaker::layer as circuit_breaker_layer,
        rate_limiter::rate_limit::layer as ratelimiter_layer,
        request_id::request_id::layer as request_id_layer,
        tracing_ctx::layer as tracing_ctx_layer,
        access_log::layer as access_log_layer,
    },
    proxy::proxy_handler,
    aggregate::aggregate_handler,
    grpc_proxy::grpc_proxy_handler,
    state::AppState,
    utils::metric_handler::metrics_handler,
    ws_proxy::ws_proxy_handler,
};

pub const REQUEST_ID_HEADER: &str = "x-request-id";

pub fn create_app(state: Arc<AppState>, cors: &crate::config::CorsConfig, body_limit: usize) -> Result<Router, Error> {
    let proxy_router = Router::new()
        .route("/{*path}", any(proxy_handler))
        .route_layer(from_fn_with_state(state.clone(), circuit_breaker_layer))
        .route_layer(from_fn_with_state(state.clone(), cache_layer))
        .route_layer(from_fn_with_state(state.clone(), ratelimiter_layer))
        .route_layer(from_fn_with_state(state.clone(), auth_layer));

    let ws_router = Router::new().route("/ws/{*path}", get(ws_proxy_handler));
    let agg_router = Router::new().route("/agg/{*path}", get(aggregate_handler));
    let grpc_router = Router::new().route("/grpc/{*path}", any(grpc_proxy_handler));
    let prometheus_router = Router::new().route("/metrics", get(metrics_handler));

    // Build CORS layer
    let cors_layer = if cors.enabled {
        let origins: Vec<_> = cors.origins.iter()
            .filter_map(|o| o.parse().ok())
            .collect();
        let methods: Vec<HttpMethod> = cors.methods.iter()
            .filter_map(|m| m.parse().ok())
            .collect();
        let mut layer = CorsLayer::new()
            .allow_methods(methods)
            .allow_origin(origins);
        if !cors.allow_headers.is_empty() {
            let headers: Vec<HeaderName> = cors.allow_headers.iter()
                .filter_map(|h| h.parse().ok())
                .collect();
            layer = layer.allow_headers(headers);
        } else {
            layer = layer.allow_headers(tower_http::cors::Any);
        }
        Some(layer)
    } else {
        None
    };

    let router = Router::new()
        .route("/health", get(|| async { (StatusCode::OK, "OK") }))
        .merge(ws_router)
        .merge(agg_router)
        .merge(grpc_router)
        .merge(proxy_router)
        .merge(prometheus_router)
        .layer(from_fn(tracing_ctx_layer))
        .layer(from_fn(access_log_layer))
        .with_state(state)
        .layer(ClientIpSource::ConnectInfo.into_extension());

    let router = if let Some(cors_layer) = cors_layer {
        router.layer(cors_layer)
    } else {
        router
    };

    let router = router
        .layer(CompressionLayer::new())
        .layer(axum::extract::DefaultBodyLimit::max(body_limit));

    Ok(router
        .layer(
            TraceLayer::new_for_http().make_span_with(|request: &Request<_>| {
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
            }),
        )
        .layer(from_fn(request_id_layer)))
}
