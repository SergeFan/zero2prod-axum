use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Form;
use chrono::Utc;
use uuid::Uuid;

use crate::startup::ApplicationState;

#[derive(serde::Deserialize)]
pub struct SubscriberInfo {
    name: String,
    email: String,
}

pub async fn subscribe(
    State(state): State<Arc<ApplicationState>>,
    Form(form): Form<SubscriberInfo>,
) -> Response {
    sqlx::query!(
        r#"
        INSERT INTO
        subscriptions (id, email, name, subscribed_at, status)
        VALUES
        ($1, $2, $3, $4, 'pending_confirmation')
        "#,
        Uuid::new_v4(),
        form.email,
        form.name,
        Utc::now()
    )
    .execute(&state.pool)
    .await
    .expect("Failed to insert a new subscriber to the database");

    StatusCode::OK.into_response()
}
