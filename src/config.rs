use config::ConfigError;
use serde::Deserialize;
use slog::{o, Drain, Logger};
use slog_async;
use slog_term;

extern crate slog_json;


#[derive(Deserialize)]
pub struct Config {
    // IP Address and Port this service listens on 
    pub server_host: String,
    pub server_port: u32,

    // Verbosity level of logging
    pub rust_log: String,

    // IP Address and Port the Redis db service listens on 
    pub redis_address: String,
    pub redis_port: String,

    // Number of "redis-workers" that handle new jobs
    pub num_of_workers: u32,
    
    // Maximum accepted percentage of failed requests in a job for it to be considered successful
    // e.g. 5% of responses are fails, the job fails
    pub fail_percentage_treshold: f64
}


impl Config {
    // Load ENV Vars from .env file
    pub fn from_env() -> Result<Self, ConfigError> {
        let mut config = config::Config::new();
        config.merge(config::Environment::new())?;
        config.try_into()
    }

    pub fn configure_log(&self) -> Logger {
        // create Logger
        let decorator = slog_term::TermDecorator::new().build();
        // drain to terminal
        let drain = slog_term::FullFormat::new(decorator).build().fuse();
        
        let thread_safe_drain = slog_async::Async::new(drain)
            .build()
            .fuse();
        slog::Logger::root(thread_safe_drain, o!("version" => env!("CARGO_PKG_VERSION")))
    }
}