use anyhow::Result;
use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use reqwest::Client;
use std::time::Duration;
use tokio::time::Instant;

fn benchmark_gateway_throughput(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    
    c.bench_function("gateway_throughput_public", |b| {
        b.to_async(&rt).iter_batched(
            || {
                let client = Client::new();
                let base_url = "http://127.0.0.1:8081";
                (client, base_url.to_string())
            },
            |(client, base_url)| async move {
                let _res = client
                    .get(&format!("{}/test/public", base_url))
                    .send()
                    .await
                    .unwrap();
                black_box(_res)
            },
            criterion::BatchSize::SmallInput,
        )
    });
}

fn benchmark_gateway_with_auth(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let admin_jwt = generate_jwt(vec!["admin"]);
    
    c.bench_function("gateway_throughput_with_auth", |b| {
        b.to_async(&rt).iter_batched(
            || {
                let client = Client::new();
                let base_url = "http://127.0.0.1:8081";
                let token = admin_jwt.clone();
                (client, base_url, token)
            },
            |(client, base_url, token)| async move {
                let _res = client
                    .get(&format!("{}/test/jwt", base_url))
                    .header("Authorization", format!("Bearer {}", token))
                    .send()
                    .await
                    .unwrap();
                black_box(_res)
            },
            criterion::BatchSize::SmallInput,
        )
    });
}

fn benchmark_cache_performance(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    
    c.bench_function("cache_miss_performance", |b| {
        b.to_async(&rt).iter_batched(
            || {
                let client = Client::new();
                let base_url = "http://127.0.0.1:8081";
                (client, base_url.to_string())
            },
            |(client, base_url)| async move {
                let _res = client
                    .get(&format!("{}/test/cache", base_url))
                    .send()
                    .await
                    .unwrap();
                black_box(_res)
            },
            criterion::BatchSize::SmallInput,
        )
    });
}

fn benchmark_rate_limiting(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    
    c.bench_function("rate_limiting_check", |b| {
        b.to_async(&rt).iter_batched(
            || {
                let client = Client::new();
                let base_url = "http://127.0.0.1:8081";
                (client, base_url.to_string())
            },
            |(client, base_url)| async move {
                let _res = client
                    .get(&format!("{}/test/ratelimit", base_url))
                    .send()
                    .await
                    .unwrap();
                black_box(_res)
            },
            criterion::BatchSize::SmallInput,
        )
    });
}

