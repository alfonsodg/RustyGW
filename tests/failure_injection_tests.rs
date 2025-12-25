use anyhow::Result;
use reqwest::Client;
use std::time::Duration;
use tokio::time::sleep;
use http::StatusCode;

#[tokio::test]
async fn test_backend_failure_recovery() -> Result<()> {
    let client = Client::new();
    let base_url = "http://127.0.0.1:8081";
    
    // Test that the gateway handles backend failures gracefully
    // This simulates the /test/breaker endpoint which initially fails
    
    let mut successful_requests = 0;
    let mut failed_requests = 0;
    
    // Make several requests to trigger circuit breaker
    for i in 0..10 {
        let res = client
            .get(format!("{}/test/breaker", base_url))
            .send()
            .await?;
        
        match res.status() {
            StatusCode::OK => successful_requests += 1,
            StatusCode::INTERNAL_SERVER_ERROR | StatusCode::SERVICE_UNAVAILABLE => failed_requests += 1,
            _ => {}
        }
        
        // Small delay between requests
        sleep(Duration::from_millis(100)).await;
    }
    
    // Should have both successful and failed requests
    assert!(successful_requests > 0, "Should have some successful requests");
    assert!(failed_requests > 0, "Should have some failed requests");
    
    Ok(())
}

#[tokio::test]
async fn test_circuit_breaker_recovery() -> Result<()> {
    let client = Client::new();
    let base_url = "http://127.0.0.1:8081";
    
    // Trip the circuit breaker
    for _ in 0..3 {
        let _ = client
            .get(format!("{}/test/breaker", base_url))
            .send()
            .await?;
    }
    
    // Wait for circuit breaker to open
    sleep(Duration::from_millis(200)).await;
    
    // Now requests should be rejected quickly by circuit breaker
    let start = std::time::Instant::now();
    let res = client
        .get(format!("{}/test/breaker", base_url))
        .send()
        .await?;
    let elapsed = start.elapsed();
    
    // Should be rejected with 503 (Service Unavailable)
    assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);
    
    // Should be rejected quickly (circuit breaker is fast)
    assert!(elapsed < Duration::from_millis(100));
    
    Ok(())
}

#[tokio::test]
async fn test_timeout_handling() -> Result<()> {
    let client = Client::new();
    let base_url = "http://127.0.0.1:8081";
    
    // Test with a very short timeout to simulate network issues
    let timeout_client = reqwest::ClientBuilder::new()
        .timeout(Duration::from_millis(100))
        .build()?;
    
    // This should timeout quickly
    let start = std::time::Instant::now();
    let result = timeout_client
        .get(format!("{}/test/public", base_url))
        .send()
        .await;
    let elapsed = start.elapsed();
    
    // Should either timeout or complete quickly
    match result {
        Ok(res) => {
            // If it completes, it should be fast
            assert!(elapsed < Duration::from_millis(200));
            // Response should be successful
            assert_eq!(res.status(), 200);
        }
        Err(_) => {
            // If it times out, should be quick
            assert!(elapsed < Duration::from_millis(150));
        }
    }
    
    Ok(())
}

#[tokio::test]
async fn test_connection_pool_exhaustion() -> Result<()> {
    let client = Client::new();
    let base_url = "http://127.0.0.1:8081";
    
    // Simulate connection pool exhaustion by making many concurrent requests
    let mut handles = vec![];
    
    for i in 0..50 {
        let client = client.clone();
        let handle = tokio::spawn(async move {
            let res = client
                .get(format!("{}/test/public", base_url))
                .send()
                .await;
            (i, res.is_ok(), res.map(|r| r.status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR))
        });
        handles.push(handle);
    }
    
    // Collect results
    let mut results = vec![];
    for handle in handles {
        results.push(handle.await?);
    }
    
    // Most requests should succeed even under load
    let successful_requests = results.iter().filter(|(_, success, _)| *success).count();
    let total_requests = results.len();
    
    let success_rate = successful_requests as f64 / total_requests as f64;
    assert!(success_rate >= 0.8, "Success rate should be at least 80% under load");
    
    Ok(())
}

