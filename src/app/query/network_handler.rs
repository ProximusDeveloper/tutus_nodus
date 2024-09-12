use crate::provider::ProxyProvider;
use crate::provider::{Network, Provider};
use axum::extract::State;
use axum::response::Response;
use axum::{body::Body, extract::Request, http::StatusCode};
use log::{debug, error};
use std::str::FromStr;
use std::sync::Arc;

pub async fn network_handler(
    State((provider, proxy_provider)): State<(Arc<Provider>, Arc<ProxyProvider>)>,
    req: Request<Body>,
) -> Response {
    let path = req.uri().path();
    let network = path.split('/').last().unwrap_or("");

    match Network::from_str(&network) {
        Ok(network) => {
            debug!("Handling request for network: {:?}", network);
            network.handle_request(provider, proxy_provider, req).await
        }
        Err(_) => {
            error!("Invalid network: {}", network);
            Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Body::from("Invalid network"))
                .unwrap()
        }
    }
}
