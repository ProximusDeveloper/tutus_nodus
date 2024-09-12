use crate::provider::{Network, Provider};
use axum::body::Body;
use axum::http::{HeaderMap, Method, Request, StatusCode};
use axum::response::Response;
use bytes::Bytes;
use futures_util::StreamExt;
use http_body_util::BodyExt;
use log::{debug, error, info, warn};
use reqwest::header::{HeaderValue, HOST};
use reqwest::{Client, Url};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use strum_macros::Display;

const MAX_RETRIES: usize = 5;

#[derive(Debug, Hash, Eq, PartialEq, Clone, Copy)]
pub enum ProxyType {
    Disabled,
    Socks5,
    Random,
}

impl ProxyType {
    pub fn from_str(s: &str) -> Result<Self, ProxyProviderError> {
        match s.to_lowercase().as_str() {
            "disabled" => Ok(ProxyType::Disabled),
            "socks5" => Ok(ProxyType::Socks5),
            "random" => Ok(ProxyType::Random),
            _ => Err(ProxyProviderError::InvalidProxyType),
        }
    }
}

#[derive(Debug)]
pub struct ProxyProvider {
    proxies: HashMap<ProxyType, Vec<String>>,
    indices: HashMap<ProxyType, Arc<AtomicUsize>>,
    pub is_enabled: bool,
}

#[derive(Debug, Display)]
pub enum ProxyProviderError {
    ReadProxyListError(std::io::Error),
    ParseProxyListError(serde_json::Error),
    InvalidProxyType,
}

impl ProxyProvider {
    pub fn new(path: String, is_enabled: bool) -> Result<Self, ProxyProviderError> {
        if !is_enabled {
            return Ok(ProxyProvider {
                proxies: HashMap::new(),
                indices: HashMap::new(),
                is_enabled,
            });
        }

        let contents =
            std::fs::read_to_string(&path).map_err(ProxyProviderError::ReadProxyListError)?;

        let json: Value =
            serde_json::from_str(&contents).map_err(ProxyProviderError::ParseProxyListError)?;

        let mut proxies = HashMap::new();
        let mut indices = HashMap::new();

        if let Value::Object(proxy_types) = json {
            for (proxy_type_str, urls) in proxy_types {
                let proxy_type = ProxyType::from_str(&proxy_type_str)?;
                if let Value::Array(url_list) = urls {
                    let urls: Vec<String> = url_list
                        .into_iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect();

                    if !urls.is_empty() {
                        proxies.insert(proxy_type, urls);
                        indices.insert(proxy_type, Arc::new(AtomicUsize::new(0)));
                    }
                }
            }
        }

        debug!(
            "ProxyProvider initialized with {} proxy types",
            proxies.len()
        );
        debug!("Proxies: {:?}", proxies);

        Ok(ProxyProvider {
            proxies,
            indices,
            is_enabled,
        })
    }

    pub fn get_proxy_url(&self, proxy_type: ProxyType) -> Option<String> {
        match proxy_type {
            ProxyType::Disabled => None,
            _ => {
                if let Some(urls) = self.proxies.get(&proxy_type) {
                    let index = self.indices.get(&proxy_type).unwrap();
                    let current_index = index.fetch_add(1, Ordering::SeqCst) % urls.len();
                    Some(urls[current_index].clone())
                } else {
                    None
                }
            }
        }
    }
}

pub struct Proxy {
    pub proxy_provider: Arc<ProxyProvider>,
    current_proxy_url: Option<String>,
}

impl Proxy {
    pub fn new(proxy_provider: Arc<ProxyProvider>) -> Self {
        Self {
            proxy_provider,
            current_proxy_url: None,
        }
    }

