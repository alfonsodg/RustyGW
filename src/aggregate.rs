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

pub async fn aggregate_handler(
    State(state): State<Arc<AppState>>,
    Path(path): Path<String>,
) -> Response {
    let request_path = format!("/{}", path);

    let (sources, route_name) = {
        let config = state.config.read().await;
        match config.find_route_for_path(&request_path) {
            Some(route) if route.aggregate.is_some() => {
                (route.aggregate.clone().unwrap(), route.name.clone())
            }
            _ => {
                return Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .body(Body::from("No aggregate route found"))
                    .unwrap();
            }
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
                Ok(resp) if resp.status().is_success() => {
                    let body = resp.text().await.unwrap_or_default();
                    let value: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
                    (field, value)
                }
                Ok(resp) => {
                    error!(service = %field, status = %resp.status(), "Aggregate source failed");
                    (field, Value::Null)
                }
                Err(e) => {
                    error!(service = %field, "Aggregate source error: {}", e);
                    (field, Value::Null)
                }
            }
        }));
    }

    let mut merged = serde_json::Map::new();
    for handle in handles {
        if let Ok((field, value)) = handle.await {
            merged.insert(field, value);
        }
    }

    let body = serde_json::to_string(&Value::Object(merged)).unwrap_or_default();

    Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/json")
        .body(Body::from(body))
        .unwrap()
}
