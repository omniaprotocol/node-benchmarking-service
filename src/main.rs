mod rest_api;
mod config;
mod models;
mod redis_workers;

use std::str::FromStr;

use crate::rest_api::{health, handlers};
use crate::config::Config;
use crate::models::AppState;

use dotenv;
use actix_web::{middleware ,App, HttpServer, web};
use slog::{info};
use rsmq_async::{Rsmq, RsmqConnection, RsmqOptions};


#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Config logging & ENV
    dotenv::dotenv().ok();

    let config = Config::from_env().unwrap();
    let log = config.configure_log();
    info!(log, 
        "BENCHMARKING service started at http://{}:{}", 
        config.server_host, 
        config.server_port
    );
    let options = RsmqOptions {
        host: config.redis_address.clone(),
        port: config.redis_port.clone(),
        db: 0,
        realtime: false,
        password: None,
        ns: String::from_str("rsmq").unwrap()
    };
    
    // Redis & RSMQ setup
    let connection = redis::Client::open(format!("redis://{}", config.redis_address)).unwrap().get_async_connection().await.unwrap(); 
    let mut rsmq = Rsmq::new_with_connection(options.clone(), connection);

    rsmq.delete_queue("jobs_q").await;
    rsmq.create_queue("jobs_q", None, None, None).await;
    
    let thread_log = log.clone();
    let mut worker_handlers:Vec<actix_web::rt::task::JoinHandle<()>> = Vec::new();
    for _i in 0..config.num_of_workers {
        worker_handlers.push(actix_web::rt::spawn(redis_workers::worker::start_worker(options.clone(), thread_log.clone(), config.fail_percentage_treshold)));
    }
    let result = HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(AppState{
                log: log.clone(),
                redis_options: options.clone()
            }))
            .wrap(middleware::Logger::default())
            .service(health::health)
            .service(handlers::get_job)
            .service(handlers::new_job)
    })
    .bind(format!("{}:{}", config.server_host, config.server_port))
    .unwrap()
    .run()
    .await;

    for handler in worker_handlers.into_iter() {
        handler.abort();
    }
    return result;
}