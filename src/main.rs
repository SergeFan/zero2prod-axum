use zero2prod_axum::configuration::get_configuration;
use zero2prod_axum::startup::Application;
use zero2prod_axum::telemetry::{get_subscriber, init_subscriber};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    // Setup logger
    let tracing_subscriber =
        get_subscriber("zero2prod-axum".into(), "info".into(), std::io::stdout);
    init_subscriber(tracing_subscriber);

    let configuration = get_configuration().expect("Failed to read configuration");

    let application = Application::build(configuration).await?;

    application.start_service().await
}
