use crate::models::{AppState};
use crate::rest_api::errors::*;

use actix_web::{get, web, HttpResponse, Responder};



#[get("/health")]
pub async fn health(
    state: web::Data<AppState>
) -> Result<impl Responder, AppError> {
    Ok(HttpResponse::Ok().finish())
}