use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;

use crate::state::AppState;

pub async fn metrics_handler(state: State<Arc<AppState>>) -> impl IntoResponse {
    match state.prometheus_handle.as_ref() {
        Some(handle) => (StatusCode::OK, handle.render()),
        None => (StatusCode::SERVICE_UNAVAILABLE, "Metrics disabled".to_string()),
    }
}
