use axum::http::HeaderValue;
// src/handlers.rs
use axum::response::IntoResponse;
use axum::{Form, Json};
use axum::{
    extract::{Multipart, Path, Query, State},
    http::{HeaderMap, StatusCode},
};
use axum_extra::TypedHeader;
use axum_extra::extract::cookie::{Cookie, SameSite};
use chrono::{Duration, Utc};
use maud::{Markup, html};
use serde_json::{Value, json};
use sqlx::{Postgres, QueryBuilder};
use time;

use crate::cart_utils::build_cart_details_response;
use crate::cloudinary::{delete_image_from_cloudinary, extract_public_id_from_url};
#[allow(unused_imports)]
use crate::email_service::{send_order_confirmation_email, send_password_reset_email};
use crate::errors::AppError;
use crate::filters::{ListingParams, OrderListingParams};
use crate::htmx_handlers::{
    render_admin_product_list_row_maud, render_checkout_error_page_maud, render_thank_you_page_maud,
};
use crate::middleware::OptionalTokenClaims;
use crate::models::Product;
use crate::models::*;
use crate::pagination::{PaginatedOrdersResponse, PaginatedProductsResponse};
use crate::{
    auth::{create_jwt, hash_password, verify_password},
    cloudinary::upload_image_to_cloudinary,
    state::AppState,
};
use crate::{
    auth_models::{LoginPayload, RegistrationPayload, TokenClaims},
    models::{Order, OrderStatus, ProductGender, ProductStatus, Role, User},
};
use futures::future::try_join_all;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use uuid::Uuid;
use validator::Validate;

pub async fn get_product_details(
    State(app_state): State<Arc<AppState>>,
    Path(product_id): Path<Uuid>,
) -> Result<Json<Product>, AppError> {
    // KROK 1: Sprawdź cache
    if let Some(product) = app_state.product_cache.get(&product_id).await {
        tracing::info!("Cache HIT dla produktu o ID: {}", product_id);
        return Ok(Json(product));
    }

    // KROK 2: Jeśli w cache'u nie ma, pobierz z bazy danych
    tracing::info!(
        "Cache MISS dla produktu o ID: {}. Pobieranie z bazy danych.",
        product_id
    );

    let product_result = sqlx::query_as::<_, Product>(r#"SELECT * FROM products WHERE id = $1"#)
        .bind(product_id)
        .fetch_one(&app_state.db_pool)
        .await;

    match product_result {
        Ok(product) => {
            // KROK 3: Zapisz pobrany produkt w cache'u na przyszłość
            app_state
                .product_cache
                .insert(product.id, product.clone())
                .await;
            Ok(Json(product))
        }
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
            Err(AppError::from(e))
        }
    }
}

