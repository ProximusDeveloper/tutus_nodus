use crate::provider::proxy::Proxy;
use crate::provider::ProxyProvider;
use crate::provider::{Network, Provider};
use axum::response::Response;
use axum::{body::Body, extract::Request};
use std::sync::Arc;

pub struct Solana;

impl Solana {
    pub async fn handle_request(
        network: Network,
        provider: Arc<Provider>,
        proxy_provider: Arc<ProxyProvider>,
        req: Request<Body>,
    ) -> Response {
        Proxy::handle_request(network, provider, proxy_provider, req).await
    }
}
