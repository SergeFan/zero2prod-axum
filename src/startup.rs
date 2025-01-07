use axum::body::Body;
use axum::http::Request;
use axum::routing::{get, post};
use axum::serve::Serve;
use axum::Router;
use sea_orm::sqlx::postgres::PgPoolOptions;
use sea_orm::sqlx::PgPool;
use sea_orm::{Database, DatabaseConnection, SqlxPostgresConnector};
use std::sync::Arc;
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use tower_request_id::{RequestId, RequestIdLayer};
use tracing::info_span;

use crate::configuration::{DatabaseSettings, Settings};
use crate::routes::{health_check, subscribe};

pub struct Application {
    serve: Serve<TcpListener, Router, Router>,
    port: u16,
}

impl Application {
    pub async fn build(configuration: Settings) -> Result<Self, std::io::Error> {
        let pg_pool = get_connection_pool(&configuration.database);
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
        .layer(
            TraceLayer::new_for_http().make_span_with(|request: &Request<Body>| {
                // We get the request id from the extensions
                let request_id = request
                    .extensions()
                    .get::<RequestId>()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "unknown".into());
                // And then we put it along with other information into the `request` span
                info_span!(
                    "request",
                    id = %request_id,
                    method = %request.method(),
                    uri = %request.uri(),
                )
            }),
        )
        .layer(RequestIdLayer);

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
