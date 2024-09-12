use std::io::Error as IOError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Error sending request to RPC node")]
    RequestRPCError,
    #[error("Error parsing response from RPC node")]
    InitializeProviderError,
}

#[derive(Error, Debug)]
pub enum ProviderError {
    #[error("Error reading node list file")]
    ReadNodeListError(IOError),
    #[error("Error while parsing node list file")]
    ParseNodeListError(IOError),
    #[error("Error while parsing network name")]
    ParseNetworkNameError,
    #[error("Error while initializing provider")]
    InitializeProviderError,
}
