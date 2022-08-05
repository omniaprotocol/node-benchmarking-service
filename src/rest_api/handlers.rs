use crate::models::*;
use crate::rest_api::errors::*;

use actix_web::{get, post, web, HttpResponse, Responder, HttpRequest, http::StatusCode};
use redis::{AsyncCommands, RedisError, Client};
use rsmq_async::{Rsmq, RsmqConnection};
use serde_json::json;



#[get("/jobs/{job_id}")]
pub async fn get_job(
    request: HttpRequest,
    state: web::Data<AppState>
) -> Result<impl Responder, AppError> {
    // Get job_id from url
    let job_id = match request.match_info().get("job_id") {
        Some(j) => j,
        None => {
            let sublog = state.log.new(o!(
                "handler" => "get_job",
            ));
            return Err(
                AppError {
                    message: "Job not found".to_string(),
                    cause: Some("job_id header not provided".to_string()),
                    error_type: AppErrorType::NotFoundError
                }
            ).map_err(log_error(sublog));
        }
    };

    // Connect to Redis db
    let redis_client = match Client::open(format!("redis://{}", state.redis_options.host.as_str())) {
        Ok(r) => r,
        Err(e) => {
            let sublog = state.log.new(o!(
                "handler" => "get_job",
            ));
            return Err(AppError {
                message: format!("Failed to open RSMQ client -> Redis connection"),
                cause:Some(e.to_string()),
                error_type:AppErrorType::InternalServerError
            }).map_err(log_error(sublog));
        }
    };
    let mut redis_connection_manager = match redis::aio::ConnectionManager::new(redis_client.clone()).await {
        Ok(r) => r,
        Err(e) => {
            let sublog = state.log.new(o!(
                "handler" => "get_job",
            ));
            return Err(AppError {
                message: format!("Failed to create Redis ConnectionManager"),
                cause:Some(e.to_string()),
                error_type:AppErrorType::InternalServerError
            }).map_err(log_error(sublog));
        }
    };

    // Search by job_id in Redis db
    // {job_id:job_rps}
    let redis_response: Result<i64, RedisError> = redis_connection_manager.get(job_id).await;
    if redis_response.is_err() {
        return Ok(HttpResponse::with_body(StatusCode::NOT_FOUND, String::from("")));
    }
    let job_rps = redis_response.unwrap();
    
    // -1 => job is waiting to be scheduled, 
    // 0 => job is allocated to a redis-worker, 
    // >0 => job has been finished by a worker
    if job_rps == 0 || job_rps == -1 { 
        return Ok(HttpResponse::with_body(StatusCode::OK, serde_json::to_string_pretty(&json!({"status":"PENDING"})).unwrap()));
    }
    // -2 => job's treshold of fails/requests exceeded, so job failed and dropped
    if job_rps == -2 { 
        let res: Result<i32, RedisError> = redis_connection_manager.del(job_id).await;
        if res.is_err() {
            let sublog = state.log.new(o!(
                "handler" => "get_job",
            ));
            error!(sublog, "Failed to delete job {} from Redis", job_id);
        }
        return Ok(HttpResponse::with_body(StatusCode::OK, serde_json::to_string_pretty(&json!({"status":"ERRORED"})).unwrap()));
    }
    let res: Result<i32, RedisError> = redis_connection_manager.del(job_id).await;
    if res.is_err() {
        let sublog = state.log.new(o!(
            "handler" => "get_job",
        ));
        error!(sublog, "Failed to delete job {} from Redis", job_id);
    }
    Ok(HttpResponse::with_body(StatusCode::OK, serde_json::to_string_pretty(&json!({"status":"FINISHED", "rps":job_rps})).unwrap()))
}

