use reqwest::Client;

use crate::helper::spawn_app;

#[tokio::test]
async fn test_health_check_works() {
    // Arrange
    let test_app = spawn_app().await;
    let client = Client::new();

    // Act
    let response = client
        .get(format!("{}/health_check", test_app.address))
        .send()
        .await
        .expect("Failed to execute request");

    // Assert
    assert!(response.status().is_success());
    assert_eq!(Some(0), response.content_length())
}
