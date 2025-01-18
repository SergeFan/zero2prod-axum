use std::sync::Arc;

use anyhow::Context;
use axum::extract::rejection::JsonRejection;
use axum::extract::{FromRequest, Json, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use base64::Engine;
use entity::prelude::Subscriptions;
use entity::subscriptions;
use sea_orm::{
    ColumnTrait, DatabaseConnection, DerivePartialModel, EntityTrait, FromQueryResult, QueryFilter,
};
use secrecy::SecretString;
use serde::Deserialize;

use crate::authentication::{validate_credentials, AuthError, Credentials};
use crate::domain::SubscriberEmail;
use crate::routes::error_chain_fmt;
use crate::startup::ApplicationState;

#[derive(thiserror::Error)]
pub enum PublishError {
    #[error("Authentication failed.")]
    AuthenticationError(#[source] anyhow::Error),
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
            PublishError::AuthenticationError(_) => {
                let mut response = StatusCode::UNAUTHORIZED.into_response();

                response.headers_mut().insert(
                    header::WWW_AUTHENTICATE,
                    HeaderValue::from_static(r#"Basic realm="publish""#),
                );

                response
            }
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

#[tracing::instrument(
    name = "Publish a newsletter issue",
    skip(header, state, body),
    fields(username=tracing::field::Empty, user_id=tracing::field::Empty)
)]
pub async fn publish_newsletters(
    header: HeaderMap,
    State(state): State<Arc<ApplicationState>>,
    PublishBody(body): PublishBody<BodyData>,
) -> Result<Response, PublishError> {
    let credentials = basic_authentication(&header).map_err(PublishError::AuthenticationError)?;
    tracing::Span::current().record("username", tracing::field::display(&credentials.username));

    let user_id = validate_credentials(credentials, &state.db_connection)
        .await
        .map_err(|e| match e {
            AuthError::InvalidCredentials(_) => PublishError::AuthenticationError(e.into()),
            AuthError::UnexpectedError(_) => PublishError::UnexpectedError(e.into()),
        })?;
    tracing::Span::current().record("user_id", tracing::field::display(&user_id));

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
                    "Skipping a confirmed subscriber. The stored contact details are invalid."
                )
            }
        }
    }

    Ok(StatusCode::OK.into_response())
}

fn basic_authentication(headers: &HeaderMap) -> Result<Credentials, anyhow::Error> {
    let header_value = headers
        .get("Authorization")
        .context("The 'Authorization' header is missing.")?
        .to_str()
        .context("The 'Authorization' header is no a valid UTF-8 string.")?;

    let base64_encoded_segment = header_value
        .strip_prefix("Basic ")
        .context("The authorization scheme is not 'Basic'.")?;

    let decoded_bytes = base64::engine::general_purpose::STANDARD
        .decode(base64_encoded_segment)
        .context("Failed to base64-decode 'Basic' credentials.")?;
    let decoded_credentials = String::from_utf8(decoded_bytes)
        .context("The decoded credential string is not valid UTF-8.")?;

    // Split into 2 segments, using ':' as delimiter
    let mut credentials = decoded_credentials.splitn(2, ':');

    let username = credentials
        .next()
        .ok_or_else(|| anyhow::anyhow!("A username must be provide in 'Basic' auth."))?
        .to_string();
    let password = credentials
        .next()
        .ok_or_else(|| anyhow::anyhow!("A password must be provided in 'Basic' auth."))?;

    Ok(Credentials {
        username,
        password: SecretString::new(Box::from(password)),
    })
}

struct ConfirmedSubscriber {
    email: SubscriberEmail,
}

#[tracing::instrument(name = "Get confirmed subscribers", skip(db_connection))]
async fn get_confirmed_subscribers(
    db_connection: &DatabaseConnection,
) -> Result<Vec<Result<ConfirmedSubscriber, anyhow::Error>>, anyhow::Error> {
    #[derive(DerivePartialModel, FromQueryResult)]
    #[sea_orm(entity = "Subscriptions")]
    struct Row {
        email: String,
    }

    let confirmed_subscribers = Subscriptions::find()
        .filter(subscriptions::Column::Status.eq("confirmed"))
        .into_partial_model::<Row>()
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
