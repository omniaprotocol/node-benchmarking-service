use serde::{Deserialize, Serialize};
use slog;

#[derive(Clone)]
pub struct AppState {
    pub log: slog::Logger,
    pub redis_options: rsmq_async::RsmqOptions
}


#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TodoJob {
    // Either "EVM" or "BTC"
    pub chain: String,
    // Ex: https://endpoints.omniatech.io/v1/btc/mainnet/test
    pub endpoint_url: String,
    // How many threads to make concurrent requests to the endpoint_url
    pub num_threads: u32,
    pub duration: u32,
    // Auth token for endpoint (can be null/not provided)
    pub authorization: Option<String>
}

#[derive(Clone, Debug)]
pub struct JsonRpcMethod {
    pub payload: &'static str,
    pub weight: u32
}