use std::{sync::Arc, time::Instant};

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
}

pub struct CircuitBreakerStore  {
    curcuits: DashMap<String, Arc<CircuitState>>,
}

impl Default for CircuitBreakerStore {
    fn default() -> Self {
        Self::new()
    }
}

impl CircuitBreakerStore {
    pub fn new() -> Self {
        Self {
            curcuits: DashMap::new(),
        }
    }

    pub fn get_or_insert(&self, route_name: &str) -> Arc<CircuitState> {
        self.curcuits
            .entry(route_name.to_string())
            .or_insert_with(|| {
                Arc::new(
                    CircuitState { 
                        state: RwLock::new(
                            State::Closed { consecutive_failures: 0 }
                        ) 
                    }
                )
            })
            .clone()
    }

}