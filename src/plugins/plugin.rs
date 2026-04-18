//! Core plugin trait and types.

use async_trait::async_trait;
use axum::{body::Body, extract::Request, response::Response};
use std::sync::Arc;

pub type PluginResult<T> = Result<T, PluginError>;

#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error("Plugin execution failed: {0}")]
    Execution(String),
    #[error("Plugin configuration error: {0}")]
    Config(String),
    #[error("Request rejected: {0}")]
    Rejected(String),
}

#[derive(Clone)]
pub struct PluginContext {
    pub route_path: String,
    pub client_ip: Option<String>,
    pub metadata: std::collections::HashMap<String, String>,
}

impl PluginContext {
    pub fn new(route_path: String) -> Self {
        Self {
            route_path,
            client_ip: None,
            metadata: std::collections::HashMap::new(),
        }
    }

    pub fn with_client_ip(mut self, ip: Option<String>) -> Self {
        self.client_ip = ip;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginPhase {
    PreAuth,
    PostAuth,
    PreProxy,
    PostProxy,
}

#[async_trait]
pub trait Plugin: Send + Sync {
    fn name(&self) -> &str;
    fn phase(&self) -> PluginPhase;
    fn priority(&self) -> i32 {
        100
    }

    async fn on_request(
        &self,
        request: Request<Body>,
        _ctx: &PluginContext,
    ) -> PluginResult<(Request<Body>, Option<Response<Body>>)> {
        Ok((request, None))
    }

    async fn on_response(&self, response: Response<Body>, _ctx: &PluginContext) -> PluginResult<Response<Body>> {
        Ok(response)
    }

    fn is_enabled_for_route(&self, _route_path: &str) -> bool {
        true
    }
}

pub type BoxedPlugin = Arc<dyn Plugin>;
