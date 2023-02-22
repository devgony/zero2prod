#[test]
fn test() {
    // Arrange
    spawn_app().expect("Failed to spawn our App.");
    let client = reqwest::Client::new();

    // Act
    let response = client
        .get("http://127.0.0.1:8000/health_check")
        .send()
        .await
        .expect("Failed to execute request.");

    // Assert
    assert!(response.status().is_success());
    assert_eq!(Some(0), response.content_length());
}

fn spawn_app() -> std::io::Result<()> {
    todo!();
}
