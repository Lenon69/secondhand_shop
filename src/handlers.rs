use axum::http::HeaderValue;
// src/handlers.rs
use axum::response::IntoResponse;
use axum::{Form, Json};
use axum::{
    extract::{Multipart, Path, Query, State},
    http::{HeaderMap, StatusCode},
};
use axum_extra::TypedHeader;
use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use sqlx::{Postgres, QueryBuilder};

use crate::cart_utils::build_cart_details_response;
use crate::cloudinary::{delete_image_from_cloudinary, extract_public_id_from_url};
use crate::errors::AppError;
use crate::filters::{ListingParams, OrderListingParams};
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
use uuid::Uuid;
use validator::Validate;

pub async fn get_product_details(
    State(app_state): State<AppState>,
    Path(product_id): Path<Uuid>,
) -> Result<Json<Product>, AppError> {
    let product_result = sqlx::query_as::<_, Product>(
        r#"SELECT id, name, description, price, gender, condition, category, status, images, created_at, updated_at
           FROM products
           WHERE id = $1"#,
    )
    .bind(product_id)
    .fetch_one(&app_state.db_pool)
    .await;

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
            Err(AppError::from(e))
        }
    }
}

pub async fn list_products(
    State(app_state): State<AppState>,
    Query(params): Query<ListingParams>,
) -> Result<Json<PaginatedProductsResponse>, AppError> {
    tracing::info!(
        "Obsłużono zapytanie GET /api/products z parametrami: {:?}",
        params
    );

    let limit = params.limit();
    let offset = params.offset();

    // --- Budowanie zapytania COUNT ---
    let mut count_builder: QueryBuilder<Postgres> =
        QueryBuilder::new("SELECT COUNT(*) FROM products");
    let mut count_added_where = false; // Użyj innej nazwy zmiennej lub zdefiniuj na nowo później

    // Definicja domknięcia tylko dla tej sekcji
    let mut append_where_or_and_count = |builder: &mut QueryBuilder<Postgres>| {
        if !count_added_where {
            builder.push(" WHERE ");
            count_added_where = true;
        } else {
            builder.push(" AND ");
        }
    };

    if let Some(gender) = params.gender() {
        append_where_or_and_count(&mut count_builder);
        count_builder.push("gender = ").push_bind(gender);
    }

    if let Some(category) = params.category() {
        append_where_or_and_count(&mut count_builder);
        count_builder.push("category = ").push_bind(category);
    }
    if let Some(condition) = params.condition() {
        append_where_or_and_count(&mut count_builder);
        count_builder.push("condition = ").push_bind(condition);
    }

    // let status_to_filter = params.status().unwrap_or(ProductStatus::Available);
    // append_where_or_and_count(&mut count_builder);
    // count_builder.push("status = ").push_bind(&status_to_filter);
    if let Some(status_val) = params.status() {
        append_where_or_and_count(&mut count_builder);
        count_builder.push("status = ").push_bind(status_val);
    }

    if let Some(price_min) = params.price_min() {
        append_where_or_and_count(&mut count_builder);
        count_builder.push("price >= ").push_bind(price_min);
    }
    if let Some(price_max) = params.price_max() {
        append_where_or_and_count(&mut count_builder);
        count_builder.push("price <= ").push_bind(price_max);
    }

    if let Some(search_term) = params.search() {
        append_where_or_and_count(&mut count_builder);
        let like_pattern = format!("%{}%", search_term);
        count_builder
            .push("(name ILIKE ")
            .push_bind(like_pattern.clone())
            .push(" OR description ILIKE ")
            .push_bind(like_pattern)
            .push(")");
    }

    // Wykonanie zapytania COUNT
    let total_items_result = count_builder
        .build_query_scalar::<i64>()
        .fetch_one(&app_state.db_pool)
        .await;

    let total_items = match total_items_result {
        Ok(count) => count,
        Err(e) => {
            tracing::error!(
                "Błąd bazy danych podczas liczenia produktów (filtrowane): {:?}",
                e
            );
            return Err(AppError::SqlxError(e));
        }
    };

    // --- Budowanie zapytania o DANE ---
    let mut data_builder: QueryBuilder<Postgres> = QueryBuilder::new(
        r#"
            SELECT id, name, description, price, gender, condition, category, status, images, created_at, updated_at
            FROM products
        "#,
    );

    let mut data_added_where = false;
    let mut append_where_or_and_data = |builder: &mut QueryBuilder<Postgres>| {
        if !data_added_where {
            builder.push(" WHERE ");
            data_added_where = true;
        } else {
            builder.push(" AND ");
        }
    };

    if let Some(gender) = params.gender() {
        append_where_or_and_data(&mut data_builder);
        data_builder.push("gender = ").push_bind(gender);
    }

    if let Some(category) = params.category() {
        append_where_or_and_data(&mut data_builder);
        data_builder.push("category = ").push_bind(category);
    }
    if let Some(condition) = params.condition() {
        append_where_or_and_data(&mut data_builder);
        data_builder.push("condition = ").push_bind(condition);
    }
    if let Some(status_val) = params.status() {
        append_where_or_and_data(&mut data_builder);
        data_builder.push("status = ").push_bind(status_val);
    }

    if let Some(price_min) = params.price_min() {
        append_where_or_and_data(&mut data_builder);
        data_builder.push("price >= ").push_bind(price_min);
    }
    if let Some(price_max) = params.price_max() {
        append_where_or_and_data(&mut data_builder);
        data_builder.push("price <= ").push_bind(price_max);
    }

    if let Some(search_term) = params.search() {
        append_where_or_and_data(&mut data_builder);
        let like_pattern = format!("%{}%", search_term);
        data_builder
            .push("(name ILIKE ")
            .push_bind(like_pattern.clone())
            .push(" OR description ILIKE ")
            .push_bind(like_pattern)
            .push(")");
    }

    let sort_by_column = match params.sort_by() {
        "price" => "price",
        "created_at" => "created_at",
        "name" | _ => "name",
    };
    let order_direction = params.order();

    data_builder.push(format!(" ORDER BY {} {}", sort_by_column, order_direction));

    data_builder.push(" LIMIT ").push_bind(limit);
    data_builder.push(" OFFSET ").push_bind(offset);

    let products = data_builder
        .build_query_as::<Product>()
        .fetch_all(&app_state.db_pool)
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

pub async fn create_product_handler(
    State(app_state): State<AppState>,
    claims: TokenClaims,
    mut multipart: Multipart,
) -> Result<(StatusCode, HeaderMap, String), AppError> {
    // Sprawdzanie roli admina
    if claims.role != Role::Admin {
        return Err(AppError::UnauthorizedAccess(
            "Tylko administrator może dodawać produkty".to_string(),
        ));
    }

    tracing::info!("Obsłużono zapytanie POST /api/products - tworzenie produktu");

    // Przetwarzanie danych multipart
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

            // Odczytywanie bajtów pola (zwraca Result)
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

    // Walidacja i ekstrakcja pól tekstowych
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

    // Sprawdzenie czy przynajmniej jeden plik został przesłany
    if image_uploads.is_empty() {
        return Err(AppError::UnprocessableEntity(
            "Należy przesłac conajmniej jeden plik obrazu ('image_file)".to_string(),
        ));
    }

    // Prasowanie i walidacja typów
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

    // Wysyłąnie obrazów do Cloudinary równolegle
    let mut image_upload_futures = Vec::new();
    for (filename, bytes) in image_uploads {
        let config_clone = app_state.cloudinary_config.clone();
        image_upload_futures
            .push(async move { upload_image_to_cloudinary(bytes, filename, &config_clone).await });
    }

    // Czekanie na zakończenie wszystkich operacji upload
    let cloudinary_urls: Vec<String> = try_join_all(image_upload_futures).await?;
    tracing::info!(
        "Wszystkie obrazy przesłane do Cloudinary, URL'e: {:?}",
        cloudinary_urls
    );

    // Zapis produktu do bazy danych
    let new_product_id = Uuid::new_v4();
    let product_status = ProductStatus::Available;

    let _new_product_db = sqlx::query_as::<_, Product>(
        r#"
            INSERT INTO products (id, name, description, price, gender, condition, category, status, images)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING id, name, description, price, gender, condition , category, status, images, created_at, updated_at
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
    .fetch_one(&app_state.db_pool)
    .await?;

    tracing::info!("Utworzono produkt o ID: {}", new_product_id);

    let mut headers = HeaderMap::new();

    // 1. Trigger dla toasta
    let toast_payload = serde_json::json!({
        "showMessage": {
            "message": "Pomyslnie dodano produkt.",
            "type": "success"
        }
    });
    if let Ok(val) = HeaderValue::from_str(&toast_payload.to_string()) {
        headers.insert("HX-Trigger", val);
    }

    let location_payload = serde_json::json!({
        "path": "/htmx/admin/products", // Cel przekierowania (ładowania nowej treści)
        "target": "#admin-content",      // Gdzie nowa treść ma być załadowana
        "swap": "innerHTML"              // Jak ma być załadowana
    });
    if let Ok(val) = HeaderValue::from_str(&location_payload.to_string()) {
        headers.insert("HX-Location", val);
    }

    //    Opcjonalnie: Trigger do wyczyszczenia formularza (jeśli nie robisz tego w JS)
    //    Można to też zrobić w JS po otrzymaniu zdarzenia "showMessage" lub specyficznego zdarzenia.
    //    headers.insert("HX-Trigger-After-Swap", HeaderValue::from_static("clearNewProductForm"));

    // Dla czyszczenia formularza przez HX-Trigger, ciało odpowiedzi może być puste.
    Ok((StatusCode::CREATED, headers, String::new()))
}

pub async fn update_product_partial_handler(
    State(app_state): State<AppState>,
    Path(product_id): Path<Uuid>, // Zmieniono z id na product_id dla jasności, ale w main.rs ścieżka używa {id} - trzymajmy się {id}
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
        product_id,
    );

    // Rozpocznij transakcję (zalecane)
    // let mut tx = app_state.db_pool.begin().await.map_err(AppError::SqlxError)?;

    let mut existing_product = sqlx::query_as::<_, Product>(
        "SELECT * FROM products WHERE id = $1 FOR UPDATE", // FOR UPDATE w transakcji
    )
    .bind(product_id)
    .fetch_one(&app_state.db_pool) // Jeśli używasz transakcji, zmień na &mut *tx
    .await
    .map_err(|err| match err {
        sqlx::Error::RowNotFound => {
            tracing::warn!("PATCH: Nie znaleziono produktu o ID: {}", product_id);
            AppError::NotFound
        }
        _ => {
            tracing::error!("PATCH: Błąd bazy danych (pobieranie): {}", err);
            AppError::SqlxError(err)
        }
    })?;

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
                    tracing::info!("Dodano nowy plik do wgrania: {}", filename);
                }
            }
        } else if field_name == "urls_to_delete" {
            urls_to_delete_json_opt = Some(field.text().await.map_err(AppError::from)?);
            tracing::info!(
                "Odebrano listę URLi do usunięcia: {:?}",
                urls_to_delete_json_opt
            );
        } else {
            text_fields.insert(field_name, field.text().await.map_err(AppError::from)?);
        }
    }

    // Aktualizacja pól tekstowych produktu
    if let Some(name) = text_fields.get("name") {
        existing_product.name = name.clone();
    }
    if let Some(description) = text_fields.get("description") {
        existing_product.description = description.clone();
    }
    if let Some(price_str) = text_fields.get("price") {
        existing_product.price = price_str
            .parse()
            .map_err(|_| AppError::UnprocessableEntity("Nieprawidłowy format ceny".to_string()))?;
    }
    if let Some(gender_str) = text_fields.get("gender") {
        existing_product.gender = ProductGender::from_str(gender_str).map_err(|e| {
            AppError::UnprocessableEntity(format!(
                "Nieprawidłowa płeć: {}, błąd: {}",
                gender_str, e
            ))
        })?;
    }
    if let Some(condition_str) = text_fields.get("condition") {
        existing_product.condition = ProductCondition::from_str(condition_str).map_err(|e| {
            AppError::UnprocessableEntity(format!(
                "Nieprawidłowy stan: {}, błąd: {}",
                condition_str, e
            ))
        })?;
    }
    if let Some(category_str) = text_fields.get("category") {
        existing_product.category = Category::from_str(category_str).map_err(|e| {
            AppError::UnprocessableEntity(format!(
                "Nieprawidłowa kategoria: {}, błąd: {}",
                category_str, e
            ))
        })?;
    }
    if let Some(status_str) = text_fields.get("status") {
        existing_product.status = ProductStatus::from_str(status_str).map_err(|e| {
            AppError::UnprocessableEntity(format!(
                "Nieprawidłowy status: {}, błąd: {}",
                status_str, e
            ))
        })?;
    }

    let mut final_image_urls = existing_product.images.clone();

    // 1. Usuwanie obrazków
    if let Some(json_str) = urls_to_delete_json_opt {
        if !json_str.is_empty() && json_str != "[]" {
            match serde_json::from_str::<Vec<String>>(&json_str) {
                Ok(urls_to_delete) => {
                    if !urls_to_delete.is_empty() {
                        tracing::info!("Oznaczono do usunięcia z Cloudinary: {:?}", urls_to_delete);
                        let mut delete_futures = Vec::new();
                        for url_to_delete in &urls_to_delete {
                            if let Some(public_id) = extract_public_id_from_url(
                                url_to_delete,
                                &app_state.cloudinary_config.cloud_name,
                            ) {
                                let config_clone = app_state.cloudinary_config.clone();
                                let url_clone_log = url_to_delete.clone();
                                delete_futures.push(async move {
                                    delete_image_from_cloudinary(&public_id, &config_clone).await.map_err(|e| {
                                        tracing::error!("Błąd usuwania z Cloudinary (public_id: {} z URL: {}): {:?}", public_id, url_clone_log, e);
                                        e // Przekaż błąd dalej, aby try_join_all go złapał
                                    })
                                });
                            } else {
                                tracing::warn!(
                                    "Nie udało się wyekstrahować public_id z URL: {}",
                                    url_to_delete
                                );
                            }
                        }
                        if !delete_futures.is_empty() {
                            if let Err(e) = try_join_all(delete_futures).await {
                                // Logujemy błąd, ale kontynuujemy, chyba że polityka jest inna
                                tracing::error!(
                                    "Wystąpiły błędy podczas usuwania obrazków z Cloudinary: {:?}. Aktualizacja będzie kontynuowana.",
                                    e
                                );
                            } else {
                                tracing::info!("Pomyślnie usunięto obrazki z Cloudinary.");
                            }
                        }
                        // Usuń z listy final_image_urls te, które były do usunięcia
                        final_image_urls.retain(|url| !urls_to_delete.contains(url));
                    }
                }
                Err(e) => {
                    tracing::error!(
                        "Błąd parsowania JSON dla urls_to_delete: '{}'. Treść JSON: '{}'",
                        e,
                        json_str
                    );
                    return Err(AppError::UnprocessableEntity(
                        "Nieprawidłowy format listy URLi do usunięcia.".to_string(),
                    ));
                }
            }
        }
    }

    // 2. Wgrywanie nowych obrazków
    if !new_image_uploads.is_empty() {
        tracing::info!("Wgrywanie {} nowych obrazków...", new_image_uploads.len());
        let mut upload_futures = Vec::new();
        for (filename, bytes) in new_image_uploads {
            let config_clone = app_state.cloudinary_config.clone();
            upload_futures.push(async move {
                upload_image_to_cloudinary(bytes, filename, &config_clone).await
            });
        }
        match try_join_all(upload_futures).await {
            Ok(new_urls) => {
                tracing::info!("Pomyślnie wgrano nowe obrazki. URL-e: {:?}", new_urls);
                final_image_urls.extend(new_urls);
            }
            Err(e) => {
                tracing::error!("Krytyczny błąd podczas wgrywania nowych obrazków: {:?}", e);
                // tx.rollback().await.ok(); // Wykofaj transakcję, jeśli jest
                return Err(AppError::InternalServerError(format!(
                    "Błąd wgrywania nowych obrazków: {:?}",
                    e
                )));
            }
        }
    }

    // Walidacja: produkt musi mieć co najmniej jeden obrazek
    if final_image_urls.is_empty() {
        // tx.rollback().await.ok();
        return Err(AppError::UnprocessableEntity(
            "Produkt musi mieć przynajmniej jeden obrazek. Wszystkie zostały usunięte lub żadne nie zostały dodane.".to_string(),
        ));
    }
    existing_product.images = final_image_urls; // Zaktualizowana lista obrazków

    // Aktualizacja w bazie danych
    let updated_product_db = sqlx::query_as::<_, Product>(
        r#"
            UPDATE products
            SET name = $1, description = $2, price = $3, gender = $4, condition = $5, category = $6, status = $7, images = $8, updated_at = NOW()
            WHERE id = $9
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
    .bind(product_id)
    .fetch_one(&app_state.db_pool) // Jeśli używasz transakcji, zmień na &mut *tx
    .await.map_err(|e| {
        tracing::error!("Błąd aktualizacji produktu w DB: {}", e);
        // tx.rollback().await.ok();
        AppError::SqlxError(e)
    })?;

    // tx.commit().await.map_err(AppError::SqlxError)?; // Zatwierdź transakcję

    tracing::info!("Pomyślnie zaktualizowano produkt o ID: {}", product_id);
    Ok(Json(updated_product_db))
}

