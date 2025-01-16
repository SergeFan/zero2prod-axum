use std::sync::Arc;

use anyhow::Context;
use axum::extract::rejection::QueryRejection;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use entity::prelude::{SubscriptionTokens, Subscriptions};
use sea_orm::ActiveValue::Set;
use sea_orm::{ActiveModelTrait, DatabaseConnection, DbErr, EntityTrait, IntoActiveModel};
use serde::Deserialize;
use uuid::Uuid;

use crate::routes::error_chain_fmt;
use crate::startup::ApplicationState;

#[derive(thiserror::Error)]
pub enum ConfirmationError {
    #[error(transparent)]
    QueryRejection(#[from] QueryRejection),
    #[error("{0}")]
    IdNotFoundError(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for ConfirmationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl IntoResponse for ConfirmationError {
    fn into_response(self) -> Response {
        tracing::error!("{:?}", self);

        match self {
            ConfirmationError::QueryRejection(_) => StatusCode::BAD_REQUEST.into_response(),
            ConfirmationError::IdNotFoundError(_) => StatusCode::UNAUTHORIZED.into_response(),
            ConfirmationError::UnexpectedError(_) => {
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        }
    }
}

#[derive(Deserialize)]
pub struct Parameters {
    subscription_token: String,
}

#[tracing::instrument(name = "Confirm a pending subscriber", skip(state, parameters))]
pub async fn confirm(
    State(state): State<Arc<ApplicationState>>,
    Query(parameters): Query<Parameters>,
) -> Result<Response, ConfirmationError> {
    if let Some(subscriber_id) =
        get_subscriber_id_from_token(&state.db_connection, &parameters.subscription_token)
            .await
            .context("Failed to fetch subscriber ID from the database")?
    {
        confirm_subscriber(&state.db_connection, subscriber_id)
            .await
            .context("Failed to complete subscriber confirmation")?
    } else {
        return Err(ConfirmationError::IdNotFoundError(format!(
            "Unauthorized token detected: {}",
            &parameters.subscription_token
        )));
    };

    Ok(StatusCode::OK.into_response())
}

#[tracing::instrument(
    name = "Get subscriber_id from token",
    skip(db_connection, subscription_token)
)]
pub async fn get_subscriber_id_from_token(
    db_connection: &DatabaseConnection,
    subscription_token: &str,
) -> Result<Option<Uuid>, DbErr> {
    let result = SubscriptionTokens::find_by_id(subscription_token)
        .one(db_connection)
        .await?;

    Ok(result.map(|data| data.subscriber_id))
}

#[tracing::instrument(
    name = "Mark subscriber as confirmed",
    skip(db_connection, subscriber_id)
)]
pub async fn confirm_subscriber(
    db_connection: &DatabaseConnection,
    subscriber_id: Uuid,
) -> Result<(), DbErr> {
    let mut subscription = Subscriptions::find_by_id(subscriber_id)
        .one(db_connection)
        .await?
        .unwrap()
        .into_active_model();

    subscription.status = Set("confirmed".to_owned());

    subscription.update(db_connection).await?;

    Ok(())
}
