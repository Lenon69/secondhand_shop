// src/main.rs

use axum::Router;
use axum::extract::{DefaultBodyLimit, State};
use axum::response::Html;
use axum::routing::{delete, get, post};
use axum_server::tls_rustls::RustlsConfig;
use dotenvy::dotenv;
use moka::future::Cache;
use reqwest::StatusCode;
use sqlx::postgres::PgPoolOptions;
use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

// Deklaracje modułów
mod auth;
mod auth_models;
mod cart_utils;
mod cloudinary;
mod email_service;
mod errors;
mod filters;
mod handlers;
mod htmx_handlers;
mod middleware;
mod models;
mod pagination;
mod response;
mod seo;
mod services;
mod sitemap_generator;
mod state;

use crate::handlers::{
    add_item_to_cart_handler, add_item_to_guest_cart, archivize_product_handler,
    create_order_handler, create_product_handler, forgot_password_handler, get_cart_handler,
    get_guest_cart, get_order_details_handler, get_product_details, init_guest_session_handler,
    list_orders_handler, list_products, login_handler, logout_handler, merge_cart_handler,
    permanent_delete_order_handler, permanent_delete_product_handler, protected_route_handler,
    register_handler, remove_item_from_cart_handler, remove_item_from_guest_cart,
    reset_password_handler, update_order_status_handler, update_product_partial_handler,
    upsert_user_shipping_details_handler,
};

