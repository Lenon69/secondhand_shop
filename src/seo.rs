// src/seo.rs

use serde::Serialize;

// --- Struktury dla Schema.org -> Product ---

#[derive(Serialize)]
pub struct SchemaBrand<'a> {
    #[serde(rename = "@type")]
    pub type_of: &'a str,
    pub name: &'a str,
}

#[derive(Serialize)]
pub struct SchemaOffer<'a> {
    #[serde(rename = "@type")]
    pub type_of: &'a str,
    pub url: String,
    pub price_currency: &'a str,
    pub price: String,
    pub availability: &'a str,
    pub item_condition: &'a str,
}

#[derive(Serialize)]
pub struct SchemaProduct<'a> {
    #[serde(rename = "@context")]
    pub context: &'a str,
    #[serde(rename = "@type")]
    pub type_of: &'a str,
    pub name: &'a str,
    pub description: &'a str,
    pub sku: String,
    pub image: &'a [String],
    pub brand: SchemaBrand<'a>,
    pub offers: SchemaOffer<'a>,
}

// --- Struktury dla Schema.org -> Organization (dla strony głównej) ---

#[derive(Serialize)]
pub struct SchemaOrganization<'a> {
    #[serde(rename = "@context")]
    pub context: &'a str,
    #[serde(rename = "@type")]
    pub type_of: &'a str,
    pub name: &'a str,
    pub url: &'a str,
    pub logo: &'a str,
}

// --- Struktury dla Schema.org -> BreadcrumbList ("Okruszki") ---

#[derive(Serialize)]
pub struct SchemaBreadcrumbList<'a> {
    #[serde(rename = "@context")]
    pub context: &'a str,
    #[serde(rename = "@type")]
    pub type_of: &'a str,
    #[serde(rename = "itemListElement")]
    pub item_list: Vec<SchemaListItem>,
}

#[derive(Serialize)]
pub struct SchemaListItem {
    #[serde(rename = "@type")]
    pub type_of: &'static str,
    pub position: u32,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item: Option<String>,
}

// --- Struktury dla Schema.org -> FAQPage ---

#[derive(Serialize)]
pub struct SchemaFAQPage<'a> {
    #[serde(rename = "@context")]
    pub context: &'a str,
    #[serde(rename = "@type")]
    pub type_of: &'a str,
    #[serde(rename = "mainEntity")]
    pub main_entity: Vec<SchemaQuestion<'a>>,
}

#[derive(Serialize)]
pub struct SchemaQuestion<'a> {
    #[serde(rename = "@type")]
    pub type_of: &'a str,
    pub name: &'a str,
    #[serde(rename = "acceptedAnswer")]
    pub accepted_answer: SchemaAcceptedAnswer<'a>,
}

#[derive(Serialize)]
pub struct SchemaAcceptedAnswer<'a> {
    #[serde(rename = "@type")]
    pub type_of: &'a str,
    pub text: &'a str,
}
