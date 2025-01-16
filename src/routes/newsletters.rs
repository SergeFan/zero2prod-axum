use axum::extract::rejection::JsonRejection;
use axum::extract::{FromRequest, Json};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;

use crate::routes::error_chain_fmt;

#[derive(thiserror::Error)]
pub enum PublishError {
    #[error(transparent)]
    JsonRejection(#[from] JsonRejection),
}

impl std::fmt::Debug for PublishError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl IntoResponse for PublishError {
    fn into_response(self) -> Response {
        tracing::error!("{:?}", self);

        match self {
            PublishError::JsonRejection(_) => StatusCode::BAD_REQUEST.into_response(),
        }
    }
}

#[derive(FromRequest)]
#[from_request(via(Json), rejection(PublishError))]
pub struct PublishBody<T>(T);

#[derive(Deserialize, Debug)]
pub struct BodyData {
    pub title: String,
    pub content: Content,
}

#[derive(Deserialize, Debug)]
pub struct Content {
    pub html: String,
    pub text: String,
}

pub async fn publish_newsletters(
    PublishBody(_body): PublishBody<BodyData>,
) -> Result<Response, PublishError> {
    Ok(StatusCode::OK.into_response())
}
