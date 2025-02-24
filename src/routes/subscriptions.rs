use std::iter::repeat_with;
use std::sync::Arc;

use anyhow::Context;
use axum::extract::rejection::FormRejection;
use axum::extract::FromRequest;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Form;
use chrono::Utc;
use entity::{subscription_tokens, subscriptions};
use rand::distr::Alphanumeric;
use rand::Rng;
use sea_orm::prelude::DateTimeWithTimeZone;
use sea_orm::ActiveValue::Set;
use sea_orm::{ActiveModelTrait, DatabaseTransaction, DbErr, TransactionTrait};
use uuid::Uuid;

use crate::domain::{NewSubscriber, SubscriberEmail, SubscriberName};
use crate::email_client::EmailClient;
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

impl std::fmt::Debug for SubscribeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl IntoResponse for SubscribeError {
    fn into_response(self) -> Response {
        tracing::error!("{:?}", self);

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

    let transaction = state
        .db_connection
        .begin()
        .await
        .context("Failed to begin a Postgres transaction")?;

    let subscriber_id = insert_subscriber(&transaction, &new_subscriber)
        .await
        .context("Failed to insert new subscriber in the database")?;

    let subscription_token = generate_subscription_token();

    store_token(&transaction, subscriber_id, &subscription_token)
        .await
        .context("Failed to store subscription token in the database")?;

    transaction
        .commit()
        .await
        .context("Failed to commit the Postgres transaction")?;

    // Send confirmation email to the new subscriber
    send_confirmation_email(
        &state.email_client,
        new_subscriber,
        &state.base_url,
        &subscription_token,
    )
    .await
    .context("Failed to send a confirmation email")?;

    Ok(StatusCode::OK.into_response())
}

#[tracing::instrument(
    name = "Saving new subscriber details in the database",
    skip(new_subscriber, transaction)
)]
pub async fn insert_subscriber(
    transaction: &DatabaseTransaction,
    new_subscriber: &NewSubscriber,
) -> Result<Uuid, DbErr> {
    let subscription = subscriptions::ActiveModel {
        id: Set(Uuid::new_v4()),
        email: Set(new_subscriber.email.as_ref().to_string()),
        name: Set(new_subscriber.name.as_ref().to_string()),
        subscribed_at: Set(DateTimeWithTimeZone::from(Utc::now())),
        status: Default::default(),
    }
    .insert(transaction)
    .await?;

    Ok(subscription.id)
}

#[tracing::instrument(
    name = "Store subscription token in the database",
    skip(transaction, subscription_token)
)]
pub async fn store_token(
    transaction: &DatabaseTransaction,
    subscriber_id: Uuid,
    subscription_token: &str,
) -> Result<(), DbErr> {
    subscription_tokens::ActiveModel {
        subscriber_id: Set(subscriber_id),
        subscription_token: Set(subscription_token.to_string()),
    }
    .insert(transaction)
    .await?;

    Ok(())
}

#[tracing::instrument(
    name = "Send a confirmation email to the new subscriber",
    skip(email_client, new_subscriber, base_url, subscription_token)
)]
pub async fn send_confirmation_email(
    email_client: &EmailClient,
    new_subscriber: NewSubscriber,
    base_url: &str,
    subscription_token: &str,
) -> Result<(), reqwest::Error> {
    let confirmation_link = format!(
        "{}/subscriptions/confirm?subscription_token={}",
        base_url, subscription_token
    );

    let plain_text_body = format!(
        "Welcome to our newsletter!\nVisit {} to confirm your subscriptions.",
        confirmation_link
    );
    let html_body = format!(
        "Welcome to our newsletter!<br />Click <a href=\"{}\">here</a> to confirm your subscription.",
        confirmation_link
    );

    email_client
        .send_email(
            &new_subscriber.email,
            "Welcome!",
            &html_body,
            &plain_text_body,
        )
        .await
}

fn generate_subscription_token() -> String {
    let mut rng = rand::rng();

    repeat_with(|| rng.sample(Alphanumeric))
        .map(char::from)
        .take(25)
        .collect()
}
