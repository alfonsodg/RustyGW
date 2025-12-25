//! Example plugins.

use async_trait::async_trait;
use axum::{body::Body, extract::Request, response::Response};
use http::header::HeaderValue;
use tracing::debug;
use super::plugin::{Plugin, PluginContext, PluginPhase, PluginResult};

pub struct HeaderInjectorPlugin {
    headers: Vec<(String, String)>,
}

impl HeaderInjectorPlugin {
    pub fn new(headers: Vec<(String, String)>) -> Self {
        Self { headers }
    }
}

#[async_trait]
impl Plugin for HeaderInjectorPlugin {
    fn name(&self) -> &str { "header-injector" }
    fn phase(&self) -> PluginPhase { PluginPhase::PostProxy }

    async fn on_response(&self, mut response: Response<Body>, _ctx: &PluginContext) -> PluginResult<Response<Body>> {
        for (key, value) in &self.headers {
            if let Ok(hv) = HeaderValue::from_str(value)
                && let Ok(hn) = http::header::HeaderName::try_from(key.as_str()) {
                    response.headers_mut().insert(hn, hv);
                }
        }
        debug!("HeaderInjectorPlugin: added {} headers", self.headers.len());
        Ok(response)
    }
}

pub struct RequestLoggerPlugin;

#[async_trait]
impl Plugin for RequestLoggerPlugin {
    fn name(&self) -> &str { "request-logger" }
    fn phase(&self) -> PluginPhase { PluginPhase::PreAuth }
    fn priority(&self) -> i32 { 1 }

    async fn on_request(&self, request: Request<Body>, ctx: &PluginContext) -> PluginResult<(Request<Body>, Option<Response<Body>>)> {
        debug!("RequestLoggerPlugin: {} {} from {:?}", request.method(), ctx.route_path, ctx.client_ip);
        Ok((request, None))
    }
}
