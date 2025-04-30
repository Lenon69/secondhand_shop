// src/main.rs

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response, Result},
    routing::{delete, get, patch, post},
};
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use std::net::SocketAddr;
// use std::sync::Arc;
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use uuid::Uuid;

use dotenvy::dotenv;
use std::env;

pub mod filters;
pub mod models;
pub mod pagination;

use models::{
    CreateProductPayload, Product, ProductCondition, ProductStatus, UpdateProductPayload,
};
use pagination::{PaginatedProductsResponse, PaginationParams};

#[tokio::main]
async fn main() {
    dotenv().ok();

    // Inicjalizacja systemu logowania (tracing)
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "secondhand_shop_backend=debug,tower_http=debug".into()), // Ustaw poziom logowania, np. RUST_LOG=info cargo run
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Inicjalizacja serwera...");

    // --- Połączenie z bazą danych ---
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = match PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
    {
        Ok(pool) => {
            tracing::info!("Pomyślnie połączono z bazą danych");
            pool
        }
        Err(err) => {
            tracing::error!("Nie można połączyć z bazą danych: {:?}", err);
            std::process::exit(1);
        }
    };

    // Definicja routingu aplikacji
    let app = Router::new()
        .route("/", get(root_handler)) // Dodajemy prosty handler dla ścieżki "/"
        .route("/api/products", get(list_products).post(create_product))
        .route(
            "/api/products/{:id}",
            get(get_product_details)
                .patch(update_product_partial)
                .delete(delete_product),
        )
        .layer(TraceLayer::new_for_http())
        .with_state(pool); // Dodajemy middleware do logowania każdego zapytania HTTP

    // Adres i port, na którym serwer będzie nasłuchiwał
    let addr = SocketAddr::from(([0, 0, 0, 0], 3000)); // Nasłuchuj na wszystkich interfejsach na porcie 3000
    tracing::info!("Serwer nasłuchuje na {}", addr);

    // Utworzenie listenera TCP
    let listener = match TcpListener::bind(addr).await {
        Ok(listener) => listener,
        Err(e) => {
            tracing::error!("Nie można powiązać adresu {}: {}", addr, e);
            return; // Zakończ, jeśli nie można uruchomić serwera
        }
    };

    // Uruchomienie serwera Axum
    if let Err(e) = axum::serve(listener, app.into_make_service()).await {
        tracing::error!("Błąd serwera: {}", e);
    }
}

#[derive(Debug)]
enum AppError {
    SqlxError(sqlx::Error),
    NotFound,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            AppError::SqlxError(sqlx_error) => {
                tracing::error!("Błąd SQLx: {:?}", sqlx_error);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Wystąpił wewnętrzny błąd serwera".to_string(),
                )
            }
            AppError::NotFound => (
                StatusCode::NOT_FOUND,
                "Zasób nie został znaleziony".to_string(),
            ),
        };

        (status, error_message).into_response()
    }
}

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        AppError::SqlxError(err)
    }
}

// ---------------- //
// --- HANDLERS --- //
// ---------------- //

async fn root_handler() -> &'static str {
    tracing::info!("Obsłużono zapytanie do /");
    "Witaj w backendzie sklepu second-hand!"
}

async fn list_products(
    State(pool): State<PgPool>,
    Query(pagination): Query<PaginationParams>,
) -> Result<Json<PaginatedProductsResponse>, AppError> {
    tracing::info!(
        "Obsłużono zapytanie GET /api/products - pobieranie listy z paginacja: {:?}",
        pagination
    );

    let limit = pagination.limit();
    let offset = pagination.offset();

    let total_items_result = sqlx::query_scalar::<_, i64>(
        r#"
            SELECT COUNT(*)
            FROM products
            WHERE status = $1
        "#,
    )
    .bind(ProductStatus::Available)
    .fetch_one(&pool)
    .await;

    let total_items = match total_items_result {
        Ok(count) => count,
        Err(e) => {
            tracing::error!("Błąd bazy danych podczas liczenia produktów: {:?}", e);
            return Err(AppError::SqlxError(e));
        }
    };

    let products = sqlx::query_as::<_, Product>(
        r#"
            SELECT id, name, description, price, condition AS "condition: _", category AS "category: _", status AS "status: _", images
            FROM products
            WHERE status = $1
            ORDER BY name
            LIMIT $2 OFFSET $3
        "#,
    ).bind(ProductStatus::Available)
    .bind(limit)
    .bind(offset)
    .fetch_all(&pool)
    .await?;

    let total_pages = if total_items == 0 {
        0
    } else {
        (total_items as f64 / limit as f64).ceil() as i64
    };
    let current_page = (offset as f64 / limit as f64).floor() as i64 + 1;

    let response = PaginatedProductsResponse {
        total_items,
        total_pages,
        current_page,
        per_page: limit,
        data: products,
    };

    Ok(Json(response))
}

