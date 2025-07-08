// src/main.rs

use axum::Router;
use axum::extract::{DefaultBodyLimit, State};
use axum::response::Html;
use axum::routing::{delete, get, post};
use axum_server::tls_rustls::RustlsConfig;
use dotenvy::dotenv;
use maud::Markup;
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
pub mod auth;
pub mod auth_models;
pub mod cart_utils;
pub mod cloudinary;
pub mod email_service;
pub mod errors;
pub mod extractor;
pub mod filters;
pub mod handlers;
pub mod htmx_handlers;
pub mod middleware;
pub mod models;
pub mod pagination;
pub mod response;
pub mod seo;
pub mod services;
pub mod sitemap_generator;
pub mod state;

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
    about_us_page_handler, admin_dashboard_htmx_handler, admin_order_details_htmx_handler,
    admin_orders_list_htmx_handler, admin_product_edit_form_htmx_handler,
    admin_product_new_form_htmx_handler, admin_products_list_htmx_handler, checkout_page_handler,
    contact_page_handler, dla_gender_handler, dla_gender_with_category_handler, faq_page_handler,
    forgot_password_form_handler, get_cart_details_htmx_handler, get_product_detail_htmx_handler,
    handler_404, home_page_handler, list_products_htmx_handler, live_search_handler,
    login_page_htmx_handler, my_account_data_htmx_handler, my_account_page_handler,
    my_order_details_htmx_handler, my_orders_htmx_handler, news_page_htmx_handler,
    payment_finalization_page_handler, privacy_policy_page_handler, registration_page_htmx_handler,
    remove_item_from_cart_htmx_handler, reset_password_form_handler, sale_page_htmx_handler,
    search_page_handler, shipping_returns_page_handler, terms_of_service_page_handler,
    toggle_cart_item_htmx_handler,
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
        .max_connections(10)
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

    async fn warm_static_cache(state: Arc<AppState>) {
        tracing::info!("Rozpoczynanie rozgrzewania cache'u dla stron statycznych...");

        type StaticPageRenderer = fn() -> Markup;

        use crate::htmx_handlers::{
            render_about_us_content, render_contact_page, render_faq_page,
            render_privacy_policy_content, render_shipping_returns_page, render_terms_of_service,
        };

        // Użyj zdefiniowanego typu w wektorze
        let pages_to_cache: Vec<(&str, StaticPageRenderer)> = vec![
            (
                "about_us_cache_key",
                render_about_us_content as StaticPageRenderer,
            ),
            (
                "privacy_policy_cache_key",
                render_privacy_policy_content as StaticPageRenderer,
            ),
            (
                "terms_of_policy_cache_key",
                render_terms_of_service as StaticPageRenderer,
            ),
            (
                "contact_page_cache_key",
                render_contact_page as StaticPageRenderer,
            ),
            ("faq_page_cache_key", render_faq_page as StaticPageRenderer),
            (
                "shipping_returns_cache_key",
                render_shipping_returns_page as StaticPageRenderer,
            ),
        ];

        for (key, renderer) in pages_to_cache {
            // Teraz `renderer` jest wskaźnikiem, więc wywołujemy go normalnie
            let content_html = renderer();
            let content_str = content_html.into_string();
            state
                .static_html_cache
                .insert(key.to_string(), content_str)
                .await;
        }
        tracing::info!(
            "Zakończono rozgrzewanie cache'u dla {} stron statycznych.",
            state.static_html_cache.entry_count()
        );
    }

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

    let category_list_cache = Arc::new(
        Cache::builder()
            .max_capacity(20)
            .time_to_live(Duration::from_secs(3600))
            .build(),
    );

    // Definicja AppState
    let app_state = Arc::new(AppState {
        db_pool: pool,
        jwt_secret,
        jwt_expiration_hours,
        cloudinary_config,
        resend_api_key,
        product_cache,
        static_html_cache,
        category_list_cache,
    });

    let state_for_warming = app_state.clone();
    tokio::spawn(async move {
        warm_static_cache(state_for_warming).await;
    });

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
            get(|State(state): State<Arc<AppState>>| async move {
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
            "/htmx/cart/toggle/{product_id}",
            post(toggle_cart_item_htmx_handler),
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
}

#[allow(dead_code)]
async fn serve_index() -> Result<Html<String>, StatusCode> {
    match tokio::fs::read_to_string("static/index.html").await {
        Ok(content) => Ok(Html(content)),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}
