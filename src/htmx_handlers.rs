// src/htmx_handlers.rs

use crate::AppState;
use crate::auth_models::TokenClaims;
use crate::cart_utils;
use crate::errors::AppError;
use crate::filters::ListingParams;
use crate::handlers::XGuestCartId;
use crate::models;
use crate::models::Product;
use crate::models::{CartDetailsResponse, Category, ProductGender};
use askama::Template;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::http::header::HeaderValue;
use axum::response::AppendHeaders;
use axum::response::{Html, IntoResponse, Response};
use axum_extra::TypedHeader;
use serde::Deserialize;
use serde_json::to_string;
use std::str::FromStr;
use strum::IntoEnumIterator;
use uuid::Uuid;

#[derive(Template, Debug, Clone)]
#[template(path = "product_grid.html")]
pub struct ProductGridTemplate {
    pub products: Vec<Product>,
    pub current_page: i64,
    pub total_pages: i64,
    pub per_page: i64,
    pub filter_query_string: String,
    pub current_listing_params_qs: String,
}

impl IntoResponse for ProductGridTemplate {
    fn into_response(self) -> Response {
        match self.render() {
            Ok(html_content) => Html(html_content).into_response(),
            Err(e) => {
                tracing::error!("Askama template rendering error: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!(
                        "Internal Server Error: Failed to render template. Details: {}",
                        e
                    ),
                )
                    .into_response()
            }
        }
    }
}

#[axum::debug_handler]
pub async fn list_products_htmx_handler(
    State(app_state): State<AppState>,
    Query(params): Query<ListingParams>,
) -> Result<ProductGridTemplate, AppError> {
    tracing::info!("HTMX: /htmx/products z parametrami: {:?}", params);

    let paginated_response_json =
        crate::handlers::list_products(State(app_state.clone()), Query(params.clone())).await?;
    let paginated_response = paginated_response_json.0;

    let filter_query_string = build_filter_only_query_string(&params);
    let current_listing_params_qs = build_full_query_string_from_params(&params);

    Ok(ProductGridTemplate {
        products: paginated_response.data,
        current_page: paginated_response.current_page,
        total_pages: paginated_response.total_pages,
        per_page: paginated_response.per_page,
        filter_query_string,
        current_listing_params_qs,
    })
}

pub struct HtmxCartAddResponse {
    new_item_count: usize,
    new_guest_cart_id: Option<Uuid>,
}

impl IntoResponse for HtmxCartAddResponse {
    fn into_response(self) -> Response {
        let mut event_map: serde_json::Map<String, serde_json::Value> = serde_json::Map::new();
        let mut update_cart_details: serde_json::Map<String, serde_json::Value> =
            serde_json::Map::new();

        update_cart_details.insert(
            "newCount".to_string(),
            serde_json::json!(self.new_item_count),
        );

        if let Some(id) = self.new_guest_cart_id {
            update_cart_details.insert(
                "newGuestCartId".to_string(),
                serde_json::json!(id.to_string()),
            );
        }
        event_map.insert(
            "updateCartCount".to_string(),
            serde_json::Value::Object(update_cart_details),
        );

        let hx_trigger_json_string = serde_json::to_string(&event_map).unwrap_or_else(|e| {
            tracing::error!(
                "Failed to serialize HX-Trigger details for updateCartCount: {}",
                e
            );
            // Prosty, bezpieczny fallback JSON
            format!(
                "{{\"updateCartCount\":{{\"newCount\":{}}}}}",
                self.new_item_count
            )
        });

        // Dodatkowo, można wyzwolić ogólny event 'cartUpdated', jeśli inne części strony mają na niego reagować
        // let final_hx_trigger_value = format!("{}, \"cartUpdated\":null", hx_trigger_json_string.trim_end_matches('}'));
        // Wymagałoby to jednak ostrożnego łączenia JSON, jeśli chcesz wiele eventów.
        // Na razie skupmy się na jednym poprawnym.

        match HeaderValue::from_str(&hx_trigger_json_string) {
            Ok(header_val) => {
                let headers = AppendHeaders([("HX-Trigger", header_val)]);
                (headers, StatusCode::OK).into_response()
            }
            Err(e) => {
                tracing::error!(
                    "Failed to create HeaderValue for HX-Trigger: {}. JSON string was: {}",
                    e,
                    hx_trigger_json_string
                );
                // Zwróć błąd serwera, jeśli nie można utworzyć nagłówka
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to construct HX-Trigger header",
                )
                    .into_response()
            }
        }
    }
}

