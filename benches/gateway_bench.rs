//! Performance benchmarks for the API Gateway.

use criterion::{Criterion, Throughput, black_box, criterion_group, criterion_main};
use std::collections::HashMap;
use std::time::Duration;

fn bench_hashmap_lookup(c: &mut Criterion) {
    let mut map = HashMap::new();
    for i in 0..1000 {
        map.insert(format!("/api/v1/route{}", i), i);
    }

    let mut group = c.benchmark_group("route_matching");
    group.throughput(Throughput::Elements(1));

    group.bench_function("hashmap_exact_match", |b| {
        b.iter(|| black_box(map.get("/api/v1/route500")))
    });

    group.bench_function("hashmap_miss", |b| b.iter(|| black_box(map.get("/api/v1/nonexistent"))));

    group.finish();
}

fn bench_string_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("string_ops");

    let path = "/api/v1/users/123/profile";

    group.bench_function("path_split", |b| {
        b.iter(|| black_box(path.split('/').collect::<Vec<_>>()))
    });

    group.bench_function("path_starts_with", |b| {
        b.iter(|| black_box(path.starts_with("/api/v1")))
    });

    group.finish();
}

fn bench_token_bucket(c: &mut Criterion) {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Instant;

    struct SimpleBucket {
        tokens: AtomicU64,
        last_refill: std::sync::Mutex<Instant>,
        capacity: u64,
        refill_rate: u64,
    }

    impl SimpleBucket {
        fn new(capacity: u64, refill_rate: u64) -> Self {
            Self {
                tokens: AtomicU64::new(capacity),
                last_refill: std::sync::Mutex::new(Instant::now()),
                capacity,
                refill_rate,
            }
        }

        fn try_acquire(&self) -> bool {
            loop {
                let current = self.tokens.load(Ordering::Relaxed);
                if current == 0 {
                    return false;
                }
                if self
                    .tokens
                    .compare_exchange(current, current - 1, Ordering::SeqCst, Ordering::Relaxed)
                    .is_ok()
                {
                    return true;
                }
            }
        }
    }

    let bucket = SimpleBucket::new(1000, 100);

    let mut group = c.benchmark_group("rate_limiting");
    group.throughput(Throughput::Elements(1));

    group.bench_function("token_acquire", |b| b.iter(|| black_box(bucket.try_acquire())));

    group.finish();
}

fn bench_cache_key_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache");

    let method = "GET";
    let path = "/api/v1/users/123";
    let query = "include=profile&fields=name,email";

    group.bench_function("key_format", |b| {
        b.iter(|| black_box(format!("{}:{}:{}", method, path, query)))
    });

    group.bench_function("key_concat", |b| {
        b.iter(|| {
            let mut key = String::with_capacity(method.len() + path.len() + query.len() + 2);
            key.push_str(method);
            key.push(':');
            key.push_str(path);
            key.push(':');
            key.push_str(query);
            black_box(key)
        })
    });

    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .measurement_time(Duration::from_secs(5))
        .sample_size(100);
    targets = bench_hashmap_lookup, bench_string_operations, bench_token_bucket, bench_cache_key_generation
}

criterion_main!(benches);
