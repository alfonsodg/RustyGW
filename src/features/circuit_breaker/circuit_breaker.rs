use std::{sync::Arc, time::Instant};
use std::time::Duration;

use dashmap::DashMap;
use tokio::sync::RwLock;


#[derive(Debug, Clone)]
pub enum State { 
    Closed {consecutive_failures: u32},
    Open{opened_at: Instant},
    HalfOpen{consecutive_successes:u32},
}

pub struct CircuitState {
    pub state: RwLock<State>,
    pub last_access: Instant, // Track when circuit breaker was last accessed
}

pub struct CircuitBreakerStore  {
    curcuits: DashMap<String, Arc<CircuitState>>,
    ttl_seconds: u64, // Time to live for inactive circuit breakers in seconds
}

impl CircuitBreakerStore {
    pub fn new() -> Self {
        Self::with_ttl(3600) // 1 hour default TTL
    }
    
    pub fn with_ttl(ttl_seconds: u64) -> Self {
        Self {
            curcuits: DashMap::new(),
            ttl_seconds,
        }
    }

    pub fn get_or_insert(&self, route_name: &str) -> Arc<CircuitState> {
        let now = Instant::now();
        
        self.curcuits
            .entry(route_name.to_string())
            .or_insert_with(|| {
                Arc::new(
                    CircuitState { 
                        state: RwLock::new(
                            State::Closed { consecutive_failures: 0 }
                        ),
                        last_access: now,
                    }
                )
            })
            .clone()
    }
    
    /// Clean up circuit breakers that haven't been accessed for longer than TTL
    pub fn cleanup_expired_circuits(&self) {
        let now = Instant::now();
        let ttl_duration = Duration::from_secs(self.ttl_seconds);
        
        // Collect keys to remove
        let keys_to_remove: Vec<String> = self.curcuits
            .iter()
            .filter_map(|entry| {
                let circuit_state = entry.value();
                // Check if circuit breaker hasn't been accessed recently
                if now.duration_since(circuit_state.last_access) > ttl_duration {
                    Some(entry.key().clone())
                } else {
                    None
                }
            })
            .collect();
        
        // Remove expired circuit breakers
        let removed_count = keys_to_remove.len();
        for key in &keys_to_remove {
            self.curcuits.remove(key);
        }
        
        if removed_count > 0 {
            tracing::info!("Cleaned up {} expired circuit breakers", removed_count);
        }
    }
    
    /// Get current number of active circuit breakers for monitoring
    pub fn get_active_circuits_count(&self) -> usize {
        self.curcuits.len()
    }
}