async fn get_product_details(
    State(pool): State<PgPool>,
    Path(product_id): Path<Uuid>,
) -> Result<Json<Product>, AppError> {
    tracing::info!(
        "Obsłużono zapytanie GET /api/products/{} - pobieranie bazy danych",
        product_id
    );

    let product_result = sqlx::query_as::<_, Product>(
        r#"SELECT id, name, description, price, condition AS "condition: _", category AS "category: _", status AS "status: _", images
           FROM products
           WHERE id = $1"#,
    )
    .bind(product_id)
    .fetch_one(&pool)
    .await;

    match product_result {
        Ok(product) => Ok(Json(product)),
        Err(sqlx::Error::RowNotFound) => {
            tracing::warn!("Nie znaleziono produktu o ID: {}", product_id);
            Err(AppError::NotFound)
        }
        Err(e) => {
            tracing::info!(
                "Błąd bazy danych podczas pobierania produktu {}: {:?}",
                product_id,
                e
            );
            Err(AppError::SqlxError(e))
        }
    }
}

async fn create_product(
    State(pool): State<PgPool>,
    Json(payload): Json<CreateProductPayload>,
) -> Result<(StatusCode, Json<Product>), AppError> {
    tracing::info!("Obsłużono zapytanie POST /api/products - tworzenie produktu");

    let new_id = Uuid::new_v4();
    let default_status = ProductStatus::Available;

    let created_product = sqlx::query_as::<_, Product>(
        r#"
            INSERT INTO products (id, name, description, price, condition, category, status, images)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING id, name, description, price, condition AS "condition: _", category AS "category: _", status AS "status: _", images
        "#,
    ).bind(new_id)
    .bind(&payload.name)
    .bind(&payload.description)
    .bind(&payload.price)
    .bind(&payload.condition)
    .bind(&payload.category)
    .bind(default_status)
    .bind(&payload.images)
    .fetch_one(&pool)
    .await?;

    tracing::info!("Utworzono produkt o ID: {}", new_id);

    Ok((StatusCode::CREATED, Json(created_product)))
}

async fn update_product_partial(
    State(pool): State<PgPool>,
    Path(product_id): Path<Uuid>,
    Json(payload): Json<UpdateProductPayload>,
) -> Result<Json<Product>, AppError> {
    tracing::info!(
        "Obsłużono zapytanie PATCH /api/products/{} - aktualizacja: {:?}",
        product_id,
        payload
    );

    let mut existing_product = sqlx::query_as::<_, Product>(
        r#"
            SELECT id, name, description, price, condition AS "condition: _", category AS "category: _", status AS "status: _", images
            FROM products
            WHERE id = $1"#,
    ).bind(product_id)
    .fetch_one(&pool)
    .await
    .map_err(|err| match err {
            sqlx::Error::RowNotFound => {
                tracing::warn!("PATCH: Nie znaleziono produktu o ID: {}", product_id);
                AppError::NotFound
            }
            _ => {
                tracing::error!("PATCH: Błąd bazy danych podczas pobierania produktu {}: {:?}", product_id, err);
                AppError::SqlxError(err)
            }
        })?;

    if let Some(name) = payload.name {
        existing_product.name = name;
    }
    if let Some(description) = payload.description {
        existing_product.description = description;
    }
    if let Some(price) = payload.price {
        existing_product.price = price;
    }
    if let Some(condition) = payload.condition {
        existing_product.condition = condition;
    }
    if let Some(category) = payload.category {
        existing_product.category = category;
    }
    if let Some(status) = payload.status {
        existing_product.status = status;
    }
    if let Some(images) = payload.images {
        existing_product.images = images;
    }

    let updated_product = sqlx::query_as::<_, Product>(r#"
            UPDATE products
            SET name = $1, description = $2, price = $3, condition = $4, category = $5, status = $6, images = $7
            WHERE id = $8
            RETURNING id, name, description, price, condition AS "condition: _", category AS "category: _", status AS "status: _", images
        "#).bind(&existing_product.name)
        .bind(&existing_product.description)
        .bind(&existing_product.price)
        .bind(&existing_product.condition)
        .bind(&existing_product.category)
        .bind(&existing_product.status)
        .bind(&existing_product.images)
        .bind(product_id)
        .fetch_one(&pool)
        .await?;

    tracing::info!("Zaktualizowano produkt o ID: {}", product_id);

    Ok(Json(updated_product))
}

async fn delete_product(
    State(pool): State<PgPool>,
    Path(product_id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    tracing::info!("Obsłużono zapytanie DELETE /api/products/{}", product_id);

    let result = sqlx::query(
        r#"
            DELETE FROM products
            WHERE id = $1
        "#,
    )
    .bind(product_id)
    .execute(&pool)
    .await;

    match result {
        Ok(query_result) => {
            //Sprawdź ile wierszy zostało usuniętych
            if query_result.rows_affected() == 0 {
                tracing::warn!(
                    "DELETE: Nie znaleziono produktu do usunięcia o ID {}",
                    product_id
                );
                Err(AppError::NotFound)
            } else {
                tracing::info!("Usunięto produkt o ID: {}", product_id);
                Ok(StatusCode::NO_CONTENT)
            }
        }
        Err(err) => {
            tracing::error!(
                "DELETE: Błąd bazy danych podczas usuwania produktu {}: {:?}",
                product_id,
                err
            );

            Err(AppError::SqlxError(err))
        }
    }
}
