use anyhow::Result;
use reqwest::Client;
use std::time::Duration;
use tokio::time::sleep;

#[tokio::test]
async fn test_invalid_jwt_token() -> Result<()> {
    let client = Client::new();
    let base_url = "http://127.0.0.1:8081";
    
    // Test with completely invalid JWT
    let res = client
        .get(format!("{}/test/jwt", base_url))
        .header("Authorization", "Bearer invalid.jwt.token")
        .send()
        .await?;
    
    assert_eq!(res.status(), 401);
    let body = res.text().await?;
    assert!(body.contains("Authentication failed"));
    
    Ok(())
}

#[tokio::test]
async fn test_malformed_authorization_header() -> Result<()> {
    let client = Client::new();
    let base_url = "http://127.0.0.1:8081";
    
    // Test with malformed Authorization header
    let res = client
        .get(format!("{}/test/jwt", base_url))
        .header("Authorization", "InvalidFormat token")
        .send()
        .await?;
    
    assert_eq!(res.status(), 401);
    let body = res.text().await?;
    assert!(body.contains("Invalid"));
    
    Ok(())
}

#[tokio::test]
async fn test_missing_authorization_header() -> Result<()> {
    let client = Client::new();
    let base_url = "http://127.0.0.1:8081";
    
    // Test without Authorization header
    let res = client
        .get(format!("{}/test/jwt", base_url))
        .send()
        .await?;
    
    assert_eq!(res.status(), 401);
    let body = res.text().await?;
    assert!(body.contains("Missing"));
    
    Ok(())
}

#[tokio::test]
async fn test_expired_jwt_token() -> Result<()> {
    use jsonwebtoken::{encode, Header, EncodingKey};
    use serde_json::json;
    use std::time::{SystemTime, UNIX_EPOCH};
    
    let client = Client::new();
    let base_url = "http://127.0.0.1:8081";
    
    // Create JWT with expired timestamp
    let expired_timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_secs() - 3600; // 1 hour ago
    
    let claims = json!({
        "sub": "test-user",
        "roles": ["admin"],
        "exp": expired_timestamp,
    });
    
    let secret = "super-secret-jwt-key-for-testing-only";
    let token = encode(&Header::default(), &claims, &EncodingKey::from_secret(secret.as_ref()))?;
    
    let res = client
        .get(format!("{}/test/jwt", base_url))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await?;
    
    assert_eq!(res.status(), 401);
    let body = res.text().await?;
    assert!(body.contains("expired") || body.contains("Token"));
    
    Ok(())
}

#[tokio::test]
async fn test_nonexistent_route() -> Result<()> {
    let client = Client::new();
    let base_url = "http://127.0.0.1:8081";
    
    let res = client
        .get(format!("{}/nonexistent/route", base_url))
        .send()
        .await?;
    
    assert_eq!(res.status(), 404);
    
    Ok(())
}

#[tokio::test]
async fn test_invalid_api_key() -> Result<()> {
    let client = Client::new();
    let base_url = "http://127.0.0.1:8081";
    
    let res = client
        .get(format!("{}/test/apikey", base_url))
        .header("Authorization", "Bearer invalid-api-key")
        .send()
        .await?;
    
    assert_eq!(res.status(), 401);
    let body = res.text().await?;
    assert!(body.contains("Authentication failed"));
    
    Ok(())
}

#[tokio::test]
async fn test_oversized_request() -> Result<()> {
    let client = Client::new();
    let base_url = "http://127.0.0.1:8081";
    
    // Create a large payload that exceeds the 10MB limit
    let large_data = "x".repeat(11 * 1024 * 1024); // 11MB
    
    let res = client
        .post(format!("{}/test/public", base_url))
        .body(large_data)
        .send()
        .await?;
    
    // Should be rejected due to size limits
    assert!(res.status() == 413 || res.status() == 400);
    
    Ok(())
}

#[tokio::test]
async fn test_sql_injection_attempt() -> Result<()> {
    let client = Client::new();
    let base_url = "http://127.0.0.1:8081";
    
    // Test SQL injection attempts in various parameters
    let sql_injection_payloads = vec![
        "'; DROP TABLE users; --",
        "1' OR '1'='1",
        "admin'--",
        "' UNION SELECT * FROM users--",
    ];
    
    for payload in sql_injection_payloads {
        let res = client
            .get(format!("{}/test/public?user={}", base_url, payload))
            .send()
            .await?;
        
        // Should not execute the SQL, just return the backend response
        // The gateway should pass through the parameter without modification
        assert!(res.status() == 200 || res.status() == 404);
    }
    
    Ok(())
}