pub async fn list_products(
    State(app_state): State<Arc<AppState>>,
    Query(params): Query<ListingParams>,
) -> Result<Json<PaginatedProductsResponse>, AppError> {
    tracing::info!(
        "Obsłużono zapytanie GET /api/products z parametrami: {:?}",
        params
    );

    let limit = params.limit();
    let offset = params.offset();

    let mut query_builder: QueryBuilder<Postgres> =
        QueryBuilder::new("SELECT *, COUNT(*) OVER() as total_count FROM products");

    let mut conditions_added = false;
    let mut append_where_or_and = |builder: &mut QueryBuilder<Postgres>| {
        if !conditions_added {
            builder.push(" WHERE ");
            conditions_added = true;
        } else {
            builder.push(" AND ");
        }
    };

    // --- KROK 2: Dodajemy wszystkie filtry (klauzule WHERE) ---
    if let Some(gender) = params.gender() {
        append_where_or_and(&mut query_builder);
        query_builder.push("gender = ").push_bind(gender);
    }
    if let Some(category) = params.category() {
        append_where_or_and(&mut query_builder);
        query_builder.push("category = ").push_bind(category);
    }
    if let Some(condition) = params.condition() {
        append_where_or_and(&mut query_builder);
        query_builder.push("condition = ").push_bind(condition);
    }
    match params.status.as_deref() {
        Some("all") => {}
        Some(status_str) => {
            if let Ok(status_enum) = ProductStatus::from_str(status_str) {
                append_where_or_and(&mut query_builder);
                query_builder.push("status = ").push_bind(status_enum);
            }
        }
        None => {
            append_where_or_and(&mut query_builder);
            query_builder.push("status IN ('Available', 'Reserved')");
        }
    }
    if let Some(price_min) = params.price_min() {
        append_where_or_and(&mut query_builder);
        query_builder.push("price >= ").push_bind(price_min);
    }
    if let Some(price_max) = params.price_max() {
        append_where_or_and(&mut query_builder);
        query_builder.push("price <= ").push_bind(price_max);
    }
    if let Some(on_sale_filter) = params.on_sale() {
        append_where_or_and(&mut query_builder);
        query_builder.push("on_sale = ").push_bind(on_sale_filter);
    }
    if let Some(search_term) = params.search() {
        append_where_or_and(&mut query_builder);
        let like_pattern = format!("%{}%", search_term);
        query_builder
            .push("(name ILIKE ")
            .push_bind(like_pattern.clone())
            .push(" OR description ILIKE ")
            .push_bind(like_pattern)
            .push(")");
    }

    // --- KROK 3: Dodajemy sortowanie i paginację ---
    let sort_by_column = match params.sort_by() {
        "price" => "price",
        "created_at" => "created_at",
        "name" | _ => "name",
    };
    query_builder.push(format!(
        " ORDER BY {} {}, id ASC",
        sort_by_column,
        params.order()
    ));
    query_builder.push(" LIMIT ").push_bind(limit);
    query_builder.push(" OFFSET ").push_bind(offset);

    // --- KROK 4: Wykonujemy zapytanie i mapujemy wyniki ---
    let products_with_count: Vec<ProductWithTotalCount> = query_builder
        .build_query_as()
        .fetch_all(&app_state.db_pool)
        .await?;

    let total_items = products_with_count
        .first()
        .map_or(0, |row| row.total_count.unwrap_or(0));

    let products: Vec<Product> = products_with_count
        .into_iter()
        .map(|p_wc| Product {
            id: p_wc.id,
            name: p_wc.name,
            description: p_wc.description,
            price: p_wc.price,
            gender: p_wc.gender,
            condition: p_wc.condition,
            category: p_wc.category,
            status: p_wc.status,
            images: p_wc.images,
            on_sale: p_wc.on_sale,
            created_at: p_wc.created_at,
            updated_at: p_wc.updated_at,
        })
        .collect();

    // --- KROK 5: Obliczamy paginację i zwracamy odpowiedź ---
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

pub async fn create_product_handler(
    State(app_state): State<Arc<AppState>>,
    claims: TokenClaims,
    mut multipart: Multipart,
) -> Result<(StatusCode, HeaderMap, String), AppError> {
    if claims.role != Role::Admin {
        return Err(AppError::UnauthorizedAccess(
            "Tylko administrator może dodawać produkty".to_string(),
        ));
    }
    tracing::info!("Obsłużono zapytanie POST /api/products - tworzenie produktu");

    let mut text_fields: HashMap<String, String> = HashMap::new();
    let mut image_uploads: Vec<(String, Vec<u8>)> = Vec::new();

    while let Some(field) = multipart.next_field().await? {
        let field_name = match field.name() {
            Some(name) => name.to_string(),
            None => {
                tracing::warn!("Odebrano pole multipart bez nazwy, pomijam");
                continue;
            }
        };
        let original_filename_opt = field.file_name().map(|s| s.to_string());
        tracing::info!(
            "Przetwarzanie pola: name={}, filename='{:?}'",
            field_name,
            original_filename_opt
        );
        if field_name.starts_with("image_file_") {
            let filename = original_filename_opt.unwrap_or_else(|| format!("{}.jpg", field_name));
            match field.bytes().await {
                Ok(bytes) => {
                    if !bytes.is_empty() {
                        image_uploads.push((filename.clone(), bytes.to_vec()));
                        tracing::info!(
                            "Dodano plik do image_uploads: {}, rozmiar: {} bajtów",
                            filename,
                            bytes.len()
                        )
                    } else {
                        tracing::warn!(
                            "Odebrano puste pole pliku (po odczytaniu bajtów): {}",
                            filename
                        );
                    }
                }
                Err(e) => {
                    tracing::error!("Błąd odczytu bajtów z pola pliku '{}': {:?}", field_name, e);
                    return Err(AppError::from(e));
                }
            }
        } else {
            match field.text().await {
                Ok(value) => {
                    text_fields.insert(field_name.clone(), value);
                    tracing::info!(
                        "Dodano pole tekstowe: name={}, value='{}'",
                        field_name,
                        text_fields.get(&field_name).unwrap_or(&"".to_string()),
                    );
                }
                Err(e) => {
                    tracing::error!("Błąd odczytu tekstu z pola '{}': {:?}", field_name, e);
                    return Err(AppError::from(e));
                }
            }
        }
    }

    let name = text_fields
        .get("name")
        .ok_or_else(|| AppError::UnprocessableEntity("Brak pola 'name'.".to_string()))?
        .clone();
    let description = text_fields
        .get("description")
        .ok_or_else(|| AppError::UnprocessableEntity("Brak pola 'description'".to_string()))?
        .clone();
    let price_str = text_fields
        .get("price")
        .ok_or_else(|| AppError::UnprocessableEntity("Brak pola 'price'.".to_string()))?
        .clone();
    let gender_str = text_fields
        .get("gender")
        .ok_or_else(|| AppError::UnprocessableEntity("Brak pola 'gender'.".to_string()))?
        .clone();
    let condition_str = text_fields
        .get("condition")
        .ok_or_else(|| AppError::UnprocessableEntity("Brak pola 'condition'.".to_string()))?
        .clone();
    let category_str = text_fields
        .get("category")
        .ok_or_else(|| AppError::UnprocessableEntity("Brak pola 'category'.".to_string()))?
        .clone();
    let on_sale_str = text_fields.get("on_sale").map_or("false", |s| s.as_str());
    let on_sale = on_sale_str.eq_ignore_ascii_case("true") || on_sale_str == "on";
    if image_uploads.is_empty() {
        return Err(AppError::UnprocessableEntity(
            "Należy przesłac conajmniej jeden plik obrazu ('image_file)".to_string(),
        ));
    }

    let price: i64 = price_str.parse().map_err(|_| {
        AppError::UnprocessableEntity("Pole 'price' musi być liczbą całkowitą".to_string())
    })?;
    let gender = ProductGender::from_str(&gender_str).map_err(|_| {
        AppError::UnprocessableEntity(format!(
            "Nieprawidłowa wartość pola 'gender': {}",
            gender_str
        ))
    })?;
    let condition = ProductCondition::from_str(&condition_str).map_err(|_| {
        AppError::UnprocessableEntity(format!(
            "Nieprawidłowa wartość pola 'condition': {}",
            condition_str
        ))
    })?;
    let category = Category::from_str(&category_str).map_err(|_| {
        AppError::UnprocessableEntity(format!(
            "Nieprawidłowa wartość pola 'category': {}",
            category_str
        ))
    })?;

    if name.is_empty() || name.len() > 255 {
        return Err(AppError::UnprocessableEntity(
            "Nieprawidłowa długość pola 'name'".to_string(),
        ));
    }
    if description.len() > 5000 {
        return Err(AppError::UnprocessableEntity(
            "Pole 'description' jest za długie".to_string(),
        ));
    }
    if price < 0 {
        return Err(AppError::UnprocessableEntity(
            "Cena nie może być ujemna".to_string(),
        ));
    }

    let mut image_upload_futures = Vec::new();
    for (filename, bytes) in image_uploads {
        let config_clone = app_state.cloudinary_config.clone();
        image_upload_futures
            .push(async move { upload_image_to_cloudinary(bytes, filename, &config_clone).await });
    }

    let cloudinary_urls: Vec<String> = try_join_all(image_upload_futures).await?;
    tracing::info!(
        "Wszystkie obrazy przesłane do Cloudinary, URL'e: {:?}",
        cloudinary_urls
    );

    let new_product_id = Uuid::new_v4();
    let product_status = ProductStatus::Available;
    sqlx::query_as::<_, Product>(
        r#"
            INSERT INTO products (id, name, description, price, gender, condition, category, status, images, on_sale)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            RETURNING id, name, description, price, gender, condition , category, status, images, on_sale, created_at, updated_at
        "#,
    )
    .bind(new_product_id)
    .bind(&name)
    .bind(&description)
    .bind(price)
    .bind(gender)
    .bind(condition)
    .bind(category)
    .bind(product_status)
    .bind(&cloudinary_urls)
    .bind(on_sale)
    .fetch_one(&app_state.db_pool)
    .await?;
    tracing::info!("Utworzono produkt o ID: {}", new_product_id);

    let mut headers = HeaderMap::new();
    let toast_payload = json!({
        "showMessage": {
            "message": "Pomyslnie dodano produkt.",
            "type": "success"
        }
    });
    if let Ok(val) = HeaderValue::from_str(&toast_payload.to_string()) {
        headers.insert("HX-Trigger", val);
    }
    let location_payload = json!({
        "path": "/htmx/admin/products",
        "target": "#admin-content",
        "swap": "innerHTML"
    });
    if let Ok(val) = HeaderValue::from_str(&location_payload.to_string()) {
        headers.insert("HX-Location", val);
    }
    Ok((StatusCode::CREATED, headers, String::new()))
}

pub async fn update_product_partial_handler(
    State(app_state): State<Arc<AppState>>,
    Path(product_id): Path<Uuid>,
    claims: TokenClaims,
    mut multipart: Multipart,
) -> Result<Json<Product>, AppError> {
    if claims.role != Role::Admin {
        return Err(AppError::UnauthorizedAccess(
            "Tylko administrator może aktualizować produkty".to_string(),
        ));
    }
    tracing::info!(
        "Obsłużono zapytanie PATCH /api/products/{} - aktualizacja (multipart)",
        product_id
    );

    // --- POCZĄTEK REFAKTORYZACJI ---

    // KROK 1: Przetwarzamy dane z formularza i wgrywamy pliki W PAMIĘCI, bez otwierania transakcji.
    let mut text_fields: HashMap<String, String> = HashMap::new();
    let mut new_image_uploads: Vec<(String, Vec<u8>)> = Vec::new();
    let mut urls_to_delete_json_opt: Option<String> = None;

    while let Some(field) = multipart.next_field().await.map_err(AppError::from)? {
        let field_name = match field.name() {
            Some(name) => name.to_string(),
            None => continue,
        };
        if field_name.starts_with("image_file_") {
            if let Some(filename) = field.file_name().map(|s| s.to_string()) {
                let bytes = field.bytes().await.map_err(AppError::from)?;
                if !bytes.is_empty() {
                    new_image_uploads.push((filename.clone(), bytes.into()));
                }
            }
        } else if field_name == "urls_to_delete" {
            urls_to_delete_json_opt = Some(field.text().await.map_err(AppError::from)?);
        } else {
            text_fields.insert(field_name, field.text().await.map_err(AppError::from)?);
        }
    }

    // KROK 2: Wykonujemy operacje na Cloudinary (usuwanie) - nadal BEZ transakcji.
    let urls_to_delete: Vec<String> = if let Some(json_str) = urls_to_delete_json_opt {
        if !json_str.is_empty() && json_str != "[]" {
            serde_json::from_str(&json_str).map_err(|e| {
                tracing::error!("Błąd parsowania JSON dla urls_to_delete: '{}'", e);
                AppError::UnprocessableEntity(
                    "Nieprawidłowy format listy URLi do usunięcia.".to_string(),
                )
            })?
        } else {
            vec![]
        }
    } else {
        vec![]
    };

    if !urls_to_delete.is_empty() {
        let mut delete_futures = Vec::new();
        for url_to_delete in &urls_to_delete {
            if let Some(public_id) =
                extract_public_id_from_url(url_to_delete, &app_state.cloudinary_config.cloud_name)
            {
                let config_clone = app_state.cloudinary_config.clone();
                delete_futures.push(async move {
                    delete_image_from_cloudinary(&public_id, &config_clone).await
                });
            }
        }
        if let Err(e) = try_join_all(delete_futures).await {
            return Err(AppError::from(e));
        }
    }

    // KROK 3: Wykonujemy operacje na Cloudinary (upload) - nadal BEZ transakcji.
    let mut uploaded_urls: Vec<String> = Vec::new();
    if !new_image_uploads.is_empty() {
        let mut upload_futures = Vec::new();
        for (filename, bytes) in new_image_uploads {
            let config_clone = app_state.cloudinary_config.clone();
            upload_futures.push(async move {
                upload_image_to_cloudinary(bytes, filename, &config_clone).await
            });
        }
        uploaded_urls = try_join_all(upload_futures).await?;
    }

    // KROK 4: DOPIERO TERAZ, gdy wszystkie operacje zewnętrzne się powiodły, otwieramy krótką transakcję.
    let mut tx = app_state.db_pool.begin().await?;

    let mut existing_product =
        sqlx::query_as::<_, Product>("SELECT * FROM products WHERE id = $1 FOR UPDATE")
            .bind(product_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(|_| AppError::NotFound)?;

    // Aktualizujemy pola produktu w pamięci
    if let Some(name) = text_fields.get("name") {
        existing_product.name = name.clone();
    }
    if let Some(desc) = text_fields.get("description") {
        existing_product.description = desc.clone();
    }
    if let Some(price) = text_fields.get("price") {
        existing_product.price = price
            .parse()
            .map_err(|_| AppError::UnprocessableEntity("Zły format ceny".into()))?;
    }
    if let Some(gender) = text_fields.get("gender") {
        existing_product.gender = ProductGender::from_str(gender)
            .map_err(|_| AppError::UnprocessableEntity("Zła płeć".into()))?;
    }
    if let Some(cond) = text_fields.get("condition") {
        existing_product.condition = ProductCondition::from_str(cond)
            .map_err(|_| AppError::UnprocessableEntity("Zły stan".into()))?;
    }
    if let Some(cat) = text_fields.get("category") {
        existing_product.category = Category::from_str(cat)
            .map_err(|_| AppError::UnprocessableEntity("Zła kategoria".into()))?;
    }
    if let Some(stat) = text_fields.get("status") {
        existing_product.status = ProductStatus::from_str(stat)
            .map_err(|_| AppError::UnprocessableEntity("Zły status".into()))?;
    }
    existing_product.on_sale = text_fields
        .get("on_sale")
        .map_or(false, |s| s.eq_ignore_ascii_case("true") || s == "on");

    // Aktualizujemy listę obrazków
    existing_product
        .images
        .retain(|url| !urls_to_delete.contains(url));
    existing_product.images.extend(uploaded_urls);

    if existing_product.images.is_empty() {
        return Err(AppError::UnprocessableEntity(
            "Produkt musi mieć co najmniej jeden obrazek.".to_string(),
        ));
    }

    // KROK 5: Wykonujemy JEDNO zapytanie UPDATE w naszej krótkiej transakcji.
    let updated_product_db = sqlx::query_as::<_, Product>(
        r#"
            UPDATE products
            SET name = $1, description = $2, price = $3, gender = $4, condition = $5, category = $6, status = $7, images = $8, on_sale = $9, updated_at = NOW()
            WHERE id = $10
            RETURNING *
        "#,
    )
    .bind(&existing_product.name)
    .bind(&existing_product.description)
    .bind(existing_product.price)
    .bind(existing_product.gender)
    .bind(existing_product.condition)
    .bind(existing_product.category)
    .bind(existing_product.status)
    .bind(&existing_product.images)
    .bind(existing_product.on_sale)
    .bind(product_id)
    .fetch_one(&mut *tx)
    .await?;

    // KROK 6: Zamykamy transakcję. Całość trwała ułamki sekund.
    tx.commit().await?;

    tracing::info!("Pomyślnie zaktualizowano produkt o ID: {}", product_id);
    Ok(Json(updated_product_db))
}

pub async fn archivize_product_handler(
    State(app_state): State<Arc<AppState>>,
    Path(product_id): Path<Uuid>,
    claims: TokenClaims,
    Query(params): Query<ListingParams>,
) -> Result<Markup, AppError> {
    tracing::info!(
        "Obsłużono żądanie SOFT DELETE / ARCHIVIZE /api/products/{}",
        product_id
    );

    if claims.role != Role::Admin {
        return Err(AppError::UnauthorizedAccess(
            "Tylko administrator może usuwać produkty".to_string(),
        ));
    }

    // Aktualizujemy status na "Archived"
    let update_result =
        sqlx::query("UPDATE products SET status = $1, updated_at = NOW() WHERE id = $2")
            .bind(ProductStatus::Archived)
            .bind(product_id)
            .execute(&app_state.db_pool)
            .await?;

    if update_result.rows_affected() == 0 {
        tracing::warn!(
            "ARCHIVIZE: Nie znaleziono produktu o ID {} do zarchiwizowania",
            product_id
        );
        return Err(AppError::NotFound); // Zwracamy błąd, jeśli produkt nie istnieje
    }

    // Pobieramy zaktualizowany produkt z bazy, aby mieć świeże dane
    let updated_product = sqlx::query_as::<_, Product>("SELECT * FROM products WHERE id = $1")
        .bind(product_id)
        .fetch_one(&app_state.db_pool)
        .await?;

    tracing::info!("Zarchiwizowano produkt o ID: {}", product_id);

    // Renderujemy i zwracamy HTML dla zaktualizowanego wiersza
    Ok(render_admin_product_list_row_maud(
        &updated_product,
        &params,
    ))
}

// ZMIANA: Nowa funkcja do trwałego usuwania produktów
pub async fn permanent_delete_product_handler(
    State(app_state): State<Arc<AppState>>,
    Path(product_id): Path<Uuid>,
    claims: TokenClaims,
) -> Result<(StatusCode, HeaderMap), AppError> {
    tracing::info!(
        "Obsłużono żądanie PERMANENT DELETE /api/products/{}",
        product_id
    );

    if claims.role != Role::Admin {
        return Err(AppError::UnauthorizedAccess(
            "Tylko administrator może trwale usuwać produkty".to_string(),
        ));
    }

    let mut tx = app_state.db_pool.begin().await?;

    // KROK 1: Sprawdź, czy produkt nie jest powiązany z żadnym zamówieniem.
    let is_in_order = sqlx::query("SELECT 1 FROM order_items WHERE product_id = $1 LIMIT 1")
        .bind(product_id)
        .fetch_optional(&mut *tx)
        .await?
        .is_some();

    if is_in_order {
        tx.rollback().await?; // Zakończ transakcję
        tracing::warn!(
            "Próba trwałego usunięcia produktu (ID: {}), który jest częścią zamówienia.",
            product_id
        );
        return Err(AppError::Conflict("Nie można usunąć produktu, który jest częścią istniejących zamówień. Zamiast tego zarchiwizuj go.".to_string()));
    }

    // KROK 2: Pobierz produkt, aby uzyskać listę obrazów do usunięcia z Cloudinary
    let product_to_delete =
        sqlx::query_as::<_, Product>("SELECT * FROM products WHERE id = $1 FOR UPDATE")
            .bind(product_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(AppError::NotFound)?;

    // KROK 3: Usuń obrazy z Cloudinary
    if !product_to_delete.images.is_empty() {
        let mut delete_futures = Vec::new();
        for image_url in &product_to_delete.images {
            if let Some(public_id) =
                extract_public_id_from_url(image_url, &app_state.cloudinary_config.cloud_name)
            {
                let config_clone = app_state.cloudinary_config.clone();
                let public_id_clone = public_id.to_string();
                delete_futures.push(async move {
                    delete_image_from_cloudinary(&public_id_clone, &config_clone).await
                });
            }
        }
        if let Err(e) = try_join_all(delete_futures).await {
            tx.rollback().await.ok();
            tracing::error!(
                "Błąd usuwania obrazów z Cloudinary podczas trwałego usuwania: {:?}",
                e
            );
            return Err(AppError::from(e));
        }
    }

    // KROK 4: Trwale usuń produkt z bazy danych
    let delete_result = sqlx::query("DELETE FROM products WHERE id = $1")
        .bind(product_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

    if delete_result.rows_affected() > 0 {
        tracing::info!("Trwale usunięto produkt o ID: {}", product_id);
    }

    // KROK 5: Wyślij odpowiedź do HTMX
    let mut headers = HeaderMap::new();
    headers.insert(
        "HX-Trigger",
        HeaderValue::from_static("reloadAdminProductList"),
    );
    let toast_payload = json!({
        "showMessage": {
            "message": "Produkt zostal trwale usuniety.",
            "type": "success"
        }
    });
    if let Ok(val) = HeaderValue::from_str(&toast_payload.to_string()) {
        headers.insert("HX-Trigger", val);
    }
    Ok((StatusCode::OK, headers))
}

pub async fn register_handler(
    State(app_state): State<Arc<AppState>>,
    Form(payload): Form<RegistrationPayload>,
) -> Result<impl IntoResponse, AppError> {
    // 1. Walidacja danych wejściowych
    if let Err(validation_errors) = payload.validate() {
        tracing::warn!("Błąd walidacji danych rejestracji: {:?}", validation_errors);
        let mut headers = HeaderMap::new();
        headers.insert("HX-Reswap", HeaderValue::from_static("none"));

        let mut error_message = "Niepoprawne dane w formularzu.".to_string();
        if let Some(field_errors) = validation_errors.field_errors().values().next() {
            if let Some(first_error) = field_errors.get(0) {
                if let Some(msg) = &first_error.message {
                    error_message = msg.to_string();
                } else {
                    error_message = first_error.code.to_string();
                }
            }
        }

        let trigger_payload = json!({
            "showMessage": {"message": error_message, "type": "error"}
        });
        if let Ok(trigger_value) = HeaderValue::from_str(&trigger_payload.to_string()) {
            headers.insert("HX-Trigger", trigger_value);
        }
        return Ok((
            StatusCode::UNPROCESSABLE_ENTITY,
            headers,
            Json(
                json!({ "error": "Validation failed", "details_str": validation_errors.to_string() }),
            ), // Zmieniono "details" na "details_str" lub serializuj inaczej
        ));
    }

    // 2. Sprawdzanie czy użytkownik istnieje
    let existing_user: Option<User> = sqlx::query_as(
        r#"
            SELECT id, email, password_hash, role, created_at, updated_at
            FROM users
            WHERE email = $1
        "#,
    )
    .bind(&payload.email)
    .fetch_optional(&app_state.db_pool)
    .await
    .map_err(|e| {
        tracing::error!(
            "Błąd bazy danych podczas sprawdzania emaila {}: {:?}",
            payload.email,
            e
        );
        AppError::SqlxError(e)
    })?;

    if existing_user.is_some() {
        tracing::warn!("Próba rejestracji z istniejącym emailem: {}", payload.email);
        let mut headers = HeaderMap::new();
        headers.insert("HX-Reswap", HeaderValue::from_static("none"));
        let trigger_payload = json!({
            "showMessage": {"message": "Podany adres email jest juz zarejestrowany.", "type": "error"}
        });
        if let Ok(trigger_value) = HeaderValue::from_str(&trigger_payload.to_string()) {
            headers.insert("HX-Trigger", trigger_value);
        }
        return Ok((
            StatusCode::CONFLICT,
            headers,
            Json(json!({"message": "Email już istnieje"})),
        ));
    }

    // 3. Hash hasła
    let password_hash = match hash_password(&payload.password) {
        Ok(ph) => ph,
        Err(e) => {
            tracing::error!("Błąd hashowania hasła: {:?}", e);
            let mut headers = HeaderMap::new();
            headers.insert("HX-Reswap", HeaderValue::from_static("none"));
            let trigger_payload = json!({
                "showMessage": {"message": "Błąd serwera podczas przetwarzania danych.", "type": "error"}
            });
            if let Ok(trigger_value) = HeaderValue::from_str(&trigger_payload.to_string()) {
                headers.insert("HX-Trigger", trigger_value);
            }
            return Ok((
                StatusCode::INTERNAL_SERVER_ERROR,
                headers,
                Json(json!({"message": "Błąd serwera"})),
            ));
        }
    };

    // 4. Wstawianie nowego użytkownika
    let new_user = match sqlx::query_as::<_, User>(
        r#"INSERT INTO users (email, password_hash, role) 
           VALUES ($1, $2, $3)
           RETURNING id, email, password_hash, role, created_at, updated_at"#,
    )
    .bind(&payload.email)
    .bind(&password_hash)
    .bind(Role::Customer)
    .fetch_one(&app_state.db_pool)
    .await
    {
        Ok(user) => user,
        Err(e) => {
            tracing::error!("Błąd wstawiania nowego użytkownika do bazy danych: {:?}", e);
            let mut headers = HeaderMap::new();
            headers.insert("HX-Reswap", HeaderValue::from_static("none"));
            let trigger_payload = json!({
                "showMessage": {"message": "Nie udało się utworzyć konta. Spróbuj ponownie.", "type": "error"}
            });
            if let Ok(trigger_value) = HeaderValue::from_str(&trigger_payload.to_string()) {
                headers.insert("HX-Trigger", trigger_value);
            }
            return Ok((
                StatusCode::INTERNAL_SERVER_ERROR,
                headers,
                Json(json!({"message": "Błąd bazy danych"})),
            ));
        }
    };

    tracing::info!(
        "Zarejestrowano nowego użytkownika: {} (ID: {})",
        new_user.email,
        new_user.id
    );

    // 5. Sukces - przygotowanie odpowiedzi z nagłówkami HTMX
    let mut headers = HeaderMap::new();
    headers.insert("HX-Reswap", HeaderValue::from_static("none"));

    let trigger_payload = json!({
        "registrationComplete": { "userId": new_user.id.to_string() },
        "showMessage": {"message": "Rejestracja pomyslna! Mozesz sie teraz zalogowac.", "type": "success"}
    });
    if let Ok(trigger_value) = HeaderValue::from_str(&trigger_payload.to_string()) {
        headers.insert("HX-Trigger", trigger_value);
    }

    let user_public_data: UserPublic = new_user.into();

    // Zmieniona linia: user_public_data jest konwertowane na serde_json::Value
    Ok((StatusCode::CREATED, headers, Json(json!(user_public_data))))
}

pub async fn login_handler(
    State(app_state): State<Arc<AppState>>,
    Form(payload): Form<LoginPayload>,
) -> Result<impl IntoResponse, AppError> {
    // 1. Walidacja danych wejściowych
    if let Err(validation_errors) = payload.validate() {
        // Możesz chcieć przekształcić validation_errors na bardziej przyjazny komunikat
        // lub zwrócić szczegóły błędów walidacji.
        // Na razie zwracamy generyczny AppError::Validation.
        tracing::warn!("Błąd walidacji danych logowania: {:?}", validation_errors);
        let mut headers = HeaderMap::new();
        headers.insert("HX-Reswap", HeaderValue::from_static("none"));
        let trigger_payload = json!({
            "showMessage": {"message": "Niepoprawne dane w formularzu.", "type": "error"}
        });
        if let Ok(trigger_value) = HeaderValue::from_str(&trigger_payload.to_string()) {
            headers.insert("HX-Trigger", trigger_value);
        }
        // Użyj Statuscode::UNPROCESSABLE_ENTITY dla błędów walidacji, jeśli AppError::Validation tego nie robi.
        // Tutaj zakładam, że AppError::Validation(validation_errors) poprawnie zwróci 422.
        return Err(AppError::Validation("Błąd walidacji danych".to_string()));
    }

    // 2. Znajdowanie użytkownika po emailu
    let user_optional = sqlx::query_as::<_, User>(
        r#"
            SELECT id, email, password_hash, role, created_at, updated_at
            FROM users
            WHERE email = $1
        "#,
    )
    .bind(&payload.email)
    .fetch_optional(&app_state.db_pool)
    .await
    .map_err(|e| {
        tracing::error!(
            "Błąd bazy danych podczas wyszukiwania użytkownika {}: {:?}",
            payload.email,
            e
        );
        AppError::SqlxError(e) // Lub bardziej generyczny błąd serwera
    })?;

    let user = match user_optional {
        Some(u) => u,
        None => {
            // Użytkownik nie znaleziony
            tracing::warn!(
                "Nieudana próba logowania: użytkownik {} nie znaleziony.",
                payload.email
            );
            let mut headers = HeaderMap::new();
            headers.insert("HX-Reswap", HeaderValue::from_static("none"));
            let trigger_payload = json!({
                "showMessage": {"message": "Nieprawidlowy email lub haslo.", "type": "error"}
            });
            if let Ok(trigger_value) = HeaderValue::from_str(&trigger_payload.to_string()) {
                headers.insert("HX-Trigger", trigger_value);
            }
            return Ok((
                StatusCode::UNAUTHORIZED,
                headers,
                Json(json!({"message": "Nieprawidłowy email lub hasło."})),
            ));
        }
    };

    // 3. Weryfikacja hasła
    match verify_password(&user.password_hash, &payload.password) {
        Ok(is_valid) => {
            if !is_valid {
                tracing::warn!(
                    "Nieudana próba logowania dla {}: nieprawidłowe hasło.",
                    payload.email
                );
                let mut headers = HeaderMap::new();
                headers.insert("HX-Reswap", HeaderValue::from_static("none"));
                let trigger_payload = json!({
                    "showMessage": {"message": "Nieprawidlowy email lub haslo.", "type": "error"}
                });
                if let Ok(trigger_value) = HeaderValue::from_str(&trigger_payload.to_string()) {
                    headers.insert("HX-Trigger", trigger_value);
                }
                return Ok((
                    StatusCode::UNAUTHORIZED,
                    headers,
                    Json(json!({"message": "Nieprawidłowy email lub hasło."})),
                ));
            }
        }
        Err(e) => {
            tracing::error!(
                "Błąd podczas weryfikacji hasła dla {}: {:?}",
                payload.email,
                e
            );
            // To jest błąd serwera, a nie błędne hasło per se
            let mut headers = HeaderMap::new();
            headers.insert("HX-Reswap", HeaderValue::from_static("none"));
            let trigger_payload = json!({
                "showMessage": {"message": "Blad serwera podczas weryfikacji danych.", "type": "error"}
            });
            if let Ok(trigger_value) = HeaderValue::from_str(&trigger_payload.to_string()) {
                headers.insert("HX-Trigger", trigger_value);
            }
            return Ok((
                StatusCode::INTERNAL_SERVER_ERROR,
                headers,
                Json(json!({"message": "Blad serwera"})),
            ));
        }
    }

    // 4. Logowanie pomyślne - generowanie tokenu JWT
    match create_jwt(
        user.id, // Używamy ID i roli użytkownika pobranego z bazy
        user.role,
        &app_state.jwt_secret,
        app_state.jwt_expiration_hours,
    ) {
        Ok(token_str) => {
            // --- POCZĄTEK NOWEGO KODU ---
            // Tworzymy bezpieczne ciasteczko z naszym tokenem JWT.
            let cookie = Cookie::build(("token", token_str.clone())) // Klonujemy token, bo użyjemy go też w triggerze
                .path("/") // Ciasteczko będzie dostępne na całej stronie
                .http_only(true) // Ważne: ciasteczko niedostępne dla JavaScript po stronie klienta
                .secure(true) // Wysyłane tylko przez HTTPS
                .same_site(SameSite::Lax) // Dobra ochrona przed atakami CSRF - Lax
                .max_age(time::Duration::days(365))
                .build();

            let mut headers = HeaderMap::new();
            // Dodajemy nagłówek `Set-Cookie`, który każe przeglądarce zapisać nasze ciasteczko.
            headers.insert(
                axum::http::header::SET_COOKIE,
                cookie.to_string().parse().unwrap(),
            );
            // --- KONIEC NOWEGO KODU ---

            // Istniejąca logika nagłówków HTMX pozostaje bez zmian
            headers.insert("HX-Reswap", HeaderValue::from_static("none"));

            let trigger_payload = json!({
                // Przekazujemy token do JS, aby mógł go zapisać w localStorage (dla HTMX)
                "loginSuccessDetails": {"token": token_str},
                "showMessage": {"message": "Zalogowano pomyslnie!", "type": "success"}
            });
            if let Ok(trigger_value) = HeaderValue::from_str(&trigger_payload.to_string()) {
                headers.insert("HX-Trigger", trigger_value);
            }

            tracing::info!(
                "Użytkownik {} ({}) zalogowany pomyślnie. Ustawiono ciasteczko.",
                user.email,
                user.id
            );

            Ok((StatusCode::OK, headers, Json(json!({"status": "success"}))))
        }
        Err(e) => {
            tracing::error!("Błąd generowania tokenu JWT dla {}: {:?}", user.email, e);
            let mut headers = HeaderMap::new();
            headers.insert("HX-Reswap", HeaderValue::from_static("none"));
            let trigger_payload = json!({
                "showMessage": {"message": "Blad serwera podczas finalizowania logowania.", "type": "error"}
            });
            if let Ok(trigger_value) = HeaderValue::from_str(&trigger_payload.to_string()) {
                headers.insert("HX-Trigger", trigger_value);
            }
            Ok((
                StatusCode::INTERNAL_SERVER_ERROR,
                headers,
                Json(json!({"message": "Błąd serwera"})),
            ))
        }
    }
}

pub async fn protected_route_handler(claims: TokenClaims) -> Result<Json<Value>, AppError> {
    Ok(Json(
        json!({ "message": "Gratulacje! Masz dostep do chronionego zasobu.",
            "user_id": claims.sub,
            "user_role": claims.role,
            "expires_at": claims.exp }),
    ))
}

#[axum::debug_handler]
pub async fn create_order_handler(
    State(app_state): State<Arc<AppState>>,
    OptionalTokenClaims(user_claims_opt): OptionalTokenClaims,
    guest_cart_id_header: Option<TypedHeader<XGuestCartId>>,
    Form(payload): Form<CheckoutFormPayload>,
) -> Result<(HeaderMap, Markup), AppError> {
    if let Err(validation_errors) = payload.validate() {
        tracing::warn!("Błąd walidacji danych checkout: {:?}", validation_errors);
        let mut headers = HeaderMap::new();
        let error_message_str = validation_errors.to_string();
        headers.insert(
            "HX-Trigger",
            HeaderValue::from_str(&format!(
                r#"{{"showMessage": {{"message": "Bledy w formularzu: {}", "type": "error"}}}}"#,
                error_message_str.replace('"', "\\\"")
            ))
            .map_err(|_| {
                AppError::InternalServerError("Failed to create HX-Trigger header".to_string())
            })?,
        );
        headers.insert("HX-Reswap", HeaderValue::from_static("none"));
        return Ok((headers, html! {}));
    }

    let mut order_user_id: Option<Uuid> = None;
    let mut order_guest_email: Option<String> = None;
    let mut order_guest_session_id: Option<Uuid> = None;
    let cart_query_id: Uuid;
    let cart_selector_sql: String;

    if let Some(claims) = user_claims_opt {
        let user_id = claims.sub;
        order_user_id = Some(user_id);
        cart_query_id = user_id;
        cart_selector_sql =
            "SELECT * FROM shopping_carts WHERE user_id = $1 FOR UPDATE".to_string();
        tracing::info!("Zalogowany użytkownik {} składa zamówienie.", user_id);
    } else if let Some(TypedHeader(XGuestCartId(guest_id))) = guest_cart_id_header {
        // Sprawdzamy, czy gość podał e-mail i czy ten e-mail istnieje już w bazie użytkowników.
        if let Some(email_to_check) = payload.guest_checkout_email.as_deref() {
            if !email_to_check.trim().is_empty() {
                // Wykonujemy zapytanie do bazy PRZED rozpoczęciem transakcji.
                let user_exists: Option<i32> =
                    sqlx::query_scalar("SELECT 1 FROM users WHERE email = $1")
                        .bind(email_to_check)
                        .fetch_optional(&app_state.db_pool)
                        .await?;

                if user_exists.is_some() {
                    // E-mail istnieje! Blokujemy zamówienie i wysyłamy błąd.
                    tracing::warn!(
                        "Gość (sesja: {}) próbował złożyć zamówienie na zarejestrowany adres e-mail: {}",
                        guest_id,
                        email_to_check
                    );

                    let mut headers = HeaderMap::new();
                    let trigger_payload = json!({
                        "showMessage": {
                            "message": "Ten adres e-mail jest juz zarejestrowany. Zaloguj sie, aby kontynuowac.",
                            "type": "error"
                        }
                    });

                    // Używamy .unwrap(), ponieważ wiemy, że nasz JSON jest poprawny
                    headers.insert(
                        "HX-Trigger",
                        HeaderValue::from_str(&trigger_payload.to_string()).unwrap(),
                    );
                    headers.insert("HX-Reswap", HeaderValue::from_static("none")); // Nie podmieniaj widoku!

                    // Zwracamy nasz nowy, niestandardowy błąd z nagłówkami.
                    return Err(AppError::ConflictWithHeaders(
                        "Email jest już zarejestrowany.".to_string(),
                        headers,
                    ));
                }
            }
        }

        order_guest_session_id = Some(guest_id);
        cart_query_id = guest_id;
        cart_selector_sql =
            "SELECT * FROM shopping_carts WHERE guest_session_id = $1 FOR UPDATE".to_string();
        if payload.guest_checkout_email.is_none()
            || payload
                .guest_checkout_email
                .as_deref()
                .unwrap_or("")
                .trim()
                .is_empty()
        {
            tracing::warn!("Gość próbował złożyć zamówienie bez podania emaila.");
            let mut headers = HeaderMap::new();
            headers.insert("HX-Trigger", HeaderValue::from_static(r#"{"showMessage": {"message": "Adres email jest wymagany dla zamówień gości.", "type": "error"}}"#));
            headers.insert("HX-Reswap", HeaderValue::from_static("none"));
            return Ok((headers, html! {}));
        }
        order_guest_email = payload
            .guest_checkout_email
            .clone()
            .filter(|s| !s.is_empty());
        tracing::info!(
            "Gość (sesja: {:?}, email: {:?}) składa zamówienie.",
            guest_id,
            order_guest_email
        );
    } else {
        tracing::error!("Próba złożenia zamówienia bez identyfikacji użytkownika lub gościa.");
        return Err(AppError::UnauthorizedAccess(
            "Nie można zidentyfikować użytkownika ani sesji gościa.".to_string(),
        ));
    }

    let mut tx = app_state.db_pool.begin().await?;

    let cart = match sqlx::query_as::<_, ShoppingCart>(&cart_selector_sql)
        .bind(cart_query_id)
        .fetch_optional(&mut *tx)
        .await?
    {
        Some(c) => c,
        None => {
            tracing::warn!(
                "Nie znaleziono koszyka dla identyfikatora: {}",
                cart_query_id
            );
            return Err(AppError::UnprocessableEntity(
                "Twój koszyk nie został znaleziony lub jest pusty.".to_string(),
            ));
        }
    };

    let cart_items_db =
        sqlx::query_as::<_, CartItem>("SELECT * FROM cart_items WHERE cart_id = $1 FOR UPDATE")
            .bind(cart.id)
            .fetch_all(&mut *tx)
            .await?;

    if cart_items_db.is_empty() {
        tracing::warn!("Koszyk (ID: {}) jest pusty.", cart.id);
        return Err(AppError::UnprocessableEntity(
            "Twój koszyk jest pusty.".to_string(),
        ));
    }

    // ZMIANA: Optymalizacja N+1 - pobieranie wszystkich produktów jednym zapytaniem.
    let product_ids: Vec<Uuid> = cart_items_db.iter().map(|item| item.product_id).collect();
    let products_in_cart =
        sqlx::query_as::<_, Product>("SELECT * FROM products WHERE id = ANY($1) FOR UPDATE")
            .bind(&product_ids)
            .fetch_all(&mut *tx)
            .await?;

    let products_map: HashMap<Uuid, Product> =
        products_in_cart.into_iter().map(|p| (p.id, p)).collect();

    let mut order_items_to_create: Vec<(Uuid, i64)> = Vec::with_capacity(cart_items_db.len());
    let mut total_price_items: i64 = 0;
    let mut product_ids_to_mark_sold: Vec<Uuid> = Vec::new();

    for cart_item in &cart_items_db {
        match products_map.get(&cart_item.product_id) {
            Some(p) => {
                if p.status != ProductStatus::Available {
                    tracing::warn!(
                        "Produkt {} (ID: {}) w koszyku jest niedostępny (status: {:?}).",
                        p.name,
                        p.id,
                        p.status
                    );
                    let error_html = render_checkout_error_page_maud(&p.name);
                    return Err(AppError::UnprocessableEntityWithHtml(error_html));
                }
                order_items_to_create.push((p.id, p.price));
                total_price_items += p.price;
                product_ids_to_mark_sold.push(p.id);
            }
            None => {
                tracing::error!(
                    "Produkt o ID {} (z koszyka) nie został znaleziony w bazie.",
                    cart_item.product_id
                );
                return Err(AppError::InternalServerError(
                    "Błąd spójności danych: produkt z koszyka nie istnieje.".to_string(),
                ));
            }
        }
    }

    // REFAKTORYZACJA: Przeniesienie stałych do bardziej elastycznej konfiguracji.
    // Na razie zostawiamy je tutaj, ale z komentarzem.
    // TODO: Przenieść mapowanie kluczy metod dostawy na koszty i nazwy do konfiguracji lub bazy danych.
    const SHIPPING_INPOST_COST: i64 = 1199;
    const SHIPPING_INPOST_NAME: &str = "Paczkomat InPost 24/7";
    const SHIPPING_POCZTA_COST: i64 = 1799;
    const SHIPPING_POCZTA_NAME: &str = "Poczta Polska S.A.";
    const SHIPPING_FREE_NAME: &str = "Darmowa dostawa";
    const FREE_SHIPPING_THRESHOLD: i64 = 25000;

    let (derived_shipping_cost, shipping_method_name_to_store): (i64, String) = match payload
        .shipping_method_key
        .as_str()
    {
        "inpost" => (SHIPPING_INPOST_COST, SHIPPING_INPOST_NAME.to_string()),
        "poczta" => (SHIPPING_POCZTA_COST, SHIPPING_POCZTA_NAME.to_string()),
        "darmowa" => {
            // WAŻNA WALIDACJA: Sprawdź, czy serwer zgadza się, że dostawa jest darmowa.
            if total_price_items >= FREE_SHIPPING_THRESHOLD {
                (0, SHIPPING_FREE_NAME.to_string())
            } else {
                // Jeśli ktoś spróbuje oszukać i wysłać "darmowa" przy zbyt małym zamówieniu
                tracing::warn!(
                    "Próba użycia darmowej dostawy dla zamówienia poniżej progu: {}",
                    total_price_items
                );
                // Zwracamy błąd lub przypisujemy domyślną, płatną metodę
                return Err(AppError::BadRequest(
                    "Nie kwalifikujesz się do darmowej dostawy.".to_string(),
                ));
            }
        }
        _ => {
            tracing::warn!(
                "Nieprawidłowy lub brakujący klucz metody dostawy: '{}'",
                payload.shipping_method_key
            );
            let mut headers = HeaderMap::new();
            headers.insert("HX-Trigger", HeaderValue::from_str(r#"{{"showMessage": {{"message": "Proszę wybrać prawidłową metodę dostawy.", "type": "error"}}}}"#)
                .map_err(|_| AppError::InternalServerError("Failed to create HX-Trigger for shipping method".to_string()))?);
            headers.insert("HX-Reswap", HeaderValue::from_static("none"));
            return Ok((headers, html! {}));
        }
    };

    let payment_method_enum = PaymentMethod::from_str(&payload.payment_method)
        .map_err(|_| AppError::Validation("Nieprawidłowa metoda płatności.".to_string()))?;

    let final_total_price = total_price_items + derived_shipping_cost;
    let initial_status = OrderStatus::Pending;
    let order_id = Uuid::new_v4();

    sqlx::query(
        r#"
            INSERT INTO orders (
                id, user_id, guest_email, guest_session_id, status, total_price,
                shipping_first_name, shipping_last_name, shipping_address_line1, shipping_address_line2,
                shipping_city, shipping_postal_code, shipping_country, shipping_phone, 
                payment_method, shipping_method_name
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
        "#,
    )
    .bind(order_id)
    .bind(order_user_id)
    .bind(option_string_empty_as_none(order_guest_email))
    .bind(order_guest_session_id)
    .bind(initial_status)
    .bind(final_total_price)
    .bind(&payload.shipping_first_name)
    .bind(&payload.shipping_last_name)
    .bind(&payload.shipping_address_line1)
    .bind(option_string_empty_as_none(payload.shipping_address_line2.clone()))
    .bind(&payload.shipping_city)
    .bind(&payload.shipping_postal_code)
    .bind(&payload.shipping_country)
    .bind(&payload.shipping_phone)
    .bind(payment_method_enum)
    .bind(Some(shipping_method_name_to_store.clone()))
    .execute(&mut *tx)
    .await?;

    for (product_id, price_at_purchase) in order_items_to_create {
        sqlx::query(
            "INSERT INTO order_items (order_id, product_id, price_at_purchase) VALUES ($1, $2, $3)",
        )
        .bind(order_id)
        .bind(product_id)
        .bind(price_at_purchase)
        .execute(&mut *tx)
        .await?;
    }

    sqlx::query("DELETE FROM cart_items WHERE cart_id = $1")
        .bind(cart.id)
        .execute(&mut *tx)
        .await?;

    if order_user_id.is_none() && cart.guest_session_id.is_some() {
        sqlx::query("DELETE FROM shopping_carts WHERE id = $1")
            .bind(cart.id)
            .execute(&mut *tx)
            .await?;
        tracing::info!(
            "Usunięto koszyk gościa (ID: {}) po złożeniu zamówienia.",
            cart.id
        );
    }

    // ZMIANA: Status produktu zmieniony na 'Sold', nie 'Reserved'
    if !product_ids_to_mark_sold.is_empty() {
        sqlx::query(r#"UPDATE products SET status = $1 WHERE id = ANY($2)"#)
            .bind(ProductStatus::Sold)
            .bind(&product_ids_to_mark_sold)
            .execute(&mut *tx)
            .await?;
    }

    tx.commit().await?;

    // === WYSYŁANIE E-MAIL ===
    // match fetch_order_details_service(&app_state.db_pool, order_id).await {
    //     Ok(details) => {
    //         if let Err(e) = send_order_confirmation_email(&app_state, &details).await {
    //             tracing::error!(
    //                 "Nie udało się wysłać e-maila z potwierdzeniem dla zamówienia {}: {:?}",
    //                 order_id,
    //                 e
    //             );
    //         }
    //     }
    //     Err(e) => {
    //         tracing::error!(
    //             "Nie udało się pobrać szczegółów zamówienia {} do wysłania e-maila: {:?}",
    //             order_id,
    //             e
    //         );
    //     }
    // }

    tracing::info!(
        "Utworzono nowe zamówienie ID: {} z metodą dostawy: '{}', koszt dostawy: {} gr, suma końcowa: {} gr",
        order_id,
        shipping_method_name_to_store,
        derived_shipping_cost,
        final_total_price
    );

    // 1. Pobierz pełne szczegóły właśnie utworzonego zamówienia
    // Używamy `fetch_order_details_service`, który już mamy!
    let order_details = fetch_order_details_service(&app_state.db_pool, order_id).await?;

    // 2. Wyrenderuj widok strony z podziękowaniem, używając naszej nowej funkcji
    let final_response_html =
        render_thank_you_page_maud(&order_details.order, &order_details.items);

    // 5. Przygotuj nagłówki dla HTMX
    let mut headers = HeaderMap::new();

    // Ustaw nagłówek HX-Push, aby zaktualizować URL w przeglądarce.
    // Atrybut hx-push-url="true" na formularzu go użyje.
    let final_url = format!("/zamowienie/dziekujemy/{}", order_id);
    headers.insert("HX-Push", HeaderValue::from_str(&final_url).unwrap());

    // Wyślij zdarzenia do wyczyszczenia licznika koszyka i pokazania toasta.
    // Nadal używamy HX-Trigger do tych pobocznych zadań.
    let trigger_payload = json!({
        "clearCartDisplay": {},
        "showMessage": {
            "message": "Twoje zamowienie zostalo pomyslnie zlozone!",
            "type": "success"
        }
    });
    headers.insert(
        "HX-Trigger",
        HeaderValue::from_str(&trigger_payload.to_string()).unwrap(),
    );

    // 6. Zwróć nagłówki i wyrenderowany kod HTML jako ciało odpowiedzi
    Ok((headers, final_response_html))
}

pub async fn list_orders_handler(
    State(app_state): State<Arc<AppState>>,
    claims: TokenClaims, // Potrzebne do rozróżnienia admin/klient
    Query(params): Query<OrderListingParams>, // Nowe parametry filtrowania
) -> Result<Json<PaginatedOrdersResponse<OrderWithCustomerInfo>>, AppError> {
    // Zmieniony typ odpowiedzi
    let user_id = claims.sub;
    let user_role = claims.role;
    let limit = params.limit();
    let offset = params.offset();

    // --- Budowanie zapytania COUNT ---
    let mut count_query_builder: QueryBuilder<Postgres> = QueryBuilder::new(
        "SELECT COUNT(DISTINCT o.id) FROM orders o LEFT JOIN users u ON o.user_id = u.id",
    );
    let mut conditions_added_count = false;
    let mut append_where_or_and_count = |builder: &mut QueryBuilder<Postgres>| {
        if !conditions_added_count {
            builder.push(" WHERE ");
            conditions_added_count = true;
        } else {
            builder.push(" AND ");
        }
    };

    // --- Budowanie zapytania o DANE ---
    let mut data_query_builder: QueryBuilder<Postgres> = QueryBuilder::new(
        r#"
            SELECT
                o.id, o.user_id,
                o.order_date,
                o.status,
                o.total_price,
                o.shipping_first_name,
                o.shipping_last_name,
                o.shipping_address_line1,
                o.shipping_address_line2,
                o.shipping_city,
                o.shipping_postal_code,
                o.shipping_country,
                o.shipping_phone,
                o.shipping_method_name,
                o.payment_method,
                o.guest_email,
                o.guest_session_id,
                o.created_at, o.updated_at,
                COALESCE(u.email, o.guest_email) as customer_email
            FROM orders o
            LEFT JOIN users u ON o.user_id = u.id
        "#, // Wybieramy customer_email
    );
    let mut conditions_added_data = false;
    let mut append_where_or_and_data = |builder: &mut QueryBuilder<Postgres>| {
        if !conditions_added_data {
            builder.push(" WHERE ");
            conditions_added_data = true;
        } else {
            builder.push(" AND ");
        }
    };

    if user_role != Role::Admin {
        // Klient widzi tylko swoje zamówienia
        append_where_or_and_count(&mut count_query_builder);
        count_query_builder.push(" o.user_id = ").push_bind(user_id);
        append_where_or_and_data(&mut data_query_builder);
        data_query_builder.push(" o.user_id = ").push_bind(user_id);
        tracing::info!(
            "Użytkownik {} pobrał listę swoich zamówień z filtrami: {:?}",
            user_id,
            params
        );
    } else {
        // Admin może filtrować
        tracing::info!(
            "Admin {} pobrał listę zamówień z filtrami: {:?}",
            user_id,
            params
        );
        if let Some(status_filter) = params.status() {
            append_where_or_and_count(&mut count_query_builder);
            count_query_builder
                .push(" o.status = ")
                .push_bind(status_filter.clone()); // .clone() jeśli status_filter jest referencją
            append_where_or_and_data(&mut data_query_builder);
            data_query_builder
                .push(" o.status = ")
                .push_bind(status_filter);
        }
        if let Some(date_from) = params.date_from_dt() {
            append_where_or_and_count(&mut count_query_builder);
            count_query_builder
                .push(" o.order_date >= ")
                .push_bind(date_from);
            append_where_or_and_data(&mut data_query_builder);
            data_query_builder
                .push(" o.order_date >= ")
                .push_bind(date_from);
        }
        if let Some(date_to) = params.date_to_dt() {
            append_where_or_and_count(&mut count_query_builder);
            count_query_builder
                .push(" o.order_date <= ")
                .push_bind(date_to);
            append_where_or_and_data(&mut data_query_builder);
            data_query_builder
                .push(" o.order_date <= ")
                .push_bind(date_to);
        }
        if let Some(search_term) = params.search() {
            let like_pattern = format!("%{}%", search_term);
            append_where_or_and_count(&mut count_query_builder);
            count_query_builder
                .push(" (CAST(o.id AS TEXT) ILIKE ")
                .push_bind(like_pattern.clone())
                .push(" OR o.shipping_last_name ILIKE ")
                .push_bind(like_pattern.clone())
                .push(" OR o.guest_email ILIKE ")
                .push_bind(like_pattern.clone())
                .push(" OR u.email ILIKE ")
                .push_bind(like_pattern.clone())
                .push(") ");
            append_where_or_and_data(&mut data_query_builder);
            data_query_builder
                .push(" (CAST(o.id AS TEXT) ILIKE ")
                .push_bind(like_pattern.clone())
                .push(" OR o.shipping_last_name ILIKE ")
                .push_bind(like_pattern.clone())
                .push(" OR o.guest_email ILIKE ")
                .push_bind(like_pattern.clone())
                .push(" OR u.email ILIKE ")
                .push_bind(like_pattern) // Nie klonujemy ostatniego
                .push(") ");
        }
    }

    // Wykonanie zapytania COUNT
    let total_items = count_query_builder
        .build_query_scalar::<i64>()
        .fetch_one(&app_state.db_pool)
        .await?;

    // Dodanie sortowania i paginacji do zapytania o DANE
    let sort_column = match params.sort_by() {
        "total_price" => "o.total_price",
        "status" => "o.status",
        // Dodaj inne kolumny, jeśli potrzebujesz
        _ => "o.order_date", // Domyślnie po dacie zamówienia
    };
    data_query_builder.push(format_args!(" ORDER BY {} {}", sort_column, params.order()));
    data_query_builder.push(" LIMIT ").push_bind(limit);
    data_query_builder.push(" OFFSET ").push_bind(offset);

    // Wykonanie zapytania o DANE
    let orders_with_info = data_query_builder
        .build_query_as::<OrderWithCustomerInfo>() // Używamy nowej struktury
        .fetch_all(&app_state.db_pool)
        .await?;

    let total_pages = if total_items == 0 {
        0
    } else {
        (total_items as f64 / limit as f64).ceil() as i64
    };
    let current_page = (offset as f64 / limit as f64).floor() as i64 + 1;

    Ok(Json(PaginatedOrdersResponse {
        total_items,
        total_pages,
        current_page,
        per_page: limit,
        data: orders_with_info,
    }))
}

pub async fn get_order_details_handler(
    State(app_state): State<Arc<AppState>>,
    claims: TokenClaims, // Tutaj claims jest wymagane do autoryzacji
    Path(order_id): Path<Uuid>,
) -> Result<Json<OrderDetailsResponse>, AppError> {
    let user_id = claims.sub;
    let user_role = claims.role;

    // Krok 1: Użyj nowej funkcji serwisowej do pobrania danych
    let order_details = fetch_order_details_service(&app_state.db_pool, order_id).await?;

    // Krok 2: Sprawdź uprawnienia na pobranych danych
    if user_role != Role::Admin && order_details.order.user_id != Some(user_id) {
        tracing::warn!(
            "Nieautoryzowany dostęp do zamówienia: order_id={}, user_id={}, user_role={:?}",
            order_id,
            user_id,
            user_role
        );
        return Err(AppError::UnauthorizedAccess(
            "Nie masz uprawnień do tego zamówienia".to_string(),
        ));
    }

    tracing::info!(
        "Pobrano szczegóły zamówienia: order_id={}, user_id={}",
        order_id,
        user_id
    );
    Ok(Json(order_details))
}

pub async fn update_order_status_handler(
    State(app_state): State<Arc<AppState>>,
    claims: TokenClaims,
    Path(order_id): Path<Uuid>,
    Form(payload): Form<UpdateOrderStatusPayload>,
) -> Result<(StatusCode, HeaderMap, Json<Order>), AppError> {
    // Zwracamy też zaktualizowany Order
    if claims.role != Role::Admin {
        return Err(AppError::UnauthorizedAccess(
            "Tylko administrator może zmieniać status zamówienia".to_string(),
        ));
    }

    let updated_order_opt = sqlx::query_as::<_, Order>(
        r#"
            UPDATE orders
            SET status = $1, updated_at = CURRENT_TIMESTAMP
            WHERE id = $2
            RETURNING *
        "#,
    )
    .bind(&payload.status)
    .bind(order_id)
    .fetch_optional(&app_state.db_pool)
    .await?;

    match updated_order_opt {
        Some(order) => {
            tracing::info!(
                "Zaktualizowano status zamówienia: order_id={}, nowy_status={:?}, admin_id={}",
                order_id,
                payload.status,
                claims.sub
            );

            let mut headers = HeaderMap::new();

            // Jeden HX-Trigger z obiektem JSON zawierającym wiele zdarzeń
            let trigger_payload = serde_json::json!({
                "reloadAdminOrderList": true, // Zdarzenie do przeładowania listy
                "showMessage": {              // Zdarzenie do wyświetlenia toasta
                    "message": "Status zamowienia zostal pomyslnie zaktualizowany.",
                    "type": "success"
                }
            });

            if let Ok(val) = HeaderValue::from_str(&trigger_payload.to_string()) {
                headers.insert("HX-Trigger", val);
            }

            Ok((StatusCode::OK, headers, Json(order))) // Zwracamy OK, nagłówki i zaktualizowany obiekt Order
        }
        None => {
            tracing::warn!(
                "Nie znaleziono zamówienia do aktualizacji statusu: order_id={}",
                order_id
            );
            Err(AppError::NotFound)
        }
    }
}

pub async fn add_item_to_cart_handler(
    State(app_state): State<Arc<AppState>>,
    claims: TokenClaims,
    Json(payload): Json<AddProductToCartPayload>,
) -> Result<(StatusCode, Json<CartDetailsResponse>), AppError> {
    let user_id = claims.sub;
    let mut tx = app_state.db_pool.begin().await?;

    let cart = match sqlx::query_as::<_, ShoppingCart>(
        "SELECT * FROM shopping_carts WHERE user_id = $1 FOR UPDATE",
    )
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await?
    {
        Some(existing_cart) => existing_cart,
        None => {
            sqlx::query_as::<_, ShoppingCart>(
                "INSERT INTO shopping_carts (user_id) VALUES ($1) RETURNING *",
            )
            .bind(user_id)
            .fetch_one(&mut *tx)
            .await?
        }
    };

    let product_to_add_opt =
        sqlx::query_as::<_, Product>("SELECT * FROM products WHERE id = $1 FOR UPDATE")
            .bind(payload.product_id)
            .fetch_optional(&mut *tx)
            .await?;

    match product_to_add_opt {
        Some(product) => {
            if product.status != ProductStatus::Available {
                tracing::warn!(
                    "Użytkownik {} próbował dodać niedostępny produkt {} (status: {:?}) do koszyka {}",
                    user_id,
                    payload.product_id,
                    product.status,
                    cart.id
                );
                return Err(AppError::UnprocessableEntity(
                    "Produkt jest niedostępny.".to_string(),
                ));
            }
            sqlx::query("INSERT INTO cart_items (cart_id, product_id) VALUES ($1, $2) ON CONFLICT (cart_id, product_id) DO NOTHING")
                .bind(cart.id)
                .bind(payload.product_id)
                .execute(&mut *tx)
                .await?;
            tracing::info!(
                "Produkt {} dodany (lub już był) w koszyku {} dla użytkownika {}",
                payload.product_id,
                cart.id,
                user_id
            );
        }
        None => {
            tracing::warn!(
                "Użytkownik {} próbował dodać nieistniejący produkt {} do koszyka {}",
                user_id,
                payload.product_id,
                cart.id
            );
            return Err(AppError::NotFound);
        }
    }

    // ZMIANA: Zamiast budować odpowiedź ręcznie, używamy build_cart_details_response po zatwierdzeniu
    // Najpierw zatwierdzamy zmiany...
    tx.commit().await?;

    // ...a potem pobieramy świeże dane i budujemy odpowiedź.
    // To oddziela logikę zapisu od logiki odczytu.
    let mut conn = app_state.db_pool.acquire().await?;
    let final_cart =
        sqlx::query_as::<_, ShoppingCart>("SELECT * FROM shopping_carts WHERE id = $1")
            .bind(cart.id)
            .fetch_one(&mut *conn)
            .await?;

    let response_cart = build_cart_details_response(&final_cart, &mut conn).await?;

    Ok((StatusCode::OK, Json(response_cart)))
}

pub async fn get_cart_handler(
    State(app_state): State<Arc<AppState>>,
    claims: TokenClaims,
) -> Result<Json<CartDetailsResponse>, AppError> {
    let user_id = claims.sub;
    tracing::info!("Użytkownik {} żąda zawartości swojego koszyka", user_id);

    // Pobieramy połączenie z puli, aby przekazać je do funkcji pomocniczej.
    let mut conn = app_state.db_pool.acquire().await.map_err(|e| {
        tracing::error!("Nie można uzyskać połączenia z puli: {}", e);
        AppError::InternalServerError("Błąd serwera".to_string())
    })?;

    // Znajdź koszyk użytkownika
    let cart_optional = sqlx::query_as::<_, ShoppingCart>(
        r#"
            SELECT *
            FROM shopping_carts
            WHERE user_id = $1
        "#,
    )
    .bind(user_id)
    .fetch_optional(&mut *conn)
    .await?;

    match cart_optional {
        Some(cart) => {
            //Koszyk istnieje, zbuduj odpowiedź
            let cart_details = build_cart_details_response(&cart, &mut *conn).await?;
            Ok(Json(cart_details))
        }
        None => {
            tracing::info!("Użytkownik {} nie ma jeszcze koszyka", user_id);
            Err(AppError::NotFound)
        }
    }
}

pub async fn remove_item_from_cart_handler(
    State(app_state): State<Arc<AppState>>,
    claims: TokenClaims,
    Path(product_id_to_remove): Path<Uuid>,
) -> Result<Json<CartDetailsResponse>, AppError> {
    let user_id = claims.sub;
    tracing::info!(
        "Użytkownik {} żąda usunięcia produktu {} ze swojego koszyka",
        user_id,
        product_id_to_remove
    );

    let mut tx = app_state.db_pool.begin().await?;

    let cart = match sqlx::query_as::<_, ShoppingCart>(
        "SELECT * FROM shopping_carts WHERE user_id = $1 FOR UPDATE",
    )
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await?
    {
        Some(existing_cart) => existing_cart,
        None => {
            tracing::warn!(
                "Użytkownik {} próbował usunąć produkt, ale nie ma koszyka.",
                user_id
            );
            return Err(AppError::NotFound);
        }
    };

    let delete_result =
        sqlx::query("DELETE FROM cart_items WHERE cart_id = $1 AND product_id = $2")
            .bind(cart.id)
            .bind(product_id_to_remove)
            .execute(&mut *tx)
            .await?;

    if delete_result.rows_affected() > 0 {
        tracing::info!(
            "Produkt {} usunięty z koszyka {} dla użytkownika {}",
            product_id_to_remove,
            cart.id,
            user_id
        );
    } else {
        tracing::warn!(
            "Produkt {} nie został znaleziony w koszyku {} do usunięcia.",
            product_id_to_remove,
            cart.id,
        );
    }

    // ZMIANA: Użycie build_cart_details_response po zatwierdzeniu transakcji
    tx.commit().await?;

    let mut conn = app_state.db_pool.acquire().await?;
    let final_cart =
        sqlx::query_as::<_, ShoppingCart>("SELECT * FROM shopping_carts WHERE id = $1")
            .bind(cart.id)
            .fetch_one(&mut *conn)
            .await?;

    let cart_details = build_cart_details_response(&final_cart, &mut conn).await?;

    Ok(Json(cart_details))
}

#[derive(Debug, Clone)]
pub struct XGuestCartId(pub Uuid);

impl axum_extra::headers::Header for XGuestCartId {
    fn name() -> &'static axum::http::HeaderName {
        static NAME: once_cell::sync::Lazy<axum::http::HeaderName> =
            // Upewnij się, że once_cell jest w Cargo.toml
            once_cell::sync::Lazy::new(|| {
                    axum::http::HeaderName::from_static("x-guest-cart-id")
                });
        &NAME
    }

    fn decode<'i, I>(values: &mut I) -> Result<Self, axum_extra::headers::Error>
    where
        Self: Sized,
        I: Iterator<Item = &'i axum::http::HeaderValue>,
    {
        let value = values
            .next()
            .ok_or_else(axum_extra::headers::Error::invalid)?;
        let uuid = Uuid::parse_str(
            value
                .to_str()
                .map_err(|_| axum_extra::headers::Error::invalid())?,
        )
        .map_err(|_| axum_extra::headers::Error::invalid())?;
        Ok(XGuestCartId(uuid))
    }

    fn encode<E: Extend<axum::http::HeaderValue>>(&self, values: &mut E) {
        let s = self.0.to_string();
        let value = axum::http::HeaderValue::from_str(&s).unwrap_or_else(|_| {
            panic!(
                "XGuestCartId to_string() produced invalid header value: {}",
                s
            )
        });
        values.extend(std::iter::once(value));
    }
}

#[allow(dead_code)]
pub async fn add_item_to_cart_htmx_handler(
    State(app_state): State<Arc<AppState>>,
    Path(product_id): Path<Uuid>,
    user_claims_result: Result<TokenClaims, AppError>,
    guest_cart_id_header: Option<TypedHeader<XGuestCartId>>,
) -> Result<(HeaderMap, Markup), AppError> {
    let mut tx = app_state.db_pool.begin().await.map_err(|e| {
        tracing::error!("MAUD AddToCart: Błąd rozpoczynania transakcji: {}", e);
        AppError::InternalServerError("Błąd serwera przy dodawaniu do koszyka".to_string())
    })?;
    let mut headers = HeaderMap::new();
    let cart: ShoppingCart;
    let mut new_guest_cart_id_to_set: Option<Uuid> = None;

    if let Ok(claims) = user_claims_result {
        // --- SCENARIUSZ 1: Użytkownik zalogowany ---
        let user_id = claims.sub;
        cart = match sqlx::query_as("SELECT * FROM shopping_carts WHERE user_id = $1 FOR UPDATE")
            .bind(user_id)
            .fetch_optional(&mut *tx)
            .await?
        {
            Some(existing_cart) => existing_cart,
            None => {
                sqlx::query_as("INSERT INTO shopping_carts (user_id) VALUES ($1) RETURNING *")
                    .bind(user_id)
                    .fetch_one(&mut *tx)
                    .await?
            }
        };
    } else {
        // --- SCENARIUSZ 2: Użytkownik jest gościem ---
        if let Some(TypedHeader(XGuestCartId(guest_id))) = guest_cart_id_header {
            // Gość ma już ID sesji, szukamy jego koszyka
            if let Some(existing_cart) = sqlx::query_as(
                "SELECT * FROM shopping_carts WHERE guest_session_id = $1 FOR UPDATE",
            )
            .bind(guest_id)
            .fetch_optional(&mut *tx)
            .await?
            {
                cart = existing_cart;
                new_guest_cart_id_to_set = Some(guest_id);
            } else {
                // ID sesji było, ale koszyk zniknął (rzadkie) - tworzymy nowy z tym samym ID
                cart = sqlx::query_as(
                    "INSERT INTO shopping_carts (guest_session_id) VALUES ($1) RETURNING *",
                )
                .bind(guest_id)
                .fetch_one(&mut *tx)
                .await?;
                new_guest_cart_id_to_set = Some(guest_id);
            }
        } else {
            // Nowy gość, tworzymy mu ID sesji i koszyk
            let new_id = Uuid::new_v4();
            new_guest_cart_id_to_set = Some(new_id);
            cart = sqlx::query_as(
                "INSERT INTO shopping_carts (guest_session_id) VALUES ($1) RETURNING *",
            )
            .bind(new_id)
            .fetch_one(&mut *tx)
            .await?;

            // <<< KLUCZOWA POPRAWKA: Ustawiamy ciasteczko dla nowego gościa >>>
            let guest_cookie = Cookie::build(("guest_cart_id", new_id.to_string()))
                .path("/")
                .http_only(true)
                .secure(true)
                .same_site(SameSite::Lax) // Użyj Lax zamiast None dla lepszego bezpieczeństwa
                .max_age(time::Duration::days(365))
                .build();
            headers.insert(
                axum::http::header::SET_COOKIE,
                guest_cookie.to_string().parse().unwrap(),
            );
            tracing::info!("Ustawiono ciasteczko guest_cart_id dla nowego gościa.");
        }
    }

    let product_opt =
        sqlx::query_as::<_, Product>("SELECT * FROM products WHERE id = $1 FOR UPDATE")
            .bind(product_id)
            .fetch_optional(&mut *tx)
            .await?;

    match product_opt {
        Some(product) => {
            if product.status != ProductStatus::Available {
                tx.rollback().await?;
                let trigger_payload = serde_json::json!({
                    "showMessage": { "type": "warning", "message": format!("Produkt '{}' jest obecnie niedostepny.", product.name) }
                });
                if let Ok(val) = HeaderValue::from_str(&trigger_payload.to_string()) {
                    headers.insert("HX-Trigger", val);
                }
                return Ok((headers, html!()));
            }

            sqlx::query("INSERT INTO cart_items (cart_id, product_id) VALUES ($1, $2) ON CONFLICT (cart_id, product_id) DO NOTHING")
                .bind(cart.id)
                .bind(product_id)
                .execute(&mut *tx)
                .await?;
        }
        None => {
            tx.rollback().await?;
            let trigger_payload = serde_json::json!({
                "showMessage": { "type": "error", "message": "Wybrany produkt nie został znaleziony." }
            });
            if let Ok(val) = HeaderValue::from_str(&trigger_payload.to_string()) {
                headers.insert("HX-Trigger", val);
            }
            return Ok((headers, html!()));
        }
    }

    let cart_details: CartDetailsResponse =
        crate::cart_utils::build_cart_details_response(&cart, &mut tx).await?;

    tx.commit().await.map_err(|e| {
        tracing::error!("MAUD AddToCart: Błąd przy zatwierdzaniu transakcji: {}", e);
        AppError::InternalServerError("Błąd serwera przy zapisie koszyka".to_string())
    })?;

    let trigger_payload = serde_json::json!({
        "product-added": { "productId": product_id },
        "updateCartCount": {
            "newCount": cart_details.total_items,
            "newCartTotalPrice": cart_details.total_price,
            "newGuestCartId": new_guest_cart_id_to_set
        },
        "showMessage": {
            "type": "success",
            "message": "Dodano produkt do koszyka!"
        }
    });

    if let Ok(trigger_value) = HeaderValue::from_str(&trigger_payload.to_string()) {
        headers.insert("HX-Trigger", trigger_value);
    }

    Ok((headers, render_added_to_cart_button(product_id)))
}

#[allow(dead_code)]
/// Renderuje włączony przycisk "Dodaj do koszyka".
fn render_add_to_cart_button(product_id: Uuid) -> Markup {
    html! {
        // Ważne: przycisk ma teraz unikalne ID, aby HTMX mógł go znaleźć
        button id=(format!("product-cart-button-{}", product_id))
               type="button"
               hx-post=(format!("/htmx/cart/add/{}", product_id))
               hx-target=(format!("#product-cart-button-{}", product_id)) // Celuje w samego siebie
               hx-swap="outerHTML" // Podmienia cały swój kod HTML odpowiedzią z serwera
               class="w-full text-white font-medium py-2 px-4 rounded-lg transition-all duration-200 ease-in-out inline-flex items-center justify-center bg-pink-600 hover:bg-pink-700"
        {
            div class="flex items-center" {
                svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor" class="w-5 h-5 mr-2" {
                    path stroke-linecap="round" stroke-linejoin="round" d="M12 9v6m3-3H9m12 0a9 9 0 1 1-18 0 9 9 0 0 1 18 0Z";
                }
                span { "Dodaj do koszyka" }
            }
        }
    }
}

/// Renderuje wyłączony przycisk "Dodano!".
fn render_added_to_cart_button(product_id: Uuid) -> Markup {
    html! {
        // Ważne: ten przycisk ma to samo ID co jego włączona wersja.
        button id=(format!("product-cart-button-{}", product_id))
               type="button"
               disabled
               class="w-full text-white font-semibold py-2 px-4 rounded-lg transition-all inline-flex items-center justify-center bg-green-600 cursor-default"
        {
            div class="flex items-center" {
                svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="2.5" stroke="currentColor" class="w-5 h-5 mr-2" {
                    path stroke-linecap="round" stroke-linejoin="round" d="m4.5 12.75 6 6 9-13.5";
                }
                span { "Dodano!" }
            }
        }
    }
}

//GET /api/guest-cart
pub async fn get_guest_cart(
    State(app_state): State<Arc<AppState>>,
    guest_cart_id_header: Option<TypedHeader<XGuestCartId>>,
) -> Result<impl IntoResponse, AppError> {
    if let Some(TypedHeader(XGuestCartId(guest_id))) = guest_cart_id_header {
        let mut conn = app_state.db_pool.acquire().await?;
        if let Some(cart) = sqlx::query_as::<_, ShoppingCart>(
            "SELECT * FROM shopping_carts WHERE guest_session_id = $1",
        )
        .bind(guest_id)
        .fetch_optional(&mut *conn)
        .await?
        {
            let response = build_cart_details_response(&cart, &mut conn).await?;
            return Ok((StatusCode::OK, Json(response)));
        }
    }
    Ok((StatusCode::OK, Json(CartDetailsResponse::default())))
}

pub async fn remove_item_from_guest_cart(
    State(app_state): State<Arc<AppState>>,
    guest_cart_id_header: Option<TypedHeader<XGuestCartId>>,
    Path(product_id_to_remove): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let TypedHeader(XGuestCartId(guest_id)) = guest_cart_id_header
        .ok_or_else(|| AppError::BadRequest("Missing X-Guest-Cart-Id header".to_string()))?;

    let mut tx = app_state.db_pool.begin().await?;

    let cart = sqlx::query_as::<_, ShoppingCart>(
        "SELECT * FROM shopping_carts WHERE guest_session_id = $1",
    )
    .bind(guest_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::NotFound)?;

    sqlx::query("DELETE FROM cart_items WHERE cart_id = $1 AND product_id = $2")
        .bind(cart.id)
        .bind(product_id_to_remove)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

    let mut conn = app_state.db_pool.acquire().await?;
    let final_cart =
        sqlx::query_as::<_, ShoppingCart>("SELECT * FROM shopping_carts WHERE id = $1")
            .bind(cart.id)
            .fetch_one(&mut *conn)
            .await?;

    let response_details = build_cart_details_response(&final_cart, &mut conn).await?;
    let response = GuestCartOperationResponse {
        guest_cart_id: guest_id,
        cart_details: response_details,
    };

    Ok((StatusCode::OK, Json(response)))
}

// POST /api/cart/merge/ (Chroniony endpoint)
pub async fn merge_cart_handler(
    State(app_state): State<Arc<AppState>>,
    user_claims: TokenClaims,
    Json(payload): Json<MergeCartPayload>,
) -> Result<impl IntoResponse, AppError> {
    let user_id = user_claims.sub;
    let guest_cart_id_to_merge = payload.guest_cart_id;
    let mut tx = app_state.db_pool.begin().await?;

    let user_cart =
        match sqlx::query_as::<_, ShoppingCart>("SELECT * FROM shopping_carts WHERE user_id = $1")
            .bind(user_id)
            .fetch_optional(&mut *tx)
            .await?
        {
            Some(cart) => cart,
            None => {
                sqlx::query_as::<_, ShoppingCart>(
                    "INSERT INTO shopping_carts (user_id) VALUES ($1) RETURNING *",
                )
                .bind(user_id)
                .fetch_one(&mut *tx)
                .await?
            }
        };

    if let Some(guest_cart) = sqlx::query_as::<_, ShoppingCart>(
        "SELECT * FROM shopping_carts WHERE guest_session_id = $1",
    )
    .bind(guest_cart_id_to_merge)
    .fetch_optional(&mut *tx)
    .await?
    {
        if guest_cart.id != user_cart.id {
            // Przeniesienie itemów z koszyka gościa do koszyka użytkownika za pomocą jednego zapytania UPDATE
            sqlx::query(
                r#"
                    UPDATE cart_items
                    SET cart_id = $1
                    WHERE cart_id = $2 AND product_id NOT IN (
                        SELECT product_id FROM cart_items WHERE cart_id = $1
                    )
                "#,
            )
            .bind(user_cart.id)
            .bind(guest_cart.id)
            .execute(&mut *tx)
            .await?;

            // Usunięcie koszyka gościa (itemy, które nie zostały przeniesione, zostaną usunięte kaskadowo)
            sqlx::query("DELETE FROM shopping_carts WHERE id = $1")
                .bind(guest_cart.id)
                .execute(&mut *tx)
                .await?;
        } else {
            sqlx::query(
                "UPDATE shopping_carts SET guest_session_id = NULL WHERE id = $1 AND user_id = $2",
            )
            .bind(user_cart.id)
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
        }
    }

    tx.commit().await?;

    let mut conn = app_state.db_pool.acquire().await?;
    let final_cart =
        sqlx::query_as::<_, ShoppingCart>("SELECT * FROM shopping_carts WHERE id = $1")
            .bind(user_cart.id)
            .fetch_one(&mut *conn)
            .await?;

    let response = build_cart_details_response(&final_cart, &mut *conn).await?;

    Ok((StatusCode::OK, Json(response)))
}

fn option_string_empty_as_none(opt_s: Option<String>) -> Option<String> {
    opt_s.filter(|s| !s.is_empty())
}

pub async fn upsert_user_shipping_details_handler(
    State(app_state): State<Arc<AppState>>,
    claims: TokenClaims,
    Form(payload): Form<UpdateUserShippingDetailsPayload>,
) -> Result<impl IntoResponse, AppError> {
    let user_id = claims.sub;
    tracing::info!("Użytkownik {} aktualizuje swoje dane wysyłki.", user_id);

    // Walidacja payloadu
    if let Err(validation_errors) = payload.validate() {
        tracing::warn!(
            "Błąd walidacji danych wysyłki od użytkownika {}: {:?}",
            user_id,
            validation_errors
        );
        // Możesz chcieć zwrócić bardziej szczegółowe błędy walidacji do HTMX
        let mut headers = HeaderMap::new();
        headers.insert("HX-Reswap", HeaderValue::from_static("none")); // Nie zamieniaj treści formularza
        let error_message = validation_errors
            .field_errors()
            .into_iter()
            .map(|(field, errors)| {
                format!(
                    "{}: {}",
                    field,
                    errors
                        .iter()
                        .filter_map(|e| e.message.as_ref())
                        .map(|m| m.to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })
            .collect::<Vec<_>>()
            .join("; ");

        let trigger_payload = serde_json::json!({
            "showMessage": {"message": format!("Błąd walidacji: {}", error_message), "type": "error"}
        });
        if let Ok(trigger_value) = HeaderValue::from_str(&trigger_payload.to_string()) {
            headers.insert("HX-Trigger", trigger_value);
        }
        return Ok((
            StatusCode::UNPROCESSABLE_ENTITY,
            headers,
            Json(serde_json::json!({"error": "Validation failed"})),
        ));
    }

    // Konwersja Some("") na None dla każdego pola przed zapisem do bazy
    let first_name = option_string_empty_as_none(payload.shipping_first_name);
    let last_name = option_string_empty_as_none(payload.shipping_last_name);
    let address1 = option_string_empty_as_none(payload.shipping_address_line1);
    let address2 = option_string_empty_as_none(payload.shipping_address_line2);
    let city = option_string_empty_as_none(payload.shipping_city);
    let postal_code = option_string_empty_as_none(payload.shipping_postal_code);
    let country = option_string_empty_as_none(payload.shipping_country);
    let phone = option_string_empty_as_none(payload.shipping_phone);

    // Logika UPSERT (INSERT OR UPDATE)
    // ON CONFLICT (user_id) DO UPDATE ...
    let query_result = sqlx::query_as::<_, UserShippingDetails>(
        r#"
            INSERT INTO user_shipping_details (
                user_id, shipping_first_name, shipping_last_name, shipping_address_line1,
                shipping_address_line2, shipping_city, shipping_postal_code, shipping_country, shipping_phone
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            ON CONFLICT (user_id) DO UPDATE SET
                shipping_first_name = EXCLUDED.shipping_first_name,
                shipping_last_name = EXCLUDED.shipping_last_name,
                shipping_address_line1 = EXCLUDED.shipping_address_line1,
                shipping_address_line2 = EXCLUDED.shipping_address_line2,
                shipping_city = EXCLUDED.shipping_city,
                shipping_postal_code = EXCLUDED.shipping_postal_code,
                shipping_country = EXCLUDED.shipping_country,
                shipping_phone = EXCLUDED.shipping_phone,
                updated_at = NOW()
            RETURNING *
        "#,
    )
    .bind(user_id)
    .bind(first_name)
    .bind(last_name)
    .bind(address1)
    .bind(address2)
    .bind(city)
    .bind(postal_code)
    .bind(country)
    .bind(phone)
    .fetch_one(&app_state.db_pool)
    .await;

    match query_result {
        Ok(_) => {
            tracing::info!(
                "Dane wysyłki dla użytkownika {} zostały pomyślnie zaktualizowane/utworzone.",
                user_id
            );
            let mut headers = HeaderMap::new();
            // HX-Trigger do wyświetlenia komunikatu o sukcesie
            let trigger_payload = serde_json::json!({
                "showMessage": {"message": "Twoje dane zostaly zapisane.", "type": "success"}
                // Można też dodać trigger do odświeżenia formularza, jeśli nie jest on
                // automatycznie odświeżany przez HTMX po sukcesie (zależy od hx-target i hx-swap na formularzu)
                // np. "loadMyDataSection": {}
            });
            if let Ok(trigger_value) = HeaderValue::from_str(&trigger_payload.to_string()) {
                headers.insert("HX-Trigger", trigger_value);
            }
            // Aby formularz się nie "czyścił" przez HTMX po sukcesie,
            // można zwrócić pustą odpowiedź z odpowiednim statusem i `HX-Reswap: none`
            // lub pozwolić HTMX podmienić fragment z komunikatem.
            // Jeśli formularz ma się sam odświeżyć, można zwrócić go ponownie.
            // Na razie prosta odpowiedź OK z triggerem.
            Ok((
                StatusCode::OK,
                headers,
                Json(serde_json::json!({"message": "Dane zapisane"})),
            ))
        }
        Err(e) => {
            tracing::error!(
                "Błąd podczas zapisu danych wysyłki dla użytkownika {}: {:?}",
                user_id,
                e
            );
            Err(AppError::from(e)) // Lub bardziej szczegółowy błąd
        }
    }
}

/// Ta operacja jest nieodwracalna.
pub async fn permanent_delete_order_handler(
    State(app_state): State<Arc<AppState>>,
    claims: TokenClaims,
    Path(order_id): Path<Uuid>,
) -> Result<(StatusCode, HeaderMap), AppError> {
    // Krok 1: Sprawdzenie uprawnień. Tylko admin może usuwać zamówienia.
    if claims.role != Role::Admin {
        return Err(AppError::UnauthorizedAccess(
            "Brak uprawnień administratora.".to_string(),
        ));
    }

    tracing::info!(
        "Admin ID: {} zażądał trwałego usunięcia zamówienia ID: {}",
        claims.sub,
        order_id
    );

    // Krok 2: Rozpoczęcie transakcji bazodanowej. To kluczowe dla bezpieczeństwa!
    let mut tx = app_state.db_pool.begin().await?;
    tracing::debug!(
        "Rozpoczęto transakcję dla usunięcia zamówienia {}",
        order_id
    );

    // Krok 3: Znajdź wszystkie ID produktów w tym zamówieniu.
    let product_ids: Vec<Uuid> =
        sqlx::query_scalar("SELECT product_id FROM order_items WHERE order_id = $1")
            .bind(order_id)
            .fetch_all(&mut *tx)
            .await?;

    // Krok 4: Jeśli znaleziono produkty, zmień ich status z powrotem na "Available".
    if !product_ids.is_empty() {
        tracing::debug!(
            "Znaleziono produkty {:?} do przywrócenia statusu na 'Available'.",
            product_ids
        );
        sqlx::query("UPDATE products SET status = $1 WHERE id = ANY($2)")
            .bind(ProductStatus::Available) // Używamy enuma dla bezpieczeństwa
            .bind(&product_ids)
            .execute(&mut *tx)
            .await?;
    }

    // Krok 5: Usuń pozycje z tabeli pośredniczącej `order_items`.
    // To zerwie powiązania i pozwoli bezpiecznie usunąć zamówienie.
    sqlx::query("DELETE FROM order_items WHERE order_id = $1")
        .bind(order_id)
        .execute(&mut *tx)
        .await?;
    tracing::debug!("Usunięto pozycje z order_items dla zamówienia {}", order_id);

    // Krok 6: Usuń właściwe zamówienie z tabeli `orders`.
    let delete_result = sqlx::query("DELETE FROM orders WHERE id = $1")
        .bind(order_id)
        .execute(&mut *tx)
        .await?;

    // Krok 7: Zatwierdź transakcję. Dopiero teraz wszystkie zmiany zostaną trwale zapisane w bazie.
    tx.commit().await?;
    tracing::info!(
        "Transakcja zakończona pomyślnie. Zamówienie {} zostało usunięte.",
        order_id
    );

    // Krok 8: Przygotuj odpowiedź dla HTMX.
    let mut headers = HeaderMap::new();

    if delete_result.rows_affected() == 0 {
        tracing::warn!(
            "Próbowano usunąć zamówienie {}, ale nie znaleziono go w bazie.",
            order_id
        );
    }

    // Wyślij komunikat toast o sukcesie.
    let toast_payload = serde_json::json!({
        "showMessage": {
            "message": "Zamowienie zostalo trwale usuniete.",
            "type": "success"
        }
    });
    if let Ok(val) = HeaderValue::from_str(&toast_payload.to_string()) {
        headers.insert("HX-Trigger", val);
    }

    // Zwróć pustą odpowiedź z kodem 200 OK. HTMX usunie wiersz z tabeli.
    Ok((StatusCode::OK, headers))
}

// =========================================================================
// NOWA FUNKCJA SERWISOWA (REUŻYWALNA)
// =========================================================================
/// Pobiera pełne szczegóły zamówienia (zamówienie + pozycje z produktami).
/// Ta funkcja nie sprawdza uprawnień, robi to handler, który ją wywołuje.
pub async fn fetch_order_details_service(
    pool: &sqlx::PgPool,
    order_id: Uuid,
) -> Result<OrderDetailsResponse, AppError> {
    let order = sqlx::query_as::<_, Order>("SELECT * FROM orders WHERE id = $1")
        .bind(order_id)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)?; // Jeśli nie ma zamówienia, zwracamy błąd

    let order_items_db =
        sqlx::query_as::<_, OrderItem>("SELECT * FROM order_items WHERE order_id = $1")
            .bind(order_id)
            .fetch_all(pool)
            .await?;

    let mut items_details_public: Vec<OrderItemDetailsPublic> =
        Vec::with_capacity(order_items_db.len());

    if !order_items_db.is_empty() {
        let product_ids: Vec<Uuid> = order_items_db.iter().map(|item| item.product_id).collect();
        let products = sqlx::query_as::<_, Product>("SELECT * FROM products WHERE id = ANY($1)")
            .bind(&product_ids)
            .fetch_all(pool)
            .await?;

        let products_map: HashMap<Uuid, Product> =
            products.into_iter().map(|p| (p.id, p)).collect();

        for item_db in order_items_db {
            if let Some(product) = products_map.get(&item_db.product_id) {
                items_details_public.push(OrderItemDetailsPublic {
                    order_item_id: item_db.id,
                    product: product.clone(),
                    price_at_purchase: item_db.price_at_purchase,
                });
            } else {
                tracing::error!(
                    "Krytyczny błąd spójności danych: Produkt (ID: {}) dla pozycji zamówienia (ID: {}) nie został znaleziony. OrderID: {}.",
                    item_db.product_id,
                    item_db.id,
                    order_id
                );
            }
        }
    }

    Ok(OrderDetailsResponse {
        order,
        items: items_details_public,
    })
}

pub async fn forgot_password_handler(
    State(app_state): State<Arc<AppState>>,
    Form(payload): Form<ForgotPasswordPayload>,
) -> Result<Markup, AppError> {
    // Zawsze zwracamy ten sam komunikat, aby nie ujawniać, czy e-mail istnieje w bazie.
    let success_message = "Jeśli konto powiązane z tym adresem e-mail istnieje, wysłaliśmy na nie link do zresetowania hasła.";

    if let Some(user) = sqlx::query_as::<_, User>("SELECT * FROM users WHERE email = $1")
        .bind(&payload.email)
        .fetch_optional(&app_state.db_pool)
        .await?
    {
        // Użytkownik istnieje, kontynuujemy logikę
        let mut tx = app_state.db_pool.begin().await?;

        // Usuń stare tokeny tego użytkownika, aby uniknąć bałaganu
        sqlx::query("DELETE FROM password_resets WHERE user_id = $1")
            .bind(user.id)
            .execute(&mut *tx)
            .await?;

        // Wygeneruj nowy token
        let token = Uuid::new_v4();
        let expires_at = Utc::now() + Duration::minutes(30);

        sqlx::query("INSERT INTO password_resets (token, user_id, expires_at) VALUES ($1, $2, $3)")
            .bind(token)
            .bind(user.id)
            .bind(expires_at)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;

        // Wyślij e-mail
        if let Err(e) = send_password_reset_email(&app_state, &user.email, &token.to_string()).await
        {
            tracing::error!(
                "Nie udało się wysłać e-maila z resetem hasła do {}: {:?}",
                user.email,
                e
            );
        }
    }

    // Niezależnie od tego, czy użytkownik istniał, zwracamy ten sam komunikat
    Ok(html! {
        p class="text-green-700" { (success_message) }
    })
}

pub async fn reset_password_handler(
    State(app_state): State<Arc<AppState>>,
    Form(payload): Form<ResetPasswordPayload>,
) -> Result<(HeaderMap, Markup), AppError> {
    payload.validate()?; // Walidacja czy hasła pasują i mają min. 6 znaków

    let token_uuid = Uuid::from_str(&payload.token)
        .map_err(|_| AppError::InvalidToken("Format tokenu jest nieprawidłowy.".into()))?;

    let mut tx = app_state.db_pool.begin().await?;

    // Znajdź token i upewnij się, że jest ważny
    let token_data = sqlx::query_as::<_, PasswordResetToken>(
        "SELECT * FROM password_resets WHERE token = $1 FOR UPDATE",
    )
    .bind(token_uuid)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::InvalidToken("Token nie istnieje.".into()))?;

    if token_data.expires_at <= Utc::now() {
        return Err(AppError::TokenExpired);
    }

    // Zmień hasło
    let new_password_hash = hash_password(&payload.new_password)?;
    sqlx::query("UPDATE users SET password_hash = $1 WHERE id = $2")
        .bind(new_password_hash)
        .bind(token_data.user_id)
        .execute(&mut *tx)
        .await?;

    // Usuń zużyty token
    sqlx::query("DELETE FROM password_resets WHERE token = $1")
        .bind(token_uuid)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

    // Sukces! Przekieruj na logowanie z komunikatem.
    let mut headers = HeaderMap::new();
    let trigger_payload = json!({
        "showMessage": {
            "message": "Hasło zostało pomyślnie zmienione! Możesz się teraz zalogować.",
            "type": "success"
        }
    });
    headers.insert(
        "HX-Trigger",
        HeaderValue::from_str(&trigger_payload.to_string()).unwrap(),
    );
    headers.insert("HX-Location", HeaderValue::from_static("/htmx/logowanie"));

    Ok((headers, html! {}))
}

/// Obsługuje wylogowanie użytkownika po stronie serwera.
/// Głównym zadaniem jest wyczyszczenie ciasteczka 'token'.
#[allow(deprecated)]
pub async fn logout_handler() -> Result<(StatusCode, HeaderMap), AppError> {
    tracing::info!("Obsługa żądania wylogowania: czyszczenie ciasteczka.");

    // Tworzymy ciasteczko, które "nadpisze" istniejące i natychmiast wygaśnie.
    let cookie = Cookie::build(("token", "")) // Pusta wartość
        .path("/")
        .http_only(true)
        .secure(true)
        .same_site(SameSite::Lax)
        .max_age(time::Duration::ZERO) // Ustawia Max-Age=0, co każe przeglądarce usunąć ciasteczko
        .finish();

    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::SET_COOKIE,
        cookie.to_string().parse().unwrap(),
    );

    // Dodatkowo wysyłamy trigger, który poinformuje klienta, że ma dokończyć wylogowanie
    // (np. wyczyścić localStorage i przekierować).
    let trigger_payload = json!({"logoutClient": {}});
    headers.insert(
        "HX-Trigger",
        HeaderValue::from_str(&trigger_payload.to_string()).unwrap(),
    );

    Ok((StatusCode::OK, headers))
}

/// Inicjalizuje nową sesję gościa, tworzy koszyk w bazie i ustawia ciasteczko.
pub async fn init_guest_session_handler(
    State(app_state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, AppError> {
    tracing::info!("Inicjalizacja nowej sesji gościa.");

    let new_guest_id = Uuid::new_v4();

    // Utwórz nowy koszyk dla gościa w bazie danych
    let cart = sqlx::query_as::<_, ShoppingCart>(
        "INSERT INTO shopping_carts (guest_session_id) VALUES ($1) RETURNING *",
    )
    .bind(new_guest_id)
    .fetch_one(&app_state.db_pool)
    .await?;

    tracing::info!(
        "Utworzono nowy koszyk ID: {} dla gościa z sesją ID: {}",
        cart.id,
        new_guest_id
    );

    // Ustaw ciasteczko DOKŁADNIE tak, jak dla zalogowanego użytkownika,
    // aby zapewnić spójne zachowanie przeglądarki.
    let cookie = Cookie::build(("guest_cart_id", new_guest_id.to_string()))
        .path("/")
        .http_only(true)
        .secure(true) // Zakładając, że działasz na HTTPS
        .same_site(SameSite::None) // Lax to najlepszy i najbezpieczniejszy wybór tutaj
        .max_age(time::Duration::days(365))
        .build();

    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::SET_COOKIE,
        cookie.to_string().parse().unwrap(),
    );

    // Zwracamy ID gościa w ciele odpowiedzi, aby frontend mógł je zapisać w localStorage
    Ok((
        StatusCode::OK,
        headers,
        Json(json!({ "guestCartId": new_guest_id })),
    ))
}

pub async fn add_item_to_guest_cart(
    State(app_state): State<Arc<AppState>>,
    guest_cart_id_header: Option<TypedHeader<XGuestCartId>>,
    Json(payload): Json<AddProductToCartPayload>,
) -> Result<impl IntoResponse, AppError> {
    let mut tx = app_state.db_pool.begin().await?;
    let product_id = payload.product_id;
    let mut headers = HeaderMap::new();

    // Logika jest teraz identyczna jak w htmx_handler
    let (cart, guest_cart_uuid) = if let Some(TypedHeader(XGuestCartId(id))) = guest_cart_id_header
    {
        if let Some(existing_cart) = sqlx::query_as::<_, ShoppingCart>(
            "SELECT * FROM shopping_carts WHERE guest_session_id = $1",
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        {
            (existing_cart, id)
        } else {
            let new_cart = sqlx::query_as::<_, ShoppingCart>(
                "INSERT INTO shopping_carts (guest_session_id) VALUES ($1) RETURNING *",
            )
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
            (new_cart, id)
        }
    } else {
        let new_generated_id = Uuid::new_v4();
        let new_cart = sqlx::query_as::<_, ShoppingCart>(
            "INSERT INTO shopping_carts (guest_session_id) VALUES ($1) RETURNING *",
        )
        .bind(new_generated_id)
        .fetch_one(&mut *tx)
        .await?;

        // <<< KLUCZOWA POPRAWKA: Ustawiamy ciasteczko także tutaj >>>
        let guest_cookie = Cookie::build(("guest_cart_id", new_generated_id.to_string()))
            .path("/")
            .http_only(true)
            .secure(true)
            .same_site(SameSite::Lax)
            .max_age(time::Duration::days(365))
            .build();
        headers.insert(
            axum::http::header::SET_COOKIE,
            guest_cookie.to_string().parse().unwrap(),
        );

        (new_cart, new_generated_id)
    };

    sqlx::query("INSERT INTO cart_items (cart_id, product_id) VALUES ($1, $2) ON CONFLICT (cart_id, product_id) DO NOTHING")
        .bind(cart.id)
        .bind(product_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

    let mut conn = app_state.db_pool.acquire().await?;
    let final_cart =
        sqlx::query_as::<_, ShoppingCart>("SELECT * FROM shopping_carts WHERE id = $1")
            .bind(cart.id)
            .fetch_one(&mut *conn)
            .await?;

    let cart_details_response = build_cart_details_response(&final_cart, &mut conn).await?;

    let response_payload = GuestCartOperationResponse {
        guest_cart_id: guest_cart_uuid,
        cart_details: cart_details_response,
    };

    // Dodajemy ID do nagłówka X-Guest-Cart-Id, aby HTMX mógł je przechwycić, jeśli jest potrzebne
    headers.insert(
        "X-Guest-Cart-Id",
        guest_cart_uuid.to_string().parse().unwrap(),
    );

    Ok((StatusCode::OK, headers, Json(response_payload)))
}
