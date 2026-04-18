use std::{collections::HashMap, sync::Arc, time::Duration};

use dashmap::DashMap;
use serde::Deserialize;
use tokio::time::Instant;
use tracing::{info, warn};

#[derive(Debug, Deserialize, Clone)]
pub struct HealthCheckConfig {
    #[serde(default = "default_interval")]
    pub interval: String,
    #[serde(default = "default_path")]
    pub path: String,
}

fn default_interval() -> String {
    "10s".to_string()
}

fn default_path() -> String {
    "/health".to_string()
}

#[derive(Debug, Clone)]
pub struct BackendHealth {
    pub healthy: bool,
    pub last_check: Instant,
}

pub struct HealthChecker {
    status: Arc<DashMap<String, BackendHealth>>,
}

impl HealthChecker {
    pub fn new() -> Self {
        Self {
            status: Arc::new(DashMap::new()),
        }
    }

    pub fn is_healthy(&self, url: &str) -> bool {
        self.status.get(url).map(|h| h.healthy).unwrap_or(true) // assume healthy if not checked yet
    }

    pub fn filter_healthy<'a>(&self, destinations: &[&'a str]) -> Vec<&'a str> {
        let healthy: Vec<&str> = destinations
            .iter()
            .filter(|url| self.is_healthy(url))
            .copied()
            .collect();
        if healthy.is_empty() {
            // fallback: return all if none healthy (avoid total outage)
            destinations.to_vec()
        } else {
            healthy
        }
    }

    pub fn start_checker(
        &self,
        client: reqwest::Client,
        routes: Vec<(String, String, Duration)>, // (url, health_path, interval)
    ) {
        let status = self.status.clone();

        tokio::spawn(async move {
            // Group by interval for efficient checking
            let mut by_interval: HashMap<u64, Vec<(String, String)>> = HashMap::new();
            for (url, path, interval) in &routes {
                by_interval
                    .entry(interval.as_secs())
                    .or_default()
                    .push((url.clone(), path.clone()));
            }

            for (secs, targets) in by_interval {
                let client = client.clone();
                let status = status.clone();
                let interval = Duration::from_secs(secs);

                tokio::spawn(async move {
                    loop {
                        for (url, path) in &targets {
                            let check_url = format!("{}{}", url, path);
                            let healthy = client
                                .get(&check_url)
                                .timeout(Duration::from_secs(3))
                                .send()
                                .await
                                .map(|r| r.status().is_success())
                                .unwrap_or(false);

                            let prev = status.get(url).map(|h| h.healthy);
                            status.insert(
                                url.clone(),
                                BackendHealth {
                                    healthy,
                                    last_check: Instant::now(),
                                },
                            );

                            match (prev, healthy) {
                                (Some(true), false) => warn!(backend = %url, "Backend is DOWN"),
                                (Some(false), true) => info!(backend = %url, "Backend recovered"),
                                _ => {}
                            }
                        }
                        tokio::time::sleep(interval).await;
                    }
                });
            }
        });
    }
}

impl Default for HealthChecker {
    fn default() -> Self {
        Self::new()
    }
}

pub fn parse_duration(s: &str) -> Duration {
    let s = s.trim();
    if let Some(ms) = s.strip_suffix("ms") {
        Duration::from_millis(ms.parse().unwrap_or(100))
    } else if let Some(secs) = s.strip_suffix('s') {
        Duration::from_secs(secs.parse().unwrap_or(10))
    } else if let Some(mins) = s.strip_suffix('m') {
        Duration::from_secs(mins.parse::<u64>().unwrap_or(1) * 60)
    } else {
        Duration::from_secs(10)
    }
}

pub fn parse_body_limit(s: &str) -> usize {
    let s = s.trim().to_lowercase();
    if let Some(mb) = s.strip_suffix("mb") {
        mb.parse::<usize>().unwrap_or(10) * 1024 * 1024
    } else if let Some(kb) = s.strip_suffix("kb") {
        kb.parse::<usize>().unwrap_or(1024) * 1024
    } else {
        s.parse::<usize>().unwrap_or(10 * 1024 * 1024)
    }
}
