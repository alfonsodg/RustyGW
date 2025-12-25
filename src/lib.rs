//! # Rust API Gateway
//!
//! A high-performance API Gateway built with Axum, featuring:
//! - Rate limiting with token bucket algorithm
//! - Circuit breaker pattern for fault tolerance
//! - Response caching with configurable TTL
//! - JWT and API key authentication
//! - Hot configuration reload
//! - Prometheus metrics

pub mod config;
pub mod errors;
pub mod app;
pub mod state;
pub mod proxy;
pub mod middleware;
pub mod features;
pub mod utils;
pub mod constants;


use std::{net::SocketAddr, path::PathBuf, sync::Arc,};

use anyhow::Result;
use axum_prometheus::{PrometheusMetricLayer};
use dotenvy::dotenv;
use moka::future::Cache;
use reqwest::Client;
use tokio::{net::TcpListener, sync::RwLock};
use tracing::{info, error, Level};
use crate::utils::logging::*;

use crate::{config::{ApiKeyStore, GatewayConfig, SecretsConfig}, features::{circuit_breaker::circuit_breaker::CircuitBreakerStore, rate_limiter::state::{InMemoryRateLimitState, RateLimitState}}, utils::hot_reload};
use crate::state::{AppState, CachedResponse};

/// Starts the API Gateway server with the given configuration file.
///
/// # Arguments
/// * `config_path` - Path to the gateway.yaml configuration file
///
/// # Returns
/// * `Result<()>` - Ok if server started successfully
pub async fn run(
    config_path: PathBuf,
) -> Result<()> {

    dotenv().ok();

    tracing_subscriber::fmt()
    .with_max_level(Level::INFO)
    .init();

    log_startup("secrets", "loading", None);
    let secrets = Arc::new(SecretsConfig::from_env()?);
    log_startup("secrets", "loaded", None);

    log_startup("configuration", "loading", None);
    let config = Arc::new(RwLock::new(GatewayConfig::load(
        config_path.clone(),
    )?));
    log_startup("configuration", "loaded", None);

    let key_store_path   = config.read().await.identity.api_key_store_path.clone(); 
    
    log_info("Loading API key store", "startup", "api_key_store_loading");

    let key_store = Arc::new(RwLock::new(ApiKeyStore::load(&key_store_path)?));

    use crate::constants::{cache, monitoring};
    
    let response_cache: Arc<Cache<String, Arc<CachedResponse>>> = Arc::new(
        Cache::builder()
            .max_capacity(cache::MAX_CAPACITY)
            .time_to_live(std::time::Duration::from_secs(cache::TTL_SECONDS))
            .time_to_idle(std::time::Duration::from_secs(cache::IDLE_TIMEOUT_SECONDS))
            .build(),
    );

    // Add cache monitoring to track eviction effectiveness
    let cache_clone = response_cache.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(monitoring::METRICS_INTERVAL_SECONDS));
        loop {
            interval.tick().await;
            let _cache_size = cache_clone.weighted_size();
            let cache_entries = cache_clone.iter().count();
            log_performance_metric("cache_entries", cache_entries as f64, "count", "monitoring");
        }
    });

    let rate_limit_store: Arc<dyn RateLimitState> = Arc::new(InMemoryRateLimitState::new());

    // Start periodic cleanup of rate limit buckets to prevent memory leaks
    let cleanup_store = rate_limit_store.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(cache::TTL_SECONDS));
        loop {
            interval.tick().await;
            cleanup_store.cleanup_expired_buckets();
            
            // Log memory usage for monitoring
            let bucket_count = cleanup_store.get_active_buckets_count();
            log_performance_metric("rate_limit_buckets", bucket_count as f64, "count", "cleanup");
        }
    });

    let (prometheus_layer, prometheus_handle) = {
        let config_guard = config.read().await;
        if config_guard.observability.metrics.enabled{
            info!("Metrics reporting is enabled");
            let (layer,handle)= PrometheusMetricLayer::pair();
            (Some(layer), Some(handle))
        }else{
            (None, None)
        }
    };

    let circuit_breaker_store = Arc::new(
        CircuitBreakerStore::new(),
    );

    // Start periodic cleanup of circuit breakers to prevent memory leaks
    let cleanup_breaker_store = circuit_breaker_store.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(600)); // Every 10 minutes
        loop {
            interval.tick().await;
            cleanup_breaker_store.cleanup_expired_circuits();
            
            // Log memory usage for monitoring
            let breaker_count = cleanup_breaker_store.get_active_circuits_count();
            log_performance_metric("circuit_breakers", breaker_count as f64, "count", "cleanup");
        }
    });

    let app_state = Arc::new(AppState {
        config: config.clone(),
        secrets,
        key_store: key_store.clone(),
        rate_limit_store: rate_limit_store,
        cache: response_cache,
        http_client: Client::new(),
        prometheus_handle,
        circuit_breaker_store,
    });

    // start hot reloader
    let config_for_spawn = config.clone();
    let key_store_for_spawn = key_store.clone();
    tokio::spawn(async move {
        match hot_reload::watch_config_files(
            config_path,
            config_for_spawn,
            key_store_for_spawn, // Clone for the watcher task
        ).await {
            Ok(_) => info!("Hot reload watcher started successfully"),
            Err(e) => {
                error!("Hot reload watcher failed to start: {}. Configuration changes will not be automatically reloaded.", e);
                // We don't return the error here because we want the server to continue running
                // even if hot reload fails
            }
        }
    });

    let mut app = app::create_app(app_state)?;

    if let Some(layer) = prometheus_layer {
        app = app.layer(layer);
    }

    let config_guard = config.read().await;

    let addr  = config_guard.server.addr.clone();

    let listener = TcpListener::bind(&addr).await?;
    info!("Gateway listening on {}", &addr);
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>()).await?;

    Ok(())
}