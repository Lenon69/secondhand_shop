use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response, Result},
    routing::get,
};
use core::panicking::panic_const::panic_const_async_gen_fn_resumed_panic;
use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, postgres::PgAdvisoryLockGuard};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use uuid::Uuid;

use dotenvy::dotenv;
use std::env;

pub mod models;

use models::{CreateProductPayload, Product, ProductCondition, ProductStatus};

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
        .route("/api/products", get(list_products))
        .route("/api/products/:id", get(get_product_details))
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

async fn list_products(State(pool): State<PgPool>) -> Result<Json<Vec<Product>>, AppError> {
    tracing::info!("Obsłużono zapytanie GET /api/products - pobieranie z bazy");

    let target_status = ProductStatus::Available;

    let products = sqlx::query_as::<_, Product>(
        r#"SELECT id, name, description, price, condition AS "condition: _", category AS "category: _", status AS "status: _", images
        FROM products
        WHERE status = $1
        ORDER BY name"#
    )
    .bind(target_status)
    .fetch_all(&pool)
    .await?;

    Ok(Json(products))
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
