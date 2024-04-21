use axum::routing::{get, post};
use axum::serve::Serve;
use axum::Router;
use tokio::net::TcpListener;

use crate::configuration::Settings;
use crate::routes::{health_check, subscribe};

pub struct Application {
    serve: Serve<Router, Router>,
    port: u16,
}

impl Application {
    pub async fn build(configuration: Settings) -> Result<Self, std::io::Error> {
        let address = format!("127.0.0.1:{}", configuration.application_port);
        let tcp_listener = listener(address).await;

        let port = tcp_listener.local_addr().unwrap().port();

        let serve = run(tcp_listener).await?;

        Ok(Self { serve, port })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub async fn start_service(self) -> Result<(), std::io::Error> {
        self.serve.await
    }
}

pub async fn run(tcp_listener: TcpListener) -> Result<Serve<Router, Router>, std::io::Error> {
    let app = app();

    Ok(axum::serve(tcp_listener, app))
}

pub fn app() -> Router {
    Router::new()
        .route("/", get(root))
        .route("/health_check", get(health_check))
        .route("/subscriptions", post(subscribe))
}

pub async fn listener(address: String) -> TcpListener {
    TcpListener::bind(address).await.unwrap()
}

pub async fn root() -> &'static str {
    "Hello, world!"
}