fn benchmark_concurrent_requests(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("concurrent_requests");
    
    for num_requests in [1, 5, 10, 20, 50].iter() {
        group.throughput(Throughput::Elements(*num_requests as u64));
        
        group.bench_with_input(
            criterion::BenchmarkId::new("concurrent", num_requests),
            num_requests,
            |b, &num_requests| {
                b.to_async(&rt).iter_batched(
                    || {
                        let client = Client::new();
                        let base_url = "http://127.0.0.1:8081";
                        (client, base_url.to_string())
                    },
                    |(client, base_url)| async move {
                        let mut handles = vec![];
                        
                        for _ in 0..num_requests {
                            let client = client.clone();
                            let base_url = base_url.clone();
                            let handle = tokio::spawn(async move {
                                let _res = client
                                    .get(&format!("{}/test/public", base_url))
                                    .send()
                                    .await
                                    .unwrap();
                                black_box(_res)
                            });
                            handles.push(handle);
                        }
                        
                        // Wait for all requests to complete
                        for handle in handles {
                            let _ = handle.await;
                        }
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }
    
    group.finish();
}

fn benchmark_authentication_methods(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let admin_jwt = generate_jwt(vec!["admin"]);
    
    // Benchmark different authentication methods
    c.bench_function("auth_api_key", |b| {
        b.to_async(&rt).iter_batched(
            || {
                let client = Client::new();
                let base_url = "http://127.0.0.1:8081";
                (client, base_url.to_string())
            },
            |(client, base_url)| async move {
                let _res = client
                    .get(&format!("{}/test/apikey", base_url))
                    .header("Authorization", "Bearer user-key-for-alice")
                    .send()
                    .await
                    .unwrap();
                black_box(_res)
            },
            criterion::BatchSize::SmallInput,
        )
    });
    
    c.bench_function("auth_jwt", |b| {
        b.to_async(&rt).iter_batched(
            || {
                let client = Client::new();
                let base_url = "http://127.0.0.1:8081";
                let token = admin_jwt.clone();
                (client, base_url.to_string(), token)
            },
            |(client, base_url, token)| async move {
                let _res = client
                    .get(&format!("{}/test/jwt", base_url))
                    .header("Authorization", format!("Bearer {}", token))
                    .send()
                    .await
                    .unwrap();
                black_box(_res)
            },
            criterion::BatchSize::SmallInput,
        )
    });
}

fn benchmark_memory_usage(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    
    c.bench_function("memory_per_request", |b| {
        b.to_async(&rt).iter_batched(
            || {
                let client = Client::new();
                let base_url = "http://127.0.0.1:8081";
                (client, base_url.to_string())
            },
            |(client, base_url)| async move {
                // Make multiple requests to stress memory usage
                for _ in 0..10 {
                    let _res = client
                        .get(&format!("{}/test/public", base_url))
                        .send()
                        .await
                        .unwrap();
                }
                black_box(())
            },
            criterion::BatchSize::SmallInput,
        )
    });
}

fn benchmark_error_handling(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    
    c.bench_function("error_404_handling", |b| {
        b.to_async(&rt).iter_batched(
            || {
                let client = Client::new();
                let base_url = "http://127.0.0.1:8081";
                (client, base_url.to_string())
            },
            |(client, base_url)| async move {
                let _res = client
                    .get(&format!("{}/nonexistent/route", base_url))
                    .send()
                    .await
                    .unwrap();
                black_box(_res)
            },
            criterion::BatchSize::SmallInput,
        )
    });
    
    c.bench_function("error_401_handling", |b| {
        b.to_async(&rt).iter_batched(
            || {
                let client = Client::new();
                let base_url = "http://127.0.0.1:8081";
                (client, base_url.to_string())
            },
            |(client, base_url)| async move {
                let _res = client
                    .get(&format!("{}/test/jwt", base_url))
                    .header("Authorization", "Bearer invalid-token")
                    .send()
                    .await
                    .unwrap();
                black_box(_res)
            },
            criterion::BatchSize::SmallInput,
        )
    });
    
    c.bench_function("error_429_handling", |b| {
        b.to_async(&rt).iter_batched(
            || {
                let client = Client::new();
                let base_url = "http://127.0.0.1:8081";
                (client, base_url.to_string())
            },
            |(client, base_url)| async move {
                // First make requests to trigger rate limiting
                for _ in 0..5 {
                    let _ = client
                        .get(&format!("{}/test/ratelimit", base_url))
                        .send()
                        .await;
                }
                
                // Then test the rate limited response
                let _res = client
                    .get(&format!("{}/test/ratelimit", base_url))
                    .send()
                    .await
                    .unwrap();
                black_box(_res)
            },
            criterion::BatchSize::SmallInput,
        )
    });
}

criterion_group!(
    benches,
    benchmark_gateway_throughput,
    benchmark_gateway_with_auth,
    benchmark_cache_performance,
    benchmark_rate_limiting,
    benchmark_concurrent_requests,
    benchmark_authentication_methods,
    benchmark_memory_usage,
    benchmark_error_handling
);
criterion_main!(benches);

// Custom async benchmark for measuring request latency
#[tokio::test]
async fn benchmark_request_latency() -> Result<()> {
    let client = Client::new();
    let base_url = "http://127.0.0.1:8081";
    
    // Warm up the connection
    let _ = client.get(format!("{}/test/public", base_url)).send().await?;
    
    // Measure latency for 100 requests
    let mut latencies = Vec::new();
    
    for _ in 0..100 {
        let start = Instant::now();
        let _res = client.get(format!("{}/test/public", base_url)).send().await?;
        let elapsed = start.elapsed();
        latencies.push(elapsed);
    }
    
    // Calculate statistics
    let total_latency: Duration = latencies.iter().sum();
    let avg_latency = total_latency / latencies.len() as u32;
    
    let mut sorted_latencies = latencies;
    sorted_latencies.sort();
    
    let p50_latency = sorted_latencies[50];
    let p95_latency = sorted_latencies[95];
    let p99_latency = sorted_latencies[99];
    
    println!("Request Latency Statistics:");
    println!("Average: {:.2}ms", avg_latency.as_secs_f64() * 1000.0);
    println!("P50: {:.2}ms", p50_latency.as_secs_f64() * 1000.0);
    println!("P95: {:.2}ms", p95_latency.as_secs_f64() * 1000.0);
    println!("P99: {:.2}ms", p99_latency.as_secs_f64() * 1000.0);
    
    // Assertions for performance expectations
    assert!(avg_latency < Duration::from_millis(50), "Average latency should be under 50ms");
    assert!(p95_latency < Duration::from_millis(100), "P95 latency should be under 100ms");
    
    Ok(())
}

#[tokio::test]
async fn benchmark_throughput_sustained() -> Result<()> {
    let client = Client::new();
    let base_url = "http://127.0.0.1:8081";
    
    // Measure sustained throughput over 10 seconds
    let start = Instant::now();
    let duration = Duration::from_secs(10);
    let mut request_count = 0;
    let mut error_count = 0;
    
    let mut handles = vec![];
    
    // Create a task that continuously makes requests
    let handle = tokio::spawn(async move {
        let client = Client::new();
        
        loop {
            let res = client
                .get(format!("{}/test/public", base_url))
                .send()
                .await;
            
            match res {
                Ok(_) => request_count += 1,
                Err(_) => error_count += 1,
            }
            
            // Check if we've run for the specified duration
            if start.elapsed() >= duration {
                break;
            }
        }
    });
    
    handles.push(handle);
    
    // Wait for all tasks to complete
    for handle in handles {
        handle.await?;
    }
    
    let elapsed = start.elapsed();
    let throughput = request_count as f64 / elapsed.as_secs_f64();
    let error_rate = error_count as f64 / (request_count + error_count) as f64;
    
    println!("Sustained Throughput Statistics:");
    println!("Duration: {:.2}s", elapsed.as_secs_f64());
    println!("Total requests: {}", request_count);
    println!("Total errors: {}", error_count);
    println!("Throughput: {:.2} requests/second", throughput);
    println!("Error rate: {:.2}%", error_rate * 100.0);
    
    // Performance assertions
    assert!(throughput > 100.0, "Throughput should be over 100 requests/second");
    assert!(error_rate < 0.01, "Error rate should be under 1%");
    
    Ok(())
}

// Helper function to generate JWT tokens for testing
fn generate_jwt(roles: Vec<&str>) -> String {
    use jsonwebtoken::{encode, EncodingKey, Header};
    use serde_json::json;
    use std::time::{SystemTime, UNIX_EPOCH};
    
    let claims = json!({
        "sub": "test-user",
        "roles": roles,
        "exp": SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() + 3600,
    });
    
    let secret = "super-secret-jwt-key-for-testing-only";
    encode(&Header::default(), &claims, &EncodingKey::from_secret(secret.as_ref())).unwrap()
}
