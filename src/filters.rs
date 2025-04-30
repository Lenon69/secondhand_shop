// src/filters.rs
use crate::models::{Category, ProductCondition, ProductStatus};
use serde::Deserialize;

const DEFAULT_PAGE_LIMIT: i64 = 10;
const MAX_PAGE_LIMIT: i64 = 50;
const DEFAULT_SORT_BY: &str = "name";
const DEFAULT_SORT_ORDER: &str = "asc";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ListingParams {
    // Paginacja
    #[serde(default)]
    limit: Option<i64>,
    #[serde(default)]
    offset: Option<i64>,

    // Filtry
    #[serde(default)]
    category: Option<Category>,
    #[serde(default)]
    condition: Option<ProductCondition>,
    #[serde(default)]
    status: Option<ProductStatus>,
    #[serde(default)]
    price_min: Option<i64>,
    #[serde(default)]
    price_max: Option<i64>,

    //Sortowanie
    #[serde(default)]
    sort_by: Option<String>,
    #[serde(default)]
    order: Option<String>,
}

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

    pub fn category(&self) -> Option<Category> {
        self.category.clone()
    }
    pub fn condition(&self) -> Option<ProductCondition> {
        self.condition.clone()
    }
    pub fn status(&self) -> Option<ProductStatus> {
        self.status.clone()
    }
    pub fn price_min(&self) -> Option<i64> {
        self.price_min.clone()
    }
    pub fn price_max(&self) -> Option<i64> {
        self.price_max.clone()
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
}
