use axum::body::{Body, Bytes};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use lol_html::{HtmlRewriter, Settings, element};
use maud::{Markup, html};
use reqwest::header;
use sha1::Digest;
use sha1::Sha1;
use tokio::fs;

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
// Zmieniamy zwracany typ na `Result<Vec<u8>, AppError>`
pub async fn serve_full_page(page_builder: PageBuilder<'_>) -> Result<Vec<u8>, AppError> {
    let shell_content = match fs::read("static/index.html").await {
        Ok(bytes) => Bytes::from(bytes),
        Err(e) => {
            tracing::error!("Nie można wczytać pliku szablonu static/index.html: {}", e);
            return Err(AppError::InternalServerError(
                "Błąd wczytywania szablonu strony".to_string(),
            ));
        }
    };

    let PageBuilder {
        title,
        main_content,
        head_scripts,
        body_scripts,
    } = page_builder;

    let content_string = main_content.into_string();
    let mut response_body = Vec::new();

    let mut element_handlers = vec![
        element!("#content", move |el| {
            el.set_inner_content(&content_string, lol_html::html_content::ContentType::Html);
            el.remove_attribute("hx-trigger");
            el.remove_attribute("hx-get");
            Ok(())
        }),
        element!("head > title", |el| {
            el.set_inner_content(title, lol_html::html_content::ContentType::Text);
            Ok(())
        }),
    ];

    if let Some(scripts) = head_scripts {
        let scripts_string = scripts.into_string();
        element_handlers.push(element!("#head-scripts-placeholder", move |el| {
            el.replace(&scripts_string, lol_html::html_content::ContentType::Html);
            Ok(())
        }));
    }

    if let Some(scripts) = body_scripts {
        let scripts_string = scripts.into_string();
        element_handlers.push(element!("#body-scripts-placeholder", move |el| {
            el.replace(&scripts_string, lol_html::html_content::ContentType::Html);
            Ok(())
        }));
    }

    let mut rewriter = HtmlRewriter::new(
        Settings {
            element_content_handlers: element_handlers,
            ..Settings::default()
        },
        |c: &[u8]| response_body.extend_from_slice(c),
    );

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

    // Zamiast budować `Response`, zwracamy gotowy `Vec<u8>`
    Ok(response_body)
}

pub async fn build_response<'a>(
    headers: HeaderMap,
    page_builder: PageBuilder<'a>,
) -> Result<Response, AppError> {
    let body_bytes: Vec<u8>;
    let mut is_full_page_request = false;

    if headers.contains_key("HX-Request") {
        let oob_title = html! {
            title hx-swap-oob="true" { (page_builder.title) }
        };
        let final_markup = html! {
            (page_builder.main_content)
            (oob_title)
        };
        // <<< POPRAWKA 2: Konwertujemy markup do stringa, a potem na bajty
        body_bytes = final_markup.into_string().into_bytes();
    } else {
        // <<< POPRAWKA 3: `serve_full_page` zwraca `Vec<u8>`, więc `body_bytes` jest poprawnego typu
        body_bytes = serve_full_page(page_builder).await?;
        is_full_page_request = true;
    }

    // Obliczamy ETag
    let mut hasher = Sha1::new(); // Teraz to działa, bo `Digest` jest w zasięgu
    hasher.update(&body_bytes);
    let etag = format!("\"{}\"", hex::encode(hasher.finalize()));

    // Sprawdzamy ETag z nagłówka
    if let Some(if_none_match) = headers.get(header::IF_NONE_MATCH) {
        if if_none_match.to_str().unwrap_or_default() == etag {
            tracing::info!("ETag match! Zwracam 304 Not Modified.");
            return Ok(StatusCode::NOT_MODIFIED.into_response());
        }
    }

    // Budujemy pełną odpowiedź 200 OK
    tracing::info!("ETag mismatch or first request. Zwracam 200 OK z nową treścią.");
    let mut response_builder = Response::builder()
        .status(StatusCode::OK)
        .header(header::ETAG, HeaderValue::from_str(&etag).unwrap())
        .header("Vary", HeaderValue::from_static("HX-Request, ETag"));

    if is_full_page_request {
        response_builder = response_builder.header("Content-Type", "text/html; charset=utf-8");
    }

    let response = response_builder.body(Body::from(body_bytes)).unwrap();

    Ok(response)
}

// Nowa struktura do budowania kompleksowych odpowiedzi
pub struct PageBuilder<'a> {
    pub title: &'a str,
    pub main_content: Markup,
    pub head_scripts: Option<Markup>,
    pub body_scripts: Option<Markup>,
}

impl<'a> PageBuilder<'a> {
    pub fn new(
        title: &'a str,
        main_content: Markup,
        head_scripts: Option<Markup>,
        body_scripts: Option<Markup>,
    ) -> Self {
        Self {
            title,
            main_content,
            head_scripts,
            body_scripts,
        }
    }
}
