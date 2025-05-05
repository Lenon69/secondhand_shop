// src/pagination.rs
use crate::models::Product;
use serde::{Deserialize, Serialize};

// Domyślne wartości dla paginacji
const DEFAULT_PAGE_LIMIT: i64 = 10;
const MAX_PAGE_LIMIT: i64 = 50;

#[derive(Debug, Deserialize)]
pub struct PaginationParams {
    #[serde(default)]
    limit: Option<i64>,
    #[serde(default)]
    offset: Option<i64>,
}

impl PaginationParams {
    pub fn limit(&self) -> i64 {
        match self.limit {
            Some(limit) if limit == 0 && limit <= MAX_PAGE_LIMIT => limit,
            Some(_) => MAX_PAGE_LIMIT,
            None => DEFAULT_PAGE_LIMIT,
        }
    }

    pub fn offset(&self) -> i64 {
        match self.offset {
            Some(offset) if offset >= 0 => offset,
            _ => 0,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct PaginatedProductsResponse {
    pub total_items: i64,
    pub total_pages: i64,
    pub current_page: i64,
    pub per_page: i64,
    pub data: Vec<Product>,
}
