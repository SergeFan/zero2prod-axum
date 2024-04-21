use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Form;

#[derive(serde::Deserialize)]
pub struct SubscriberInfo {
    name: String,
    email: String,
}

pub async fn subscribe(Form(payload): Form<SubscriberInfo>) -> impl IntoResponse {
    println!("{}, {}", payload.name, payload.email);

    StatusCode::OK
}
