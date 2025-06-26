// src/filters.rs
use crate::models::{Category, OrderStatus, ProductCondition, ProductGender};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize, de};
use std::str::FromStr;

const DEFAULT_PAGE_LIMIT: i64 = 8;
const MAX_PAGE_LIMIT: i64 = 50;
const DEFAULT_SORT_BY: &str = "name";
const DEFAULT_SORT_ORDER: &str = "asc";

const DEFAULT_ORDER_PAGE_LIMIT: i64 = 15;
const MAX_ORDER_PAGE_LIMIT: i64 = 50;
const DEFAULT_ORDER_SORT_BY: &str = "order_date";
const DEFAULT_ORDER_SORT_ORDER: &str = "desc";

#[allow(dead_code)]
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
#[serde(rename_all = "kebab-case")]
pub struct ListingParams {
    // Paginacja
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub offset: Option<i64>,

    // Filtry
    #[serde(
        default,
        deserialize_with = "deserialize_optional_enum_from_empty_string"
    )]
    pub gender: Option<ProductGender>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_enum_from_empty_string"
    )]
    pub category: Option<Category>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_enum_from_empty_string"
    )]
    pub condition: Option<ProductCondition>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub price_min: Option<i64>,
    #[serde(default)]
    pub price_max: Option<i64>,
    #[serde(default)]
    pub on_sale: Option<bool>,

    //Sortowanie
    #[serde(default)]
    pub sort_by: Option<String>,
    #[serde(default)]
    pub order: Option<String>,
    #[serde(default)]
    pub search: Option<String>,
    #[serde(default)]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub updated_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub source: Option<String>,
}

#[allow(dead_code)]
impl ListingParams {
    pub fn limit(&self) -> i64 {
        match self.limit {
            Some(limit) if limit > 0 && limit <= MAX_PAGE_LIMIT => limit,
            Some(_) => MAX_PAGE_LIMIT,
            None => DEFAULT_PAGE_LIMIT,
        }
    }

    pub fn offset(&self) -> i64 {
        self.offset.unwrap_or(0).max(0)
    }
    pub fn gender(&self) -> Option<ProductGender> {
        self.gender.clone()
    }
    pub fn category(&self) -> Option<Category> {
        self.category.clone()
    }
    pub fn condition(&self) -> Option<ProductCondition> {
        self.condition.clone()
    }
    pub fn status(&self) -> Option<String> {
        self.status.clone()
    }
    pub fn price_min(&self) -> Option<i64> {
        self.price_min.clone()
    }
    pub fn price_max(&self) -> Option<i64> {
        self.price_max.clone()
    }

    pub fn search(&self) -> Option<String> {
        self.search.clone()
    }

    pub fn on_sale(&self) -> Option<bool> {
        self.on_sale.clone()
    }

    pub fn sort_by(&self) -> &str {
        self.sort_by.as_deref().unwrap_or(DEFAULT_SORT_BY)
    }

    pub fn order(&self) -> &str {
        self.order.as_deref().map_or(DEFAULT_SORT_ORDER, |o| {
            if o.eq_ignore_ascii_case("desc") {
                "desc"
            } else {
                "asc"
            }
        })
    }

    pub fn created_at(&self) -> Option<DateTime<Utc>> {
        self.created_at.clone()
    }

    pub fn updated_at(&self) -> Option<DateTime<Utc>> {
        self.updated_at.clone()
    }

