// src/sitemap_generator.rs

use crate::errors::AppError;
use crate::models::{Category, Product, ProductGender, ProductStatus};
use crate::state::AppState;
use axum::{
    http::{header, HeaderValue},
    response::{IntoResponse, Response},
};
use chrono::{DateTime, Utc};
use quick_xml::se::to_string;
use serde::Serialize;
use strum::IntoEnumIterator;

// --- Struktury danych odzwierciedlające format sitemap.xml ---

#[derive(Serialize)]
#[serde(rename = "urlset")]
pub struct UrlSet {
    #[serde(rename = "@xmlns")]
    xmlns: String,
    #[serde(rename = "url")]
    pub urls: Vec<UrlEntry>,
}

#[derive(Serialize)]
pub struct UrlEntry {
    #[serde(rename = "loc")]
    pub location: String,
    #[serde(rename = "lastmod")]
    pub last_modified: String,
    #[serde(rename = "changefreq")]
    pub change_frequency: ChangeFreq,
    #[serde(rename = "priority")]
    pub priority: f32,
}

#[derive(Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ChangeFreq {
    Always,
    Hourly,
    Daily,
    Weekly,
    Monthly,
    Yearly,
    Never,
}

// --- Główny Handler ---

pub async fn generate_sitemap_handler(
    app_state: &AppState,
) -> Result<Response, AppError> {
    let base_url = "https://messvintage.com"; // WAŻNE: Użyj swojego prawdziwego adresu URL
    let mut urls = Vec::new();

    // 1. Strony Statyczne (wysoki priorytet, rzadkie zmiany)
    let static_pages = vec![
        ("", 1.0, ChangeFreq::Weekly), // Strona główna
        ("/dla-niej", 0.9, ChangeFreq::Daily),
        ("/dla-niego", 0.9, ChangeFreq::Daily),
        ("/nowosci", 0.9, ChangeFreq::Daily),
        ("/okazje", 0.9, ChangeFreq::Daily),
        ("/o-nas", 0.5, ChangeFreq::Monthly),
        ("/kontakt", 0.5, ChangeFreq::Monthly),
        ("/regulamin", 0.3, ChangeFreq::Yearly),
        ("/polityka-prywatnosci", 0.3, ChangeFreq::Yearly),
        ("/faq", 0.5, ChangeFreq::Monthly),
        ("/wysylka-i-zwroty", 0.5, ChangeFreq::Monthly),
    ];

    for (loc, prio, freq) in static_pages {
        urls.push(UrlEntry {
            location: format!("{}{}", base_url, loc),
            last_modified: Utc::now().to_rfc3339(), // Można by pobrać datę modyfikacji pliku
            change_frequency: freq,
            priority: prio,
        });
    }

    // 2. Strony Kategorii (dla obu płci)
    for gender in [ProductGender::Damskie, ProductGender::Meskie].iter() {
        let gender_slug = if *gender == ProductGender::Damskie { "dla-niej" } else { "dla-niego" };
        for category in Category::iter() {
            urls.push(UrlEntry {
                location: format!("{}/{}/{}", base_url, gender_slug, category.as_ref()),
                last_modified: Utc::now().to_rfc3339(),
                change_frequency: ChangeFreq::Weekly,
                priority: 0.8,
            });
        }
    }

    // 3. Strony Produktów (dynamicznie z bazy danych)
    let products = sqlx::query_as::<_, Product>(
        "SELECT * FROM products WHERE status = $1",
    )
    .bind(ProductStatus::Available) // Tylko dostępne produkty
    .fetch_all(&app_state.db_pool)
    .await?;

    for product in products {
        urls.push(UrlEntry {
            location: format!("{}/produkty/{}", base_url, product.id),
            last_modified: product.updated_at.to_rfc3339(), // Używamy daty aktualizacji produktu
            change_frequency: ChangeFreq::Monthly, // Produkty się nie zmieniają, ale lista tak
            priority: 0.7,
        });
    }

    let url_set = UrlSet {
        xmlns: "http://www.sitemaps.org/schemas/sitemap/0.9".to_string(),
        urls,
    };

    // Serializacja do XML
    let mut xml_output = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>".to_string();
    xml_output.push_str(&to_string(&url_set).map_err(|_| {
        AppError::InternalServerError("Błąd podczas generowania XML mapy strony".to_string())
    })?);

    // Zwrócenie odpowiedzi z poprawnym typem zawartości
    Ok((
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/xml; charset=utf-8"),
        )],
        xml_output,
    )
        .into_response())
}
