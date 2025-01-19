use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse};
use minijinja::context;
use tower_cookies::Cookies;

use crate::startup::ApplicationState;

pub async fn login_form(
    State(state): State<Arc<ApplicationState>>,
    cookies: Cookies,
) -> impl IntoResponse {
    let template = state
        .env
        .get_template("login.html")
        .expect("Failed to retrieve template.");

    let error_message = match cookies.get("_flash") {
        None => "".to_string(),
        Some(cookie) => format!("<p><i>{}</i></p>", cookie.value()),
    };

    let rendered_html = template
        .render(context! {error_message => error_message})
        .expect("Failed to render template.");

    (StatusCode::OK, Html::from(rendered_html))
}
