use crate::app::networks::solana::Solana;
use crate::provider::proxy::Proxy;
use crate::provider::ProxyProvider;
use crate::utils::error::ProviderError;
use axum::response::Response;
use axum::{body::Body, extract::Request};
use log::debug;
use serde_json::Value;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use strum::AsRefStr;
use strum_macros::{Display, EnumIter, EnumString};

#[derive(Debug, Hash, Eq, PartialEq, Clone, Copy, EnumString, Display, EnumIter, AsRefStr)]
#[strum(serialize_all = "kebab-case")]
pub enum Network {
    Solana,
    SolanaDevnet,
    Ethereum,
    BSC,
    BSCTestnet,
}

impl Network {
    pub async fn handle_request(
        self,
        provider: Arc<Provider>,
        proxy_provider: Arc<ProxyProvider>,
        req: Request<Body>,
    ) -> Response {
        match self {
            Network::Solana => Solana::handle_request(self, provider, proxy_provider, req).await,
            _ => Proxy::handle_request(self, provider, proxy_provider, req).await,
        }
    }
}

#[derive(Debug)]
pub struct Provider {
    pub nodes: HashMap<Network, Vec<String>>,
    pub indices: HashMap<Network, Arc<AtomicUsize>>,
}

impl Provider {
    pub fn new(path: String) -> Result<Self, ProviderError> {
        let mut file = match File::open(path) {
            Ok(file) => file,
            Err(e) => return Err(ProviderError::ReadNodeListError(e)),
        };
        let mut contents = String::new();

        match file.read_to_string(&mut contents) {
            Ok(_) => (),
            Err(e) => return Err(ProviderError::ReadNodeListError(e)),
        };

        let json: Value = match serde_json::from_str(&contents) {
            Ok(json) => json,
            Err(e) => return Err(ProviderError::ParseNodeListError(e.into())),
        };

        let mut nodes = HashMap::new();
        let mut indices = HashMap::new();

        if let Value::Object(networks) = json {
            for (network_str, urls) in networks {
                match Network::from_str(&network_str) {
                    Ok(network) => {
                        if let Value::Array(url_list) = urls {
                            let urls: Vec<String> = url_list
                                .into_iter()
                                .filter_map(|v| v.as_str().map(String::from))
                                .collect();

                            if !urls.is_empty() {
                                nodes.insert(network, urls);
                                indices.insert(network, Arc::new(AtomicUsize::new(0)));
                            }
                        }
                    }
                    Err(_) => return Err(ProviderError::ParseNetworkNameError),
                }
            }
        }

        debug!("Provider initialized with {} nodes", nodes.len());
        debug!("Nodes: {:?}", nodes);

        Ok(Provider { nodes, indices })
    }

    pub async fn get_node_url(&self, network: Network) -> Option<String> {
        if let Some(urls) = self.nodes.get(&network) {
            let index = self.indices.get(&network).unwrap();
            let current_index = index.fetch_add(1, Ordering::SeqCst) % urls.len();
            Some(urls[current_index].clone())
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_network_methods() {
        // Test from_str
        assert_eq!(Network::from_str("solana").unwrap(), Network::Solana);
        assert_eq!(
            Network::from_str("solana-devnet").unwrap(),
            Network::SolanaDevnet
        );

        // Test to_string
        assert_eq!(Network::Solana.to_string(), "solana");
        assert_eq!(Network::SolanaDevnet.to_string(), "solana-devnet");
        assert_eq!(Network::BSCTestnet.to_string(), "bsc-testnet");

        // Test as_ref
        assert_eq!(Network::Ethereum.as_ref(), "ethereum");
        assert_eq!(Network::BSC.as_ref(), "bsc");
    }
}
