use crate::app::query::{fallback_handler, network_handler};
use crate::provider::Provider;
use crate::provider::ProxyProvider;
use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::IntoResponse,
    routing::get,
    Router,
};
use std::sync::Arc;
use tokio::sync::broadcast::Sender;

pub fn get_router(
    tx: Sender<String>,
    provider: Arc<Provider>,
    proxy_provider: Arc<ProxyProvider>,
) -> Router {
    let router = Router::new().route(
        "/ws",
        get(move |ws: WebSocketUpgrade| ws_handler(ws, tx.clone())),
    );

    let router = generate_network_routes!(router, network_handler);

    router
        .fallback(fallback_handler)
        .with_state((provider, proxy_provider))
}

async fn ws_handler(ws: WebSocketUpgrade, tx: Sender<String>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, tx))
}

async fn handle_socket(mut socket: WebSocket, tx: Sender<String>) {
    let mut rx = tx.subscribe();

    loop {
        tokio::select! {
            // Receive messages from the WebSocket
            Some(Ok(msg)) = socket.recv() => {
                if let Message::Text(text) = msg {
                    tx.send(text).unwrap();
                }
            }
            // Send messages to the WebSocket
            Ok(msg) = rx.recv() => {
                if socket.send(Message::Text(msg)).await.is_err() {
                    break;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::config::Config;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use futures_util::{SinkExt, StreamExt};
    use http_body_util::BodyExt;
    use tokio::sync::broadcast;
    use tokio::time::Duration;
    use tokio::{net::TcpListener, time::timeout};
    use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_get_router() {
        let (tx, _) = broadcast::channel(100);
        let config = Config::load().expect("Failed to load config");
        let provider =
            Arc::new(Provider::new(config.node_list_path).expect("Failed to initialize provider"));
        let proxy_provider =
            Arc::new(ProxyProvider::new(config.proxy_list_path, config.proxy_is_enabled).unwrap());
        let app = get_router(tx, provider, proxy_provider);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/hello")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(&body[..], b"Hello, world!");
    }

    #[tokio::test]
    async fn test_websocket_connection() {
        let config = Config::load().expect("Failed to load config");
        let (tx, _rx) = tokio::sync::broadcast::channel(100);

        let provider =
            Arc::new(Provider::new(config.node_list_path).expect("Failed to initialize provider"));
        let proxy_provider =
            Arc::new(ProxyProvider::new(config.proxy_list_path, config.proxy_is_enabled).unwrap());
        let app = get_router(tx, provider, proxy_provider);

        let listener = TcpListener::bind(config.http_server_address).await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let url = format!("ws://{}/ws", addr);
        let (ws_stream, _) = connect_async(url)
            .await
            .expect("Failed to connect to websocket");
        let (mut write, mut read) = ws_stream.split();

        let test_message = "Hello, WebSocket!";
        write
            .send(Message::Text(test_message.to_string()))
            .await
            .expect("Failed to send message");

        match timeout(Duration::from_secs(5), read.next()).await {
            Ok(Some(Ok(msg))) => {
                assert_eq!(
                    msg,
                    Message::Text(test_message.to_string()),
                    "Unexpected response from server"
                );
                println!(
                    "Test passed: Server echoed the message correctly through broadcast system"
                );
            }
            Ok(Some(Err(e))) => panic!("Error receiving message: {:?}", e),
            Ok(None) => panic!("WebSocket closed unexpectedly"),
            Err(_) => panic!("Timeout waiting for server response"),
        }

        let second_message = "Second test message";
        write
            .send(Message::Text(second_message.to_string()))
            .await
            .expect("Failed to send second message");

        match timeout(Duration::from_secs(5), read.next()).await {
            Ok(Some(Ok(msg))) => {
                assert_eq!(
                    msg,
                    Message::Text(second_message.to_string()),
                    "Unexpected response for second message"
                );
                println!("Test passed: Server correctly handled second message");
            }
            Ok(Some(Err(e))) => panic!("Error receiving second message: {:?}", e),
            Ok(None) => panic!("WebSocket closed unexpectedly after second message"),
            Err(_) => panic!("Timeout waiting for server response to second message"),
        }
    }
}
