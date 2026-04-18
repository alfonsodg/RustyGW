use axum::{
    body::Body,
    extract::{Path, State},
    response::Response,
};
use http::StatusCode;
use serde_json::Value;
use std::sync::Arc;
use tracing::{error, info};

use crate::{features::health_check::parse_duration, state::AppState};

fn json_response(status: StatusCode, body: impl Into<Body>) -> Response {
    Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .body(body.into())
        .unwrap_or_else(|_| Response::new(Body::empty()))
}

pub async fn aggregate_handler(
    State(state): State<Arc<AppState>>,
    Path(path): Path<String>,
) -> Response {
    let request_path = format!("/{}", path);

    let (sources, route_name) = {
        let config = state.config.read().await;
        match config.find_route_for_path(&request_path) {
            Some(route) if route.aggregate.is_some() => {
                // Safe: we just checked is_some()
                (route.aggregate.clone().unwrap_or_default(), route.name.clone())
            }
            _ => return json_response(StatusCode::NOT_FOUND, "No aggregate route found"),
        }
    };

    info!(route = %route_name, sources = sources.len(), "Aggregating responses");

    let mut handles = Vec::new();
    for source in &sources {
        let client = state.http_client.clone();
        let url = source.path.clone();
        let field = source.field.clone();
        let timeout = source
            .timeout
            .as_ref()
            .map(|t| parse_duration(t))
            .unwrap_or(std::time::Duration::from_secs(5));

        handles.push(tokio::spawn(async move {
            let result = client.get(&url).timeout(timeout).send().await;
            match result {
                std::result::Result::Ok(resp) if resp.status().is_success() => {
                    let body = resp.text().await.unwrap_or_default();
                    let value: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
                    (field, value)
                }
                std::result::Result::Ok(resp) => {
                    error!(service = %field, status = %resp.status(), "Aggregate source failed");
                    (field, Value::Null)
                }
                std::result::Result::Err(e) => {
                    error!(service = %field, "Aggregate source error: {}", e);
                    (field, Value::Null)
                }
            }
        }));
    }

    let mut merged = serde_json::Map::new();
    for handle in handles {
        if let std::result::Result::Ok((field, value)) = handle.await {
            merged.insert(field, value);
        }
    }

    let body = serde_json::to_string(&Value::Object(merged)).unwrap_or_default();
    json_response(StatusCode::OK, body)
}