#[axum::debug_handler]
pub async fn add_item_to_cart_htmx_handler(
    State(app_state): State<AppState>,
    Path(product_id): Path<Uuid>,
    user_claims_result: Result<TokenClaims, AppError>,
    guest_cart_id_header: Option<TypedHeader<XGuestCartId>>,
) -> Result<HtmxCartAddResponse, AppError> {
    tracing::info!("HTMX: Próba dodania produktu {} do koszyka", product_id);

    let mut tx = app_state.db_pool.begin().await?;
    let cart: models::ShoppingCart; // Zadeklaruj jako mut, jeśli new_guest_cart_id_generated jest używane do modyfikacji
    let mut new_guest_cart_id_generated: Option<Uuid> = None;

    if let Ok(claims) = user_claims_result {
        let user_id = claims.sub;
        cart = match sqlx::query_as("SELECT * FROM shopping_carts WHERE user_id = $1 FOR UPDATE")
            .bind(user_id)
            .fetch_optional(&mut *tx)
            .await?
        {
            Some(c) => c,
            None => {
                sqlx::query_as("INSERT INTO shopping_carts (user_id) VALUES ($1) RETURNING *")
                    .bind(user_id)
                    .fetch_one(&mut *tx)
                    .await?
            }
        };
        tracing::info!(
            "HTMX: Zalogowany użytkownik (ID: {}), koszyk ID: {}",
            user_id,
            cart.id
        );
    } else if let Some(TypedHeader(XGuestCartId(guest_id))) = guest_cart_id_header {
        cart = match sqlx::query_as(
            "SELECT * FROM shopping_carts WHERE guest_session_id = $1 FOR UPDATE",
        )
        .bind(guest_id)
        .fetch_optional(&mut *tx)
        .await?
        {
            Some(c) => c,
            None => {
                tracing::warn!(
                    "HTMX: Koszyk gościa z nagłówka (ID: {}) nie znaleziony. Tworzenie nowego koszyka.",
                    guest_id
                );
                let new_id = Uuid::new_v4();
                new_guest_cart_id_generated = Some(new_id);

                let created_cart = sqlx::query_as::<_, models::ShoppingCart>(
                    "INSERT INTO shopping_carts (guest_session_id) VALUES ($1) RETURNING *",
                )
                .bind(new_id)
                .fetch_one(&mut *tx)
                .await?;
                tracing::info!("HTMX: Stworzono NOWY koszyk gościa (ID: {})", new_id);
                created_cart // <-- Zwracamy utworzony koszyk
            }
        };
        // Jeśli koszyk został znaleziony (nie wszedł do bloku None), new_guest_cart_id_generated pozostanie None
        if new_guest_cart_id_generated.is_none() {
            tracing::info!(
                "HTMX: Użytkownik-gość (Session ID z nagłówka: {}), koszyk ID: {}",
                guest_id,
                cart.id
            );
        }
    } else {
        let new_id = Uuid::new_v4();
        new_guest_cart_id_generated = Some(new_id);
        cart =
            sqlx::query_as("INSERT INTO shopping_carts (guest_session_id) VALUES ($1) RETURNING *")
                .bind(new_id)
                .fetch_one(&mut *tx)
                .await?;
        tracing::info!("HTMX: Stworzono nowy koszyk gościa (ID: {})", new_id);
    }

    let product_opt = sqlx::query_as::<_, models::Product>(
        "SELECT * FROM products WHERE id = $1 AND status = 'Available' FOR UPDATE",
    )
    .bind(product_id)
    .fetch_optional(&mut *tx)
    .await?;

    if product_opt.is_none() {
        tracing::warn!("HTMX: Produkt {} niedostępny lub nie istnieje.", product_id);
        tx.rollback().await?;
        return Err(AppError::NotFound);
    }

    sqlx::query("INSERT INTO cart_items (cart_id, product_id) VALUES ($1, $2) ON CONFLICT (cart_id, product_id) DO NOTHING")
        .bind(cart.id).bind(product_id).execute(&mut *tx).await?;

    let cart_details = cart_utils::build_cart_details_response(&cart, &mut *tx).await?;
    tx.commit().await?;

    tracing::info!(
        "HTMX: Produkt {} dodany do koszyka {}. Nowa liczba pozycji: {}. Nowa suma: {}",
        product_id,
        cart.id,
        cart_details.total_items,
        cart_details.total_price
    );

    Ok(HtmxCartAddResponse {
        new_item_count: cart_details.total_items,
        new_guest_cart_id: new_guest_cart_id_generated,
    })
}

