use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use entity::prelude::{SubscriptionTokens, Subscriptions};
use sea_orm::ActiveValue::Set;
use sea_orm::{ActiveModelTrait, DatabaseConnection, DbErr, EntityTrait, IntoActiveModel};
use serde::Deserialize;
use uuid::Uuid;

use crate::startup::ApplicationState;

#[derive(Deserialize)]
pub struct Parameters {
    subscription_token: String,
}

#[tracing::instrument(name = "Confirm a pending subscriber", skip(state, parameters))]
pub async fn confirm(
    State(state): State<Arc<ApplicationState>>,
    Query(parameters): Query<Parameters>,
) -> Response {
    println!("{}", parameters.subscription_token);

    let subscriber_id =
        match get_subscriber_id_from_token(&state.db_connection, &parameters.subscription_token)
            .await
        {
            Ok(id) => id,
            Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        };

    match subscriber_id {
        None => StatusCode::UNAUTHORIZED.into_response(),
        Some(subscriber_id) => {
            if confirm_subscriber(&state.db_connection, subscriber_id)
                .await
                .is_err()
            {
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }

            StatusCode::OK.into_response()
        }
    }
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
        .await
        .expect("Failed to fetch data");

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