    pub fn to_query_string_with_skips(&self, skip_params: &[&str]) -> String {
        let mut query_parts = Vec::new();
        if !skip_params.contains(&"limit") {
            if let Some(val) = self.limit {
                query_parts.push(format!("limit={}", val));
            }
        }
        if !skip_params.contains(&"offset") {
            if let Some(val) = self.offset {
                query_parts.push(format!("offset={}", val));
            }
        }
        if !skip_params.contains(&"gender") {
            if let Some(val) = &self.gender {
                query_parts.push(format!("gender={}", val.as_ref()));
            }
        }
        if !skip_params.contains(&"category") {
            if let Some(val) = &self.category {
                query_parts.push(format!("category={}", val.as_ref()));
            }
        }
        if !skip_params.contains(&"condition") {
            if let Some(val) = &self.condition {
                query_parts.push(format!("condition={}", val.as_ref()));
            }
        }
        if !skip_params.contains(&"status") {
            if let Some(val) = &self.status {
                query_parts.push(format!("status={}", val));
            }
        }
        if !skip_params.contains(&"price_min") {
            if let Some(val) = self.price_min {
                query_parts.push(format!("price_min={}", val));
            }
        }
        if !skip_params.contains(&"price_max") {
            if let Some(val) = self.price_max {
                query_parts.push(format!("price_max={}", val));
            }
        }
        if !skip_params.contains(&"on-sale") {
            if let Some(val) = self.on_sale {
                query_parts.push(format!("on-sale={}", val));
            }
        }
        if !skip_params.contains(&"sort_by") {
            if let Some(val) = &self.sort_by {
                query_parts.push(format!("sort-by={}", val));
            }
        }
        if !skip_params.contains(&"order") {
            if let Some(val) = &self.order {
                query_parts.push(format!("order={}", val));
            }
        }
        if !skip_params.contains(&"search") {
            if let Some(val) = &self.search {
                query_parts.push(format!("search={}", urlencoding::encode(val)));
            }
        }
        query_parts.join("&")
    }

    pub fn clone_with_new_offset(&self, new_offset: i64) -> Self {
        ListingParams {
            limit: self.limit,
            offset: Some(new_offset), // Ustawia nowy offset
            gender: self.gender.clone(),
            category: self.category.clone(),
            condition: self.condition.clone(),
            status: self.status.clone(),
            price_min: self.price_min,
            price_max: self.price_max,
            on_sale: self.on_sale.clone(),
            sort_by: self.sort_by.clone(),
            order: self.order.clone(),
            search: self.search.clone(),
            created_at: self.created_at.clone(),
            updated_at: self.updated_at.clone(),
            source: self.source.clone(),
        }
    }

    pub fn to_query_string_for_pagination(&self) -> String {
        let mut query_parts = Vec::new();

        // Limit: użyj wartości z params lub domyślnej, jeśli nie ma
        query_parts.push(format!(
            "limit={}",
            self.limit.unwrap_or(DEFAULT_PAGE_LIMIT)
        ));

        if let Some(gender) = &self.gender {
            query_parts.push(format!("gender={}", gender.as_ref()));
        }
        if let Some(category) = &self.category {
            query_parts.push(format!("category={}", category.as_ref()));
        }
        if let Some(condition) = &self.condition {
            query_parts.push(format!("condition={}", condition.as_ref()));
        }
        if let Some(status) = &self.status {
            query_parts.push(format!("status={}", status));
        }
        if let Some(p_min) = self.price_min {
            query_parts.push(format!("price_min={}", p_min));
        }
        if let Some(p_max) = self.price_max {
            query_parts.push(format!("price_max={}", p_max));
        }
        if let Some(on_sale_val) = self.on_sale {
            query_parts.push(format!("on-sale={}", on_sale_val));
        }

        // Sortowanie: użyj metod, które zwracają domyślne wartości
        query_parts.push(format!("sort-by={}", self.sort_by()));
        query_parts.push(format!("order={}", self.order()));

        if let Some(search_term) = &self.search {
            if !search_term.is_empty() {
                query_parts.push(format!("search={}", urlencoding::encode(search_term)));
            }
        }
        query_parts.join("&")
    }
}

