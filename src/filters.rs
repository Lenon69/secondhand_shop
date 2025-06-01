// src/filters.rs
use crate::models::{Category, ProductCondition, ProductGender, ProductStatus};
use chrono::{DateTime, Utc};
use serde::Deserialize;

const DEFAULT_PAGE_LIMIT: i64 = 10;
const MAX_PAGE_LIMIT: i64 = 50;
const DEFAULT_SORT_BY: &str = "name";
const DEFAULT_SORT_ORDER: &str = "asc";

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct ListingParams {
    // Paginacja
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub offset: Option<i64>,

    // Filtry
    #[serde(default)]
    pub gender: Option<ProductGender>,
    #[serde(default)]
    pub category: Option<Category>,
    #[serde(default)]
    pub condition: Option<ProductCondition>,
    #[serde(default)]
    pub status: Option<ProductStatus>,
    #[serde(default)]
    pub price_min: Option<i64>,
    #[serde(default)]
    pub price_max: Option<i64>,

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
    pub fn gender(&self) -> Option<ProductGender> {
        self.gender.clone()
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

    pub fn search(&self) -> Option<String> {
        self.search.clone()
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
                query_parts.push(format!("status={}", val.as_ref()));
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
            sort_by: self.sort_by.clone(),
            order: self.order.clone(),
            search: self.search.clone(),
            created_at: self.created_at.clone(),
            updated_at: self.updated_at.clone(),
        }
    }

    pub fn to_query_string_for_pagination(&self) -> String {
        let mut query_parts = Vec::new();
        if let Some(limit) = self.limit {
            query_parts.push(format!("limit={}", limit));
        }
        if let Some(gender) = &self.gender {
            query_parts.push(format!("gender={}", gender.as_ref()));
        } // Użyj as_ref() dla enumów
        if let Some(category) = &self.category {
            query_parts.push(format!("category={}", category.as_ref()));
        }
        if let Some(condition) = &self.condition {
            query_parts.push(format!("condition={}", condition.as_ref()));
        }
        if let Some(status) = &self.status {
            query_parts.push(format!("status={}", status.as_ref()));
        }
        if let Some(price_min) = self.price_min {
            query_parts.push(format!("price_min={}", price_min));
        }
        if let Some(price_max) = self.price_max {
            query_parts.push(format!("price_max={}", price_max));
        }
        if let Some(sort_by) = &self.sort_by {
            query_parts.push(format!("sort-by={}", sort_by));
        }
        if let Some(order) = &self.order {
            query_parts.push(format!("order={}", order));
        }
        if let Some(search) = &self.search {
            query_parts.push(format!("search={}", urlencoding::encode(search)));
        }
        query_parts.join("&")
    }
}
