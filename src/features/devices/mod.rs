//! Device protocol module for med365x RPM.
//! Handles USB serial, RS232, and RF device communication.
//! Streams vital signs via WebSocket and publishes to NATS.

pub mod serial;
pub mod protocols;
pub mod registry;

use std::sync::Arc;
use tokio::sync::RwLock;

/// Device manager state shared across the application.
pub struct DeviceManager {
    pub registry: Arc<RwLock<registry::DeviceRegistry>>,
    pub enabled: bool,
}

impl DeviceManager {
    pub fn new(enabled: bool) -> Self {
        Self {
            registry: Arc::new(RwLock::new(registry::DeviceRegistry::new())),
            enabled,
        }
    }
}
