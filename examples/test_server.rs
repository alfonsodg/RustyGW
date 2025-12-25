use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc,
};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::{net::TcpListener, sync::RwLock, time::sleep};
use tracing::info;

#[derive(Clone)]
struct AppState {
    is_failing: Arc<AtomicBool>,
    request_counter: Arc<AtomicU64>,
    latency_ms: Arc<AtomicU64>,
    users: Arc<RwLock<Vec<User>>>,
}

#[derive(Clone, Serialize, Deserialize)]
struct User {
    id: u64,
    name: String,
    email: String,
}

#[derive(Deserialize)]
struct PaginationParams {
    page: Option<u32>,
    limit: Option<u32>,
}

#[derive(Deserialize)]
struct LatencyParams {
    ms: u64,
}

async fn public_handler() -> &'static str { "public ok" }
async fn private_handler() -> &'static str { "private ok" }

async fn cacheable_handler(State(state): State<AppState>) -> Json<Value> {
    let count = state.request_counter.fetch_add(1, Ordering::SeqCst);
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    Json(json!({ "timestamp": now, "request_count": count }))
}

async fn failing_handler(State(state): State<AppState>) -> impl IntoResponse {
    if state.is_failing.load(Ordering::SeqCst) {
        (StatusCode::INTERNAL_SERVER_ERROR, "service is failing")
    } else {
        (StatusCode::OK, "service is healthy")
    }
}

// Simulates variable latency
async fn slow_handler(State(state): State<AppState>) -> Json<Value> {
    let latency = state.latency_ms.load(Ordering::SeqCst);
    if latency > 0 {
        sleep(Duration::from_millis(latency)).await;
    }
    Json(json!({ "latency_ms": latency, "status": "completed" }))
}

// CRUD operations for users
async fn list_users(
    State(state): State<AppState>,
    Query(params): Query<PaginationParams>,
) -> Json<Value> {
    let users = state.users.read().await;
    let page = params.page.unwrap_or(1) as usize;
    let limit = params.limit.unwrap_or(10) as usize;
    let start = (page - 1) * limit;
    let items: Vec<_> = users.iter().skip(start).take(limit).cloned().collect();
    Json(json!({ "data": items, "page": page, "limit": limit, "total": users.len() }))
}

async fn get_user(State(state): State<AppState>, Path(id): Path<u64>) -> impl IntoResponse {
    let users = state.users.read().await;
    match users.iter().find(|u| u.id == id) {
        Some(user) => (StatusCode::OK, Json(json!(user))).into_response(),
        None => (StatusCode::NOT_FOUND, Json(json!({"error": "User not found"}))).into_response(),
    }
}

async fn create_user(State(state): State<AppState>, Json(mut user): Json<User>) -> impl IntoResponse {
    let mut users = state.users.write().await;
    user.id = users.len() as u64 + 1;
    users.push(user.clone());
    (StatusCode::CREATED, Json(json!(user)))
}

async fn update_user(
    State(state): State<AppState>,
    Path(id): Path<u64>,
    Json(updated): Json<User>,
) -> impl IntoResponse {
    let mut users = state.users.write().await;
    if let Some(user) = users.iter_mut().find(|u| u.id == id) {
        user.name = updated.name;
        user.email = updated.email;
        (StatusCode::OK, Json(json!(user.clone()))).into_response()
    } else {
        (StatusCode::NOT_FOUND, Json(json!({"error": "User not found"}))).into_response()
    }
}

async fn delete_user(State(state): State<AppState>, Path(id): Path<u64>) -> impl IntoResponse {
    let mut users = state.users.write().await;
    let len_before = users.len();
    users.retain(|u| u.id != id);
    if users.len() < len_before {
        StatusCode::NO_CONTENT
    } else {
        StatusCode::NOT_FOUND
    }
}

// Echo headers for debugging
async fn echo_headers(headers: HeaderMap) -> Json<Value> {
    let map: std::collections::HashMap<_, _> = headers
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
        .collect();
    Json(json!(map))
}

// Control endpoints
async fn set_healthy(State(state): State<AppState>) { state.is_failing.store(false, Ordering::SeqCst); }
async fn set_unhealthy(State(state): State<AppState>) { state.is_failing.store(true, Ordering::SeqCst); }
async fn set_latency(State(state): State<AppState>, Query(params): Query<LatencyParams>) {
    state.latency_ms.store(params.ms, Ordering::SeqCst);
}
async fn reset_counter(State(state): State<AppState>) { state.request_counter.store(0, Ordering::SeqCst); }

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().with_env_filter("info").init();
    
    let state = AppState {
        is_failing: Arc::new(AtomicBool::new(true)),
        request_counter: Arc::new(AtomicU64::new(0)),
        latency_ms: Arc::new(AtomicU64::new(0)),
        users: Arc::new(RwLock::new(vec![
            User { id: 1, name: "Alice".into(), email: "alice@example.com".into() },
            User { id: 2, name: "Bob".into(), email: "bob@example.com".into() },
        ])),
    };

    let app = Router::new()
        .route("/public", get(public_handler))
        .route("/private", get(private_handler))
        .route("/cacheable", get(cacheable_handler))
        .route("/failing", get(failing_handler))
        .route("/slow", get(slow_handler))
        .route("/echo", get(echo_headers))
        .route("/users", get(list_users).post(create_user))
        .route("/users/{id}", get(get_user).put(update_user).delete(delete_user))
        .route("/control/healthy", post(set_healthy))
        .route("/control/unhealthy", post(set_unhealthy))
        .route("/control/latency", post(set_latency))
        .route("/control/reset", post(reset_counter))
        .with_state(state);

    let listener = TcpListener::bind("127.0.0.1:8000").await.unwrap();
    info!("Test server listening on http://127.0.0.1:8000");
    info!("Endpoints: /public, /private, /cacheable, /failing, /slow, /echo, /users");
    info!("Control: /control/healthy, /control/unhealthy, /control/latency?ms=N, /control/reset");
    axum::serve(listener, app).await.unwrap();
}