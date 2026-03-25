pub mod aggregate;
pub mod app;
pub mod config;
pub mod errors;
pub mod features;
pub mod grpc_proxy;
pub mod middleware;
pub mod plugins;
pub mod proxy;
pub mod state;
pub mod utils;
pub mod ws_proxy;

use std::{net::SocketAddr, path::PathBuf, sync::Arc};

use anyhow::Result;
use axum_prometheus::PrometheusMetricLayer;
use dotenvy::dotenv;
use moka::future::Cache;
use reqwest::Client;
use tokio::{net::TcpListener, sync::RwLock};
use tracing::{Level, info};

use crate::state::{AppState, CachedResponse};
use crate::{
    config::{ApiKeyStore, GatewayConfig, SecretsConfig},
    features::{
        circuit_breaker::circuit_breaker::CircuitBreakerStore,
        rate_limiter::state::{InMemoryRateLimitState, RateLimitState},
    },
    utils::hot_reload,
};

pub async fn run(config_path: PathBuf) -> Result<()> {
    dotenv().ok();

    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    info!("Loading secrets...");
    let secrets = Arc::new(SecretsConfig::from_env()?);

    info!("Loading gateway configuration...");
    let config = Arc::new(RwLock::new(GatewayConfig::load(config_path.clone())?));
    info!("Configuration loaded successfully.");

    let key_store_path = config.read().await.identity.api_key_store_path.clone();

    info!(path = ?key_store_path, "Loading API key store...");

    let key_store = Arc::new(RwLock::new(ApiKeyStore::load(&key_store_path)?));

    let cache: Arc<Cache<String, Arc<CachedResponse>>> = Arc::new(
        Cache::builder()
            .max_capacity(10_000) // Default 5 minute TTL
            .build(),
    );

    let rate_limit_store: Arc<dyn RateLimitState> = Arc::new(InMemoryRateLimitState::new());

    let (prometheus_layer, prometheus_handle) = {
        let config_guard = config.read().await;
        if config_guard.observability.metrics.enabled {
            info!("Metrics reporting is enabled");
            let (layer, handle) = PrometheusMetricLayer::pair();
            (Some(layer), Some(handle))
        } else {
            (None, None)
        }
    };

    let circuit_breaker_store = Arc::new(CircuitBreakerStore::new());

    let plugin_registry = Arc::new(plugins::PluginRegistry::new());

    let health_checker = Arc::new(features::health_check::HealthChecker::new());

    let http_client = {
        let cfg = config.read().await;
        let pool = &cfg.server.pool;
        Client::builder()
            .connect_timeout(features::health_check::parse_duration(&pool.connect_timeout))
            .timeout(features::health_check::parse_duration(&pool.request_timeout))
            .pool_idle_timeout(features::health_check::parse_duration(&pool.idle_timeout))
            .pool_max_idle_per_host(pool.max_idle_per_host)
            .build()
            .expect("Failed to build HTTP client")
    };

    // Collect health check targets from routes
    {
        let cfg = config.read().await;
        let mut targets = Vec::new();
        for route in &cfg.routes {
            if let Some(hc) = &route.health_check {
                let interval = features::health_check::parse_duration(&hc.interval);
                for dest in route.all_destinations() {
                    targets.push((dest.to_string(), hc.path.clone(), interval));
                }
            }
        }
        if !targets.is_empty() {
            health_checker.start_checker(http_client.clone(), targets);
            info!("Health checks started");
        }
    }

    let app_state = Arc::new(AppState {
        config: config.clone(),
        secrets,
        key_store: key_store.clone(),
        rate_limit_store,
        cache,
        http_client: http_client.clone(),
        http_client_insecure: Client::builder()
            .danger_accept_invalid_certs(true)
            .connect_timeout(std::time::Duration::from_secs(5))
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to build insecure HTTP client"),
        prometheus_handle,
        circuit_breaker_store,
        load_balancer: features::load_balancer::LoadBalancer::new(),
        health_checker,
        plugin_registry,
    });

    // start hot reloader
    tokio::spawn(hot_reload::watch_config_files(
        config_path,
        config.clone(),
        key_store.clone(), // Clone for the watcher task
    ));

    let cors_config = {
        let cfg = config.read().await;
        cfg.cors.clone()
    };
    let mut app = app::create_app(app_state, &cors_config)?;

    if let Some(layer) = prometheus_layer {
        app = app.layer(layer);
    }

    let config_guard = config.read().await;

    let addr = config_guard.server.addr.clone();

    let listener = TcpListener::bind(&addr).await?;
    info!("Gateway listening on {}", &addr);
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;

    Ok(())
}
