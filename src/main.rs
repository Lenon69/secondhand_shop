// src/main.rs

use axum::{
    Router,
    routing::{get, post},
};
use sqlx::postgres::PgPoolOptions;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use dotenvy::dotenv;
use std::env;

// Deklaracje modułów
mod auth; // dla src/auth.rs
mod auth_models; // dla src/auth_models.rs
mod errors; // dla src/errors.rs
mod filters; // dla src/filters.rs
mod handlers; // dla src/handlers.rs
mod middleware;
mod models; // dla src/models.rs
mod pagination; // dla src/pagination.rs
mod state; // dla src/state.rs // dla src/middleware.rs

// Importy z własnych modułów
use crate::handlers::*;
use crate::state::AppState;

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

    // --- Konfiguracja JWT ---
    let jwt_secret = env::var("JWT_SECRET").expect("JWT_SECRET must be set");
    let jwt_expiration_hours = env::var("JWT_EXPIRATION_HOURS")
        .unwrap_or_else(|_| "1".to_string())
        .parse::<i64>()
        .expect("JWT_EXPIRATION_HOURS must be a valid number");

    // Definicja AppState
    let app_state = AppState {
        db_pool: pool,
        jwt_secret,
        jwt_expiration_hours,
    };

    // Definicja routingu aplikacji
    let app = Router::new()
        .route("/", get(root_handler)) // Dodajemy prosty handler dla ścieżki "/"
        .route("/api/products", get(list_products).post(create_product))
        .route(
            "/api/products/{id}",
            get(get_product_details)
                .patch(update_product_partial)
                .delete(delete_product),
        )
        .route("/api/auth/register", post(register_handler))
        .route("/api/auth/login", post(login_handler))
        .layer(TraceLayer::new_for_http())
        .with_state(app_state); // Dodajemy middleware do logowania każdego zapytania HTTP

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
