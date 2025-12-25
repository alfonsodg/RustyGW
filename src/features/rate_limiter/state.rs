use std::sync::Arc;

use async_trait::async_trait;
use dashmap::DashMap;
use tokio::{sync::RwLock, time::Instant};

#[async_trait]
pub trait RateLimitState: Send + Sync {
    async fn check_and_update(&self, key: &str, capacity: u64, refill_rate:f64) -> bool;
}

struct Bucket {
    tokens: f64,
    last_refill: Instant
}

pub struct InMemoryRateLimitState {
    clients: DashMap<String, Arc<RwLock<Bucket>>>,
}

impl Default for InMemoryRateLimitState {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryRateLimitState {
    pub fn new() -> Self {
        Self {
            clients: DashMap::new(),
        }
    }
}

#[async_trait]
impl RateLimitState for InMemoryRateLimitState {
   
    async fn check_and_update(& self,key: & str,capacity:u64,refill_rate:f64) ->  bool {
        
        let entry = self.clients.entry(key.to_string()).or_insert_with(||{
            Arc::new(RwLock::new(Bucket { 
                tokens: capacity as f64, 
                last_refill: Instant::now(), 
            }))
        });

        let last_refill_time = {
            let bucket = entry.read().await;
            bucket.last_refill
        };
        
        let elapsed = last_refill_time.elapsed().as_secs_f64();
        let tokens_to_add = elapsed * refill_rate;
        
        let mut bucket = entry.write().await;
        
        bucket.tokens = (bucket.tokens + tokens_to_add).min(capacity as f64);
        bucket.last_refill = Instant::now();
        
        if bucket.tokens >= 1.0 {
            bucket.tokens -= 1.0;
            true // Allowed
        } else {
            false // Denied
        }

    }
}