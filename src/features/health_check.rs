//! Proactive health checking for backend services.

use std::{sync::Arc, time::Duration};
use dashmap::DashMap;
use reqwest::Client;
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::{config::GatewayConfig, utils::parse_duration};

/// Health status of a backend destination
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HealthStatus {
    Healthy,
    Unhealthy,
    Unknown,
}

/// Tracks health state for a destination
struct DestinationHealth {
    status: HealthStatus,
    consecutive_failures: u32,
    consecutive_successes: u32,
}

/// Store for tracking backend health status
pub struct HealthCheckStore {
    destinations: DashMap<String, DestinationHealth>,
}

impl HealthCheckStore {
    pub fn new() -> Self {
        Self {
            destinations: DashMap::new(),
        }
    }

    /// Check if a destination is healthy
    pub fn is_healthy(&self, destination: &str) -> bool {
        self.destinations
            .get(destination)
            .map(|h| h.status == HealthStatus::Healthy || h.status == HealthStatus::Unknown)
            .unwrap_or(true)
    }

    /// Record a successful health check
    fn record_success(&self, destination: &str, healthy_threshold: u32) {
        self.destinations
            .entry(destination.to_string())
            .and_modify(|h| {
                h.consecutive_successes += 1;
                h.consecutive_failures = 0;
                if h.consecutive_successes >= healthy_threshold {
                    h.status = HealthStatus::Healthy;
                }
            })
            .or_insert(DestinationHealth {
                status: HealthStatus::Healthy,
                consecutive_failures: 0,
                consecutive_successes: 1,
            });
    }

    /// Record a failed health check
    fn record_failure(&self, destination: &str, unhealthy_threshold: u32) {
        self.destinations
            .entry(destination.to_string())
            .and_modify(|h| {
                h.consecutive_failures += 1;
                h.consecutive_successes = 0;
                if h.consecutive_failures >= unhealthy_threshold {
                    h.status = HealthStatus::Unhealthy;
                }
            })
            .or_insert(DestinationHealth {
                status: HealthStatus::Unknown,
                consecutive_failures: 1,
                consecutive_successes: 0,
            });
    }
}

/// Starts background health check tasks for all configured routes
pub async fn start_health_checks(
    config: Arc<RwLock<GatewayConfig>>,
    health_store: Arc<HealthCheckStore>,
    http_client: Client,
) {
    let config_guard = config.read().await;
    
    for route in &config_guard.routes {
        if let Some(health_config) = &route.health_check {
            let destinations = if route.destinations.is_empty() && !route.destination.is_empty() {
                vec![route.destination.clone()]
            } else {
                route.destinations.clone()
            };

            let interval = parse_duration(&health_config.interval).unwrap_or(Duration::from_secs(30));
            let timeout = parse_duration(&health_config.timeout).unwrap_or(Duration::from_secs(5));
            let path = health_config.path.clone();
            let unhealthy_threshold = health_config.unhealthy_threshold;
            let healthy_threshold = health_config.healthy_threshold;

            for dest in destinations {
                let store = health_store.clone();
                let client = http_client.clone();
                let health_path = path.clone();
                
                tokio::spawn(async move {
                    let mut ticker = tokio::time::interval(interval);
                    loop {
                        ticker.tick().await;
                        let url = format!("{}{}", dest.trim_end_matches('/'), health_path);
                        
                        let result = client
                            .get(&url)
                            .timeout(timeout)
                            .send()
                            .await;

                        match result {
                            Ok(resp) if resp.status().is_success() => {
                                store.record_success(&dest, healthy_threshold);
                            }
                            _ => {
                                warn!(destination = %dest, "Health check failed");
                                store.record_failure(&dest, unhealthy_threshold);
                            }
                        }
                    }
                });
            }
        }
    }
    
    info!("Health check tasks started");
}
