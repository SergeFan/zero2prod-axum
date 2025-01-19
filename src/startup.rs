use std::sync::Arc;

use axum::body::Body;
use axum::extract::Request;
use axum::routing::{get, post};
use axum::serve::Serve;
use axum::Router;
use minijinja::AutoEscape;
use sea_orm::sqlx::postgres::PgPoolOptions;
use sea_orm::{DatabaseConnection, SqlxPostgresConnector};
use tokio::net::TcpListener;
use tower_cookies::CookieManagerLayer;
use tower_http::trace::TraceLayer;
use tower_request_id::{RequestId, RequestIdLayer};

use crate::configuration::{DatabaseSettings, Settings};
use crate::email_client::EmailClient;
use crate::routes::{
    confirm, health_check, home, login, login_form, publish_newsletters, subscribe,
};

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

        // Template engine environment
        let mut env = minijinja::Environment::new();
        env.set_loader(minijinja::path_loader("templates"));
        env.set_auto_escape_callback(|_| AutoEscape::None);

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
            env,
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
    pub env: minijinja::Environment<'static>,
}

pub async fn run(
    tcp_listener: TcpListener,
    db_connection: DatabaseConnection,
    email_client: EmailClient,
    base_url: String,
    env: minijinja::Environment<'static>,
) -> Result<Serve<TcpListener, Router, Router>, std::io::Error> {
    let application_state = Arc::new(ApplicationState {
        db_connection,
        email_client,
        base_url,
        env,
    });

    let app = Router::new()
        .route("/", get(home))
        .route("/health_check", get(health_check))
        .route("/login", get(login_form))
        .route("/login", post(login))
        .route("/newsletters", post(publish_newsletters))
        .route("/subscriptions", post(subscribe))
        .route("/subscriptions/confirm", get(confirm))
        .layer(CookieManagerLayer::new())
        .layer(
            TraceLayer::new_for_http().make_span_with(|request: &Request<Body>| {
                // Get the request id from the extensions
                let request_id = request
                    .extensions()
                    .get::<RequestId>()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "unknown".into());

                // Put it along with other information into the `request` span
                tracing::info_span!(
                    "request",
                    id = %request_id,
                    method = %request.method(),
                    uri = %request.uri(),
                )
            }),
        )
        // This layer creates a new id for each request and puts it into the request extensions.
        // Note that it should be added after the Trace layer.
        .layer(RequestIdLayer)
        .with_state(application_state);

    Ok(axum::serve(tcp_listener, app))
}
