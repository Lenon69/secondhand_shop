use axum::body::Body;
use axum::http::{HeaderMap, StatusCode};
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
pub async fn serve_full_page<'a>(page_builder: PageBuilder<'a>) -> Result<Response, AppError> {
    let shell_content = match fs::read("static/index.html").await {
        Ok(bytes) => Bytes::from(bytes),
        Err(e) => {
            tracing::error!("Nie można wczytać pliku szablonu static/index.html: {}", e);
            return Err(AppError::InternalServerError(
                "Błąd wczytywania szablonu strony".to_string(),
            ));
        }
    };

    // KROK 1: Dekonstrukcja PageBuilder. Teraz mamy bezpośrednią własność nad każdą częścią.
    let PageBuilder {
        title,
        main_content,
        head_scripts,
        body_scripts,
    } = page_builder;

    // KROK 2: Konwertujemy Markup na String *jeden raz*, przed utworzeniem domknięć.
    // .into_string() zużywa `main_content`, co jest w porządku, bo już go nie potrzebujemy.
    let content_string = main_content.into_string();

    let mut response_body = Vec::new();
    let mut element_handlers = vec![
        // KROK 3: Używamy `move`, aby domknięcie przejęło własność nad `content_string`.
        element!("#content", move |el| {
            // ZNIKA .clone()! Przekazujemy referencję do stringa, który jest teraz własnością domknięcia.
            el.set_inner_content(&content_string, lol_html::html_content::ContentType::Html);
            el.remove_attribute("hx-trigger");
            el.remove_attribute("hx-get");
            Ok(())
        }),
        element!("head > title", |el| {
            // `title` to `&'a str`, który jest `Copy`, więc można go używać bez `move`.
            el.set_inner_content(title, lol_html::html_content::ContentType::Text);
            Ok(())
        }),
    ];

    // Ta sama optymalizacja dla skryptów
    if let Some(scripts) = head_scripts {
        let scripts_string = scripts.into_string(); // Konwersja bez klonowania
        element_handlers.push(element!("#head-scripts-placeholder", move |el| {
            el.replace(&scripts_string, lol_html::html_content::ContentType::Html);
            Ok(())
        }));
    }

    if let Some(scripts) = body_scripts {
        let scripts_string = scripts.into_string(); // Konwersja bez klonowania
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

    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "text/html; charset=utf-8")
        .body(Body::from(response_body))
        .map_err(|e| {
            tracing::error!("Nie udało się zbudować odpowiedzi HTTP: {}", e);
            AppError::InternalServerError("Błąd serwera podczas tworzenia odpowiedzi".to_string())
        })
}

pub async fn build_response<'a>(
    headers: HeaderMap,
    page_builder: PageBuilder<'a>,
) -> Result<Response, AppError> {
    if headers.contains_key("HX-Request") {
        let oob_title = html! {
            title hx-swap-oob="true" { (page_builder.title) }
        };

        let final_markup = html! {
            (page_builder.main_content)
            (oob_title)

            @if let Some(head_scripts) = page_builder.head_scripts {
                div id="head-scripts-placeholder" hx-swap-oob="true" { (head_scripts) }
            }
            @if let Some(body_scripts) = page_builder.body_scripts {
                div id="body-scripts-placeholder" hx-swap-oob="true" { (body_scripts) }
            }
        };
        Ok(final_markup.into_response())
    } else {
        // POPRAWIONE WYWOŁANIE: Przekazujemy cały obiekt `page_builder`
        serve_full_page(page_builder).await
    }
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
