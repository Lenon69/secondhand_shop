use axum::body::Body;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use lol_html::{HtmlRewriter, Settings, element};
use maud::{Markup, html};
use tokio::fs;
use tokio_util::bytes::Bytes;

use crate::errors::AppError;

// pub enum AppResponse {
//     Full(Html<String>),
//     Partial(Markup),
// }

// Implementacja, która mówi Axum, jak zamienić AppResponse na odpowiedź HTTP
// impl IntoResponse for AppResponse {
//     fn into_response(self) -> Response {
//         match self {
//             AppResponse::Full(html) => html.into_response(),
//             AppResponse::Partial(markup) => markup.into_response(),
//         }
//     }
// }

/// Asynchronicznie wczytuje i modyfikuje szablon HTML.
/// Wstawia dynamiczną treść i usuwa atrybuty HTMX inicjujące ładowanie,
/// aby zapobiec konfliktom przy renderowaniu po stronie serwera.
async fn serve_full_page(content_markup: Markup, title: &str) -> Result<Response, AppError> {
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
    let title_string = title.to_string();
    let mut response_body = Vec::new();

    let mut rewriter = HtmlRewriter::new(
        Settings {
            element_content_handlers: vec![
                element!("#content", |el| {
                    el.set_inner_content(
                        &content_string,
                        lol_html::html_content::ContentType::Html,
                    );
                    el.remove_attribute("hx-trigger");
                    el.remove_attribute("hx-get");
                    Ok(())
                }),
                element!("head > title", |el| {
                    el.set_inner_content(&title_string, lol_html::html_content::ContentType::Text);
                    Ok(())
                }),
            ],
            ..Settings::default()
        },
        |c: &[u8]| response_body.extend_from_slice(c),
    );

    // Używamy 'if let Err' do obsługi błędów z rewritera
    if let Err(e) = rewriter.write(&shell_content) {
        tracing::error!("Błąd podczas przetwarzania HTML (write): {}", e);
        return Err(AppError::InternalServerError(
            "Błąd renderowania strony".to_string(),
        ));
    }
    if let Err(e) = rewriter.end() {
        tracing::error!("Błąd podczas przetwarzania HTML (end): {}", e);
        return Err(AppError::InternalServerError(
            "Błąd renderowania strony".to_string(),
        ));
    }

    // Bezpieczne budowanie odpowiedzi bez .unwrap()
    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "text/html; charset=utf-8")
        .body(Body::from(response_body))
        .map_err(|e| {
            // Konwertujemy potencjalny błąd budowania odpowiedzi na nasz AppError
            tracing::error!("Nie udało się zbudować odpowiedzi HTTP: {}", e);
            AppError::InternalServerError("Błąd serwera podczas tworzenia odpowiedzi".to_string())
        })
}

pub async fn build_response(
    headers: HeaderMap,
    page_content: Markup,
    title: &str,
) -> Result<Response, AppError> {
    if headers.contains_key("HX-Request") {
        let oob_title = html! {
            title hx-swap-oob="true" { (title) }
        };

        let final_markup = html! {
            (page_content)
            (oob_title)
        };

        Ok(final_markup.into_response())
    } else {
        serve_full_page(page_content, title).await
    }
}