#[tokio::test]
async fn test_xss_attempt() -> Result<()> {
    let client = Client::new();
    let base_url = "http://127.0.0.1:8081";
    
    // Test XSS attempts in parameters
    let xss_payloads = vec![
        "<script>alert('xss')</script>",
        "javascript:alert('xss')",
        "<img src=x onerror=alert('xss')>",
        "';alert('xss');//",
    ];
    
    for payload in xss_payloads {
        let res = client
            .get(format!("{}/test/public?name={}", base_url, payload))
            .send()
            .await?;
        
        // Should pass through without executing the script
        assert!(res.status() == 200 || res.status() == 404);
    }
    
    Ok(())
}

#[tokio::test]
async fn test_path_traversal_attempt() -> Result<()> {
    let client = Client::new();
    let base_url = "http://127.0.0.1:8081";
    
    // Test path traversal attempts
    let traversal_payloads = vec![
        "../../../etc/passwd",
        "..\\..\\..\\windows\\system32\\drivers\\etc\\hosts",
        "....//....//....//etc//passwd",
        "%2e%2e%2f%2e%2e%2f%2e%2e%2fetc%2fpasswd",
    ];
    
    for payload in traversal_payloads {
        let res = client
            .get(format!("{}/test/public/{}", base_url, payload))
            .send()
            .await?;
        
        // Should return 404 or pass through, not expose system files
        assert!(res.status() == 404 || res.status() == 200);
    }
    
    Ok(())
}

#[tokio::test]
async fn test_malformed_json_request() -> Result<()> {
    let client = Client::new();
    let base_url = "http://127.0.0.1:8081";
    
    // Test with malformed JSON
    let malformed_json = r#"{"name": "test", "invalid": json}"#;
    
    let res = client
        .post(format!("{}/test/public", base_url))
        .header("Content-Type", "application/json")
        .body(malformed_json)
        .send()
        .await?;
    
    // Should either pass through to backend or return 400
    assert!(res.status() == 200 || res.status() == 400 || res.status() == 502);
    
    Ok(())
}

#[tokio::test]
async fn test_very_long_url() -> Result<()> {
    let client = Client::new();
    let base_url = "http://127.0.0.1:8081";
    
    // Create a very long URL path
    let long_path = "a".repeat(8192); // 8KB path
    
    let res = client
        .get(format!("{}/test/public/{}", base_url, long_path))
        .send()
        .await?;
    
    // Should either handle gracefully or return 414 (URI Too Long)
    assert!(res.status() == 200 || res.status() == 414 || res.status() == 404);
    
    Ok(())
}

#[tokio::test]
async fn test_rate_limit_exceeded() -> Result<()> {
    let client = Client::new();
    let base_url = "http://127.0.0.1:8081";
    
    // Make requests rapidly to exceed rate limit
    let mut responses = Vec::new();
    
    for i in 0..10 {
        let res = client
            .get(format!("{}/test/ratelimit", base_url))
            .send()
            .await?;
        responses.push(res.status());
        
        // Small delay to simulate rapid requests
        if i < 9 {
            sleep(Duration::from_millis(50)).await;
        }
    }
    
    // Should have some 429 (Too Many Requests) responses
    let rate_limited_count = responses.iter().filter(|&&status| status == 429).count();
    assert!(rate_limited_count > 0, "Should have rate-limited requests");
    
    Ok(())
}

#[tokio::test]
async fn test_cache_poisoning_attempt() -> Result<()> {
    let client = Client::new();
    let base_url = "http://127.0.0.1:8081";
    
    // Test cache poisoning attempts via headers
    let malicious_headers = vec![
        ("X-Forwarded-Host", "evil.com"),
        ("Host", "evil.com"),
        ("X-Original-URL", "/admin"),
        ("X-Rewrite-URL", "/admin"),
    ];
    
    for (header_name, header_value) in malicious_headers {
        let res = client
            .get(format!("{}/test/cache", base_url))
            .header(header_name, header_value)
            .send()
            .await?;
        
        // Should not be affected by malicious headers
        assert!(res.status() == 200);
    }
    
    Ok(())
}
