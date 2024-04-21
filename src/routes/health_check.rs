use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

pub async fn health_check() -> Response {
    StatusCode::OK.into_response()
}
