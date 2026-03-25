use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, State,
    },
    response::Response,
};
use futures::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio_tungstenite::connect_async;
use tracing::{error, info};

use crate::state::AppState;

pub async fn ws_proxy_handler(
    State(state): State<Arc<AppState>>,
    Path(path): Path<String>,
    ws: WebSocketUpgrade,
) -> Response {
    let request_path = format!("/{}", path);

    let destination = {
        let config = state.config.read().await;
        config.find_route_for_path(&request_path).map(|route| {
            let dest_path = request_path.strip_prefix(&route.path).unwrap_or("");
            let destinations = route.all_destinations();
            let healthy = state.health_checker.filter_healthy(&destinations);
            let idx = state.load_balancer.next_index(healthy.len(), &route.load_balance);
            let base = healthy[idx].replace("http://", "ws://").replace("https://", "wss://");
            format!("{}{}", base, dest_path)
        })
    };

    ws.on_upgrade(move |socket| async move {
        match destination {
            Some(url) => proxy_websocket(socket, url).await,
            None => {
                error!("No route found for WebSocket path: {}", request_path);
            }
        }
    })
}

async fn proxy_websocket(client_ws: WebSocket, backend_url: String) {
    info!(backend = %backend_url, "Proxying WebSocket connection");

    let backend = match connect_async(&backend_url).await {
        Ok((ws, _)) => ws,
        Err(e) => {
            error!(backend = %backend_url, "Failed to connect to backend WebSocket: {}", e);
            return;
        }
    };

    let (mut client_tx, mut client_rx) = client_ws.split();
    let (mut backend_tx, mut backend_rx) = backend.split();

    let client_to_backend = async {
        while let Some(Ok(msg)) = client_rx.next().await {
            let tung_msg = match msg {
                Message::Text(t) => tokio_tungstenite::tungstenite::Message::text(t.to_string()),
                Message::Binary(b) => tokio_tungstenite::tungstenite::Message::binary(b.to_vec()),
                Message::Ping(p) => tokio_tungstenite::tungstenite::Message::Ping(p),
                Message::Pong(p) => tokio_tungstenite::tungstenite::Message::Pong(p),
                Message::Close(_) => break,
            };
            if backend_tx.send(tung_msg).await.is_err() {
                break;
            }
        }
    };

    let backend_to_client = async {
        while let Some(Ok(msg)) = backend_rx.next().await {
            let axum_msg = match msg {
                tokio_tungstenite::tungstenite::Message::Text(t) => Message::Text(t.to_string().into()),
                tokio_tungstenite::tungstenite::Message::Binary(b) => Message::Binary(b),
                tokio_tungstenite::tungstenite::Message::Ping(p) => Message::Ping(p),
                tokio_tungstenite::tungstenite::Message::Pong(p) => Message::Pong(p),
                tokio_tungstenite::tungstenite::Message::Close(_) => break,
                _ => continue,
            };
            if client_tx.send(axum_msg).await.is_err() {
                break;
            }
        }
    };

    tokio::select! {
        _ = client_to_backend => {},
        _ = backend_to_client => {},
    }

    info!(backend = %backend_url, "WebSocket connection closed");
}
