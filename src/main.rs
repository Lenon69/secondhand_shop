// src/main.rs

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::response::Html;
use axum::routing::{delete, get, post};
use dotenvy::dotenv;
use htmx_handlers::{
    about_us_page_handler, add_item_to_cart_htmx_handler, contact_page_handler, faq_page_handler,
    gender_page_handler, get_cart_details_htmx_handler, get_product_detail_htmx_handler,
    list_products_htmx_handler, login_page_htmx_handler, my_account_page_handler,
    my_orders_htmx_handler, privacy_policy_page_handler, registration_page_htmx_handler,
    remove_item_from_cart_htmx_handler, shipping_returns_page_handler,
    terms_of_service_page_handler,
};
// use htmx_handlers::*;
use reqwest::StatusCode;
use sqlx::postgres::PgPoolOptions;
use std::env;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use tracing::Instrument;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

// Deklaracje modułów
mod auth;
mod auth_models;
mod cart_utils;
mod cloudinary;
mod errors;
mod filters;
mod handlers;
mod htmx_handlers;
mod middleware;
mod models;
mod pagination;
mod state;

// Importy z własnych modułów
use crate::handlers::*;
use crate::state::{AppState, CloudinaryConfig};

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

    // --- Konfiguracja Cloudinary ---
    let cloudinary_config = CloudinaryConfig {
        cloud_name: env::var("CLOUDINARY_CLOUD_NAME").expect("CLOUDINARY_CLOUD_NAME must be set"),
        api_key: env::var("CLOUDINARY_API_KEY").expect("CLOUDINARY_API_KEY must be set"),
        api_secret: env::var("CLOUDINARY_API_SECRET").expect("CLOUDINARY_API_SECRET must be set"),
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
        cloudinary_config,
    };

    // Definicja routingu aplikacji
    let app = Router::new()
        .route(
            "/api/products",
            get(list_products).post(create_product_handler),
        )
        .route(
            "/api/products/{id}",
            get(get_product_details)
                .patch(update_product_partial_handler)
                .delete(delete_product_handler),
        )
        .route("/api/auth/register", post(register_handler))
        .route("/api/auth/login", post(login_handler))
        .route("/api/me", get(protected_route_handler))
        .route(
            "/api/orders",
            post(create_order_handler).get(list_orders_handler),
        )
        .route(
            "/api/orders/{order_id}",
            get(get_order_details_handler).patch(update_order_status_handler),
        )
        .route("/api/cart/items", post(add_item_to_cart_handler))
        .route("/api/cart", get(get_cart_handler))
        .route(
            "/api/cart/items/{product_id}",
            delete(remove_item_from_cart_handler),
        )
        .route("/api/guest-cart", get(get_guest_cart))
        .route("/api/guest-cart/items", post(add_item_to_guest_cart))
        .route(
            "/api/guest-cart/items/{product_id}",
            delete(remove_item_from_guest_cart),
        )
        .route("/api/cart/merge", post(merge_cart_handler))
        .route("/", get(serve_index))
        .route("/htmx/dla/{gender_slug}", get(gender_page_handler))
        .route(
            "/htmx/cart/add/{product_id}",
            post(add_item_to_cart_htmx_handler),
        )
        .route("/htmx/cart/details", get(get_cart_details_htmx_handler))
        .route("/htmx/products", get(list_products_htmx_handler))
        .route(
            "/htmx/cart/remove/{product_id}",
            post(remove_item_from_cart_htmx_handler),
        )
        .route(
            "/htmx/produkt/{product_id}",
            get(get_product_detail_htmx_handler),
        )
        .route(
            "/htmx/page/polityka-prywatnosci",
            get(privacy_policy_page_handler),
        )
        .route("/htmx/page/o-nas", get(about_us_page_handler))
        .route("/htmx/page/regulamin", get(terms_of_service_page_handler))
        .route("/htmx/page/kontakt", get(contact_page_handler))
        .route("/htmx/page/faq", get(faq_page_handler))
        .route(
            "/htmx/page/wysylka-i-zwroty",
            get(shipping_returns_page_handler),
        )
        .route("/htmx/logowanie", get(login_page_htmx_handler))
        .route("/htmx/rejestracja", get(registration_page_htmx_handler))
        .route("/htmx/moje-konto/zamowienia", get(my_orders_htmx_handler))
        .route("/htmx/moje-konto", get(my_account_page_handler))
        .nest_service("/static", ServeDir::new("static"))
        .layer(TraceLayer::new_for_http())
        .layer(DefaultBodyLimit::max(100 * 1024 * 1024))
        .with_state(app_state);

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

async fn serve_index() -> Result<Html<String>, StatusCode> {
    match tokio::fs::read_to_string("templates/index.html").await {
        Ok(content) => Ok(Html(content)),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}
