use anyhow::Result;
use futures::future::join_all;
use rustway::run;
use jsonwebtoken::{encode, EncodingKey, Header};
use serde_json::{json, Value};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time::sleep;


// Helper to generate a valid JWT for testing
fn generate_jwt(roles: Vec<&str>) -> String {
    let claims = json!({
        "sub": "test-user",
        "roles": roles,
        "exp": SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() + 3600,
    });
    let secret = "a-very-long-and-random-string-that-is-hard-to-guess";
    encode(&Header::default(), &claims, &EncodingKey::from_secret(secret.as_ref())).unwrap()
}

#[tokio::test]
async fn test_all_gateway_features() -> Result<()> {
    // --- 1. SETUP ---
    let mut server_process = Command::new(env!("CARGO_BIN_EXE_test_server"))
    .spawn()
    .expect("Failed to spawn simple_http_server process");

    tokio::time::sleep(Duration::from_secs(1)).await; // Give server time to start
    println!("Test: Server process spawned.");
    
    // Spawn gateway in a separate task
    let gateway_task = tokio::spawn(run("gateway.yaml".into()));
    sleep(Duration::from_secs(2)).await;



    let client = reqwest::Client::new();
    let base_url = "http://127.0.0.1:8081";

    // --- 2. AUTHENTICATION TESTS ---
    println!("\n--- TESTING: Authentication ---");
    // API Key - Success
    let res = client.get(format!("{}/test/apikey", base_url)).header("Authorization", "Bearer user-key-for-alice").send().await?;
    assert_eq!(res.status(), 200);
    println!("API Key Auth: Success -> OK");
    // JWT - Failure (wrong role)
    let user_jwt = generate_jwt(vec!["user"]);
    let res = client.get(format!("{}/test/jwt", base_url)).header("Authorization", format!("Bearer {}", user_jwt)).send().await?;
    assert_eq!(res.status(), 403); // Forbidden
    println!("JWT Auth: Wrong Role -> OK");
    // JWT - Success
    let admin_jwt = generate_jwt(vec!["admin"]);
    let res = client.get(format!("{}/test/jwt", base_url)).header("Authorization", format!("Bearer {}", admin_jwt)).send().await?;
    assert_eq!(res.status(), 200);
    println!("JWT Auth: Success -> OK");

    // --- 3. RATE LIMITING TEST ---
    println!("\n--- TESTING: Rate Limiting (3 req/min) ---");
    let mut tasks = Vec::new();
    for _i in 0..4 {
        let client = client.clone();
        tasks.push(tokio::spawn(async move {
            client.get(format!("{}/test/ratelimit", base_url)).send().await
        }));
    }
    let responses = join_all(tasks).await;
    let status_codes: Vec<_> = responses.into_iter().map(|r| r.unwrap().unwrap().status()).collect();
    assert_eq!(status_codes.iter().filter(|&&s| s == 200).count(), 3);
    assert_eq!(status_codes.iter().filter(|&&s| s == 429).count(), 1);
    println!("Rate Limiting: 3 success, 1 throttled -> OK");

    // --- 4. CACHING TEST ---
    println!("\n--- TESTING: Caching (5s TTL) ---");
    // Cache Miss
    let res1: Value = client.get(format!("{}/test/cache", base_url)).send().await?.json().await?;
    println!("Cache: First request (MISS) -> OK");
    // Cache Hit
    let res2: Value = client.get(format!("{}/test/cache", base_url)).send().await?.json().await?;
    assert_eq!(res1, res2);
    println!("Cache: Second request (HIT) -> OK");
    // Cache Stale
    sleep(Duration::from_secs(6)).await;
    let res3: Value = client.get(format!("{}/test/cache", base_url)).send().await?.json().await?;
    assert_ne!(res1, res3);
    println!("Cache: Third request after 6s (STALE) -> OK");

    // --- 5. CIRCUIT BREAKER TEST ---
    println!("\n--- TESTING: Circuit Breaker (2 failures, 5s open) ---");
    // Trip the circuit (backend starts as failing)
    assert_eq!(client.get(format!("{}/test/breaker", base_url)).send().await?.status(), 500); // internal server error
    assert_eq!(client.get(format!("{}/test/breaker", base_url)).send().await?.status(), 500);
    println!("Circuit Breaker: Tripped after 2 failures -> OK");
    // Test Open state (fail fast)
    assert_eq!(client.get(format!("{}/test/breaker", base_url)).send().await?.status(), 503); // Service Unavailable
    println!("Circuit Breaker: Open state (fail fast) -> OK");
    // Recover
    sleep(Duration::from_secs(6)).await;
    client.post("http://127.0.0.1:8000/control/healthy").send().await?; // Make backend healthy
    assert_eq!(client.get(format!("{}/test/breaker", base_url)).send().await?.status(), 200); // Half-Open success 1
    assert_eq!(client.get(format!("{}/test/breaker", base_url)).send().await?.status(), 200); // Half-Open success 2 -> Closed
    assert_eq!(client.get(format!("{}/test/breaker", base_url)).send().await?.status(), 200); // Now closed
    println!("Circuit Breaker: Recovery -> OK");

    // --- 6. SHUTDOWN ---
    gateway_task.abort();

    server_process
        .kill()
        .expect("Failed to kill server process");

    println!("\n--- HTTP Server with SSE Test completed successfully. ---");

    Ok(())
}