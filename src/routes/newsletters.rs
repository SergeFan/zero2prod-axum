use std::sync::Arc;

use anyhow::Context;
use axum::extract::rejection::JsonRejection;
use axum::extract::{FromRequest, Json, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use entity::prelude::Subscriptions;
use entity::subscriptions;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, SelectColumns};
use serde::Deserialize;

use crate::domain::SubscriberEmail;
use crate::routes::error_chain_fmt;
use crate::startup::ApplicationState;

#[derive(thiserror::Error)]
pub enum PublishError {
    #[error(transparent)]
    JsonRejection(#[from] JsonRejection),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
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
            PublishError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
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
    State(state): State<Arc<ApplicationState>>,
    PublishBody(body): PublishBody<BodyData>,
) -> Result<Response, PublishError> {
    let subscribers = get_confirmed_subscribers(&state.db_connection)
        .await
        .context("Failed to get confirmed subscribers")?;

    for subscriber in subscribers {
        match subscriber {
            Ok(subscriber) => {
                state
                    .email_client
                    .send_email(
                        &subscriber.email,
                        &body.title,
                        &body.content.html,
                        &body.content.text,
                    )
                    .await
                    // `with_context` is lazy so it only allocate memory when email delivery fails
                    .with_context(|| {
                        format!("Failed to send newsletter issue to {}", subscriber.email)
                    })?;
            }
            Err(error) => {
                tracing::warn!(
                    error.cause_chain = ?error,
                    "Skipping a confirmed subscriber. The stored contact details are invalid"
                )
            }
        }
    }

    Ok(StatusCode::OK.into_response())
}

struct ConfirmedSubscriber {
    email: SubscriberEmail,
}

#[tracing::instrument(name = "Get confirmed subscribers", skip(db_connection))]
async fn get_confirmed_subscribers(
    db_connection: &DatabaseConnection,
) -> Result<Vec<Result<ConfirmedSubscriber, anyhow::Error>>, anyhow::Error> {
    let confirmed_subscribers = Subscriptions::find()
        .select_column(subscriptions::Column::Email)
        .filter(subscriptions::Column::Status.eq("confirmed"))
        .all(db_connection)
        .await?
        .into_iter()
        .map(|row| match SubscriberEmail::parse(row.email) {
            Ok(email) => Ok(ConfirmedSubscriber { email }),
            Err(error) => Err(anyhow::anyhow!(error)),
        })
        .collect();

    Ok(confirmed_subscribers)
}