fn deserialize_optional_enum_from_empty_string<'de, D, T>(
    deserializer: D,
) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: FromStr, // Wymagamy, aby typ T implementował FromStr (co EnumString zapewnia)
    T::Err: std::fmt::Display, // Wymagane dla de::Error::custom
{
    // Deserializuj jako String. Jeśli parametr jest obecny w URL (np. category=),
    // to `s` będzie tym stringiem (nawet jeśli pustym).
    // Jeśli parametr całkowicie brakuje w URL, a pole ma #[serde(default)],
    // ta funkcja nie zostanie wywołana dla tego pola, a zadziała `default`.
    let s = String::deserialize(deserializer)?;
    if s.is_empty() {
        Ok(None) // Pusty string traktujemy jako brak wartości (None)
    } else {
        // Próbujemy sparsować string na enum T
        T::from_str(&s).map(Some).map_err(de::Error::custom)
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct OrderListingParams {
    // Paginacja
    pub limit: Option<i64>,
    pub offset: Option<i64>,

    // Filtry
    #[serde(
        default,
        deserialize_with = "deserialize_optional_enum_from_empty_string"
    )]
    pub status: Option<OrderStatus>,
    // Dla dat użyjemy String, a parsowanie do DateTime<Utc> zrobimy w handlerze
    // lub można stworzyć niestandardowe deserializatory dla dat.
    // Prostsze na start: String i parsowanie.
    pub date_from: Option<String>, // np. "YYYY-MM-DD"
    pub date_to: Option<String>,   // np. "YYYY-MM-DD"
    pub search: Option<String>,    // Wyszukiwanie po ID zamówienia, emailu klienta itp.

    // Sortowanie
    pub sort_by: Option<String>,
    pub order: Option<String>,
}

impl OrderListingParams {
    pub fn limit(&self) -> i64 {
        match self.limit {
            Some(limit) if limit > 0 && limit <= MAX_ORDER_PAGE_LIMIT => limit,
            Some(_) => MAX_ORDER_PAGE_LIMIT,
            None => DEFAULT_ORDER_PAGE_LIMIT,
        }
    }

    pub fn offset(&self) -> i64 {
        self.offset.unwrap_or(0).max(0)
    }

    pub fn status(&self) -> Option<OrderStatus> {
        self.status.clone()
    }

    pub fn date_from_dt(&self) -> Option<DateTime<Utc>> {
        self.date_from.as_ref().and_then(|s| {
            chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
                .ok()
                .map(|naive_date| {
                    DateTime::from_naive_utc_and_offset(
                        naive_date.and_hms_opt(0, 0, 0).unwrap(),
                        Utc,
                    )
                })
        })
    }

    pub fn date_to_dt(&self) -> Option<DateTime<Utc>> {
        self.date_to.as_ref().and_then(|s| {
            chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
                .ok()
                .map(|naive_date| {
                    DateTime::from_naive_utc_and_offset(
                        naive_date.and_hms_opt(23, 59, 59).unwrap(),
                        Utc,
                    )
                }) // Koniec dnia
        })
    }

    pub fn search(&self) -> Option<String> {
        self.search.clone().filter(|s| !s.is_empty())
    }

    pub fn sort_by(&self) -> &str {
        self.sort_by.as_deref().unwrap_or(DEFAULT_ORDER_SORT_BY)
    }

    pub fn order(&self) -> &str {
        self.order.as_deref().map_or(DEFAULT_ORDER_SORT_ORDER, |o| {
            if o.eq_ignore_ascii_case("asc") {
                "asc"
            } else {
                "desc"
            }
        })
    }

    // Funkcja do budowania query string dla HTMX (może być potrzebna później)
    pub fn to_query_string(&self) -> String {
        let mut query_parts = Vec::new();
        if let Some(val) = self.limit {
            query_parts.push(format!("limit={}", val));
        }
        if let Some(val) = self.offset {
            query_parts.push(format!("offset={}", val));
        }
        if let Some(val) = &self.status {
            query_parts.push(format!("status={}", val.as_ref()));
        }
        if let Some(val) = &self.date_from {
            query_parts.push(format!("date-from={}", val));
        }
        if let Some(val) = &self.date_to {
            query_parts.push(format!("date-to={}", val));
        }
        if let Some(val) = &self.search {
            query_parts.push(format!("search={}", urlencoding::encode(val)));
        }
        if let Some(val) = &self.sort_by {
            query_parts.push(format!("sort-by={}", val));
        }
        if let Some(val) = &self.order {
            query_parts.push(format!("order={}", val));
        }
        query_parts.join("&")
    }
}