    pub async fn handle_request(
        network: Network,
        provider: Arc<Provider>,
        proxy_provider: Arc<ProxyProvider>,
        req: Request<Body>,
    ) -> Response {
        let start_time = Instant::now();
        let mut retries = 0;
        let mut proxy = Proxy::new(proxy_provider.clone());

        // Extract necessary data from the original request
        let (parts, body) = req.into_parts();
        let method = parts.method;
        let headers = parts.headers;
        let body_bytes = body.collect().await.unwrap().to_bytes();

        loop {
            let rpc_url = match provider.get_node_url(network).await {
                Some(url) => url,
                None => {
                    error!("Error getting node URL. Network: {:?}", network.to_string());
                    return Self::error_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("Error getting node URL. Network: {:?}", network.to_string()),
                    );
                }
            };
            debug!("RPC URL: {}", rpc_url);

            // Get a new proxy URL if needed
            if proxy.current_proxy_url.is_none() {
                proxy.current_proxy_url = proxy_provider.get_proxy_url(ProxyType::Socks5);
                debug!("Using proxy URL: {:?}", proxy.current_proxy_url);
            }

            let response = proxy
                .send_request(&rpc_url, &method, &headers, &body_bytes)
                .await;

            match response {
                Ok(resp) => {
                    if resp.status() == StatusCode::TOO_MANY_REQUESTS && retries < MAX_RETRIES {
                        retries += 1;
                        warn!(
                            "Received 429 status. Retrying with a new node and proxy. Attempt: {}",
                            retries
                        );
                        proxy.current_proxy_url = None; // Reset proxy URL to get a new one
                        continue;
                    }
                    let duration = start_time.elapsed();
                    info!(
                        "{} request finished in {:?}. Status: {}",
                        network.to_string(),
                        duration,
                        resp.status()
                    );
                    return resp;
                }
                Err(e) => {
                    if retries < MAX_RETRIES {
                        retries += 1;
                        error!(
                            "Error sending request: {:?}. Retrying with a new node and proxy. Attempt: {}",
                            e, retries
                        );
                        proxy.current_proxy_url = None; // Reset proxy URL to get a new one
                        continue;
                    }
                    error!("Max retries reached. Error: {:?}", e);
                    return Self::error_response(StatusCode::BAD_GATEWAY, format!("Error: {}", e));
                }
            }
        }
    }

    async fn send_request(
        &self,
        rpc_url: &str,
        method: &Method,
        headers: &HeaderMap,
        body: &Bytes,
    ) -> Result<Response, reqwest::Error> {
        let mut client_builder = Client::builder()
            .timeout(Duration::from_secs(15))
            .connect_timeout(Duration::from_secs(10));

        // Add proxy if configured
        if let Some(proxy_url) = &self.current_proxy_url {
            client_builder = client_builder.proxy(reqwest::Proxy::all(proxy_url)?);
        }

        let http_client = client_builder.build()?;

        let mut request_headers = headers.clone();
        request_headers.remove(HOST);

        let url = Url::parse(rpc_url).unwrap();
        let host = url.host_str().unwrap();
        request_headers.insert(HOST, HeaderValue::from_str(host).unwrap());

        debug!("Request headers: {:?}", request_headers);
        debug!("Request body length: {} bytes", body.len());

        let reqwest_response = http_client
            .request(method.clone(), rpc_url)
            .headers(request_headers)
            .body(body.clone())
            .send()
            .await?;

        debug!("Received response: {:?}", reqwest_response);

        let status = reqwest_response.status();
        let headers = reqwest_response.headers().clone();

        let stream = reqwest_response.bytes_stream().map(|result| {
            result.map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))
        });

        let body = Body::from_stream(stream);

        debug!("Response status: {:?}", status);
        debug!("Response headers: {:?}", headers);

        let mut axum_response = Response::builder().status(status).body(body).unwrap();
        *axum_response.headers_mut() = headers;

        Ok(axum_response)
    }

    fn error_response(status: StatusCode, message: String) -> Response {
        Response::builder()
            .status(status)
            .body(Body::from(message))
            .unwrap()
    }
}
