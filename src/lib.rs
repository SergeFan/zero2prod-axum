use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Form, Router};
use tokio::net::TcpListener;

pub fn app() -> Router {
    Router::new()
        .route("/", get(root))
        .route("/health_check", get(health_check))
        .route("/subscriptions", post(subscribe))
}

pub async fn listener() -> TcpListener {
    TcpListener::bind("127.0.0.1:3000").await.unwrap()
}

pub async fn root() -> &'static str {
    "Hello, world!"
}

pub async fn health_check() -> impl IntoResponse {
    StatusCode::OK
}

#[derive(serde::Deserialize)]
struct SubscriberInfo {
    name: String,
    email: String,
}

async fn subscribe(Form(payload): Form<SubscriberInfo>) -> impl IntoResponse {
    println!("{}, {}", payload.name, payload.email);

    StatusCode::OK
}
