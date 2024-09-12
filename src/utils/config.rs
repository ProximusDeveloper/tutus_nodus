use config::{Config as Configuration, ConfigError, Environment, File};
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub http_server_address: String,
    pub node_list_path: String,
    pub proxy_is_enabled: bool,
    pub proxy_list_path: String,
}

impl Config {
    pub fn load() -> Result<Self, ConfigError> {
        let builder = Configuration::builder()
            .add_source(File::with_name("./config/config.yaml"))
            .add_source(Environment::with_prefix("APP"))
            .build()?;

        let config: Config = builder.try_deserialize::<Self>()?;

        Ok(config)
    }
}
