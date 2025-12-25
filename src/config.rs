use std::{collections::HashMap, fs, path::Path, sync::Arc};

use anyhow::{Error, Ok};
use serde::{Deserialize};

use crate::features::circuit_breaker::circuit_breaker::CircuitBreakerStore;


#[derive(Debug, Deserialize)]
pub struct GatewayConfig {
    pub server: ServerConfig,
    pub routes: Vec<Arc<RouteConfig>>,
    #[serde(default)]
    pub observability: ObservabilityConfig,
    pub identity: IdentityConfig,
    #[serde(default)]
    pub security: SecurityConfig,
}

#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    pub addr: String
}

#[derive(Debug, Deserialize, Clone)]
pub struct IdentityConfig {
    pub api_key_store_path: String,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct SecurityConfig {
    #[serde(default = "default_allowed_domains")]
    pub allowed_domains: Vec<String>,
    #[serde(default = "default_max_request_size")]
    pub max_request_size: usize, // Maximum request size in bytes
}

fn default_allowed_domains() -> Vec<String> {
    vec!["localhost".to_string(), "127.0.0.1".to_string()]
}

fn default_max_request_size() -> usize {
    10 * 1024 * 1024 // 10MB default
}

fn default_destination() -> String {
    String::new()
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct LoadBalancingConfig {
    #[serde(default = "default_strategy")]
    pub strategy: String, // "round_robin", "random", "least_connections"
    #[serde(default)]
    pub enabled: bool,
}

fn default_strategy() -> String {
    "round_robin".to_string()
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub enum AuthType {
    Jwt,
    ApiKey,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AuthConfig {
    #[serde(rename="type")]
    pub auth_type: AuthType,
    pub roles: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RateLimitConfig{
    pub requests: u64,
    pub period: String
}

#[derive(Debug, Deserialize, Clone)]
pub struct RouteConfig {
    pub name: String,
    pub path: String,
    #[serde(default)]
    pub destinations: Vec<String>,
    #[serde(default = "default_destination")]
    pub destination: String,
    pub auth: Option<AuthConfig>,
    pub rate_limit: Option<RateLimitConfig>,
    pub cache: Option<CacheConfig>,
    pub circuit_breaker: Option<CircuitBreakerConfig>,
    #[serde(default)]
    pub load_balancing: LoadBalancingConfig,
    #[serde(default)]
    pub health_check: Option<HealthCheckConfig>,
    /// Request timeout for backend calls (e.g., "30s")
    #[serde(default = "default_request_timeout")]
    pub timeout: String,
}

fn default_request_timeout() -> String { "30s".to_string() }

/// Health check configuration for backend services
#[derive(Debug, Deserialize, Clone)]
pub struct HealthCheckConfig {
    /// Health check endpoint path (e.g., "/health")
    #[serde(default = "default_health_path")]
    pub path: String,
    /// Interval between health checks (e.g., "30s")
    #[serde(default = "default_health_interval")]
    pub interval: String,
    /// Timeout for health check request (e.g., "5s")
    #[serde(default = "default_health_timeout")]
    pub timeout: String,
    /// Number of failures before marking unhealthy
    #[serde(default = "default_unhealthy_threshold")]
    pub unhealthy_threshold: u32,
    /// Number of successes before marking healthy
    #[serde(default = "default_healthy_threshold")]
    pub healthy_threshold: u32,
}

fn default_health_path() -> String { "/health".to_string() }
fn default_health_interval() -> String { "30s".to_string() }
fn default_health_timeout() -> String { "5s".to_string() }
fn default_unhealthy_threshold() -> u32 { 3 }
fn default_healthy_threshold() -> u32 { 2 }

impl GatewayConfig {
    pub fn load<P: AsRef<Path>> (path: P) -> Result<Self,anyhow::Error> {
        let content = fs::read_to_string(path)?;
        let config: GatewayConfig = serde_yaml::from_str(&content)?;
        
        // Validate configuration
        config.validate()?;
        
        Ok(config)
    }
    
    /// Validate the gateway configuration
    pub fn validate(&self) -> Result<(), anyhow::Error> {
        // Validate server configuration
        if self.server.addr.is_empty() {
            return Err(anyhow::anyhow!("Server address cannot be empty"));
        }
        
        // Validate routes
        if self.routes.is_empty() {
            return Err(anyhow::anyhow!("At least one route must be configured"));
        }
        
        let mut route_names = std::collections::HashSet::new();
        let mut route_paths = std::collections::HashSet::new();
        
        for route in &self.routes {
            // Validate route name uniqueness
            if !route_names.insert(&route.name) {
                return Err(anyhow::anyhow!("Duplicate route name: {}", route.name));
            }
            
            // Validate route path uniqueness
            if !route_paths.insert(&route.path) {
                return Err(anyhow::anyhow!("Duplicate route path: {}", route.path));
            }
            
            // Validate route name not empty
            if route.name.trim().is_empty() {
                return Err(anyhow::anyhow!("Route name cannot be empty"));
            }
            
            // Validate route path not empty
            if route.path.trim().is_empty() {
                return Err(anyhow::anyhow!("Route path cannot be empty"));
            }
            
            // Validate destinations
            let has_destinations = !route.destinations.is_empty();
            let has_single_destination = !route.destination.is_empty();
            
            if !has_destinations && !has_single_destination {
                return Err(anyhow::anyhow!("Route '{}' must have either destinations or destination", route.name));
            }
            
            if has_destinations && has_single_destination {
                // Both specified, prefer destinations but warn about single destination
                tracing::warn!("Route '{}' has both destinations and destination, using destinations", route.name);
            }
            
            // Validate auth configuration if present
            if let Some(auth) = &route.auth {
                if auth.roles.is_some() && auth.roles.as_ref().unwrap().is_empty() {
                    return Err(anyhow::anyhow!("Route '{}' has empty roles array", route.name));
                }
            }
            
            // Validate rate limit configuration if present
            if let Some(rate_limit) = &route.rate_limit {
                if rate_limit.requests == 0 {
                    return Err(anyhow::anyhow!("Route '{}' has invalid rate limit requests: must be > 0", route.name));
                }
            }
        }
        
        // Validate security configuration
        if self.security.max_request_size == 0 {
            return Err(anyhow::anyhow!("Max request size must be greater than 0"));
        }
        
        // Validate API key store path
        if self.identity.api_key_store_path.trim().is_empty() {
            return Err(anyhow::anyhow!("API key store path cannot be empty"));
        }
        
        Ok(())
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
pub struct SecretsConfig  {
    pub jwt_secret: String
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
    pub ttl: String  // 30s , 1m
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