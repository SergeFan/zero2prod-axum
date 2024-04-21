use zero2prod_axum::configuration::get_configuration;
use zero2prod_axum::startup::Application;

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    let configuration = get_configuration().expect("Failed to read configuration");

    let application = Application::build(configuration).await?;

    application.start_service().await
}
