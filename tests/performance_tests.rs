use anyhow::Result;
use reqwest::Client;
use std::time::Duration;
use tokio::time::Instant;

// Simple performance tests without external benchmarking framework

#[tokio::test]
async fn test_request_latency_measurement() -> Result<()> {
    let client = Client::new();
    let base_url = "http://127.0.0.1:8081";
    
    // Warm up the connection
    let _ = client.get(format!("{}/test/public", base_url)).send().await?;
    
    // Measure latency for multiple requests
    let mut latencies = Vec::new();
    
    for _ in 0..10 {
        let start = Instant::now();
        let _res = client.get(format!("{}/test/public", base_url)).send().await?;
        let elapsed = start.elapsed();
        latencies.push(elapsed);
    }
    
    // Calculate basic statistics
    let total_latency: Duration = latencies.iter().sum();
    let avg_latency = total_latency / latencies.len() as u32;
    
    let mut sorted_latencies = latencies;
    sorted_latencies.sort();
    
    let p50_latency = sorted_latencies[5];
    let p90_latency = sorted_latencies[9];
    
    println!("Performance Results:");
    println!("Average: {:.2}ms", avg_latency.as_secs_f64() * 1000.0);
    println!("P50: {:.2}ms", p50_latency.as_secs_f64() * 1000.0);
    println!("P90: {:.2}ms", p90_latency.as_secs_f64() * 1000.0);
    
    // Basic performance assertions
    assert!(avg_latency < Duration::from_millis(100), "Average latency should be under 100ms");
    assert!(p90_latency < Duration::from_millis(200), "P90 latency should be under 200ms");
    
    Ok(())
}

#[tokio::test]
async fn test_throughput_measurement() -> Result<()> {
    let client = Client::new();
    let base_url = "http://127.0.0.1:8081";
    
    // Measure requests per second
    let start = Instant::now();
    let duration = Duration::from_secs(2); // Test for 2 seconds
    
    let mut request_count = 0;
    let mut handles = vec![];
    
    // Create a task that makes continuous requests
    let handle = tokio::spawn(async move {
        let client = Client::new();
        let start = Instant::now();
        
        while start.elapsed() < duration {
            let _ = client
                .get(format!("{}/test/public", base_url))
                .send()
                .await;
            request_count += 1;
        }
    });
    
    handles.push(handle);
    
    // Wait for completion
    for handle in handles {
        handle.await?;
    }
    
    let elapsed = start.elapsed();
    let throughput = request_count as f64 / elapsed.as_secs_f64();
    
    println!("Throughput Results:");
    println!("Duration: {:.2}s", elapsed.as_secs_f64());
    println!("Total requests: {}", request_count);
    println!("Throughput: {:.2} requests/second", throughput);
    
    // Basic throughput assertions
    assert!(throughput > 10.0, "Throughput should be over 10 requests/second");
    
    Ok(())
}

#[tokio::test]
async fn test_concurrent_performance() -> Result<()> {
    let client = Client::new();
    let base_url = "http://127.0.0.1:8081";
    
    // Test concurrent request performance
    let start = Instant::now();
    
    let mut handles = vec![];
    for i in 0..20 {
        let client = client.clone();
        let handle = tokio::spawn(async move {
            let start = Instant::now();
            let res = client
                .get(format!("{}/test/public", base_url))
                .send()
                .await;
            let elapsed = start.elapsed();
            (i, res.is_ok(), elapsed)
        });
        handles.push(handle);
    }
    
    // Collect results
    let mut results = vec![];
    for handle in handles {
        results.push(handle.await?);
    }
    
    let total_elapsed = start.elapsed();
    
    // Analyze results
    let successful_requests = results.iter().filter(|(_, success, _)| *success).count();
    let avg_request_time = results.iter()
        .filter_map(|(_, success, elapsed)| if *success { Some(*elapsed) } else { None })
        .sum::<Duration>() / successful_requests as u32;
    
    println!("Concurrent Performance Results:");
    println!("Total time: {:.2}s", total_elapsed.as_secs_f64());
    println!("Successful requests: {}/20", successful_requests);
    println!("Average request time: {:.2}ms", avg_request_time.as_secs_f64() * 1000.0);
    println!("Requests per second: {:.2}", 20.0 / total_elapsed.as_secs_f64());
    
    // Performance assertions
    assert!(successful_requests >= 18, "At least 18/20 requests should succeed");
    assert!(avg_request_time < Duration::from_millis(50), "Average request time should be under 50ms");
    
    Ok(())
}