#[derive(Template)]
#[template(path = "cart_content.html")] // NOWY SZABLON ASKAMA
pub struct CartContentTemplate {
    items: Vec<models::CartItemPublic>, // Lub bezpośrednio Vec<Product> jeśli tak wolisz
    total_price: i64,
    cart_item_count: usize,
}

// Jeśli implementujesz IntoResponse ręcznie
impl IntoResponse for CartContentTemplate {
    fn into_response(self) -> Response {
        match self.render() {
            Ok(html) => Html(html).into_response(),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Error rendering cart content: {}", e),
            )
                .into_response(),
        }
    }
}

#[axum::debug_handler]
pub async fn get_cart_details_htmx_handler(
    State(app_state): State<AppState>,
    user_claims_result: Result<TokenClaims, AppError>, // ZMIENIONO NA RESULT
    guest_cart_id_header: Option<TypedHeader<XGuestCartId>>,
) -> Result<CartContentTemplate, AppError> {
    tracing::info!("HTMX: Pobieranie zawartości koszyka");
    let mut conn = app_state.db_pool.acquire().await?;

    let cart_optional: Option<models::ShoppingCart> = if let Ok(claims) = user_claims_result {
        // SPRAWDZAMY RESULT
        sqlx::query_as("SELECT * FROM shopping_carts WHERE user_id = $1")
            .bind(claims.sub)
            .fetch_optional(&mut *conn)
            .await?
    } else if let Some(TypedHeader(XGuestCartId(guest_id))) = guest_cart_id_header {
        sqlx::query_as("SELECT * FROM shopping_carts WHERE guest_session_id = $1")
            .bind(guest_id)
            .fetch_optional(&mut *conn)
            .await?
    } else {
        None
    };

    if let Some(cart) = cart_optional {
        let cart_details: CartDetailsResponse =
            cart_utils::build_cart_details_response(&cart, &mut conn).await?;
        Ok(CartContentTemplate {
            items: cart_details.items,
            total_price: cart_details.total_price,
            cart_item_count: cart_details.total_items,
        })
    } else {
        Ok(CartContentTemplate {
            items: vec![],
            total_price: 0,
            cart_item_count: 0,
        })
    }
}

pub struct HtmxCartRemoveResponse {
    new_cart_content_html: CartContentTemplate,
    new_item_count: usize,
}

