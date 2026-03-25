use std::sync::atomic::{AtomicUsize, Ordering};

use serde::Deserialize;

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(rename_all = "snake_case")]
pub enum LoadBalanceStrategy {
    #[default]
    RoundRobin,
    Random,
}

pub struct LoadBalancer {
    counter: AtomicUsize,
}

impl LoadBalancer {
    pub fn new() -> Self {
        Self {
            counter: AtomicUsize::new(0),
        }
    }

    pub fn next_index(&self, count: usize, strategy: &LoadBalanceStrategy) -> usize {
        match strategy {
            LoadBalanceStrategy::RoundRobin => {
                self.counter.fetch_add(1, Ordering::Relaxed) % count
            }
            LoadBalanceStrategy::Random => {
                use std::collections::hash_map::RandomState;
                use std::hash::{BuildHasher, Hasher};
                let s = RandomState::new();
                let mut hasher = s.build_hasher();
                hasher.write_usize(self.counter.fetch_add(1, Ordering::Relaxed));
                hasher.finish() as usize % count
            }
        }
    }
}

impl Default for LoadBalancer {
    fn default() -> Self {
        Self::new()
    }
}
