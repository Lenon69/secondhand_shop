use axum::body::Body;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use lol_html::{HtmlRewriter, Settings, element};
use maud::Markup;
use tokio::fs;
use tokio_util::bytes::Bytes;

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

/// Asynchronicznie wczytuje i modyfikuje szablon HTML.
/// Wstawia dynamiczną treść i usuwa atrybuty HTMX inicjujące ładowanie,
/// aby zapobiec konfliktom przy renderowaniu po stronie serwera.
async fn serve_full_page(content_markup: Markup) -> Result<Response, AppError> {
    // Wczytaj główny szablon HTML
    let shell_content = match fs::read("static/index.html").await {
        Ok(bytes) => Bytes::from(bytes),
        Err(e) => {
            tracing::error!("Nie można wczytać pliku szablonu static/index.html: {}", e);
            return Err(AppError::InternalServerError(
                "Błąd wczytywania szablonu strony".to_string(),
            ));
        }
    };

    let content_string = content_markup.into_string();
    let mut response_body = Vec::new();

    // Utwórz nowy 'rewriter' HTML
    let mut rewriter = HtmlRewriter::new(
        Settings {
            element_content_handlers: vec![
                // Handler dla naszego placeholdera
                element!("#content", |el| {
                    // Wstawia dynamiczną treść HTML w miejsce placeholdera el.set_inner_content(&content_string, lol_html::html_content::ContentType::Html);
                    // Usuwa atrybuty, aby HTMX nie nadpisał treści po załadowaniu strony
                    el.remove_attribute("hx-trigger");
                    el.remove_attribute("hx-get");
                    Ok(())
                }),
            ],
            ..Settings::default()
        },
        |c: &[u8]| response_body.extend_from_slice(c), // Zapisuje przetworzony HTML
    );

    // Przetwarzaj szablon
    rewriter.write(&shell_content);
    rewriter.end();

    // Zwróć pełną odpowiedź HTTP
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "text/html; charset=utf-8")
        .body(Body::from(response_body))
        .unwrap())
}

// Zmodyfikuj build_response, aby poprawnie obsługiwał nową odpowiedź
pub async fn build_response(
    headers: HeaderMap,
    page_content: Markup,
) -> Result<Response, AppError> {
    if headers.contains_key("HX-Request") {
        // Dla żądań HTMX zwracamy tylko fragment HTML
        Ok(page_content.into_response())
    } else {
        // Dla pełnych odświeżeń strony (F5) budujemy całą stronę
        serve_full_page(page_content).await
    }
}