impl IntoResponse for HtmxCartRemoveResponse {
    fn into_response(self) -> Response {
        // Renderuj szablon CartContentTemplate do HTML
        match self.new_cart_content_html.render() {
            Ok(html_body) => {
                // Przygotuj HX-Trigger do aktualizacji licznika w navbarze
                let event_details_json = format!("{{\"newCount\": {}}}", self.new_item_count);
                let hx_trigger_value = format!("{{\"updateCartCount\": {}}}", event_details_json);

                match HeaderValue::from_str(&hx_trigger_value) {
                    Ok(trigger_header_val) => {
                        let headers = AppendHeaders([("HX-Trigger", trigger_header_val)]);
                        // Zwróć HTML i nagłówki
                        (headers, Html(html_body)).into_response()
                    }
                    Err(e) => {
                        tracing::error!(
                            "Failed to create HeaderValue for HX-Trigger on cart remove: {}",
                            e
                        );
                        // Jeśli nie można stworzyć nagłówka, zwróć HTML, ale zaloguj błąd
                        (StatusCode::INTERNAL_SERVER_ERROR, Html(html_body)).into_response()
                    }
                }
            }
            Err(e) => {
                tracing::error!(
                    "Failed to render CartContentTemplate after item removal: {}",
                    e
                );
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Error rendering updated cart: {}", e),
                )
                    .into_response()
            }
        }
    }
}

#[axum::debug_handler]
pub async fn remove_item_from_cart_htmx_handler(
    State(app_state): State<AppState>,
    Path(product_id): Path<Uuid>,
    user_claims_result: Result<TokenClaims, AppError>,
    guest_cart_id_header: Option<TypedHeader<XGuestCartId>>,
) -> Result<HtmxCartRemoveResponse, AppError> {
    tracing::info!("HTMX: Próba usunięcia produktu {} z koszyka", product_id);

    let mut tx = app_state.db_pool.begin().await?;

    // 1. Znajdź koszyk użytkownika (zalogowanego lub gościa)
    let cart_optional: Option<models::ShoppingCart> = if let Ok(claims) = user_claims_result {
        sqlx::query_as("SELECT * FROM shopping_carts WHERE user_id = $1 FOR UPDATE")
            .bind(claims.sub)
            .fetch_optional(&mut *tx)
            .await?
    } else if let Some(TypedHeader(XGuestCartId(guest_id))) = guest_cart_id_header {
        sqlx::query_as("SELECT * FROM shopping_carts WHERE guest_session_id = $1 FOR UPDATE")
            .bind(guest_id)
            .fetch_optional(&mut *tx)
            .await?
    } else {
        None // Nie ma koszyka do modyfikacji
    };

    if cart_optional.is_none() {
        tracing::warn!("HTMX Remove: Nie znaleziono koszyka do usunięcia produktu.");
        // Zwróć pusty koszyk lub odpowiedni błąd/stan, który nie zmienia UI drastycznie
        // lub wyzwól event, aby UI pokazało, że koszyk jest pusty.
        // Na razie, jeśli nie ma koszyka, zwrócimy "pusty" zaktualizowany koszyk.
        tx.rollback().await?; // Ważne, aby wycofać transakcję
        return Ok(HtmxCartRemoveResponse {
            new_cart_content_html: CartContentTemplate {
                items: vec![],
                total_price: 0,
                cart_item_count: 0,
            },
            new_item_count: 0,
        });
    }

    let cart = cart_optional.unwrap();

    // 2. Usuń produkt z cart_items
    let _delete_result =
        sqlx::query("DELETE FROM cart_items WHERE cart_id = $1 AND product_id = $2")
            .bind(cart.id)
            .bind(product_id)
            .execute(&mut *tx)
            .await?;

    // 3. Pobierz zaktualizowaną zawartość koszyka (po usunięciu)
    // Używamy &mut *tx, ponieważ build_cart_details_response może modyfikować updated_at koszyka
    let updated_cart_details: CartDetailsResponse =
        cart_utils::build_cart_details_response(&cart, &mut *tx).await?;

    tx.commit().await?;

    tracing::info!(
        "HTMX: Produkt {} usunięty z koszyka {}. Nowa liczba pozycji: {}",
        product_id,
        cart.id,
        updated_cart_details.total_items
    );

    Ok(HtmxCartRemoveResponse {
        new_cart_content_html: CartContentTemplate {
            items: updated_cart_details.items,
            total_price: updated_cart_details.total_price,
            cart_item_count: updated_cart_details.total_items,
        },
        new_item_count: updated_cart_details.total_items,
    })
}

