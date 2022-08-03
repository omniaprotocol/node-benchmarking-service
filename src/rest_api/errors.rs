use actix_web::{error::ResponseError, http::StatusCode, HttpResponse};
use serde::Serialize;
use std::fmt;
use reqwest::Error;
pub use slog::{error, warn, o, Logger};



pub fn log_error(log: Logger) -> impl Fn(AppError) -> AppError {
    move |err| {
        let log = log.new(o!(
            "cause" => err.cause.clone()
        ));
        error!(log, "{}", err.message);
        err
    }
}

pub fn log_warn(log: Logger) -> impl Fn(AppError) -> AppError {
    move |err| {
        let log = log.new(o!(
            "cause" => err.cause.clone()
        ));
        warn!(log, "{}", err.message);
        err
    }
}


#[derive(Debug)]
pub enum AppErrorType {
    NotFoundError,
    InternalServerError,
    NotImplemented,
    BadRequest
}

#[derive(Debug)]
pub struct AppError {
    pub message: String,
    pub cause: Option<String>,
    pub error_type: AppErrorType,
}

impl AppError {
    pub fn message(&self) -> String {
        match &*self {
            AppError {
                message,
                error_type: AppErrorType::NotFoundError,
                ..
            } => message.clone(),
            AppError {
                message,
                cause,
                error_type: AppErrorType::InternalServerError,
            } => {
                let mut msg = String::new();
                msg.push_str(message.as_str());
                if cause.is_some() {
                    msg.push_str(" Cause: "); 
                    msg.push_str(cause.clone().unwrap().as_str()); 
                }
                return msg;
            },
            AppError {
                message,
                cause,
                error_type: AppErrorType::NotImplemented,
            } => {
                let mut msg = String::new();
                msg.push_str(message.as_str());
                if cause.is_some() {
                    msg.push_str(" Cause: "); 
                    msg.push_str(cause.clone().unwrap().as_str()); 
                }
                return msg;
            },
            AppError {
                message,
                cause,
                error_type: AppErrorType::BadRequest,
            } => {
                let mut msg = String::new();
                msg.push_str(message.as_str());
                if cause.is_some() {
                    msg.push_str(" Cause: "); 
                    msg.push_str(cause.clone().unwrap().as_str()); 
                }
                return msg;
            }
        }
    }
}

impl From<Error> for AppError {
    fn from(error: Error) -> AppError {
        AppError {
            message: error.to_string(), 
            cause: None,
            error_type: AppErrorType::InternalServerError
        }
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(f, "{:?}", self)
    }
}

#[derive(Serialize)]
pub struct AppErrorResponse {
    pub error: String
}

impl ResponseError for AppError {
    fn error_response(&self) -> HttpResponse {
        match self.error_type {
            AppErrorType::NotFoundError => {
                HttpResponse::build(StatusCode::NOT_FOUND).json(
                    AppErrorResponse {
                        error: self.message()
                    }
                )
            },
            AppErrorType::InternalServerError => {
                HttpResponse::build(StatusCode::INTERNAL_SERVER_ERROR).json(
                    AppErrorResponse {
                        error: self.message()
                    }
                )
            },
            AppErrorType::NotImplemented => {
                HttpResponse::build(StatusCode::NOT_IMPLEMENTED).json(
                    AppErrorResponse {
                        error: self.message()
                    }
                )
            },
            AppErrorType::BadRequest => {
                HttpResponse::build(StatusCode::BAD_REQUEST).json(
                    AppErrorResponse {
                        error: self.message()
                    }
                )
            }
        }
    }
}



#[cfg(test)]
mod tests {

    use super::{AppError, AppErrorType};


    #[test]
    fn test_default_not_found_error() {
        let app_error = AppError {
            message: "The requested item was not found".to_string(),
            cause: None,
            error_type: AppErrorType::NotFoundError,
        };

        assert_eq!(
            app_error.message(),
            "The requested item was not found".to_string(),
            "Default message should be shown"
        );
    }
}