pub async fn delete_product_handler(
    State(app_state): State<AppState>,
    Path(product_id): Path<Uuid>,
    claims: TokenClaims,
) -> Result<(StatusCode, HeaderMap), AppError> {
    tracing::info!("Obsłużono zapytanie DELETE /api/products/{}", product_id);

    if claims.role != Role::Admin {
        return Err(AppError::UnauthorizedAccess(
            "Tylko administrator może usuwać produkty".to_string(),
        ));
    }

    // Pobierz produkt, aby uzyskać listę obrazów
    let product_to_delete = sqlx::query_as::<_, Product>(
        r#"
            SELECT id, name, description, price, gender, condition, category, status, images, created_at, updated_at
            FROM products
            WHERE id = $1
        "#,
    )
    .bind(product_id)
    .fetch_optional(&app_state.db_pool)
    .await
    .map_err(|e| AppError::SqlxError(e))?
    .ok_or(AppError::NotFound)?;

    // 2. Spróbuj usunąć obrazy z Cloudinary
    if !product_to_delete.images.is_empty() {
        let mut delete_futures = Vec::new();
        for image_url in product_to_delete.images {
            if let Some(public_id) =
                extract_public_id_from_url(&image_url, &app_state.cloudinary_config.cloud_name)
            {
                let config_clone = app_state.cloudinary_config.clone();
                let public_id_clone = public_id.to_string(); // Klonujemy public_id do logowania
                delete_futures.push(async move {
                    tracing::info!(
                        "Próba usunięcia obrazka z Cloudinary, public_id: '{}' (z URL: '{}')",
                        public_id_clone,
                        image_url
                    );
                    delete_image_from_cloudinary(&public_id, &config_clone).await
                    // Tutaj możesz chcieć inaczej obsłużyć błędy, np. logować i kontynuować
                });
            } else {
                tracing::warn!(
                    "Nie można wyodrębnić public_id z URL do usunięcia: {}",
                    image_url
                );
            }
        }

        if !delete_futures.is_empty() {
            // try_join_all zwróci błąd, jeśli którakolwiek z operacji się nie powiedzie.
            // Możesz chcieć użyć join_all i ręcznie sprawdzić wyniki, jeśli chcesz kontynuować mimo błędów.
            match try_join_all(delete_futures).await {
                Ok(_) => tracing::info!(
                    "Pomyślnie usunięto obrazy z Cloudinary dla produktu {}",
                    product_id
                ),
                Err(e) => {
                    // Zdecyduj, czy ten błąd powinien zatrzymać usunięcie produktu z DB.
                    // Na razie logujemy i kontynuujemy. Możesz zwrócić AppError::InternalServerError.
                    tracing::error!(
                        "Błąd podczas usuwania niektórych obrazków z Cloudinary dla produktu {}: {:?}. Produkt zostanie usunięty z bazy danych.",
                        product_id,
                        e
                    );
                }
            }
        }
    }

    // 3. Usuń produkt z bazy danych
    let result = sqlx::query(
        r#"
            DELETE FROM products
            WHERE id = $1
        "#,
    )
    .bind(product_id)
    .execute(&app_state.db_pool)
    .await;

    match result {
        Ok(query_result) => {
            let mut headers = HeaderMap::new();
            if query_result.rows_affected() == 0 {
                tracing::warn!(
                    "DELETE: Nie znaleziono produktu do usunięcia w DB o ID {} (może już usunięty). Mimo to, trigger odświeżenia listy.",
                    product_id
                );
                // Nawet jeśli wiersz nie istniał, warto odświeżyć listę, aby UI był spójny.
            } else {
                tracing::info!("Usunięto produkt o ID: {} z bazy danych", product_id);
            }

            // Zawsze wysyłaj trigger do przeładowania listy
            headers.insert(
                "HX-Trigger",
                HeaderValue::from_static("reloadAdminProductList"),
            );

            let toast_payload = serde_json::json!({
                "showMessage": {
                    "message": "Produkt zostal pomyslnie usuniety.",
                    "type": "success"
                }
            });
            // Użyj HX-Trigger-After-Swap, aby toast pojawił się po ewentualnym swapie wykonanym przez przycisk (jeśli jakiś zostawisz)
            // Lub HX-Trigger, jeśli przycisk nie robi swapu.
            if let Ok(val) = HeaderValue::from_str(&toast_payload.to_string()) {
                headers.insert("HX-Trigger", val); // Możesz mieć wiele zdarzeń w jednym HX-Trigger, oddzielonych przecinkami,
                // lub wysłać je w osobnych nagłówkach HX-Trigger-*, np. HX-Trigger-After-Settle
            }

            // Zwróć status 200 OK, aby HTMX przetworzył nagłówki HX-*.
            // Status 204 No Content może spowodować, że niektóre przeglądarki/HTMX zignorują ciało lub nagłówki.
            Ok((StatusCode::OK, headers))
        }
        Err(err) => {
            tracing::error!(
                "DELETE: Błąd bazy danych podczas usuwania produktu {}: {:?}",
                product_id,
                err
            );
            // W przypadku błędu nie wysyłamy triggera do przeładowania,
            // błąd powinien być obsłużony przez mechanizmy błędów HTMX lub globalny handler błędów.
            Err(AppError::SqlxError(err)) // Konwertuj na AppError lub zwróć bezpośrednio
        }
    }
}

