// src/pagination.rs
use crate::models::Product;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct PaginatedProductsResponse {
    pub total_items: i64,
    pub total_pages: i64,
    pub current_page: i64,
    pub per_page: i64,
    pub data: Vec<Product>,
}

#[derive(Debug, Serialize)]
pub struct PaginatedOrdersResponse<T> {
    pub total_items: i64,
    pub total_pages: i64,
    pub current_page: i64,
    pub per_page: i64,
    pub data: Vec<T>,
}
