use std::{collections::HashMap, fs, path::Path, sync::Arc};

use anyhow::{Error, Ok};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct GatewayConfig {
    pub server: ServerConfig,
    pub routes: Vec<Arc<RouteConfig>>,
    #[serde(default)]
    pub observability: ObservabilityConfig,
    pub identity: IdentityConfig,
    #[serde(default)]
    pub cors: CorsConfig,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct CorsConfig {
    #[serde(default = "default_cors_origins")]
    pub origins: Vec<String>,
    #[serde(default = "default_cors_methods")]
    pub methods: Vec<String>,
    #[serde(default)]
    pub allow_headers: Vec<String>,
    #[serde(default)]
    pub enabled: bool,
}

fn default_cors_origins() -> Vec<String> { vec!["*".to_string()] }
fn default_cors_methods() -> Vec<String> { vec!["GET".into(), "POST".into(), "PUT".into(), "DELETE".into(), "PATCH".into(), "OPTIONS".into()] }

#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    pub addr: String,
    #[serde(default)]
    pub pool: PoolConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PoolConfig {
    #[serde(default = "default_pool_idle_timeout")]
    pub idle_timeout: String,
    #[serde(default = "default_pool_max_idle")]
    pub max_idle_per_host: usize,
    #[serde(default = "default_connect_timeout")]
    pub connect_timeout: String,
    #[serde(default = "default_request_timeout")]
    pub request_timeout: String,
}

fn default_pool_idle_timeout() -> String { "90s".to_string() }
fn default_pool_max_idle() -> usize { 32 }
fn default_connect_timeout() -> String { "5s".to_string() }
fn default_request_timeout() -> String { "30s".to_string() }

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            idle_timeout: default_pool_idle_timeout(),
            max_idle_per_host: default_pool_max_idle(),
            connect_timeout: default_connect_timeout(),
            request_timeout: default_request_timeout(),
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct IdentityConfig {
    pub api_key_store_path: String,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub enum AuthType {
    Jwt,
    ApiKey,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AuthConfig {
    #[serde(rename = "type")]
    pub auth_type: AuthType,
    pub roles: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RateLimitConfig {
    pub requests: u64,
    pub period: String,
}

use crate::features::load_balancer::LoadBalanceStrategy;
use crate::features::health_check::HealthCheckConfig;

#[derive(Debug, Deserialize, Clone)]
pub struct RouteConfig {
    pub name: String,
    pub path: String,
    pub destination: String,
    #[serde(default)]
    pub destinations: Vec<String>,
    #[serde(default)]
    pub load_balance: LoadBalanceStrategy,
    pub auth: Option<AuthConfig>,
    pub rate_limit: Option<RateLimitConfig>,
    pub cache: Option<CacheConfig>,
    pub circuit_breaker: Option<CircuitBreakerConfig>,
    pub health_check: Option<HealthCheckConfig>,
    pub retry: Option<RetryConfig>,
    pub timeout: Option<String>,
    pub transform: Option<TransformConfig>,
    #[serde(default)]
    pub tls_skip_verify: bool,
    pub aggregate: Option<Vec<AggregateSource>>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AggregateSource {
    pub service: String,
    pub path: String,
    pub field: String,
    pub timeout: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RetryConfig {
    #[serde(default = "default_retries")]
    pub count: u32,
    #[serde(default)]
    pub retry_on: Vec<u16>,
    #[serde(default = "default_backoff")]
    pub backoff: String,
}

fn default_retries() -> u32 { 2 }
fn default_backoff() -> String { "100ms".to_string() }

#[derive(Debug, Deserialize, Clone, Default)]
pub struct TransformConfig {
    #[serde(default)]
    pub request_headers: HashMap<String, String>,
    #[serde(default)]
    pub remove_request_headers: Vec<String>,
    #[serde(default)]
    pub response_headers: HashMap<String, String>,
    #[serde(default)]
    pub remove_response_headers: Vec<String>,
    pub rewrite_path: Option<String>,
}

impl RouteConfig {
    /// Returns all available destinations (single or multiple)
    pub fn all_destinations(&self) -> Vec<&str> {
        if self.destinations.is_empty() {
            vec![&self.destination]
        } else {
            self.destinations.iter().map(|s| s.as_str()).collect()
        }
    }
}

impl GatewayConfig {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, anyhow::Error> {
        let content = fs::read_to_string(path)?;
        let config: GatewayConfig = serde_yaml::from_str(&content)?;
        Ok(config)
    }

    pub fn find_route_for_path(&self, request_path: &str) -> Option<Arc<RouteConfig>> {
        self.routes
            .iter()
            .filter(|r| request_path.starts_with(&r.path))
            .max_by_key(|r| r.path.len())
            .cloned()
    }
}

//       API key store condig    //

#[derive(Debug, Deserialize, Clone)]
pub struct ApiKeyStore {
    pub keys: HashMap<String, ApiKeyDetails>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ApiKeyDetails {
    pub user_id: String,
    pub roles: Vec<String>,
    #[serde(default = "default_status")]
    pub status: String,
}

fn default_status() -> String {
    "active".to_string()
}

impl ApiKeyStore {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, anyhow::Error> {
        let content = fs::read_to_string(path)?;
        serde_yaml::from_str(&content).map_err(Into::into)
    }
}

//      Secrets Config
pub struct SecretsConfig {
    pub jwt_secret: String,
}

impl SecretsConfig {
    pub fn from_env() -> Result<Self, Error> {
        Ok(Self {
            jwt_secret: std::env::var("JWT_SECRET")
                .map_err(|_| anyhow::anyhow!("JWT_SECRET must be set in .env file"))?,
        })
    }
}

// ----- Cache config  ----
#[derive(Debug, Deserialize, Clone)]
pub struct CacheConfig {
    pub ttl: String, // 30s , 1m
}

//------  Observability config ---------

#[derive(Debug, Deserialize, Clone, Default)]
pub struct ObservabilityConfig {
    #[serde(default)] // Makes the metrics block optional
    pub metrics: MetricsConfig,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct MetricsConfig {
    #[serde(default)] // Defaults to false if not specified
    pub enabled: bool,
}

//      ---- Circuit Breaker

#[derive(Deserialize, Debug, Clone)]
pub struct CircuitBreakerConfig {
    pub failure_threshold: u32,
    pub success_threshold: u32,
    pub open_duration: String,
}
