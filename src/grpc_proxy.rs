use axum::{
    body::Body,
    extract::{Path, State},
    response::Response,
};
use http::{HeaderMap, Method, StatusCode, Uri};
use http_body_util::BodyExt;
use hyper_util::{client::legacy::Client, rt::TokioExecutor};
use std::sync::Arc;
use tracing::{error, info};

use crate::state::AppState;

/// Transparent gRPC proxy. Forwards HTTP/2 requests with content-type
/// application/grpc to backend services without protobuf deserialization.
pub async fn grpc_proxy_handler(
    State(state): State<Arc<AppState>>,
    Path(path): Path<String>,
    method: Method,
    headers: HeaderMap,
    body: Body,
) -> Response {
    let request_path = format!("/{}", path);

    let destination = {
        let config = state.config.read().await;
        config.find_route_for_path(&request_path).map(|route| {
            let dest_path = request_path.strip_prefix(&route.path).unwrap_or("");
            let destinations = route.all_destinations();
            let healthy = state.health_checker.filter_healthy(&destinations);
            let idx = state.load_balancer.next_index(healthy.len(), &route.load_balance);
            format!("{}{}", healthy[idx], dest_path)
        })
    };

    let dest_url = match destination {
        Some(url) => url,
        None => {
            return Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::empty())
                .unwrap();
        }
    };

    info!(destination = %dest_url, "Proxying gRPC request");

    let uri: Uri = match dest_url.parse() {
        Ok(u) => u,
        Err(e) => {
            error!("Invalid gRPC destination URI: {}", e);
            return Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .body(Body::empty())
                .unwrap();
        }
    };

    let mut req_builder = http::Request::builder().method(method).uri(&uri);
    for (key, value) in headers.iter() {
        req_builder = req_builder.header(key, value);
    }

    let request = match req_builder.body(body) {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to build gRPC request: {}", e);
            return Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .body(Body::empty())
                .unwrap();
        }
    };

    let client = Client::builder(TokioExecutor::new())
        .http2_only(true)
        .build_http();

    match client.request(request).await {
        Ok(resp) => {
            let (parts, body) = resp.into_parts();
            let bytes = match body.collect().await {
                Ok(b) => b.to_bytes(),
                Err(e) => {
                    error!("Failed to read gRPC response: {}", e);
                    return Response::builder()
                        .status(StatusCode::BAD_GATEWAY)
                        .body(Body::empty())
                        .unwrap();
                }
            };
            let mut response = Response::builder().status(parts.status);
            for (key, value) in parts.headers.iter() {
                response = response.header(key, value);
            }
            response.body(Body::from(bytes)).unwrap()
        }
        Err(e) => {
            error!(destination = %dest_url, "gRPC proxy error: {}", e);
            Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .header("content-type", "application/grpc")
                .header("grpc-status", "14")
                .header("grpc-message", "upstream unavailable")
                .body(Body::empty())
                .unwrap()
        }
    }
}
