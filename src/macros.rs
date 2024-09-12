#[macro_export]
macro_rules! generate_network_routes {
    ($router:expr, $handler:expr) => {{
        use crate::provider::Network;
        use log::debug;
        use strum::IntoEnumIterator;
        let mut router = $router;
        for network in Network::iter() {
            let path = format!("/rpc/{}", network.to_string());
            debug!("Registering network route: {}", path);
            router = router.route(&path, axum::routing::post($handler));
        }
        router
    }};
}
