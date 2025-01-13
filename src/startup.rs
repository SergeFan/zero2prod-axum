use std::sync::Arc;

use axum::routing::{get, post};
use axum::serve::Serve;
use axum::Router;
use axum_tracing_opentelemetry::middleware::{OtelAxumLayer, OtelInResponseLayer};
use sea_orm::sqlx::postgres::PgPoolOptions;
use sea_orm::{DatabaseConnection, SqlxPostgresConnector};
use tokio::net::TcpListener;

use crate::configuration::{DatabaseSettings, Settings};
use crate::email_client::EmailClient;
use crate::routes::{confirm, health_check, subscribe};

pub struct Application {
    serve: Serve<TcpListener, Router, Router>,
    port: u16,
}

impl Application {
    pub async fn build(configuration: Settings) -> Result<Self, std::io::Error> {
        // Database
        let db_connection = get_database_connection(&configuration.database);

        // Email client
        let sender_email = configuration
            .email_client
            .sender()
            .expect("Invalid sender email address");
        let timeout = configuration.email_client.timeout();
        let email_client = EmailClient::new(
            configuration.email_client.base_url,
            sender_email,
            configuration.email_client.authorization_token,
            timeout,
        );

        // App
        let address = format!(
            "{}:{}",
            configuration.application.host, configuration.application.port
        );
        let tcp_listener = TcpListener::bind(address).await?;
        let port = tcp_listener.local_addr()?.port();

        let serve = run(
            tcp_listener,
            db_connection,
            email_client,
            configuration.application.base_url,
        )
        .await?;

        Ok(Self { serve, port })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub async fn start_service(self) -> std::io::Result<()> {
        self.serve.await
    }
}

pub fn get_database_connection(settings: &DatabaseSettings) -> DatabaseConnection {
    SqlxPostgresConnector::from_sqlx_postgres_pool(
        PgPoolOptions::new().connect_lazy_with(settings.with_db()),
    )
}

pub struct ApplicationState {
    pub db_connection: DatabaseConnection,
    pub email_client: EmailClient,
    pub base_url: String,
}

pub async fn run(
    tcp_listener: TcpListener,
    db_connection: DatabaseConnection,
    email_client: EmailClient,
    base_url: String,
) -> Result<Serve<TcpListener, Router, Router>, std::io::Error> {
    let application_state = Arc::new(ApplicationState {
        db_connection,
        email_client,
        base_url,
    });

    let app = Router::new()
        .route("/", get(root))
        .route("/health_check", get(health_check))
        .route("/subscriptions", post(subscribe))
        .route("/subscriptions/confirm", get(confirm))
        .with_state(application_state)
        .layer(OtelInResponseLayer)
        .layer(OtelAxumLayer::default());

    Ok(axum::serve(tcp_listener, app))
}

pub async fn root() -> &'static str {
    "Hello, world!"
}
