use zero2prod_axum::{app, listener};

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    let app = app();
    let listener = listener().await;

    axum::serve(listener, app).await
}