#[derive(Debug, Deserialize)]
pub struct DetailViewParams {
    #[serde(default)]
    return_params: Option<String>,
}
#[derive(Template)]
#[template(path = "product_detail_view.html")]
pub struct ProductDetailTemplate {
    pub product: Product,                        // Cały obiekt produktu
    pub formatted_price: String,                 // Sformatowana cena
    pub product_images_json: String,             // JSON string z listą URL-i obrazków dla galerii
    pub return_query_params_str: Option<String>, // Opcjonalny query string dla linku "Wróć"
    pub product_name_for_js: String,
}

impl IntoResponse for ProductDetailTemplate {
    fn into_response(self) -> Response {
        match self.render() {
            Ok(html) => Html(html).into_response(),
            Err(e) => {
                tracing::error!("Failed to render product_detail_view.html template: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Error rendering product details: {}", e),
                )
                    .into_response()
            }
        }
    }
}

#[axum::debug_handler]
pub async fn get_product_detail_htmx_handler(
    State(app_state): State<AppState>,
    Path(product_id): Path<Uuid>,
    Query(query_params): Query<DetailViewParams>,
) -> Result<ProductDetailTemplate, AppError> {
    tracing::info!("HTMX: Pobieranie szczegółów produktu ID: {}", product_id);

    let product_result = sqlx::query_as::<_, Product>(
        r#"SELECT id, name, description, price, gender, condition, category, status, images
           FROM products
           WHERE id = $1"#,
    )
    .bind(product_id)
    .fetch_one(&app_state.db_pool)
    .await;

    match product_result {
        // Zakładam, że product_result to Twój wynik zapytania
        Ok(product_data) => {
            let formatted_price_string =
                format!("{:.2}", (product_data.price as f64) / 100.0).replace('.', ",") + " zł";

            let images_json_string =
                serde_json::to_string(&product_data.images).unwrap_or_else(|e| {
                    tracing::error!("Failed to serialize product.images for JS: {}", e);
                    "[]".to_string()
                });

            // NOWE: Przygotowanie nazwy produktu jako bezpiecznego stringu JSON/JavaScript
            let product_name_js_safe =
                serde_json::to_string(&product_data.name).unwrap_or_else(|e| {
                    tracing::error!("Failed to serialize product.name for JS: {}", e);
                    "\"Błąd nazwy produktu\"".to_string() // Fallback na poprawny string JSON
                });

            Ok(ProductDetailTemplate {
                product: product_data,
                formatted_price: formatted_price_string,
                product_images_json: images_json_string,
                return_query_params_str: query_params.return_params,
                product_name_for_js: product_name_js_safe, // PRZEKAŻ NOWE POLE
            })
        }
        Err(sqlx::Error::RowNotFound) => {
            tracing::warn!("HTMX: Nie znaleziono produktu o ID: {}", product_id);
            Err(AppError::NotFound)
        }
        Err(e) => {
            tracing::error!(
                "HTMX: Błąd bazy danych podczas pobierania produktu {}: {:?}",
                product_id,
                e
            );
            Err(AppError::from(e))
        }
    }
}

#[derive(Template)]
#[template(path = "gender_category_page.html")]
pub struct GenderPageTemplate {
    pub current_gender: ProductGender,
    pub categories: Vec<Category>,
    pub products_payload: ProductGridTemplate,
}

impl IntoResponse for GenderPageTemplate {
    fn into_response(self) -> Response {
        match self.render() {
            Ok(html) => Html(html).into_response(),
            Err(e) => {
                tracing::error!(
                    "Askama (gender_category_page) template rendering error: {}",
                    e
                );
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Error rendering gender page: {}", e),
                )
                    .into_response()
            }
        }
    }
}