pub async fn register_handler(
    State(app_state): State<AppState>,
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
    State(app_state): State<AppState>,
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
            let mut headers = HeaderMap::new();
            headers.insert("HX-Reswap", HeaderValue::from_static("none"));

            let trigger_payload = json!({
                "loginSuccessDetails": {"token": token_str}, // Przekazujemy token do JS
                "showMessage": {"message": "Zalogowano pomyslnie!", "type": "success"}
            });
            if let Ok(trigger_value) = HeaderValue::from_str(&trigger_payload.to_string()) {
                headers.insert("HX-Trigger", trigger_value);
            }

            tracing::info!(
                "Użytkownik {} ({}) zalogowany pomyślnie.",
                user.email,
                user.id
            );
            // Ciało odpowiedzi może być puste lub zawierać potwierdzenie, HTMX go nie podmieni.
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
    State(app_state): State<AppState>,
    OptionalTokenClaims(user_claims_opt): OptionalTokenClaims,
    guest_cart_id_header: Option<TypedHeader<XGuestCartId>>,
    Form(payload): Form<CheckoutFormPayload>,
) -> Result<(HeaderMap, StatusCode), AppError> {
    // Zmieniono odpowiedź na (HeaderMap, StatusCode)
    // Walidacja payloadu
    if let Err(validation_errors) = payload.validate() {
        tracing::warn!("Błąd walidacji danych checkout: {:?}", validation_errors);
        // Zwróć błędy w sposób, który HTMX może wyświetlić, np. przez HX-Trigger
        let mut headers = HeaderMap::new();
        let error_message_str = validation_errors.to_string(); // Prosty komunikat
        headers.insert(
            "HX-Trigger",
            HeaderValue::from_str(&format!(
                r#"{{"showMessage": {{"message": "Błędy w formularzu: {}", "type": "error"}}}}"#,
                error_message_str.replace('"', "\\\"")
            ))
            .map_err(|_| {
                AppError::InternalServerError("Failed to create HX-Trigger header".to_string())
            })?,
        );
        headers.insert("HX-Reswap", HeaderValue::from_static("none"));
        return Ok((headers, StatusCode::UNPROCESSABLE_ENTITY));
    }

    let mut order_user_id: Option<Uuid> = None;
    let mut order_guest_email: Option<String> = None;
    let mut order_guest_session_id: Option<Uuid> = None;

    let cart_query_id: Uuid;
    let cart_selector_sql: String;

    if let Some(claims) = user_claims_opt {
        // Użytkownik zalogowany
        let user_id = claims.sub;
        order_user_id = Some(user_id);
        cart_query_id = user_id;
        cart_selector_sql =
            "SELECT * FROM shopping_carts WHERE user_id = $1 FOR UPDATE".to_string();
        tracing::info!("Zalogowany użytkownik {} składa zamówienie.", user_id);
    } else if let Some(TypedHeader(XGuestCartId(guest_id))) = guest_cart_id_header {
        // Użytkownik-gość
        order_guest_session_id = Some(guest_id);
        cart_query_id = guest_id;
        cart_selector_sql =
            "SELECT * FROM shopping_carts WHERE guest_session_id = $1 FOR UPDATE".to_string();

        // Dla gościa, email jest wymagany
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
            return Ok((headers, StatusCode::UNPROCESSABLE_ENTITY));
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
        // Ani zalogowany, ani nagłówek gościa - błąd
        tracing::error!("Próba złożenia zamówienia bez identyfikacji użytkownika lub gościa.");
        return Err(AppError::UnauthorizedAccess(
            "Nie można zidentyfikować użytkownika ani sesji gościa.".to_string(),
        ));
    }

    let mut tx = app_state.db_pool.begin().await?;

    // 1. Znajdź koszyk (użytkownika lub gościa) i jego pozycje
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

    // 2. Przetwórz pozycje koszyka (logika pozostaje podobna)
    let mut order_items_to_create: Vec<(Uuid, i64)> = Vec::with_capacity(cart_items_db.len());
    let mut total_price_items: i64 = 0; // Tylko suma produktów
    let mut product_ids_to_mark_reserved: Vec<Uuid> = Vec::new();

    for cart_item in &cart_items_db {
        let product =
            sqlx::query_as::<_, Product>("SELECT * FROM products WHERE id = $1 FOR UPDATE")
                .bind(cart_item.product_id)
                .fetch_optional(&mut *tx)
                .await?;

        match product {
            Some(p) => {
                if p.status != ProductStatus::Available {
                    tracing::warn!(
                        "Produkt {} (ID: {}) w koszyku jest niedostępny (status: {:?}).",
                        p.name,
                        p.id,
                        p.status
                    );
                    return Err(AppError::UnprocessableEntity(format!(
                        "Produkt '{}' w Twoim koszyku stał się niedostępny.",
                        p.name
                    )));
                }
                order_items_to_create.push((p.id, p.price));
                total_price_items += p.price;
                product_ids_to_mark_reserved.push(p.id);
            }
            None => {
                tracing::error!(
                    "Produkt o ID {} (z koszyka) nie został znaleziony w bazie.",
                    cart_item.product_id
                );
                // Rozważ usunięcie tej pozycji z koszyka, ale lepiej przerwać proces
                return Err(AppError::InternalServerError(
                    "Błąd spójności danych: produkt z koszyka nie istnieje.".to_string(),
                ));
            }
        }
    }

    let payment_method_enum = PaymentMethod::from_str(&payload.payment_method).map_err(|_| {
        tracing::warn!(
            "Nieprawidłowa metoda płatności otrzymana z formularza: {}",
            payload.payment_method
        );
        // Zwróć błąd walidacji lub użyj domyślnej, jeśli to ma sens (raczej błąd)
        AppError::Validation("Nieprawidłowa metoda płatności.".to_string())
    })?;

    // TODO: Dodaj logikę dla kosztów dostawy - na razie total_price to suma produktów
    let shipping_cost: i64 = 1500;
    let final_total_price = total_price_items + shipping_cost;

    // 3. Wstaw rekord do tabeli `orders`
    let initial_status = OrderStatus::Pending;
    let order_id = Uuid::new_v4(); // Generujemy ID zamówienia

    sqlx::query(
        r#"
            INSERT INTO orders (
                id, user_id, guest_email, guest_session_id, status, total_price,
                shipping_first_name, shipping_last_name,
                shipping_address_line1, shipping_address_line2,
                shipping_city, shipping_postal_code, shipping_country,
                shipping_phone, payment_method
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
        "#,
    )
    .bind(order_id)
    .bind(order_user_id) // Może być NULL
    .bind(option_string_empty_as_none(order_guest_email)) // Może być NULL
    .bind(order_guest_session_id) // Może być NULL
    .bind(initial_status)
    .bind(final_total_price) // Całkowita cena z dostawą
    .bind(&payload.shipping_first_name)
    .bind(&payload.shipping_last_name)
    .bind(&payload.shipping_address_line1)
    .bind(option_string_empty_as_none(
        payload.shipping_address_line2.clone(),
    ))
    .bind(&payload.shipping_city)
    .bind(&payload.shipping_postal_code)
    .bind(&payload.shipping_country)
    .bind(&payload.shipping_phone)
    .bind(payment_method_enum)
    .execute(&mut *tx)
    .await?;

    // Pobierz właśnie utworzone zamówienie (lub przynajmniej jego ID i user_id/guest_email)
    // Aby uniknąć N+1 przy budowaniu OrderDetailsResponse, OrderDetailsResponse nie jest już zwracane.
    // Zamiast tego, po sukcesie wysyłamy HX-Redirect lub odpowiedni trigger.

    // 4. Wstaw rekordy do tabeli `order_items`
    for (product_id, price_at_purchase) in order_items_to_create {
        sqlx::query(
            r#"
                INSERT INTO order_items (order_id, product_id, price_at_purchase)
                VALUES ($1, $2, $3)
            "#,
        )
        .bind(order_id)
        .bind(product_id)
        .bind(price_at_purchase)
        .execute(&mut *tx)
        .await?;
    }

    // 5. Wyczyść koszyk (użytkownika lub gościa)
    sqlx::query("DELETE FROM cart_items WHERE cart_id = $1")
        .bind(cart.id)
        .execute(&mut *tx)
        .await?;
    // Opcjonalnie, jeśli koszyk gościa ma być całkowicie usunięty po zamówieniu
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

    // 6. Zaktualizuj status zamówionych produktów na 'Reserved'
    if !product_ids_to_mark_reserved.is_empty() {
        sqlx::query(r#"UPDATE products SET status = $1 WHERE id = ANY($2)"#)
            .bind(ProductStatus::Sold) // Status: Sprzedane
            .bind(&product_ids_to_mark_reserved)
            .execute(&mut *tx)
            .await?;
    }

    tx.commit().await?;

    tracing::info!("Utworzono nowe zamówienie ID: {}", order_id);

    // Przygotuj odpowiedź dla HTMX
    let mut headers = HeaderMap::new();
    // Przekieruj na stronę podsumowania zamówienia lub stronę główną z komunikatem
    // np. /zamowienie/dziekujemy/{order_id}
    // Na razie prosty komunikat i sugestia przeładowania strony przez JS klienta
    let success_payload = serde_json::json!({
        "showMessage": {
            "message": "Twoje zamowienie zostalo pomyslnie zlozone!",
            "type": "success"
        },
        "orderPlaced": { // Niestandardowe zdarzenie, które JS może obsłużyć
            "orderId": order_id.to_string(),
            "redirectTo": "/" // lub "/moje-konto/zamowienia"
        },
        "clearCartDisplay": {} // Sygnał do wyczyszczenia wyświetlania koszyka
    });
    headers.insert(
        "HX-Trigger",
        HeaderValue::from_str(&success_payload.to_string()).map_err(|_| {
            AppError::InternalServerError("Failed to create HX-Trigger header".to_string())
        })?,
    );
    // HX-Redirect może być lepszy, jeśli masz dedykowaną stronę "dziękujemy"
    // headers.insert("HX-Redirect", HeaderValue::from_static("/zamowienie/dziekujemy"));

    Ok((headers, StatusCode::OK)) // Zwracamy OK, resztę obsługuje HTMX po stronie klienta
}

pub async fn list_orders_handler(
    State(app_state): State<AppState>,
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
                o.id, o.user_id, o.order_date, o.status, o.total_price,
                o.shipping_first_name, o.shipping_last_name,
                o.shipping_address_line1, o.shipping_address_line2,
                o.shipping_city, o.shipping_postal_code, o.shipping_country, o.shipping_phone,
                o.payment_method, o.guest_email, o.guest_session_id,
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
    State(app_state): State<AppState>,
    claims: TokenClaims,
    Path(order_id): Path<Uuid>,
) -> Result<Json<OrderDetailsResponse>, AppError> {
    let user_id = claims.sub;
    let user_role = claims.role;

    // 1. Pobieranie zamówienia
    let order_optional = sqlx::query_as::<_, Order>(
        r#"
            SELECT
                id,
                user_id,
                order_date,
                status,
                total_price,
                shipping_first_name,
                shipping_last_name, 
                shipping_address_line1,
                shipping_address_line2,
                shipping_city,
                shipping_postal_code,
                shipping_country,
                shipping_phone,     
                payment_method,
                guest_email,       
                guest_session_id,  
                created_at,
                updated_at
            FROM orders
            WHERE id = $1
        "#,
    )
    .bind(order_id)
    .fetch_optional(&app_state.db_pool)
    .await?;

    let order = match order_optional {
        Some(o) => o,
        None => {
            tracing::warn!(
                "Nie znaleziono zamówienia o ID: {} (żądane przez user_id: {})",
                order_id,
                user_id
            );
            return Err(AppError::NotFound);
        }
    };

    // 2. Autoryzacja
    if user_role != Role::Admin && order.user_id != Some(user_id) {
        tracing::warn!(
            "Nieautoryzowany dostęp do zamówienia: order_id={}, user_id={}, user_role={:?}",
            order_id,
            user_id,
            user_role
        );
        return Err(AppError::UnauthorizedAccess(
            "Nie masz uprawnień do tego zamówienia".to_string(),
        ));
        // Lub return Err(AppError::NotFound); jeśli chcesz ukryć istnienie zamówienia
    }

    // 3. Pobierz pozycje zamówienia (order_items) z bazy
    let order_items_db = sqlx::query_as::<_, OrderItem>(
        r#"
            SELECT id, order_id, product_id, price_at_purchase
            FROM order_items
            WHERE order_id = $1
        "#,
    )
    .bind(order_id)
    .fetch_all(&app_state.db_pool)
    .await?;

    // 4. Dla każdej pozycji zamówienia, pobierz pełne dane produktu i stwórz OrderItemDetailsPublic
    let mut items_details_public: Vec<OrderItemDetailsPublic> =
        Vec::with_capacity(order_items_db.len());

    for item_db in order_items_db {
        // item_db jest typu OrderItem
        let product = sqlx::query_as::<_, Product>(
            r#"
                SELECT id, name, description, price, gender, condition, category, status, images, created_at, updated_at
                FROM products
                WHERE id = $1
            "#,
        )
        .bind(item_db.product_id)
        .fetch_one(&app_state.db_pool) // Zakładamy, że produkt musi istnieć, jeśli jest w order_items
        .await
        .map_err(|e| {
            // Ten błąd byłby poważny - oznaczałby niespójność danych (pozycja zamówienia
            // odwołuje się do nieistniejącego produktu)
            tracing::error!(
                "Krytyczny błąd: Produkt (ID: {}) dla pozycji zamówienia (ID: {}) nie został znaleziony. OrderID: {}. Błąd: {:?}",
                item_db.product_id, item_db.id, order_id, e
            );
            AppError::InternalServerError("Wystąpił błąd podczas pobierania szczegółów produktu dla zamówienia.".to_string())
        })?;

        items_details_public.push(OrderItemDetailsPublic {
            order_item_id: item_db.id, // ID z tabeli order_items
            product,                   // Pełne dane produktu
            price_at_purchase: item_db.price_at_purchase,
        });
    }

    // 5. Skonstruuj odpowiedź
    let response = OrderDetailsResponse {
        order,                       // Obiekt Order
        items: items_details_public, // Teraz jest to Vec<OrderItemDetailsPublic>
    };

    tracing::info!(
        "Pobrano szczegóły zamówienia: order_id={}, user_id={}",
        order_id,
        user_id
    );
    Ok(Json(response))
}

pub async fn update_order_status_handler(
    State(app_state): State<AppState>,
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
                    "message": "Status zamówienia został pomyślnie zaktualizowany.",
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
    State(app_state): State<AppState>,
    claims: TokenClaims,
    Json(payload): Json<AddProductToCartPayload>,
) -> Result<(StatusCode, Json<CartDetailsResponse>), AppError> {
    let user_id = claims.sub;

    // Rozpocznij transakcję
    let mut tx = app_state.db_pool.begin().await.map_err(|e| {
        tracing::error!("Nie można rozpocząć transakcji (koszyk): {}", e);
        AppError::InternalServerError("Błąd serwera przy dodawaniu do koszyka".to_string())
    })?;

    // Znajdź lub utwórz koszyk dla użytkownika
    let cart = match sqlx::query_as::<_, ShoppingCart>(
        r#"
            SELECT id, user_id, created_at, updated_at
            FROM shopping_carts
            WHERE user_id = $1
            FOR UPDATE
        "#,
    )
    .bind(user_id)
    .fetch_optional(&mut *tx) // Używamy &mut *tx dla typu &mut PgConnection
    .await?
    {
        Some(existing_cart) => existing_cart,
        None => {
            sqlx::query_as::<_, ShoppingCart>(
                r#"
                    INSERT INTO shopping_carts (user_id)
                    VALUES ($1)
                    RETURNING id, user_id, created_at, updated_at
                "#,
            )
            .bind(user_id)
            .fetch_one(&mut *tx) // Używamy &mut *tx
            .await?
        }
    };

    // Sprawdź czy produkt istnieje i jest dostępny (z blokadą FOR UPDATE)
    let product_to_add_opt = sqlx::query_as::<_, Product>(
        r#"
            SELECT id, name, description, price, condition, category, status, images, created_at, updated_at
            FROM products
            WHERE id = $1
            FOR UPDATE
        "#,
    )
    .bind(payload.product_id)
    .fetch_optional(&mut *tx) // Używamy &mut *tx
    .await?;

    match product_to_add_opt {
        Some(product) => {
            if product.status != ProductStatus::Available {
                tracing::warn!(
                    "Użytkownik {} próbował dodać niedostępny produkt {} (status: {:?}) do koszyka {}",
                    user_id,
                    payload.product_id,
                    product.status, // Dodano logowanie statusu produktu
                    cart.id
                );
                return Err(AppError::NotFound); // Lub bardziej specyficzny błąd, np. "Produkt niedostępny"
            }

            // Dodaj produkt do cart_items
            sqlx::query(
                r#"
                    INSERT INTO cart_items (cart_id, product_id)
                    VALUES ($1, $2)
                    ON CONFLICT (cart_id, product_id) DO NOTHING
                "#,
            )
            .bind(cart.id)
            .bind(payload.product_id) // Lub product.id
            .execute(&mut *tx) // Używamy &mut *tx
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

    // Pobierz zaktualizowaną zawartość koszyka do zwrócenia (nadal w ramach transakcji)
    // Jest to potrzebne, aby obliczyć total_price i zebrać listę items.
    let items_db = sqlx::query_as::<_, CartItem>(
        r#"
            SELECT id, cart_id, product_id, added_at
            FROM cart_items
            WHERE cart_id = $1
            ORDER BY added_at ASC
        "#, // Zmieniono DESC na ASC dla spójności (kolejność dodawania)
    )
    .bind(cart.id)
    .fetch_all(&mut *tx) // Używamy &mut *tx
    .await?;

    let mut cart_items_public: Vec<CartItemPublic> = Vec::with_capacity(items_db.len());
    let mut current_total_price: i64 = 0;

    for item_db in items_db {
        let product = sqlx::query_as::<_, Product>(
            r#"
                SELECT id, name, description, price, gender, condition, category, status, images, created_at, updated_at
                FROM products
                WHERE id = $1
            "#, // FOR UPDATE nie jest tu konieczne, bo produkt był blokowany wcześniej
        )
        .bind(item_db.product_id)
        .fetch_one(&mut *tx) // Nadal używamy &mut *tx
        .await
        .map_err(|e| {
            tracing::error!(
                "Błąd pobierania produktu dla pozycji koszyka: {:?}, produkt_id: {:?}",
                e,
                item_db.product_id
            );
            AppError::InternalServerError(
                "Błąd przy konstruowaniu odpowiedzi koszyka (produkt zniknął?).".to_string(),
            )
        })?;

        current_total_price += product.price;
        cart_items_public.push(CartItemPublic {
            cart_item_id: item_db.id,
            product,
            added_at: item_db.added_at,
        });
    }

    // Zatwierdź transakcję
    tx.commit().await.map_err(|e| {
        tracing::error!("Nie można zatwierdzić transakcji (koszyk): {}", e);
        AppError::InternalServerError("Błąd serwera przy zapisywaniu koszyka.".to_string())
    })?;

    // Po zatwierdzeniu transakcji pobieramy najświeższy updated_at dla koszyka,
    // ponieważ trigger mógł go zaktualizować.
    let final_cart_updated_at = sqlx::query_scalar::<_, DateTime<Utc>>(
        "SELECT updated_at FROM shopping_carts WHERE id = $1",
    )
    .bind(cart.id) // Używamy cart.id, który mamy z początku funkcji
    .fetch_one(&app_state.db_pool) // Wykonujemy zapytanie na puli, bo transakcja jest zakończona
    .await
    .unwrap_or_else(|e| {
        tracing::warn!("Nie udało się pobrać zaktualizowanego updated_at dla koszyka {}: {}. Używam starej wartości.", cart.id, e);
        cart.updated_at // Fallback do wartości sprzed commitu, jeśli odczyt zawiedzie
    });

    let response_cart = CartDetailsResponse {
        cart_id: cart.id,
        user_id: Some(user_id),
        total_items: cart_items_public.len(), // Poprawne użycie długości wektora
        items: cart_items_public,
        total_price: current_total_price,
        updated_at: final_cart_updated_at, // Używamy świeżo pobranej (lub fallback) wartości
    };

    Ok((StatusCode::OK, Json(response_cart)))
}

pub async fn get_cart_handler(
    State(app_state): State<AppState>,
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
    State(app_state): State<AppState>,
    claims: TokenClaims,
    Path(product_id_to_remove): Path<Uuid>, // ID produktu do usunięcia
) -> Result<Json<CartDetailsResponse>, AppError> {
    // Zwracamy zaktualizowany koszyk
    let user_id = claims.sub;
    tracing::info!(
        "Użytkownik {} żąda usunięcia produktu {} ze swojego koszyka",
        user_id,
        product_id_to_remove
    );

    // 1. Rozpocznij transakcję
    let mut tx = app_state.db_pool.begin().await.map_err(|e| {
        tracing::error!("Nie można rozpocząć transakcji (usuwanie z koszyka): {}", e);
        AppError::InternalServerError("Błąd serwera przy usuwaniu z koszyka.".to_string())
    })?;

    // 2. Znajdź koszyk użytkownika
    let cart = match sqlx::query_as::<_, ShoppingCart>(
        "SELECT * FROM shopping_carts WHERE user_id = $1 FOR UPDATE", // FOR UPDATE, bo modyfikujemy jego zawartość
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
            // Jeśli nie ma koszyka, to nie ma z czego usuwać.
            return Err(AppError::NotFound); // Można zwrócić NotFound, że koszyk nie istnieje
        }
    };

    // 3. Usuń produkt z cart_items
    let delete_result = sqlx::query(
        r#"
            DELETE FROM cart_items
            WHERE cart_id = $1 AND product_id = $2
        "#,
    )
    .bind(cart.id)
    .bind(product_id_to_remove)
    .execute(&mut *tx)
    .await?;

    if delete_result.rows_affected() == 0 {
        // Produktu nie było w koszyku lub produkt_id niepoprawne
        // Możemy to zignorować (operacja idempotentna) lub zwrócić błąd/informację
        tracing::warn!(
            "Produkt {} nie został znaleziony w koszyku {} użytkownika {} do usunięcia (lub już usunięty).",
            product_id_to_remove,
            cart.id,
            user_id
        );
        // Jeśli produkt nie został znaleziony w koszyku, nie ma potrzeby zwracać błędu,
        // po prostu koszyk się nie zmienił pod tym względem.
        // Ale jeśli chcemy być ścisli, to produkt, który chciano usunąć, nie został znaleziony.
        // Można by zwrócić NotFound, ale to może być mylące.
        // Na razie kontynuujemy i zwrócimy aktualny stan koszyka.
    } else {
        tracing::info!(
            "Produkt {} usunięty z koszyka {} dla użytkownika {}",
            product_id_to_remove,
            cart.id,
            user_id
        );
    }

    // 4. Pobierz i zwróć zaktualizowaną zawartość koszyka
    // Używamy funkcji pomocniczej, którą stworzyliśmy
    let cart_details = build_cart_details_response(&cart, &mut *tx).await?;

    // 5. Zatwierdź transakcję
    tx.commit().await.map_err(|e| {
        tracing::error!(
            "Nie można zatwierdzić transakcji (usuwanie z koszyka): {}",
            e
        );
        AppError::InternalServerError("Błąd serwera przy aktualizacji koszyka.".to_string())
    })?;

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

pub async fn add_item_to_guest_cart(
    State(app_state): State<AppState>,
    guest_cart_id_header: Option<TypedHeader<XGuestCartId>>,
    Json(payload): Json<AddProductToCartPayload>,
) -> Result<impl IntoResponse, AppError> {
    let mut tx = app_state.db_pool.begin().await?;
    let product_id = payload.product_id;

    let guest_cart_uuid: Uuid;
    let cart: ShoppingCart;

    if let Some(TypedHeader(XGuestCartId(id))) = guest_cart_id_header {
        // Nagłówek X-Guest-Cart-Id jest obecny, używamy 'id'
        if let Some(existing_cart) = sqlx::query_as::<_, ShoppingCart>(
            r#"
            SELECT id, user_id, guest_session_id, created_at, updated_at
            FROM shopping_carts
            WHERE guest_session_id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        {
            // Koszyk dla danego guest_session_id istnieje
            cart = existing_cart;
            guest_cart_uuid = id;
        } else {
            // Koszyk dla danego guest_session_id nie istnieje, tworzymy nowy z tym 'id'
            cart = sqlx::query_as::<_, ShoppingCart>(
                r#"
                    INSERT INTO shopping_carts (guest_session_id)
                    VALUES ($1)
                    RETURNING id, user_id, guest_session_id, created_at, updated_at
                "#, // Usunięto zbędny cudzysłów na końcu zapytania
            )
            .bind(id) // POPRAWKA: Użyj 'id' z nagłówka zamiast 'new_id'
            .fetch_one(&mut *tx)
            .await?;
            guest_cart_uuid = id; // POPRAWKA: Użyj 'id' z nagłówka zamiast 'new_id'
        }
    } else {
        // Nagłówek X-Guest-Cart-Id nie jest obecny, generujemy nowy UUID
        let new_generated_id = Uuid::new_v4(); // Zmieniono nazwę na bardziej opisową
        cart = sqlx::query_as::<_, ShoppingCart>(
            r#"
                INSERT INTO shopping_carts (guest_session_id)
                VALUES ($1)
                RETURNING id, user_id, guest_session_id, created_at, updated_at
            "#,
        )
        .bind(new_generated_id)
        .fetch_one(&mut *tx)
        .await?;
        guest_cart_uuid = new_generated_id;
    }

    // Sprawdź, czy produkt istnieje i jest dostępny (opcjonalne, ale zalecane)
    // Możesz dodać tutaj logikę sprawdzania produktu, podobnie jak w `add_item_to_cart_handler`

    // Dodaj produkt do cart_items
    let existing_item =
        sqlx::query("SELECT id FROM cart_items WHERE cart_id = $1 AND product_id = $2")
            .bind(cart.id)
            .bind(product_id)
            .fetch_optional(&mut *tx)
            .await?;

    if existing_item.is_none() {
        sqlx::query(
            // Usunięto niepotrzebny typ generyczny <_, CartItem> dla execute()
            "INSERT INTO cart_items (cart_id, product_id) VALUES ($1, $2)",
        )
        .bind(cart.id)
        .bind(product_id)
        .execute(&mut *tx)
        .await?;
    }

    // Zaktualizuj updated_at w koszyku
    let updated_cart = sqlx::query_as::<_, ShoppingCart>(
        r#"
        UPDATE shopping_carts SET updated_at = NOW()
        WHERE id = $1
        RETURNING id, user_id, guest_session_id, created_at, updated_at"#,
    )
    .bind(cart.id)
    .fetch_one(&mut *tx)
    .await?;

    let cart_details_response = build_cart_details_response(&updated_cart, &mut *tx).await?; // Przekazanie transakcji
    tx.commit().await?;

    let response_payload = GuestCartOperationResponse {
        // Zmieniono nazwę zmiennej dla jasności
        guest_cart_id: guest_cart_uuid,
        cart_details: cart_details_response,
    };

    let mut headers = HeaderMap::new();
    headers.insert(
        "X-Guest-Cart-Id", // Użyj stałej HeaderName jeśli to możliwe
        guest_cart_uuid.to_string().parse().map_err(|_| {
            AppError::InternalServerError("Failed to parse UUID for header".to_string())
        })?,
    );

    Ok((StatusCode::OK, headers, Json(response_payload)))
}
//GET /api/guest-cart
pub async fn get_guest_cart(
    State(app_state): State<AppState>,
    guest_cart_id_header: Option<TypedHeader<XGuestCartId>>,
) -> Result<impl IntoResponse, AppError> {
    if let Some(TypedHeader(XGuestCartId(guest_id))) = guest_cart_id_header {
        let mut conn = app_state.db_pool.acquire().await?;
        if let Some(cart) = sqlx::query_as::<_, ShoppingCart>(
            r#"
                SELECT id, user_id, guest_session_id, created_at, updated_at
                FROM shopping_carts
                WHERE guest_session_id = $1
            "#,
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
    State(app_state): State<AppState>,
    guest_cart_id_header: Option<TypedHeader<XGuestCartId>>,
    Path(product_id_to_remove): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let TypedHeader(XGuestCartId(guest_id)) = guest_cart_id_header
        .ok_or_else(|| AppError::BadRequest("Missing X-Guest-Cart-Id header".to_string()))?;

    let mut tx = app_state.db_pool.begin().await?;

    let cart = sqlx::query_as::<_, ShoppingCart>(
        // Zmieniono przypisanie 'cart'
        r#"
            SELECT id, user_id, guest_session_id, created_at, updated_at
            FROM shopping_carts
            WHERE guest_session_id = $1 -- Poprawiona literówka: guest_session_id
        "#,
    )
    .bind(guest_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::NotFound)?; // <--- Dodaj '?' tutaj

    // Usuń produkt z koszyka
    sqlx::query(
        // Usunięto zbędny typ <_, CartItem>
        r#"
            DELETE FROM cart_items
            WHERE cart_id = $1
            AND product_id = $2
        "#,
    )
    .bind(cart.id) // <--- Poprawka: Użyj bezpośrednio cart.id
    .bind(product_id_to_remove)
    .execute(&mut *tx)
    .await?;

    let updated_cart = sqlx::query_as::<_, ShoppingCart>(
        r#"
            UPDATE shopping_carts
            SET updated_at = NOW()
            WHERE id = $1
            RETURNING id, user_id, guest_session_id, created_at, updated_at
        "#,
    )
    .bind(cart.id) // <--- Poprawka: Użyj bezpośrednio cart.id (cart nie jest już Result)
    .fetch_one(&mut *tx)
    .await?;

    let response_details = build_cart_details_response(&updated_cart, &mut *tx).await?;
    tx.commit().await?;

    let response = GuestCartOperationResponse {
        guest_cart_id: guest_id,
        cart_details: response_details,
    };

    Ok((StatusCode::OK, Json(response)))
}

// POST /api/cart/merge/ (Chroniony endpoint)
pub async fn merge_cart_handler(
    State(app_state): State<AppState>,
    user_claims: TokenClaims,
    Json(payload): Json<MergeCartPayload>,
) -> Result<impl IntoResponse, AppError> {
    let user_id = user_claims.sub;
    let guest_cart_id_to_merge = payload.guest_cart_id;

    let mut tx = app_state.db_pool.begin().await?;

    let user_cart = match sqlx::query_as::<_, ShoppingCart>(
        r#"
            SELECT id, user_id, guest_session_id, created_at, updated_at
            FROM shopping_carts
            WHERE user_id = $1
        "#,
    )
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await?
    {
        Some(cart) => cart,
        None => {
            sqlx::query_as::<_, ShoppingCart>(
                r#"
                    INSERT INTO shopping_carts (user_id)
                    VALUES ($1)
                    RETURNING id, user_id, guest_session_id, created_at, updated_at
                "#,
            )
            .bind(user_id)
            .fetch_one(&mut *tx)
            .await?
        }
    };

    if let Some(guest_cart) = sqlx::query_as::<_, ShoppingCart>(
        r#"
            SELECT id, user_id, guest_session_id, created_at, updated_at
            FROM shopping_carts
            WHERE guest_session_id = $1
        "#,
    )
    .bind(guest_cart_id_to_merge)
    .fetch_optional(&mut *tx)
    .await?
    {
        if guest_cart.id != user_cart.id {
            let guest_items = sqlx::query_as::<_, CartItem>(
                r#"
                    SELECT *
                    FROM cart_items
                    WHERE cart_id = $1
                "#,
            )
            .bind(guest_cart.id)
            .fetch_all(&mut *tx)
            .await?;

            for item in guest_items {
                // Sprawdź czy produkt już jest w koszyku użytkownika
                let existing_user_item = sqlx::query(
                    r#"
                        SELECT id
                        FROM cart_items
                        WHERE cart_id = $1
                        AND product_id = $2
                    "#,
                )
                .bind(user_cart.id)
                .bind(item.product_id)
                .fetch_optional(&mut *tx)
                .await?;

                if existing_user_item.is_none() {
                    sqlx::query(
                        r#"
                            INSERT INTO cart_items (cart_id, product_id, added_at)
                            VALUES ($1, $2, $3) 
                        "#,
                    )
                    .bind(user_cart.id)
                    .bind(item.product_id)
                    .bind(item.added_at)
                    .execute(&mut *tx)
                    .await?;
                }
            }
            // Usuń stary koszyk gościa (jego pozycje zostaną usunięte przez ON DELETE CASCADE, jeśli tak jest ustawione dla cart_items.cart_id)
            // Jeśli nie ma CASCADE, usuń najpierw pozycje: DELETE FROM cart_items WHERE cart_id = $1
            sqlx::query("DELETE FROM cart_items WHERE cart_id = $1")
                .bind(guest_cart.id)
                .execute(&mut *tx)
                .await?;
            sqlx::query("DELETE FROM shopping_carts WHERE id = $1")
                .bind(guest_cart.id)
                .execute(&mut *tx)
                .await?;
        } else {
            // Koszyk gościa to ten sam, co już przypisany użytkownikowi - tylko wyczyść guest_session_id
            sqlx::query(
                "UPDATE shopping_carts SET guest_session_id = NULL WHERE id = $1 AND user_id = $2",
            )
            .bind(user_cart.id)
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
        }
    }
    // Jeśli koszyk gościa nie został znaleziony, nic nie rób, użytkownik kontynuuje ze swoim (ew. nowo utworzonym) koszykiem.

    let final_updated_cart = sqlx::query_as::<_, ShoppingCart>(
        "UPDATE shopping_carts SET updated_at = NOW() WHERE id = $1 RETURNING id, user_id, guest_session_id, created_at, updated_at"

    ).bind(user_cart.id).fetch_one(&mut *tx).await?;

    let response = build_cart_details_response(&final_updated_cart, &mut tx).await?;
    tx.commit().await?;

    Ok((StatusCode::OK, Json(response)))
}

fn option_string_empty_as_none(opt_s: Option<String>) -> Option<String> {
    opt_s.filter(|s| !s.is_empty())
}

pub async fn upsert_user_shipping_details_handler(
    State(app_state): State<AppState>,
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
