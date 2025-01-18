use argon2::password_hash::SaltString;
use argon2::{Algorithm, Argon2, Params, PasswordHasher, Version};
use entity::users;
use linkify::{LinkFinder, LinkKind};
use migration::{Migrator, MigratorTrait};
use reqwest::{Client, Response, Url};
use sea_orm::sqlx::{Connection, Executor, PgConnection};
use sea_orm::ActiveValue::Set;
use sea_orm::{ActiveModelTrait, DatabaseConnection};
use std::sync::LazyLock;
use uuid::Uuid;
use wiremock::MockServer;

use zero2prod_axum::configuration::{get_configuration, DatabaseSettings};
use zero2prod_axum::startup::{get_database_connection, Application};
use zero2prod_axum::telemetry::{get_subscriber, init_subscriber};

static TRACING: LazyLock<()> = LazyLock::new(|| {
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

pub struct TestUser {
    pub user_id: Uuid,
    pub username: String,
    pub password: String,
}

impl TestUser {
    fn generate() -> Self {
        Self {
            user_id: Uuid::new_v4(),
            username: Uuid::new_v4().to_string(),
            password: Uuid::new_v4().to_string(),
        }
    }

    async fn store(&self, db_connection: &DatabaseConnection) {
        let salt = SaltString::generate(&mut rand::thread_rng());

        // Match parameters of the default password
        let password_hash = Argon2::new(
            Algorithm::Argon2id,
            Version::V0x13,
            Params::new(15_000, 2, 1, None).unwrap(),
        )
        .hash_password(self.password.as_bytes(), &salt)
        .unwrap()
        .to_string();

        users::ActiveModel {
            user_id: Set(self.user_id.to_owned()),
            user_name: Set(self.username.to_owned()),
            password_hash: Set(password_hash),
        }
        .insert(db_connection)
        .await
        .expect("Failed to create test users.");
    }
}

pub struct TestApp {
    pub address: String,
    pub port: u16,
    pub db_connection: DatabaseConnection,
    pub email_server: MockServer,
    pub test_user: TestUser,
}

impl TestApp {
    pub async fn post_newsletters(&self, body: serde_json::Value) -> Response {
        Client::new()
            .post(format!("{}/newsletters", self.address))
            .basic_auth(&self.test_user.username, Some(&self.test_user.password))
            .json(&body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn post_subscriptions(&self, body: String) -> Response {
        Client::new()
            .post(format!("{}/subscriptions", &self.address))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub fn get_confirmation_links(&self, email_request: &wiremock::Request) -> ConfirmationLinks {
        let body = serde_json::from_slice::<serde_json::Value>(&email_request.body).unwrap();

        let get_link = |s: &str| {
            let links = LinkFinder::new()
                .links(s)
                .filter(|l| *l.kind() == LinkKind::Url)
                .collect::<Vec<_>>();

            assert_eq!(links.len(), 1);

            let raw_link = links[0].as_str().to_owned();
            let mut confirmation_link = Url::parse(&raw_link).unwrap();

            assert_eq!(confirmation_link.host_str().unwrap(), "127.0.0.1");

            confirmation_link.set_port(Some(self.port)).unwrap();

            confirmation_link
        };

        ConfirmationLinks {
            html: get_link(body["HtmlBody"].as_str().unwrap()),
            plain_text: get_link(body["TextBody"].as_str().unwrap()),
        }
    }
}

pub struct ConfirmationLinks {
    pub html: Url,
    pub plain_text: Url,
}

pub async fn spawn_app() -> TestApp {
    // Setup logger for tests,
    // `once_cell::sync::Lazy` ensures that the initialization will be executed only once
    LazyLock::force(&TRACING);

    // Create mock email server
    let email_server = MockServer::start().await;

    let configuration = {
        let mut configuration = get_configuration().expect("Failed to read configuration");

        // Use a different database for each test case
        configuration.database.database_name = Uuid::new_v4().to_string();

        // Use the mock server as email API
        configuration.email_client.base_url = email_server.uri();

        // Use a random OS port
        configuration.application.port = 0;

        configuration
    };

    // Create and migrate database
    let db_connection = configure_database(&configuration.database).await;

    let application = Application::build(configuration)
        .await
        .expect("Failed to build application");
    let application_port = application.port();

    tokio::spawn(application.start_service());

    let test_app = TestApp {
        address: format!("http://127.0.0.1:{}", application_port),
        port: application_port,
        db_connection,
        email_server,
        test_user: TestUser::generate(),
    };

    test_app.test_user.store(&test_app.db_connection).await;

    test_app
}

async fn configure_database(settings: &DatabaseSettings) -> DatabaseConnection {
    // Create database
    let mut pg_connection = PgConnection::connect_with(&settings.without_db())
        .await
        .expect("Failed to connect to Postgres");

    pg_connection
        .execute(format!(r#"CREATE DATABASE "{}";"#, settings.database_name).as_str())
        .await
        .expect("Failed to create database");

    // Migrate database
    let db_connection = get_database_connection(settings);

    Migrator::up(&db_connection, None)
        .await
        .expect("Failed to migrate database for test.");

    db_connection
}
