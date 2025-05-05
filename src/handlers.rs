// src/handlers.rs
use axum::{
    Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::{get, post},
};

use crate::auth_models::{LoginPayload, RegistrationPayload};
use crate::errors::AppError;
use crate::filters::ListingParams;
use crate::models::{CreateProductPayload, Product, Role, UpdateProductPayload, User, UserPublic};
use crate::pagination::PaginatedProductsResponse;
use crate::{
    AppError,
    auth::{create_jwt, hash_password, verify_password},
    state::AppState,
};
use uuid::Uuid;
use validator::Validate;

pub async fn root_handler() -> &'static str {
    tracing::info!("Obsłużono zapytanie do /");
    "Witaj w backendzie sklepu second-hand!"
}

pub async fn get_product_details(
    State(app_state): State<AppState>, // Zmień na AppState
    Path(product_id): Path<Uuid>,
) -> Result<Json<Product>, AppError> {
    // ... (logika bez zmian, użyj app_state.db_pool) ...
    let product_result = sqlx::query_as::<_, Product>(
        r#"SELECT id, name, description, price, condition, category, status, images
           FROM products
           WHERE id = $1"#,
    )
    .bind(product_id)
    .fetch_one(&app_state.db_pool) // Użyj app_state.db_pool
    .await;
    // ...
    match product_result {
        Ok(product) => Ok(Json(product)),
        Err(sqlx::Error::RowNotFound) => {
            tracing::warn!("Nie znaleziono produktu o ID: {}", product_id);
            Err(AppError::NotFound)
        }
        Err(e) => {
            tracing::error!(
                "Błąd bazy danych podczas pobierania produktu {}: {:?}",
                product_id,
                e
            );
            Err(AppError::from(e)) // Użyj konwersji From
        }
    }
}

pub async fn list_products(
    State(app_state): State<AppState>,
    Query(params): Query<ListingParams>,
) -> Result<Json<PaginatedProductsResponse>, AppError> {
}
