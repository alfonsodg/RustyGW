use std::{collections::HashMap, fs, path::Path, sync::Arc};

use anyhow::{Error, Ok};
use regex::Regex;
use serde::Deserialize;
use tracing::info;

use crate::features::health_check::HealthCheckConfig;
use crate::features::load_balancer::LoadBalanceStrategy;

// ==================== Top-level Config ====================

#[derive(Debug, Deserialize)]
pub struct GatewayConfig {
    pub server: ServerConfig,
    #[serde(default)]
    pub services: HashMap<String, ServiceConfig>,
    #[serde(default)]
    pub defaults: RouteDefaults,
    pub routes: Vec<Arc<RouteConfig>>,
    #[serde(default)]
    pub observability: ObservabilityConfig,
    pub identity: IdentityConfig,
    #[serde(default)]
    pub cors: CorsConfig,
    #[serde(default)]
    pub include: Vec<String>,
}

// ==================== Service Abstraction (#61) ====================

#[derive(Debug, Deserialize, Clone)]
pub struct ServiceConfig {
    #[serde(default)]
    pub urls: Vec<String>,
    pub url: Option<String>,
    #[serde(default)]
    pub load_balance: LoadBalanceStrategy,
    pub health_check: Option<HealthCheckConfig>,
    pub retry: Option<RetryConfig>,
    pub timeout: Option<String>,
    #[serde(default)]
    pub tls_skip_verify: bool,
}

// ==================== Global Defaults (#62) ====================

#[derive(Debug, Deserialize, Clone, Default)]
pub struct RouteDefaults {
    pub timeout: Option<String>,
    pub retry: Option<RetryConfig>,
    #[serde(default)]
    pub load_balance: LoadBalanceStrategy,
}

// ==================== Server Config ====================

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
    #[serde(default = "default_body_limit")]
    pub body_limit: String,
}

fn default_pool_idle_timeout() -> String { "90s".to_string() }
fn default_pool_max_idle() -> usize { 32 }
fn default_connect_timeout() -> String { "5s".to_string() }
fn default_request_timeout() -> String { "30s".to_string() }
fn default_body_limit() -> String { "10mb".to_string() }

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            idle_timeout: default_pool_idle_timeout(),
            max_idle_per_host: default_pool_max_idle(),
            connect_timeout: default_connect_timeout(),
            request_timeout: default_request_timeout(),
            body_limit: default_body_limit(),
        }
    }
}

// ==================== CORS ====================

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

// ==================== Identity ====================

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

// ==================== Route Config ====================

