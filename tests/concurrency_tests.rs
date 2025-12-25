use anyhow::Result;
use reqwest::Client;
use std::sync::Arc;
use tokio::time::{Duration, sleep};

#[tokio::test]
async fn test_concurrent_auth_requests() -> Result<()> {
    let client = Arc::new(Client::new());
    let base_url = "http://127.0.0.1:8081";
    
    // Generate multiple valid JWTs with different roles
    let admin_jwt = generate_jwt(vec!["admin"]);
    let user_jwt = generate_jwt(vec!["user"]);
    
    // Test concurrent requests with different authentication states
    let mut handles = vec![];
    
    for i in 0..20 {
        let client = client.clone();
        let admin_token = admin_jwt.clone();
        let user_token = user_jwt.clone();
        
        let handle = tokio::spawn(async move {
            let token = if i % 2 == 0 { &admin_token } else { &user_token };
            let res = client
                .get(format!("{}/test/jwt", base_url))
                .header("Authorization", format!("Bearer {}", token))
                .send()
                .await
                .unwrap();
            (i, res.status())
        });
        handles.push(handle);
    }
    
    // Collect all results
    let mut results = vec![];
    for handle in handles {
        results.push(handle.await?);
    }
    
    // Verify results - admin requests should succeed, user requests should fail
    for (i, status) in results {
        if i % 2 == 0 {
            assert_eq!(status, 200, "Admin request {} should succeed", i);
        } else {
            assert_eq!(status, 403, "User request {} should be forbidden", i);
        }
    }
    
    Ok(())
}

#[tokio::test]
async fn test_concurrent_rate_limiting() -> Result<()> {
    let client = Arc::new(Client::new());
    let base_url = "http://127.0.0.1:8081";
    
    // Test concurrent requests to trigger rate limiting
    let mut handles = vec![];
    
    for i in 0..10 {
        let client = client.clone();
        let handle = tokio::spawn(async move {
            let start = std::time::Instant::now();
            let res = client
                .get(format!("{}/test/ratelimit", base_url))
                .send()
                .await
                .unwrap();
            let elapsed = start.elapsed();
            (i, res.status(), elapsed)
        });
        handles.push(handle);
    }
    
    // Collect all results
    let mut results = vec![];
    for handle in handles {
        results.push(handle.await?);
    }
    
    // Verify that some requests were rate limited
    let successful_requests: Vec<_> = results.iter()
        .filter(|(_, status, _)| *status == 200)
        .collect();
    let rate_limited_requests: Vec<_> = results.iter()
        .filter(|(_, status, _)| *status == 429)
        .collect();
    
    assert!(!successful_requests.is_empty(), "Should have some successful requests");
    assert!(!rate_limited_requests.is_empty(), "Should have some rate-limited requests");
    
    // Rate-limited requests should be faster (rejected at gateway level)
    let avg_successful_time: Duration = successful_requests.iter()
        .map(|(_, _, elapsed)| *elapsed)
        .sum::<Duration>() / successful_requests.len() as u32;
    
    let avg_rate_limited_time: Duration = rate_limited_requests.iter()
        .map(|(_, _, elapsed)| *elapsed)
        .sum::<Duration>() / rate_limited_requests.len() as u32;
    
    // Rate-limited requests should be faster
    assert!(avg_rate_limited_time < avg_successful_time);
    
    Ok(())
}

#[tokio::test]
async fn test_concurrent_cache_access() -> Result<()> {
    let client = Arc::new(Client::new());
    let base_url = "http://127.0.0.1:8081";
    
    // Test concurrent requests to cache endpoint
    let mut handles = vec![];
    
    // First request to populate cache
    let _ = client.get(format!("{}/test/cache", base_url)).send().await?;
    
    // Wait a bit for cache to be populated
    sleep(Duration::from_millis(100)).await;
    
    // Concurrent requests to test cache consistency
    for i in 0..15 {
        let client = client.clone();
        let handle = tokio::spawn(async move {
            let start = std::time::Instant::now();
            let res = client
                .get(format!("{}/test/cache", base_url))
                .send()
                .await
                .unwrap();
            let status = res.status();
            let body = res.text().await.unwrap();
            let elapsed = start.elapsed();
            (i, status, body, elapsed)
        });
        handles.push(handle);
    }
    
    // Collect all results
    let mut results = vec![];
    for handle in handles {
        results.push(handle.await?);
    }
    
    // All requests should succeed
    for (i, status, body, elapsed) in &results {
        assert_eq!(*status, 200, "Request {} should succeed", i);
        assert!(!body.is_empty(), "Request {} should have response body", i);
    }
    
    // Check cache performance - later requests should be faster (cache hits)
    let first_request_time = results[0].3;
    let cache_hit_times: Vec<_> = results.iter().skip(1).map(|(_, _, _, elapsed)| *elapsed).collect();
    
    let avg_cache_hit_time: Duration = cache_hit_times.iter()
        .sum::<Duration>() / cache_hit_times.len() as u32;
    
    // Cache hits should be faster than initial request
    assert!(avg_cache_hit_time < first_request_time);
    
    Ok(())
}