use crate::htmx_handlers::{
    about_us_page_handler, add_item_to_cart_htmx_handler, admin_dashboard_htmx_handler,
    admin_order_details_htmx_handler, admin_orders_list_htmx_handler,
    admin_product_edit_form_htmx_handler, admin_product_new_form_htmx_handler,
    admin_products_list_htmx_handler, checkout_page_handler, contact_page_handler,
    dla_gender_handler, dla_gender_with_category_handler, faq_page_handler,
    forgot_password_form_handler, get_cart_details_htmx_handler, get_product_detail_htmx_handler,
    handler_404, home_page_handler, list_products_htmx_handler, live_search_handler,
    login_page_htmx_handler, my_account_data_htmx_handler, my_account_page_handler,
    my_order_details_htmx_handler, my_orders_htmx_handler, news_page_htmx_handler,
    payment_finalization_page_handler, privacy_policy_page_handler, registration_page_htmx_handler,
    remove_item_from_cart_htmx_handler, reset_password_form_handler, sale_page_htmx_handler,
    search_page_handler, shipping_returns_page_handler, terms_of_service_page_handler,
};
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

    let provider = rustls::crypto::aws_lc_rs::default_provider();
    if let Err(e) = provider.install_default() {
        tracing::error!(
            "Błąd podczas instalacji domyślnego dostawcy kryptograficznego: {:?}",
            e
        );
        std::process::exit(1);
    }

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

    // --- Konfiguracja Resend ---
    let resend_api_key = env::var("RESEND_API_KEY").expect("RESEND_API_KEY must be set");

    let product_cache = Arc::new(
        Cache::builder()
            .max_capacity(1000)
            .time_to_live(Duration::from_secs(3600))
            .build(),
    );

    let static_html_cache = Arc::new(
        Cache::builder()
            .max_capacity(50) // Mało wpisów
            .time_to_live(Duration::from_secs(3600 * 24)) // 24 godziny
            .build(),
    );

    let dynamic_html_cache = Arc::new(
        Cache::builder()
            .max_capacity(200) // Więcej możliwych kombinacji filtrów
            .time_to_live(Duration::from_secs(300)) // 5 minut
            .build(),
    );

    // Definicja AppState
    let app_state = AppState {
        db_pool: pool,
        jwt_secret,
        jwt_expiration_hours,
        cloudinary_config,
        resend_api_key,
        product_cache,
        static_html_cache,
        dynamic_html_cache,
    };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

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
                .delete(archivize_product_handler),
        )
        .route(
            "/api/products/{id}/permanent",
            delete(permanent_delete_product_handler),
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
        .route(
            "/api/orders/{order_id}/permanent",
            delete(permanent_delete_order_handler),
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
        .route(
            "/api/user/shipping-details",
            post(upsert_user_shipping_details_handler),
        )
        .route("/api/auth/logout", post(logout_handler))
        .route("/api/session/guest/init", post(init_guest_session_handler))
        // Trasa główna i jej aliasy
        .route("/", get(home_page_handler))
        .route(
            "/sitemap.xml",
            get(|State(state): State<AppState>| async move {
                sitemap_generator::generate_sitemap_handler(&state).await
            }),
        )
        .route(
            "/{gender_slug}/{category}",
            get(dla_gender_with_category_handler),
        )
        .route("/{gender_slug}", get(dla_gender_handler))
        .route("/kategoria", get(list_products_htmx_handler))
        .route("/nowosci", get(news_page_htmx_handler))
        .route("/okazje", get(sale_page_htmx_handler))
        .route(
            "/produkty/{product_id}",
            get(get_product_detail_htmx_handler),
        )
        .route("/o-nas", get(about_us_page_handler))
        .route("/regulamin", get(terms_of_service_page_handler))
        .route("/polityka-prywatnosci", get(privacy_policy_page_handler))
        .route("/moje-konto", get(my_account_page_handler))
        .route("/moje-konto/zamowienia", get(my_orders_htmx_handler))
        .route(
            "/moje-konto/zamowienia/{order_id}",
            get(my_order_details_htmx_handler),
        )
        .route("/moje-konto/dane", get(my_account_data_htmx_handler))
        .route("/checkout", get(checkout_page_handler))
        .route("/wyszukiwanie", get(search_page_handler))
        .route(
            "/htmx/cart/add/{product_id}",
            post(add_item_to_cart_htmx_handler),
        )
        .route("/htmx/cart/details", get(get_cart_details_htmx_handler)) // TODO
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
        .route("/kontakt", get(contact_page_handler))
        .route("/htmx/page/faq", get(faq_page_handler))
        .route("/faq", get(faq_page_handler))
        .route(
            "/htmx/page/wysylka-i-zwroty",
            get(shipping_returns_page_handler),
        )
        .route("/wysylka-i-zwroty", get(shipping_returns_page_handler))
        .route("/htmx/logowanie", get(login_page_htmx_handler))
        .route("/logowanie", get(login_page_htmx_handler))
        .route("/htmx/rejestracja", get(registration_page_htmx_handler))
        .route("/rejestracja", get(registration_page_htmx_handler))
        .route("/htmx/my-account", get(my_account_page_handler))
        .route("/htmx/moje-konto/zamowienia", get(my_orders_htmx_handler))
        .route("/htmx/moje-konto/dane", get(my_account_data_htmx_handler))
        .route("/htmx/checkout", get(checkout_page_handler))
        .route(
            "/htmx/moje-konto/zamowienie-szczegoly/{order_id}",
            get(my_order_details_htmx_handler),
        )
        .route(
            "/moje-konto/zamowienie-szczegoly/{order_id}",
            get(my_order_details_htmx_handler),
        )
        .route("/admin", get(admin_dashboard_htmx_handler))
        .route("/htmx/admin", get(admin_dashboard_htmx_handler))
        .route(
            "/htmx/admin/products",
            get(admin_products_list_htmx_handler),
        )
        .route("/admin/produkty", get(admin_products_list_htmx_handler))
        .route("/admin/zamowienia", get(admin_orders_list_htmx_handler))
        .route(
            "/htmx/admin/products/new-form",
            get(admin_product_new_form_htmx_handler),
        )
        .route(
            "/htmx/admin/products/{product_id}/edit",
            get(admin_product_edit_form_htmx_handler),
        )
        .route(
            "/htmx/admin/order-details/{order_id}",
            get(admin_order_details_htmx_handler),
        )
        .route("/htmx/admin/orders", get(admin_orders_list_htmx_handler))
        .route(
            "/zamowienie/dziekujemy/{order_id}",
            get(payment_finalization_page_handler),
        )
        .route(
            "/htmx/zamowienie/dziekujemy/{order_id}",
            get(payment_finalization_page_handler),
        )
        .route("/api/auth/forgot-password", post(forgot_password_handler))
        .route("/api/auth/reset-password", post(reset_password_handler))
        .route("/zapomnialem-hasla", get(forgot_password_form_handler))
        .route("/htmx/zapomnialem-hasla", get(forgot_password_form_handler))
        .route("/resetuj-haslo", get(reset_password_form_handler))
        .route("/htmx/live-search", get(live_search_handler))
        .nest_service("/static", ServeDir::new("static"))
        .fallback(handler_404)
        .layer(TraceLayer::new_for_http())
        .layer(DefaultBodyLimit::max(100 * 1024 * 1024))
        .layer(cors)
        .with_state(app_state.clone());

    // --- NOWA, ROZBUDOWANA SEKCJA ROZGRZEWANIA CACHE'U ---
    tracing::info!("Rozpoczynam rozgrzewanie pamięci podręcznej...");
    let state_for_warmup = app_state;
    tokio::spawn(async move {
        // --- Rozgrzewanie Danych (Licznik kategorii) ---
        tracing::info!("Cache-warming: Licznik kategorii...");
        if let Err(e) = crate::services::get_categories_with_counts(
            &state_for_warmup,
            crate::models::ProductGender::Damskie,
        )
        .await
        {
            tracing::error!("Błąd podczas rozgrzewania cache'u dla 'Damskie': {}", e);
        }
        if let Err(e) = crate::services::get_categories_with_counts(
            &state_for_warmup,
            crate::models::ProductGender::Meskie,
        )
        .await
        {
            tracing::error!("Błąd podczas rozgrzewania cache'u dla 'Meskie': {}", e);
        }

        // --- Rozgrzewanie Stron HTML (dla gościa, bez filtrów) ---
        tracing::info!("Cache-warming: Kluczowe strony HTML...");

        // Przygotowujemy "udawane" parametry, jakich wymagają handlery
        let headers = axum::http::HeaderMap::new();
        let token_claims = crate::middleware::OptionalTokenClaims(None);
        let guest_id = crate::middleware::OptionalGuestCartId(None);

        // 1. Rozgrzewanie Strony Głównej
        if let Err(e) = crate::htmx_handlers::home_page_handler(
            headers.clone(),
            axum::extract::State(state_for_warmup.clone()),
            axum::extract::Query(crate::filters::ListingParams::default()),
            token_claims.clone(),
            guest_id.clone(),
        )
        .await
        {
            tracing::error!(
                "Błąd podczas rozgrzewania cache'u dla Strony Głównej: {:?}",
                e
            );
        }

        // 2. Rozgrzewanie Strony Nowości
        if let Err(e) = crate::htmx_handlers::news_page_htmx_handler(
            headers.clone(),
            axum::extract::State(state_for_warmup.clone()),
            axum::extract::Query(crate::filters::ListingParams::default()),
            token_claims.clone(),
            guest_id.clone(),
        )
        .await
        {
            tracing::error!("Błąd podczas rozgrzewania cache'u dla Nowości: {:?}", e);
        }

        // 3. Rozgrzewanie Strony Okazji
        if let Err(e) = crate::htmx_handlers::sale_page_htmx_handler(
            headers.clone(),
            axum::extract::State(state_for_warmup.clone()),
            axum::extract::Query(crate::filters::ListingParams::default()),
            token_claims.clone(),
            guest_id.clone(),
        )
        .await
        {
            tracing::error!("Błąd podczas rozgrzewania cache'u dla Okazji: {:?}", e);
        }

        tracing::info!("Zakończono rozgrzewanie pamięci podręcznej.");
    });

    // Adres i port, na którym serwer będzie nasłuchiwał
    let addr = SocketAddr::from(([0, 0, 0, 0], 3000)); // Nasłuchuj na wszystkich interfejsach na porcie 3000
    tracing::info!("Serwer nasłuchuje na {}", addr);

    // Konfiguracja TLS
    let config = match RustlsConfig::from_pem_file(
        PathBuf::from("localhost+2.pem"),     // Ścieżka do pliku certyfikatu
        PathBuf::from("localhost+2-key.pem"), // Ścieżka do pliku klucza
    )
    .await
    {
        Ok(config) => config,
        Err(e) => {
            tracing::error!("Błąd podczas ładowania certyfikatów TLS: {:?}", e);
            std::process::exit(1);
        }
    };

    if let Err(e) = axum_server::bind_rustls(addr, config)
        .serve(app.into_make_service())
        .await
    {
        tracing::error!("Błąd serwera: {}", e);
    }

    // Utworzenie listenera TCP
    // let listener = match TcpListener::bind(addr).await {
    //     Ok(listener) => listener,
    //     Err(e) => {
    //         tracing::error!("Nie można powiązać adresu {}: {}", addr, e);
    //         return; // Zakończ, jeśli nie można uruchomić serwera
    //     }
    // };

    // Uruchomienie serwera Axum
    // if let Err(e) = axum::serve(listener, app.into_make_service()).await {
    //     tracing::error!("Błąd serwera: {}", e);
    // }
}

#[allow(dead_code)]
async fn serve_index() -> Result<Html<String>, StatusCode> {
    match tokio::fs::read_to_string("static/index.html").await {
        Ok(content) => Ok(Html(content)),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}