#[tokio::test]
async fn test_memory_pressure() -> Result<()> {
    let client = Client::new();
    let base_url = "http://127.0.0.1:8081";
    
    // Make many requests to stress memory usage
    let mut successful_requests = 0;
    let mut failed_requests = 0;
    
    for batch in 0..5 {
        // Make 20 requests in parallel
        let mut handles = vec![];
        
        for _ in 0..20 {
            let client = client.clone();
            let handle = tokio::spawn(async move {
                let res = client
                    .get(format!("{}/test/public", base_url))
                    .send()
                    .await;
                res.map(|r| r.status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
            });
            handles.push(handle);
        }
        
        // Wait for all requests in this batch
        for handle in handles {
            match handle.await {
                Ok(status) => {
                    if status == 200 {
                        successful_requests += 1;
                    } else {
                        failed_requests += 1;
                    }
                }
                Err(_) => failed_requests += 1,
            }
        }
        
        // Small delay between batches
        sleep(Duration::from_millis(50)).await;
    }
    
    let total_requests = successful_requests + failed_requests;
    let success_rate = successful_requests as f64 / total_requests as f64;
    
    println!("Memory pressure test: {}/{} requests successful", successful_requests, total_requests);
    
    // Should maintain good performance even under memory pressure
    assert!(success_rate >= 0.9, "Should maintain 90% success rate under memory pressure");
    
    Ok(())
}

#[tokio::test]
async fn test_concurrent_mutation_safety() -> Result<()> {
    let client = Client::new();
    let base_url = "http://127.0.0.1:8081";
    
    // Test concurrent requests that might cause race conditions
    // Mix of different endpoint types
    let mut handles = vec![];
    
    for i in 0..30 {
        let client = client.clone();
        let handle = tokio::spawn(async move {
            let endpoint = match i % 4 {
                0 => "/test/public",
                1 => "/test/cache",
                2 => "/test/ratelimit",
                3 => "/test/apikey",
                _ => "/test/public",
            };
            
            let mut request = client.get(format!("{}/{}", base_url, endpoint));
            
            // Add auth header for protected endpoints
            if endpoint.contains("apikey") {
                request = request.header("Authorization", "Bearer user-key-for-alice");
            }
            
            let res = request.send().await;
            (i, endpoint, res.is_ok(), res.map(|r| r.status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR))
        });
        handles.push(handle);
    }
    
    // Collect results and check for any panics or corrupted states
    let mut results = vec![];
    for handle in handles {
        match handle.await {
            Ok(result) => results.push(result),
            Err(_) => {
                // Thread panicked - this indicates a serious concurrency issue
                panic!("Thread panicked during concurrent mutation test");
            }
        }
    }
    
    // All requests should either succeed or fail gracefully
    for (i, endpoint, success, status) in &results {
        if *success {
            assert_eq!(*status, 200, "Request {} to {} should succeed", i, endpoint);
        }
        // No assertions on failures - they should be handled gracefully
    }
    
    // At least some requests should succeed
    let successful_requests = results.iter().filter(|(_, _, success, _)| *success).count();
    assert!(successful_requests > 0, "At least some requests should succeed");
    
    Ok(())
}

#[tokio::test]
async fn test_error_propagation() -> Result<()> {
    let client = Client::new();
    let base_url = "http://127.0.0.1:8081";
    
    // Test that errors are properly propagated and handled
    
    // Test 404 error
    let res = client
        .get(format!("{}/nonexistent", base_url))
        .send()
        .await?;
    assert_eq!(res.status(), 404);
    
    // Test 401 error
    let res = client
        .get(format!("{}/test/jwt", base_url))
        .header("Authorization", "Bearer invalid")
        .send()
        .await?;
    assert_eq!(res.status(), 401);
    
    // Test 403 error
    let user_jwt = generate_jwt(vec!["user"]);
    let res = client
        .get(format!("{}/test/jwt", base_url))
        .header("Authorization", format!("Bearer {}", user_jwt))
        .send()
        .await?;
    assert_eq!(res.status(), 403);
    
    // Test 429 error (rate limiting)
    let mut rate_limited = false;
    for _ in 0..5 {
        let res = client
            .get(format!("{}/test/ratelimit", base_url))
            .send()
            .await?;
        if res.status() == 429 {
            rate_limited = true;
            break;
        }
    }
    assert!(rate_limited, "Should trigger rate limiting");
    
    // Test 503 error (circuit breaker)
    for _ in 0..3 {
        let _ = client
            .get(format!("{}/test/breaker", base_url))
            .send()
            .await?;
    }
    
    sleep(Duration::from_millis(200)).await;
    
    let res = client
        .get(format!("{}/test/breaker", base_url))
        .send()
        .await?;
    assert_eq!(res.status(), 503);
    
    Ok(())
}

#[tokio::test]
async fn test_graceful_degradation() -> Result<()> {
    let client = Client::new();
    let base_url = "http://127.0.0.1:8081";
    
    // Test that the gateway degrades gracefully under various failure conditions
    
    // First, test normal operation
    let res = client
        .get(format!("{}/test/public", base_url))
        .send()
        .await?;
    assert_eq!(res.status(), 200);
    
    // Test cache under load
    let mut cache_results = vec![];
    for _ in 0..10 {
        let res = client
            .get(format!("{}/test/cache", base_url))
            .send()
            .await?;
        cache_results.push(res.status());
    }
    
    // Most cache requests should succeed
    let successful_cache_requests = cache_results.iter().filter(|&&status| status == 200).count();
    assert!(successful_cache_requests >= 8, "Most cache requests should succeed");
    
    // Test auth under load
    let mut auth_results = vec![];
    for _ in 0..5 {
        let res = client
            .get(format!("{}/test/apikey", base_url))
            .header("Authorization", "Bearer user-key-for-alice")
            .send()
            .await?;
        auth_results.push(res.status());
    }
    
    // Auth requests should be consistent
    for status in auth_results {
        assert_eq!(status, 200, "Auth requests should be consistent");
    }
    
    Ok(())
}

#[tokio::test]
async fn test_recovery_after_panic_simulation() -> Result<()> {
    let client = Client::new();
    let base_url = "http://127.0.0.1:8081";
    
    // Test that the gateway recovers after simulated failures
    // by making requests that might trigger various error conditions
    
    // Trigger multiple error conditions
    for _ in 0..5 {
        // Trigger rate limiting
        let _ = client
            .get(format!("{}/test/ratelimit", base_url))
            .send()
            .await?;
        
        // Trigger auth failures
        let _ = client
            .get(format!("{}/test/jwt", base_url))
            .header("Authorization", "Bearer invalid")
            .send()
            .await?;
        
        // Trigger 404s
        let _ = client
            .get(format!("{}/nonexistent", base_url))
            .send()
            .await?;
    }
    
    // Wait a bit for recovery
    sleep(Duration::from_millis(500)).await;
    
    // Now test that normal operation resumes
    let res = client
        .get(format!("{}/test/public", base_url))
        .send()
        .await?;
    assert_eq!(res.status(), 200);
    
    // Test cache still works
    let res = client
        .get(format!("{}/test/cache", base_url))
        .send()
        .await?;
    assert_eq!(res.status(), 200);
    
    // Test auth still works
    let res = client
        .get(format!("{}/test/apikey", base_url))
        .header("Authorization", "Bearer user-key-for-alice")
        .send()
        .await?;
    assert_eq!(res.status(), 200);
    
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
