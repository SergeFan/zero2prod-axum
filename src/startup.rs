use std::sync::Arc;

use axum::routing::{get, post};
use axum::serve::Serve;
use axum::Router;
use axum_tracing_opentelemetry::middleware::{OtelAxumLayer, OtelInResponseLayer};
use sea_orm::sqlx::postgres::PgPoolOptions;
use sea_orm::{DatabaseConnection, SqlxPostgresConnector};
use tokio::net::TcpListener;

use crate::configuration::Settings;
use crate::routes::{health_check, subscribe};

pub struct Application {
    serve: Serve<TcpListener, Router, Router>,
    port: u16,
}

impl Application {
    pub async fn build(configuration: Settings) -> Result<Self, std::io::Error> {
        let pg_pool = PgPoolOptions::new().connect_lazy_with(configuration.database.with_db());
        let db_connection = SqlxPostgresConnector::from_sqlx_postgres_pool(pg_pool);

        let address = format!("127.0.0.1:{}", configuration.application_port);
        let tcp_listener = listener(address).await;
        let port = tcp_listener.local_addr()?.port();

        let serve = run(tcp_listener, db_connection).await?;

        Ok(Self { serve, port })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub async fn start_service(self) -> std::io::Result<()> {
        self.serve.await
    }
}

pub struct ApplicationState {
    pub db_connection: DatabaseConnection,
}

pub async fn run(
    tcp_listener: TcpListener,
    db_connection: DatabaseConnection,
) -> Result<Serve<TcpListener, Router, Router>, std::io::Error> {
    let application_state = Arc::new(ApplicationState { db_connection });

    let app = Router::new()
        .route("/", get(root))
        .route("/health_check", get(health_check))
        .route("/subscriptions", post(subscribe))
        .with_state(application_state)
        .layer(OtelInResponseLayer)
        .layer(OtelAxumLayer::default());

    Ok(axum::serve(tcp_listener, app))
}

pub async fn listener(address: String) -> TcpListener {
    TcpListener::bind(address).await.unwrap()
}

pub async fn root() -> &'static str {
    "Hello, world!"
}