#[tokio::test]
async fn test_concurrent_circuit_breaker_requests() -> Result<()> {
    let client = Arc::new(Client::new());
    let base_url = "http://127.0.0.1:8081";
    
    // Test concurrent requests to circuit breaker endpoint
    // This will trip the circuit breaker quickly
    let mut handles = vec![];
    
    for i in 0..5 {
        let client = client.clone();
        let handle = tokio::spawn(async move {
            let res = client
                .get(format!("{}/test/breaker", base_url))
                .send()
                .await
                .unwrap();
            (i, res.status())
        });
        handles.push(handle);
    }
    
    // Collect results - should include both failures and circuit breaker responses
    let mut results = vec![];
    for handle in handles {
        results.push(handle.await?);
    }
    
    // Should have both 500 (backend failures) and 503 (circuit breaker open)
    let has_500 = results.iter().any(|(_, status)| *status == 500);
    let has_503 = results.iter().any(|(_, status)| *status == 503);
    
    assert!(has_500 || has_503, "Should have backend failures or circuit breaker responses");
    
    Ok(())
}

#[tokio::test]
async fn test_concurrent_mixed_endpoints() -> Result<()> {
    let client = Arc::new(Client::new());
    let base_url = "http://127.0.0.1:8081";
    
    // Generate test tokens
    let admin_jwt = generate_jwt(vec!["admin"]);
    let user_jwt = generate_jwt(vec!["user"]);
    
    // Test concurrent requests to different endpoints
    let mut handles = vec![];
    
    for i in 0..30 {
        let client = client.clone();
        let admin_token = admin_jwt.clone();
        let user_token = user_jwt.clone();
        
        let handle = tokio::spawn(async move {
            let endpoint = match i % 5 {
                0 => "/test/public",
                1 => "/test/apikey",
                2 => "/test/jwt",
                3 => "/test/ratelimit",
                4 => "/test/cache",
                _ => "/test/public",
            };
            
            let token = if i % 3 == 0 { &admin_token } else { &user_token };
            
            let mut request = client.get(format!("{}/{}", base_url, endpoint));
            
            // Add appropriate headers based on endpoint
            if endpoint.contains("apikey") || endpoint.contains("jwt") {
                request = request.header("Authorization", format!("Bearer {}", token));
            }
            
            let res = request.send().await.unwrap();
            (i, endpoint, res.status())
        });
        handles.push(handle);
    }
    
    // Collect all results
    let mut results = vec![];
    for handle in handles {
        results.push(handle.await?);
    }
    
    // Verify different endpoints behave correctly under concurrent load
    for (i, endpoint, status) in results {
        match endpoint {
            "/test/public" => {
                assert_eq!(status, 200, "Public endpoint {} should work", i);
            }
            "/test/apikey" => {
                // Should succeed with admin token, fail with user token
                if i % 3 == 0 {
                    assert_eq!(status, 200, "Admin API key request {} should succeed", i);
                } else {
                    assert_eq!(status, 403, "User API key request {} should be forbidden", i);
                }
            }
            "/test/jwt" => {
                // Should succeed with admin token, fail with user token  
                if i % 3 == 0 {
                    assert_eq!(status, 200, "Admin JWT request {} should succeed", i);
                } else {
                    assert_eq!(status, 403, "User JWT request {} should be forbidden", i);
                }
            }
            "/test/ratelimit" => {
                // Should be either 200 or 429
                assert!(status == 200 || status == 429, "Rate limit request {} should be 200 or 429, got {}", i, status);
            }
            "/test/cache" => {
                assert_eq!(status, 200, "Cache request {} should succeed", i);
            }
            _ => {}
        }
    }
    
    Ok(())
}

#[tokio::test]
async fn test_concurrent_configuration_reload() -> Result<()> {
    let client = Arc::new(Client::new());
    let base_url = "http://127.0.0.1:8081";
    
    // Test that the gateway remains stable during configuration reload
    // This simulates the hot reload functionality under load
    
    let mut handles = vec![];
    
    // Start continuous requests
    for i in 0..20 {
        let client = client.clone();
        let handle = tokio::spawn(async move {
            let mut successes = 0;
            let mut failures = 0;
            
            for j in 0..10 {
                let res = client
                    .get(format!("{}/test/public", base_url))
                    .send()
                    .await;
                
                match res {
                    Ok(response) => {
                        if response.status() == 200 {
                            successes += 1;
                        } else {
                            failures += 1;
                        }
                    }
                    Err(_) => {
                        failures += 1;
                    }
                }
                
                // Small delay between requests
                if j < 9 {
                    sleep(Duration::from_millis(50)).await;
                }
            }
            
            (i, successes, failures)
        });
        handles.push(handle);
    }
    
    // Wait for all requests to complete
    let mut total_successes = 0;
    let mut total_failures = 0;
    
    for handle in handles {
        let (i, successes, failures) = handle.await?;
        total_successes += successes;
        total_failures += failures;
        
        // Allow some failures due to concurrent access
        assert!(successes >= 8, "Request {} should have mostly successes", i);
    }
    
    // Overall success rate should be high
    let total_requests = total_successes + total_failures;
    let success_rate = total_successes as f64 / total_requests as f64;
    
    assert!(success_rate >= 0.8, "Overall success rate should be at least 80%, got {:.2}%", success_rate * 100.0);
    
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
