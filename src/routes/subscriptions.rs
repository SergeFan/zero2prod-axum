use std::fmt::{Debug, Formatter};
use std::sync::Arc;

use anyhow::Context;
use axum::extract::rejection::FormRejection;
use axum::extract::FromRequest;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Form;
use chrono::Utc;
use sqlx::{Executor, Postgres, Transaction};
use uuid::Uuid;

use crate::domain::{NewSubscriber, SubscriberEmail, SubscriberName};
use crate::routes::error_chain_fmt;
use crate::startup::ApplicationState;

#[derive(thiserror::Error)]
pub enum SubscribeError {
    #[error(transparent)]
    FormRejection(#[from] FormRejection),
    #[error("{0}")]
    ValidationError(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl Debug for SubscribeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl IntoResponse for SubscribeError {
    fn into_response(self) -> Response {
        match self {
            SubscribeError::FormRejection(_) | SubscribeError::ValidationError(_) => {
                StatusCode::BAD_REQUEST.into_response()
            }
            SubscribeError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        }
    }
}

#[derive(FromRequest)]
#[from_request(via(Form), rejection(SubscribeError))]
pub struct SubscriberForm<T>(T);

#[derive(serde::Deserialize)]
pub struct SubscriberInfo {
    name: String,
    email: String,
}

impl TryFrom<SubscriberInfo> for NewSubscriber {
    type Error = String;

    fn try_from(value: SubscriberInfo) -> Result<Self, Self::Error> {
        let email = SubscriberEmail::parse(value.email)?;
        let name = SubscriberName::parse(value.name)?;

        Ok(Self { email, name })
    }
}

#[tracing::instrument(
    name = "Adding a new subscriber",
    skip(state, form),
    fields(subscriber_email = %form.email, subscriber_name = %form.name)
)]
pub async fn subscribe(
    State(state): State<Arc<ApplicationState>>,
    SubscriberForm(form): SubscriberForm<SubscriberInfo>,
) -> Result<Response, SubscribeError> {
    let new_subscriber = form.try_into().map_err(SubscribeError::ValidationError)?;

    let mut transaction = state
        .pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;

    let _subscriber_id = insert_subscriber(&mut transaction, &new_subscriber)
        .await
        .context("Failed to insert new subscriber in the database")?;

    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to store a new subscriber")?;

    Ok(StatusCode::OK.into_response())
}

#[tracing::instrument(
    name = "Saving new subscriber details in the database",
    skip(new_subscriber, transaction)
)]
pub async fn insert_subscriber(
    transaction: &mut Transaction<'_, Postgres>,
    new_subscriber: &NewSubscriber,
) -> Result<Uuid, sqlx::Error> {
    let subscriber_id = Uuid::new_v4();

    let query = sqlx::query!(
        r#"
        INSERT INTO subscriptions (id, email, name, subscribed_at, status)
        VALUES ($1, $2, $3, $4, 'pending_confirmation')
        "#,
        subscriber_id,
        new_subscriber.email.as_ref(),
        new_subscriber.name.as_ref(),
        Utc::now()
    );

    let row_affected = transaction.execute(query).await?;
    println!("{} row affected.", row_affected.rows_affected());

    Ok(subscriber_id)
}
