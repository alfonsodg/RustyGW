//! Concurrency tests for race conditions and thread safety.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::timeout;

#[tokio::test]
async fn test_concurrent_counter_increment() {
    let counter = Arc::new(AtomicU64::new(0));
    let mut handles = vec![];

    for _ in 0..100 {
        let counter = counter.clone();
        handles.push(tokio::spawn(async move {
            for _ in 0..100 {
                counter.fetch_add(1, Ordering::SeqCst);
            }
        }));
    }

    for handle in handles {
        handle.await.unwrap();
    }

    assert_eq!(counter.load(Ordering::SeqCst), 10_000);
}

#[tokio::test]
async fn test_concurrent_rwlock_reads() {
    let data = Arc::new(RwLock::new(vec![1, 2, 3, 4, 5]));
    let mut handles = vec![];

    // Spawn many concurrent readers
    for _ in 0..50 {
        let data = data.clone();
        handles.push(tokio::spawn(async move {
            for _ in 0..100 {
                let guard = data.read().await;
                let _sum: i32 = guard.iter().sum();
            }
        }));
    }

    for handle in handles {
        handle.await.unwrap();
    }
}

#[tokio::test]
async fn test_concurrent_rwlock_writes() {
    let data = Arc::new(RwLock::new(0u64));
    let mut handles = vec![];

    for _ in 0..10 {
        let data = data.clone();
        handles.push(tokio::spawn(async move {
            for _ in 0..100 {
                let mut guard = data.write().await;
                *guard += 1;
            }
        }));
    }

    for handle in handles {
        handle.await.unwrap();
    }

    assert_eq!(*data.read().await, 1000);
}

#[tokio::test]
async fn test_concurrent_read_write_mix() {
    let data = Arc::new(RwLock::new(0u64));
    let mut handles = vec![];

    // Writers
    for _ in 0..5 {
        let data = data.clone();
        handles.push(tokio::spawn(async move {
            for _ in 0..50 {
                let mut guard = data.write().await;
                *guard += 1;
                tokio::task::yield_now().await;
            }
        }));
    }

    // Readers
    for _ in 0..20 {
        let data = data.clone();
        handles.push(tokio::spawn(async move {
            for _ in 0..100 {
                let _val = *data.read().await;
                tokio::task::yield_now().await;
            }
        }));
    }

    for handle in handles {
        handle.await.unwrap();
    }

    assert_eq!(*data.read().await, 250);
}

#[tokio::test]
async fn test_concurrent_hashmap_operations() {
    use dashmap::DashMap;

    let map: Arc<DashMap<String, u64>> = Arc::new(DashMap::new());
    let mut handles = vec![];

    // Concurrent inserts
    for i in 0..50 {
        let map = map.clone();
        handles.push(tokio::spawn(async move {
            for j in 0..20 {
                map.insert(format!("key-{}-{}", i, j), i * 100 + j);
            }
        }));
    }

    for handle in handles {
        handle.await.unwrap();
    }

    assert_eq!(map.len(), 1000);
}

#[tokio::test]
async fn test_concurrent_cache_operations() {
    use moka::future::Cache;

    let cache: Arc<Cache<String, String>> = Arc::new(
        Cache::builder()
            .max_capacity(1000)
            .time_to_live(Duration::from_secs(60))
            .build(),
    );

    let mut handles = vec![];

    // Concurrent writes
    for i in 0..20 {
        let cache = cache.clone();
        handles.push(tokio::spawn(async move {
            for j in 0..50 {
                cache.insert(format!("key-{}-{}", i, j), format!("value-{}-{}", i, j)).await;
            }
        }));
    }

    // Concurrent reads
    for i in 0..20 {
        let cache = cache.clone();
        handles.push(tokio::spawn(async move {
            for j in 0..50 {
                let _ = cache.get(&format!("key-{}-{}", i, j)).await;
            }
        }));
    }

    for handle in handles {
        handle.await.unwrap();
    }
}

#[tokio::test]
async fn test_timeout_handling() {
    let result = timeout(Duration::from_millis(100), async {
        tokio::time::sleep(Duration::from_millis(50)).await;
        42
    })
    .await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 42);

    let result = timeout(Duration::from_millis(50), async {
        tokio::time::sleep(Duration::from_millis(100)).await;
        42
    })
    .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_concurrent_token_bucket() {
    struct TokenBucket {
        tokens: AtomicU64,
        #[allow(dead_code)]
        capacity: u64,
    }

    impl TokenBucket {
        fn new(capacity: u64) -> Self {
            Self {
                tokens: AtomicU64::new(capacity),
                capacity,
            }
        }

        fn try_acquire(&self) -> bool {
            loop {
                let current = self.tokens.load(Ordering::Relaxed);
                if current == 0 {
                    return false;
                }
                if self.tokens.compare_exchange(
                    current,
                    current - 1,
                    Ordering::SeqCst,
                    Ordering::Relaxed,
                ).is_ok() {
                    return true;
                }
            }
        }
    }

    let bucket = Arc::new(TokenBucket::new(100));
    let success_count = Arc::new(AtomicU64::new(0));
    let mut handles = vec![];

    for _ in 0..50 {
        let bucket = bucket.clone();
        let success_count = success_count.clone();
        handles.push(tokio::spawn(async move {
            for _ in 0..10 {
                if bucket.try_acquire() {
                    success_count.fetch_add(1, Ordering::SeqCst);
                }
            }
        }));
    }

    for handle in handles {
        handle.await.unwrap();
    }

    // Should have exactly 100 successful acquisitions
    assert_eq!(success_count.load(Ordering::SeqCst), 100);
}