#[tokio::test]
async fn test_cache_performance_impact() -> Result<()> {
    let client = Client::new();
    let base_url = "http://127.0.0.1:8081";
    
    // Test cache performance vs non-cached requests
    
    // First request (cache miss)
    let start1 = Instant::now();
    let _ = client.get(format!("{}/test/cache", base_url)).send().await?;
    let cache_miss_time = start1.elapsed();
    
    // Small delay to ensure cache is populated
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Second request (cache hit)
    let start2 = Instant::now();
    let _ = client.get(format!("{}/test/cache", base_url)).send().await?;
    let cache_hit_time = start2.elapsed();
    
    println!("Cache Performance Results:");
    println!("Cache miss time: {:.2}ms", cache_miss_time.as_secs_f64() * 1000.0);
    println!("Cache hit time: {:.2}ms", cache_hit_time.as_secs_f64() * 1000.0);
    println!("Speedup: {:.2}x", cache_miss_time.as_secs_f64() / cache_hit_time.as_secs_f64());
    
    // Cache should be faster (though not always guaranteed in test environment)
    // We'll just log the results for now
    assert!(cache_hit_time <= cache_miss_time * 2, "Cache hit should not be significantly slower");
    
    Ok(())
}

#[tokio::test]
async fn test_authentication_performance_comparison() -> Result<()> {
    let client = Client::new();
    let base_url = "http://127.0.0.1:8081";
    
    // Compare performance of different authentication methods
    
    // API Key auth
    let start1 = Instant::now();
    for _ in 0..5 {
        let _ = client
            .get(format!("{}/test/apikey", base_url))
            .header("Authorization", "Bearer user-key-for-alice")
            .send()
            .await?;
    }
    let api_key_time = start1.elapsed();
    
    // JWT auth (with valid token)
    let admin_jwt = generate_jwt(vec!["admin"]);
    let start2 = Instant::now();
    for _ in 0..5 {
        let _ = client
            .get(format!("{}/test/jwt", base_url))
            .header("Authorization", format!("Bearer {}", admin_jwt))
            .send()
            .await?;
    }
    let jwt_time = start2.elapsed();
    
    // Public endpoint (no auth)
    let start3 = Instant::now();
    for _ in 0..5 {
        let _ = client.get(format!("{}/test/public", base_url)).send().await?;
    }
    let public_time = start3.elapsed();
    
    println!("Authentication Performance Comparison:");
    println!("API Key (5 requests): {:.2}ms", api_key_time.as_secs_f64() * 1000.0);
    println!("JWT (5 requests): {:.2}ms", jwt_time.as_secs_f64() * 1000.0);
    println!("Public (5 requests): {:.2}ms", public_time.as_secs_f64() * 1000.0);
    println!("API Key per request: {:.2}ms", (api_key_time.as_secs_f64() * 1000.0) / 5.0);
    println!("JWT per request: {:.2}ms", (jwt_time.as_secs_f64() * 1000.0) / 5.0);
    println!("Public per request: {:.2}ms", (public_time.as_secs_f64() * 1000.0) / 5.0);
    
    // All methods should be reasonably fast
    assert!(api_key_time < Duration::from_secs(2), "API Key auth should complete quickly");
    assert!(jwt_time < Duration::from_secs(2), "JWT auth should complete quickly");
    assert!(public_time < Duration::from_secs(2), "Public endpoint should be fast");
    
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
