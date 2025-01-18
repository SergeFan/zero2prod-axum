use anyhow::Context;
use argon2::{Argon2, PasswordHash, PasswordVerifier};
use entity::prelude::Users;
use entity::users;
use sea_orm::ColumnTrait;
use sea_orm::QueryFilter;
use sea_orm::{DatabaseConnection, EntityTrait};
use secrecy::{ExposeSecret, SecretString};
use uuid::Uuid;

use crate::telemetry::spawn_blocking_with_tracing;

#[derive(thiserror::Error, Debug)]
pub enum AuthError {
    #[error("Invalid credentials.")]
    InvalidCredentials(#[source] anyhow::Error),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

pub struct Credentials {
    pub username: String,
    pub password: SecretString,
}

#[tracing::instrument(name = "Validate credentials", skip(credentials, db_connection))]
pub async fn validate_credentials(
    credentials: Credentials,
    db_connection: &DatabaseConnection,
) -> Result<Uuid, AuthError> {
    let mut user_id = None;
    let mut expected_password_hash = SecretString::new(Box::from(
        "$argon2id$v=19$m=15000,t=2,p=1$gZiV/M1gPc22ElAH/Jh1Hw$CWOrkoo7oJBQ/iyh7uJ0LO2aLEfrHwTWllSAxT0zRno"
            .to_string(),
    ));

    if let Some((stored_user_id, stored_password_hash)) =
        get_stored_credentials(&credentials.username, db_connection).await?
    {
        user_id = Some(stored_user_id);
        expected_password_hash = stored_password_hash;
    }

    // Password hashing could be heavy, spawn a new task to prevent blocking the async executor
    spawn_blocking_with_tracing(move || {
        verify_password_hash(expected_password_hash, credentials.password)
    })
    .await
    .context("Failed to spawn blocking task.")??;

    // `user_id` will still be `None` unless an exising user has been found
    user_id
        .ok_or_else(|| anyhow::anyhow!("Unknown username."))
        .map_err(AuthError::InvalidCredentials)
}

#[tracing::instrument(name = "Get stored credentials", skip(username, db_connection))]
async fn get_stored_credentials(
    username: &str,
    db_connection: &DatabaseConnection,
) -> Result<Option<(Uuid, SecretString)>, anyhow::Error> {
    let row = Users::find()
        .filter(users::Column::UserName.eq(username))
        .one(db_connection)
        .await
        .context("Failed to retrieve stored credentials.")?
        .map(|row| (row.user_id, SecretString::new(Box::from(row.password_hash))));

    Ok(row)
}

#[tracing::instrument(
    name = "Verify password hash",
    skip(expected_password_hash, password_candidate)
)]
fn verify_password_hash(
    expected_password_hash: SecretString,
    password_candidate: SecretString,
) -> Result<(), AuthError> {
    let expected_password_hash = PasswordHash::new(expected_password_hash.expose_secret())
        .context("Failed to parse hash in PHC string format")?;

    Argon2::default()
        .verify_password(
            password_candidate.expose_secret().as_bytes(),
            &expected_password_hash,
        )
        .context("Invalid password.")
        .map_err(AuthError::InvalidCredentials)
}
