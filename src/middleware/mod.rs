//! Middleware module providing request processing layers.
//!
//! Available middleware:
//! - `auth` - JWT and API key authentication
//! - `rate_limiter` - Token bucket rate limiting
//! - `cache` - Response caching
//! - `circuit_breaker` - Fault tolerance pattern
//! - `request_id` - Request tracing

pub mod auth;
pub mod rate_limiter;
pub mod cache;
pub mod request_id;
pub mod circuit_breaker;
pub mod transform;

use std::sync::Arc;
use crate::{config::RouteConfig, state::AppState};

/// Retrieves route configuration for a given request path.
///
/// # Arguments
/// * `state` - Application state containing configuration
/// * `path` - Request URI path to match
///
/// # Returns
/// * `Option<Arc<RouteConfig>>` - Route config if path matches a configured route
pub async fn get_route_config(
    state: &Arc<AppState>,
    path: &str,
) -> Option<Arc<RouteConfig>> {
    let config_guard = state.config.read().await;
    config_guard.find_route_for_path(path)
}