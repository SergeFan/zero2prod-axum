use once_cell::sync::Lazy;
use sqlx::{Connection, Executor, PgConnection, PgPool};
use uuid::Uuid;

use zero2prod_axum::configuration::{get_configuration, DatabaseSettings};
use zero2prod_axum::startup::Application;
use zero2prod_axum::telemetry::{get_subscriber, init_subscriber};

static TRACING: Lazy<()> = Lazy::new(|| {
    let default_filter_level = "info".to_string();
    let subscriber_name = "test".to_string();

    if std::env::var("TEST_LOG").is_ok() {
        init_subscriber(get_subscriber(
            subscriber_name,
            default_filter_level,
            std::io::stdout,
        ));
    } else {
        init_subscriber(get_subscriber(
            subscriber_name,
            default_filter_level,
            std::io::sink,
        ));
    }
});

pub async fn spawn_app() -> String {
    // Setup logger for tests, `once_cell::sync::Lazy` ensures that the initialization will be executed only once
    Lazy::force(&TRACING);

    let configuration = {
        let mut configuration = get_configuration().expect("Failed to read configuration");

        // Use a different database for each test case
        configuration.database.database_name = Uuid::new_v4().to_string();

        // Use a random OS port
        configuration.application_port = 0;

        configuration
    };

    // Create and migrate database
    configure_database(&configuration.database).await;

    let application = Application::build(configuration)
        .await
        .expect("Failed to build application");
    let application_port = application.port();

    tokio::spawn(application.start_service());

    format!("http://127.0.0.1:{}", application_port)
}

async fn configure_database(config: &DatabaseSettings) -> PgPool {
    // Create database
    let mut connection = PgConnection::connect_with(&config.without_db())
        .await
        .expect("Failed to connect to Postgres");

    connection
        .execute(format!(r#"CREATE DATABASE "{}";"#, config.database_name).as_str())
        .await
        .expect("Failed to create database");

    // Migrate database
    let connection_pool = PgPool::connect_with(config.with_db())
        .await
        .expect("Failed to connnect to Postgres");

    sqlx::migrate!("./migrations")
        .run(&connection_pool)
        .await
        .expect("Failed to migrate database");

    connection_pool
}

#[tokio::test]
async fn test_health_check_works() {
    // Arrange
    let app_address = spawn_app().await;
    let client = reqwest::Client::new();

    // Act
    let response = client
        .get(format!("{}/health_check", app_address))
        .send()
        .await
        .expect("Failed to execute request");

    // Assert
    assert!(response.status().is_success());
    assert_eq!(Some(0), response.content_length())
}

#[tokio::test]
async fn test_subscribe_returns_200_for_valid_form_data() {
    // Arrange
    let app_address = spawn_app().await;
    let client = reqwest::Client::new();

    // Act
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";
    let response = client
        .post(&format!("{}/subscriptions", &app_address))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .expect("Failed to execute request");

    // Assert
    assert_eq!(200, response.status().as_u16());

    // let saved = sqlx::query!("SELECT email, name, status FROM subscriptions")
    //     .fetch_one(&app.db_pool)
    //     .await
    //     .expect("Failed to fetch saved subscription");
    //
    // assert_eq!(saved.email, "ursula_le_guin@gmail.com");
    // assert_eq!(saved.name, "le guin");
    // assert_eq!(saved.status, "pending_confirmation")
}

#[tokio::test]
async fn test_subscribe_returns_a_422_when_data_is_missing() {
    // Arrange
    let app_address = spawn_app().await;
    let client = reqwest::Client::new();
    let test_cases = vec![
        ("name=le%20guin", "missing the email"),
        ("email=ursula_le_guin%40gmail.com", "missing the name"),
        ("", "missing both name and email"),
    ];

    for (invalid_body, error_message) in test_cases {
        // Act
        let response = client
            .post(&format!("{}/subscriptions", &app_address))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(invalid_body)
            .send()
            .await
            .expect("Failed to execute request.");

        // Assert
        assert_eq!(
            422,
            response.status().as_u16(),
            // Additional customised error message on test failure
            "The API did not fail with 422 Unprocessable Content when the payload was {}.",
            error_message
        );
    }
}