#[derive(Debug, Deserialize, Clone)]
pub struct RouteConfig {
    pub name: String,
    pub path: String,
    #[serde(default)]
    pub destination: String,
    #[serde(default)]
    pub destinations: Vec<String>,
    pub service: Option<String>,
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
pub struct RateLimitConfig {
    pub requests: u64,
    pub period: String,
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

#[derive(Debug, Deserialize, Clone)]
pub struct AggregateSource {
    pub service: String,
    pub path: String,
    pub field: String,
    pub timeout: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CacheConfig {
    pub ttl: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct CircuitBreakerConfig {
    pub failure_threshold: u32,
    pub success_threshold: u32,
    pub open_duration: String,
}

// ==================== Observability ====================

#[derive(Debug, Deserialize, Clone, Default)]
pub struct ObservabilityConfig {
    #[serde(default)]
    pub metrics: MetricsConfig,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct MetricsConfig {
    #[serde(default)]
    pub enabled: bool,
}

// ==================== Route helpers ====================

impl RouteConfig {
    pub fn all_destinations(&self) -> Vec<&str> {
        if self.destinations.is_empty() {
            if self.destination.is_empty() {
                vec![]
            } else {
                vec![&self.destination]
            }
        } else {
            self.destinations.iter().map(|s| s.as_str()).collect()
        }
    }
}

// ==================== Config Loading ====================

impl GatewayConfig {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, anyhow::Error> {
        let path = path.as_ref();
        let content = fs::read_to_string(path)?;

        // #63: Environment variable interpolation
        let content = interpolate_env_vars(&content);

        let mut config: GatewayConfig = serde_yaml::from_str(&content)?;

        // #65: Process includes
        let base_dir = path.parent().unwrap_or(Path::new("."));
        for include_pattern in config.include.clone() {
            let include_path = base_dir.join(&include_pattern);
            if include_path.is_file() {
                let inc_content = interpolate_env_vars(&fs::read_to_string(&include_path)?);
                let inc: serde_yaml::Value = serde_yaml::from_str(&inc_content)?;
                merge_include(&mut config, &inc, &include_pattern)?;
                info!(file = %include_pattern, "Loaded include file");
            } else {
                // Glob pattern
                for entry in glob::glob(include_path.to_str().unwrap_or(""))? {
                    let entry = entry?;
                    let inc_content = interpolate_env_vars(&fs::read_to_string(&entry)?);
                    let inc: serde_yaml::Value = serde_yaml::from_str(&inc_content)?;
                    merge_include(&mut config, &inc, entry.to_str().unwrap_or(""))?;
                    info!(file = ?entry, "Loaded include file");
                }
            }
        }

        // #61: Resolve service references
        config.resolve_services();

        // #62: Apply defaults
        config.apply_defaults();

        // #64: Validate
        config.validate()?;

        Ok(config)
    }

    /// #61: Resolve service references in routes
    fn resolve_services(&mut self) {
        let services = self.services.clone();
        for route in &mut self.routes {
            if let Some(svc_name) = &route.service
                && let Some(svc) = services.get(svc_name)
            {
                    let route_mut = Arc::make_mut(route);
                    // Set destinations from service
                    if route_mut.destinations.is_empty() && route_mut.destination.is_empty() {
                        if !svc.urls.is_empty() {
                            route_mut.destinations = svc.urls.clone();
                        } else if let Some(url) = &svc.url {
                            route_mut.destination = url.clone();
                        }
                    }
                    // Inherit service config if not set on route
                    if route_mut.health_check.is_none() {
                        route_mut.health_check = svc.health_check.clone();
                    }
                    if route_mut.retry.is_none() {
                        route_mut.retry = svc.retry.clone();
                    }
                    if route_mut.timeout.is_none() {
                        route_mut.timeout = svc.timeout.clone();
                    }
                    if !route_mut.tls_skip_verify {
                        route_mut.tls_skip_verify = svc.tls_skip_verify;
                    }
                    if matches!(route_mut.load_balance, LoadBalanceStrategy::RoundRobin) {
                        route_mut.load_balance = svc.load_balance.clone();
                    }
                }
            }
        }

    /// #62: Apply global defaults to routes missing config
    fn apply_defaults(&mut self) {
        let defaults = self.defaults.clone();
        for route in &mut self.routes {
            let route_mut = Arc::make_mut(route);
            if route_mut.timeout.is_none() {
                route_mut.timeout = defaults.timeout.clone();
            }
            if route_mut.retry.is_none() {
                route_mut.retry = defaults.retry.clone();
            }
        }
    }

    /// #64: Validate config and return clear errors
    fn validate(&self) -> Result<(), anyhow::Error> {
        let mut errors = Vec::new();

        for route in &self.routes {
            // Check service reference exists
            if let Some(svc_name) = &route.service
                && !self.services.contains_key(svc_name)
            {
                errors.push(format!(
                    "Route '{}' references service '{}' which is not defined in services",
                    route.path, svc_name
                ));
            }

            // Check route has at least one destination (unless aggregate)
            if route.aggregate.is_none()
                && route.destination.is_empty()
                && route.destinations.is_empty()
                && route.service.is_none()
            {
                errors.push(format!(
                    "Route '{}' has no destination, destinations, or service defined",
                    route.path
                ));
            }

            // Check aggregate sources have required fields
            if let Some(agg) = &route.aggregate {
                for source in agg {
                    if source.field.is_empty() {
                        errors.push(format!(
                            "Route '{}' aggregate source '{}' has empty field",
                            route.path, source.service
                        ));
                    }
                    if source.path.is_empty() {
                        errors.push(format!(
                            "Route '{}' aggregate source '{}' has empty path",
                            route.path, source.service
                        ));
                    }
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(anyhow::anyhow!("Config validation errors:\n  - {}", errors.join("\n  - ")))
        }
    }

    pub fn find_route_for_path(&self, request_path: &str) -> Option<Arc<RouteConfig>> {
        self.routes
            .iter()
            .filter(|r| request_path.starts_with(&r.path))
            .max_by_key(|r| r.path.len())
            .cloned()
    }

    /// Public wrappers for testing
    pub fn resolve_services_pub(&mut self) { self.resolve_services(); }
    pub fn apply_defaults_pub(&mut self) { self.apply_defaults(); }
    pub fn validate_pub(&self) -> Result<(), anyhow::Error> { self.validate() }
}

/// Public wrapper for env var interpolation (for testing)
pub fn interpolate_env_vars_pub(content: &str) -> String {
    interpolate_env_vars(content)
}

// ==================== Env Var Interpolation (#63) ====================

fn interpolate_env_vars(content: &str) -> String {
    let re = Regex::new(r"\$\{([^}]+)\}").unwrap();
    re.replace_all(content, |caps: &regex::Captures| {
        let var_name = &caps[1];
        std::env::var(var_name).unwrap_or_else(|_| format!("${{{}}}", var_name))
    }).to_string()
}

// ==================== Include Merging (#65) ====================

fn merge_include(config: &mut GatewayConfig, inc: &serde_yaml::Value, _file: &str) -> Result<(), anyhow::Error> {
    // Merge services
    if let Some(services) = inc.get("services") {
        let svcs: HashMap<String, ServiceConfig> = serde_yaml::from_value(services.clone())?;
        config.services.extend(svcs);
    }
    // Merge routes
    if let Some(routes) = inc.get("routes") {
        let new_routes: Vec<Arc<RouteConfig>> = serde_yaml::from_value(routes.clone())?;
        config.routes.extend(new_routes);
    }
    Ok(())
}

// ==================== API Key Store ====================

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

fn default_status() -> String { "active".to_string() }

impl ApiKeyStore {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, anyhow::Error> {
        let content = fs::read_to_string(path)?;
        serde_yaml::from_str(&content).map_err(Into::into)
    }
}

// ==================== Secrets ====================

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
