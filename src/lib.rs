pub mod config;
pub mod errors;
pub mod app;
pub mod state;
pub mod proxy;
pub mod middleware;
pub mod features;
pub mod utils;


use std::{net::SocketAddr, path::PathBuf, sync::Arc,};

use anyhow::Result;
use axum_prometheus::{PrometheusMetricLayer};
use dotenvy::dotenv;
use moka::future::Cache;
use reqwest::Client;
use tokio::{net::TcpListener, sync::RwLock};
use tracing::{info, error, Level};

use crate::{config::{ApiKeyStore, GatewayConfig, SecretsConfig}, features::{circuit_breaker::circuit_breaker::CircuitBreakerStore, rate_limiter::state::{InMemoryRateLimitState, RateLimitState}}, utils::hot_reload};
use crate::state::{AppState, CachedResponse};

pub async fn run(
    config_path: PathBuf,
) -> Result<()> {


    dotenv().ok();

    tracing_subscriber::fmt()
    .with_max_level(Level::INFO)
    .init();

    info!("Loading secrets...");
    let secrets = Arc::new(SecretsConfig::from_env()?);

    info!("Loading gateway configuration...");
    let config = Arc::new(RwLock::new(GatewayConfig::load(
        config_path.clone(),
    )?));
    info!("Configuration loaded successfully.");

    let key_store_path   = config.read().await.identity.api_key_store_path.clone(); 
    
    info!(path = ?key_store_path, "Loading API key store...");

    let key_store = Arc::new(RwLock::new(ApiKeyStore::load(&key_store_path)?));

    let cache: Arc<Cache<String, Arc<CachedResponse>>> = Arc::new(
        Cache::builder()
            .max_capacity(10_000) // Maximum entries
            .time_to_live(std::time::Duration::from_secs(300)) // 5 minute TTL
            .time_to_idle(std::time::Duration::from_secs(180)) // 3 minute idle timeout
            .build(),
    );

    // Add cache monitoring to track eviction effectiveness
    let cache_clone = cache.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60)); // Every minute
        loop {
            interval.tick().await;
            let cache_size = cache_clone.weighted_size();
            // Use iter_count() to get number of entries
            let cache_entries = cache_clone.iter().count();
            tracing::debug!("Cache status: {} entries, {} bytes", cache_entries, cache_size);
        }
    });

    let rate_limit_store: Arc<dyn RateLimitState> = Arc::new(InMemoryRateLimitState::new());

    // Start periodic cleanup of rate limit buckets to prevent memory leaks
    let cleanup_store = rate_limit_store.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(300)); // Every 5 minutes
        loop {
            interval.tick().await;
            cleanup_store.cleanup_expired_buckets();
            
            // Log memory usage for monitoring
            let bucket_count = cleanup_store.get_active_buckets_count();
            tracing::debug!("Active rate limit buckets: {}", bucket_count);
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
            tracing::debug!("Active circuit breakers: {}", breaker_count);
        }
    });

    let app_state = Arc::new(AppState {
        config: config.clone(),
        secrets,
        key_store: key_store.clone(),
        rate_limit_store: rate_limit_store,
        cache: cache,
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