#[post("/jobs")]
pub async fn new_job(
    request_body: String,
    state: web::Data<AppState>
) -> Result<impl Responder, AppError> {
    // Check request body corectness
    match parse_request_body(&state, request_body.clone()) {
        Ok(j) => j,
        Err(e) => return Err(e)
    };
    
    // Redis & RSMQ setup
    let redis_client = match Client::open(format!("redis://{}", state.redis_options.host.as_str())) {
        Ok(r) => r,
        Err(e) => {
            let sublog = state.log.new(o!(
                "handler" => "new_job",
            ));
            return Err(AppError {
                message: format!("Failed to open RSMQ client -> Redis connection"),
                cause:Some(e.to_string()),
                error_type:AppErrorType::InternalServerError
            }).map_err(log_error(sublog));
        }
    };
    let mut rsmq = Rsmq::new_with_connection(state.redis_options.clone(), redis_client.get_async_connection().await.unwrap());
    let mut redis_connection_manager = match redis::aio::ConnectionManager::new(redis_client.clone()).await {
        Ok(r) => r,
        Err(e) => {
            let sublog = state.log.new(o!(
                "handler" => "new_job",
            ));
            return Err(AppError {
                message: format!("Failed to create Redis ConnectionManager"),
                cause:Some(e.to_string()),
                error_type:AppErrorType::InternalServerError
            }).map_err(log_error(sublog));
        }
    };

    // Send the new TodoJob through RSMQ to the redis-workers
    let job_id = match rsmq
                                .send_message("jobs_q", request_body.clone(), Some(1))
                                .await {
        Ok(j) => j,
        Err(e) => {
            let sublog = state.log.new(o!(
                "handler" => "new_job",
            ));
            return Err(AppError {
                message: format!("Failed to send job through RSMQ"),
                cause:Some(e.to_string()),
                error_type:AppErrorType::InternalServerError
            }).map_err(log_error(sublog));
        }
    };

    // if the job was sent successfully, mark it as waiting to be scheduled in the Redis db
    let res: Result<String, RedisError> = redis_connection_manager.set(job_id.clone(), -1).await;
    if res.is_err() {
        let sublog = state.log.new(o!(
            "handler" => "new_job",
        ));
        return Err(AppError {
            message: format!("Failed to mark job as new in Redis"),
            cause:Some(res.unwrap()),
            error_type:AppErrorType::InternalServerError
        }).map_err(log_error(sublog));
    }
    Ok(HttpResponse::with_body(StatusCode::CREATED, serde_json::to_string_pretty(&json!({"id":job_id})).unwrap()))
}

fn parse_request_body(
    state: &web::Data<AppState>, 
    request_body: String
) -> Result<TodoJob, AppError> {
    match json::parse(request_body.as_str()) {
        Ok(_) => {
            match serde_json::from_str::<TodoJob>(request_body.as_str()) {
                Ok(todo_job) => {
                    if todo_job.chain.eq("BTC") || todo_job.chain.eq("EVM") {
                        return Ok(todo_job);       
                    }
                    let sublog = state.log.new(o!(
                        "handler" => "new_job > parse_request_body",
                    ));
                    return Err(AppError {
                        message: format!("Unsupported chain"),
                        cause:Some(format!("Chain field provided: {}", todo_job.chain)),
                        error_type:AppErrorType::BadRequest
                    }).map_err(log_error(sublog));
                },
                Err(e) => {
                    let sublog = state.log.new(o!(
                        "handler" => "new_job > parse_request_body",
                    ));
                    return Err(AppError {
                        message: format!("Failed to deserialize TodoJob object"),
                        cause:Some(e.to_string()),
                        error_type:AppErrorType::BadRequest
                    }).map_err(log_error(sublog));
                }
            }
        },
        Err(e) => {
            let sublog = state.log.new(o!(
                "handler" => "new_job > parse_request_body",
            ));
            return Err(AppError {
                message: format!("Failed to parse request body"),
                cause:Some(e.to_string()),
                error_type:AppErrorType::BadRequest
            }).map_err(log_error(sublog));
        }
    }
}