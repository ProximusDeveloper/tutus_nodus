#[macro_use]
mod macros;
pub mod app;
pub mod ports;
pub mod provider;
pub mod utils;

use log::{error, info};
use ports::httpapi::get_router;
use provider::Provider;
use provider::ProxyProvider;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use utils::config::Config;
use utils::logger;

#[tokio::main]
async fn main() {
    logger::setup_logger(log::LevelFilter::Debug);

    let config = Config::load().expect("Failed to load config");

    let provider = Arc::new(match Provider::new(config.node_list_path) {
        Ok(provider) => provider,
        Err(e) => {
            error!("Failed to initialize provider: {}", e);
            panic!("Failed to initialize provider: {}", e);
        }
    });

    let proxy_provider = Arc::new(
        match ProxyProvider::new(config.proxy_list_path, config.proxy_is_enabled) {
            Ok(proxy_provider) => proxy_provider,
            Err(e) => {
                error!("Failed to initialize proxy provider: {}", e);
                panic!("Failed to initialize proxy provider: {}", e);
            }
        },
    );

    let (tx, _rx) = broadcast::channel(100);

    let app = get_router(tx, provider, proxy_provider);

    let listener = TcpListener::bind(&config.http_server_address)
        .await
        .expect("Failed to bind to address");

    info!("Listening on {}", config.http_server_address);

    axum::serve(listener, app)
        .await
        .expect("Failed to start server");
}
