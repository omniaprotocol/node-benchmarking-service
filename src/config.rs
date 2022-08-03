use config::ConfigError;
use serde::Deserialize;
use slog::{o, Drain, Logger};
use slog_async;
use slog_term;
use std::{fs::{OpenOptions}, sync::Mutex};

extern crate slog_json;


#[derive(Deserialize)]
pub struct Config {
    pub server_host: String,
    pub server_port: u32,
    pub log_file_path: String,
    pub rust_log: String,
    pub redis_address: String,
    pub redis_port: String,
    pub num_of_workers: u32,
    // 5% of responses are fails, the job fails
    pub fail_percentage_treshold: f64
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        let mut config = config::Config::new();
        config.merge(config::Environment::new())?;
        config.try_into()
    }

    pub fn configure_log(&self) -> Logger {
        // open logging file
        let log_file = OpenOptions::new()
                                        .create(true)
                                        .append(true)
                                        .open(self.log_file_path.clone())
                                        .unwrap();
        // create Logger
        let decorator = slog_term::TermDecorator::new().build();
        // d1 drain to terminal
        let d1 = slog_term::FullFormat::new(decorator).build().fuse();
        // d2 drain to log_file
        let d2 = slog_json::Json::default(log_file).fuse();
        let both = Mutex::new(slog::Duplicate::new(d1, d2)).fuse();
        let both = slog_async::Async::new(both)
            .build()
            .fuse();
        slog::Logger::root(both, o!("version" => env!("CARGO_PKG_VERSION")))
    }
}