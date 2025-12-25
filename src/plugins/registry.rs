//! Plugin registry for managing plugins.

use tokio::sync::RwLock;
use tracing::info;
use super::plugin::{BoxedPlugin, PluginPhase};

pub struct PluginRegistry {
    plugins: RwLock<Vec<BoxedPlugin>>,
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self { plugins: RwLock::new(Vec::new()) }
    }

    pub async fn register(&self, plugin: BoxedPlugin) {
        let mut plugins = self.plugins.write().await;
        info!("Registering plugin: {}", plugin.name());
        plugins.push(plugin);
        plugins.sort_by_key(|p| p.priority());
    }

    pub async fn get_plugins_for_phase(&self, phase: PluginPhase) -> Vec<BoxedPlugin> {
        self.plugins.read().await.iter()
            .filter(|p| p.phase() == phase)
            .cloned()
            .collect()
    }

    pub async fn get_plugins_for_route(&self, route_path: &str, phase: PluginPhase) -> Vec<BoxedPlugin> {
        self.plugins.read().await.iter()
            .filter(|p| p.phase() == phase && p.is_enabled_for_route(route_path))
            .cloned()
            .collect()
    }
}
