pub mod config;
pub mod errors;
pub mod app;
pub mod state;
pub mod proxy;
pub mod middleware;
pub mod features;
pub mod utils;
pub mod plugins;


use std::{net::SocketAddr, path::PathBuf, sync::Arc,};

use anyhow::Result;
use axum_prometheus::{PrometheusMetricLayer};
use dotenvy::dotenv;
use moka::future::Cache;
use reqwest::Client;
use tokio::{net::TcpListener, sync::RwLock};
use tracing::{info, Level};

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
            .max_capacity(10_000) // Default 5 minute TTL
            .build(),
    );

    let rate_limit_store: Arc<dyn RateLimitState> = Arc::new(InMemoryRateLimitState::new());

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

    let plugin_registry = Arc::new(plugins::PluginRegistry::new());

    let app_state = Arc::new(AppState {
        config: config.clone(),
        secrets,
        key_store: key_store.clone(),
        rate_limit_store,
        cache,
        http_client: Client::new(),
        prometheus_handle,
        circuit_breaker_store,
        plugin_registry,
    });

    // start hot reloader
    tokio::spawn(hot_reload::watch_config_files(
        config_path,
        config.clone(),
        key_store.clone(), // Clone for the watcher task
    ));

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