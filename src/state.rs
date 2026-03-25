use axum_prometheus::metrics_exporter_prometheus::PrometheusHandle;
use bytes::Bytes;
use http::{HeaderMap, StatusCode};
use moka::future::Cache;
use reqwest::Client;
use std::{sync::Arc, time::Instant};

use crate::{
    config::{ApiKeyStore, GatewayConfig, SecretsConfig},
    features::{
        circuit_breaker::circuit_breaker::CircuitBreakerStore,
        health_check::HealthChecker,
        load_balancer::LoadBalancer,
        rate_limiter::state::RateLimitState,
    },
    plugins::PluginRegistry,
};

use tokio::sync::RwLock;

pub struct CachedResponse {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Bytes,
    pub inserted_at: Instant,
}

pub struct AppState {
    pub config: Arc<RwLock<GatewayConfig>>,
    pub secrets: Arc<SecretsConfig>,
    pub key_store: Arc<RwLock<ApiKeyStore>>,
    pub rate_limit_store: Arc<dyn RateLimitState>,
    pub cache: Arc<Cache<String, Arc<CachedResponse>>>,
    pub http_client: Client,
    pub http_client_insecure: Client,
    pub prometheus_handle: Option<PrometheusHandle>,
    pub circuit_breaker_store: Arc<CircuitBreakerStore>,
    pub load_balancer: LoadBalancer,
    pub health_checker: Arc<HealthChecker>,
    pub plugin_registry: Arc<PluginRegistry>,
}
