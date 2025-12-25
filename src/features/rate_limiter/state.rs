use std::sync::Arc;

use async_trait::async_trait;
use dashmap::DashMap;
use tokio::sync::RwLock;
use tokio::time::Instant;

use crate::constants::rate_limiter as rl_constants;

#[async_trait]
pub trait RateLimitState: Send + Sync {
    async fn check_and_update(&self, key: &str, capacity: u64, refill_rate: f64) -> bool;
    /// Manual cleanup method to remove expired buckets
    fn cleanup_expired_buckets(&self);
    /// Get current number of active buckets for monitoring
    fn get_active_buckets_count(&self) -> usize;
}

struct Bucket {
    tokens: f64,
    last_refill: Instant,
    last_access: Instant, // Track when bucket was last accessed
}

pub struct InMemoryRateLimitState {
    clients: DashMap<String, Arc<RwLock<Bucket>>>,
    ttl_seconds: u64, // Time to live for inactive buckets in seconds
}

impl InMemoryRateLimitState {
    pub fn new() -> Self {
        Self::with_ttl(rl_constants::DEFAULT_TTL_SECONDS)
    }
    
    pub fn with_ttl(ttl_seconds: u64) -> Self {
        Self {
            clients: DashMap::new(),
            ttl_seconds,
        }
    }
    
    /// Clean up buckets that haven't been accessed for longer than TTL
    fn perform_cleanup(&self) {
        let now = Instant::now();
        let ttl_duration = std::time::Duration::from_secs(self.ttl_seconds);
        
        // Collect keys to remove
        let keys_to_remove: Vec<String> = self.clients
            .iter()
            .filter_map(|entry| {
                let bucket_arc = entry.value();
                // Access the RwLock to get bucket data
                if let Ok(bucket_guard) = bucket_arc.try_read() {
                    // Check if bucket hasn't been accessed recently
                    if now.duration_since(bucket_guard.last_access) > ttl_duration {
                        Some(entry.key().clone())
                    } else {
                        None
                    }
                } else {
                    // If we can't get the lock, assume it's still in use
                    None
                }
            })
            .collect();
        
        // Remove expired buckets
        let removed_count = keys_to_remove.len();
        for key in &keys_to_remove {
            self.clients.remove(key);
        }
        
        if removed_count > 0 {
            tracing::info!("Cleaned up {} expired rate limit buckets", removed_count);
        }
    }
    
    /// Get current number of active buckets for monitoring
    fn get_active_buckets_count(&self) -> usize {
        self.clients.len()
    }
}

#[async_trait]
impl RateLimitState for InMemoryRateLimitState {
   
    async fn check_and_update(&self, key: &str, capacity: u64, refill_rate: f64) -> bool {
        let now = Instant::now();
        
        let entry = self.clients.entry(key.to_string()).or_insert_with(|| {
            Arc::new(RwLock::new(Bucket { 
                tokens: capacity as f64, 
                last_refill: now,
                last_access: now,
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
        bucket.last_refill = now;
        bucket.last_access = now; // Update access time
        
        if bucket.tokens >= 1.0 {
            bucket.tokens -= 1.0;
            true // Allowed
        } else {
            false // Denied
        }
    }
    
    fn cleanup_expired_buckets(&self) {
        self.perform_cleanup()
    }
    
    fn get_active_buckets_count(&self) -> usize {
        self.clients.len()
    }
}