use std::sync::Arc;

use axum::routing::{get, post};
use axum::serve::Serve;
use axum::Router;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use tokio::net::TcpListener;

use crate::configuration::{DatabaseSettings, Settings};
use crate::routes::{health_check, subscribe};

pub struct Application {
    serve: Serve<Router, Router>,
    port: u16,
}

impl Application {
    pub async fn build(configuration: Settings) -> Result<Self, std::io::Error> {
        let connection_pool = get_connection_pool(&configuration.database);

        let address = format!("127.0.0.1:{}", configuration.application_port);
        let tcp_listener = listener(address).await;

        let port = tcp_listener.local_addr().unwrap().port();

        let serve = run(tcp_listener, connection_pool).await?;

        Ok(Self { serve, port })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub async fn start_service(self) -> Result<(), std::io::Error> {
        self.serve.await
    }
}

pub struct ApplicationState {
    pub pool: PgPool,
}

pub async fn run(
    tcp_listener: TcpListener,
    pool: PgPool,
) -> Result<Serve<Router, Router>, std::io::Error> {
    let application_state = Arc::new(ApplicationState { pool });

    let app = Router::new()
        .route("/", get(root))
        .route("/health_check", get(health_check))
        .route("/subscriptions", post(subscribe))
        .with_state(application_state);

    Ok(axum::serve(tcp_listener, app))
}

pub fn get_connection_pool(configuration: &DatabaseSettings) -> PgPool {
    PgPoolOptions::new().connect_lazy_with(configuration.with_db())
}

pub async fn listener(address: String) -> TcpListener {
    TcpListener::bind(address).await.unwrap()
}

pub async fn root() -> &'static str {
    "Hello, world!"
}
