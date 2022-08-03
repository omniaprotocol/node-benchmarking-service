use serde::{Deserialize, Serialize};
use slog;

#[derive(Clone)]
pub struct AppState {
    pub log: slog::Logger,
    pub redis_options: rsmq_async::RsmqOptions
}


#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TodoJob {
    pub chain: String,
    pub endpoint_url: String,
    pub num_threads: u32,
    pub duration: u32,
    pub authorization: Option<String>
}

#[derive(Clone, Debug)]
pub struct JsonRpcMethod {
    pub payload: &'static str,
    pub weight: u32
}