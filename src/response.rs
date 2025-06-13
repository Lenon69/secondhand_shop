use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use maud::Markup;
use tokio::fs;

// Nasz nowy, uniwersalny typ odpowiedzi
pub enum AppResponse {
    Full(Html<String>),
    Partial(Markup),
}

// Implementacja, która mówi Axum, jak zamienić AppResponse na odpowiedź HTTP
impl IntoResponse for AppResponse {
    fn into_response(self) -> Response {
        match self {
            AppResponse::Full(html) => html.into_response(),
            AppResponse::Partial(markup) => markup.into_response(),
        }
    }
}

// Funkcja pomocnicza, która wczytuje `index.html` i wstawia treść
pub async fn serve_full_page(content: Markup) -> Result<Html<String>, StatusCode> {
    let shell = fs::read_to_string("static/index.html")
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Wstaw wyrenderowaną treść w miejsce specjalnego "kotwicy"
    // Upewnij się, że Twój kontener #content ma teraz również id="main-content-placeholder"
    let page = shell.replace(
        r#"<div id="main-content-placeholder" class="min-h-[60vh]"></div>"#,
        &content.into_string(),
    );

    Ok(Html(page))
}
