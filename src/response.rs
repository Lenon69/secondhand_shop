use axum::http::{HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use maud::Markup;
use tokio::fs;

use crate::errors::AppError;

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

    // Używamy teraz naszego nowego, niezawodnego placeholdera w komentarzu
    let page = shell.replace("", &content.into_string());

    Ok(Html(page))
}

pub async fn build_response(
    headers: HeaderMap,
    page_content: Markup,
) -> Result<AppResponse, AppError> {
    if headers.contains_key("HX-Request") {
        Ok(AppResponse::Partial(page_content))
    } else {
        let full_page = serve_full_page(page_content).await.map_err(|_| {
            AppError::InternalServerError("Błąd ładowania szablonu strony".to_string())
        })?;
        Ok(AppResponse::Full(full_page))
    }
}