#[axum::debug_handler]
pub async fn gender_page_htmx_handler(
    State(app_state): State<AppState>,
    Path(gender_str): Path<String>,
) -> Result<GenderPageTemplate, AppError> {
    // ... (parsowanie gender, pobieranie all_categories) ...
    let gender = ProductGender::from_str(&gender_str)
        .map_err(|_| AppError::BadRequest("Nieprawidłowa płeć".to_string()))?;
    let all_categories: Vec<Category> = Category::iter().collect();

    let initial_listing_params = ListingParams::new_with_gender_and_defaults(Some(gender.clone())); // Użyj Twojego konstruktora

    let paginated_response_json = crate::handlers::list_products(
        State(app_state.clone()),
        Query(initial_listing_params.clone()),
    )
    .await?;
    let paginated_response = paginated_response_json.0;

    let filter_query_string_for_initial_grid =
        build_filter_only_query_string(&initial_listing_params);
    let initial_current_listing_params_qs =
        build_full_query_string_from_params(&initial_listing_params);

    let initial_products_payload = ProductGridTemplate {
        products: paginated_response.data,
        current_page: paginated_response.current_page,
        total_pages: paginated_response.total_pages,
        per_page: paginated_response.per_page,
        filter_query_string: filter_query_string_for_initial_grid,
        current_listing_params_qs: initial_current_listing_params_qs,
    };

    Ok(GenderPageTemplate {
        current_gender: gender,
        categories: all_categories,
        products_payload: initial_products_payload,
    })
}

impl ListingParams {
    pub fn new_with_gender_and_defaults(gender: Option<ProductGender>) -> Self {
        Self {
            gender,
            limit: None,
            offset: None,
            category: None,
            condition: None,
            status: None,
            price_min: None,
            price_max: None,
            sort_by: None,
            order: None,
        }
    }
}

fn build_full_query_string_from_params(params: &ListingParams) -> String {
    let mut query_parts = Vec::new();
    // Paginacja
    query_parts.push(format!("limit={}", params.limit()));
    query_parts.push(format!("offset={}", params.offset()));
    // Filtry
    if let Some(g) = params.gender() {
        query_parts.push(format!("gender={}", g.to_string()));
    }
    if let Some(c) = params.category() {
        query_parts.push(format!("category={}", c.to_string()));
    }
    if let Some(cond) = params.condition() {
        query_parts.push(format!("condition={}", cond.to_string()));
    }
    if let Some(stat) = params.status() {
        query_parts.push(format!("status={}", stat.to_string()));
    }
    if let Some(p_min) = params.price_min() {
        query_parts.push(format!("price_min={}", p_min));
    }
    if let Some(p_max) = params.price_max() {
        query_parts.push(format!("price_max={}", p_max));
    }
    // Sortowanie
    query_parts.push(format!("sort_by={}", params.sort_by()));
    query_parts.push(format!("order={}", params.order()));

    query_parts.join("&")
}

// Funkcja pomocnicza do budowania query string dla filtrów (do paginacji)
fn build_filter_only_query_string(params: &ListingParams) -> String {
    let mut filter_parts = Vec::new();
    if let Some(g) = params.gender() {
        filter_parts.push(format!("gender={}", g.to_string()));
    }
    if let Some(c) = params.category() {
        filter_parts.push(format!("category={}", c.to_string()));
    }
    if let Some(cond) = params.condition() {
        filter_parts.push(format!("condition={}", cond.to_string()));
    }
    if let Some(stat) = params.status() {
        filter_parts.push(format!("status={}", stat.to_string()));
    } // Dodaj status
    if let Some(p_min) = params.price_min() {
        filter_parts.push(format!("price_min={}", p_min));
    }
    if let Some(p_max) = params.price_max() {
        filter_parts.push(format!("price_max={}", p_max));
    }
    filter_parts.push(format!("sort_by={}", params.sort_by()));
    filter_parts.push(format!("order={}", params.order()));

    if filter_parts.is_empty() {
        String::new()
    } else {
        format!("&amp;{}", filter_parts.join("&amp;")) // Zaczyna się od &amp;
    }
}
