// src/handlers.rs
use axum::Json;
use axum::{
    extract::{Multipart, Path, Query, State},
    http::StatusCode,
};
use serde_json::{Value, json};
use sqlx::{Postgres, QueryBuilder};

use crate::cart_utils::build_cart_details_response;
use crate::cloudinary::{delete_image_from_cloudinary, extract_public_id_from_url};
use crate::errors::AppError;
use crate::filters::ListingParams;
use crate::models::*;
use crate::pagination::PaginatedProductsResponse;
use crate::{
    auth::{create_jwt, hash_password, verify_password},
    cloudinary::upload_image_to_cloudinary,
    state::AppState,
};
use crate::{
    auth_models::{LoginPayload, RegistrationPayload, TokenClaims},
    models::{CreateOrderPayload, Order, OrderStatus, ProductStatus},
};
use futures::future::try_join_all;
use std::collections::HashMap;
use std::str::FromStr;
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

    if let Some(category) = params.category() {
        append_where_or_and_count(&mut count_builder);
        count_builder.push("category = ").push_bind(category);
    }
    if let Some(condition) = params.condition() {
        append_where_or_and_count(&mut count_builder);
        count_builder.push("condition = ").push_bind(condition);
    }

    let status_to_filter = params.status().unwrap_or(ProductStatus::Available);
    append_where_or_and_count(&mut count_builder);
    // Poprawiona literówka i przekazanie przez referencję
    count_builder.push("status = ").push_bind(&status_to_filter);

    if let Some(price_min) = params.price_min() {
        append_where_or_and_count(&mut count_builder);
        count_builder.push("price >= ").push_bind(price_min);
    }
    if let Some(price_max) = params.price_max() {
        append_where_or_and_count(&mut count_builder);
        count_builder.push("price <= ").push_bind(price_max);
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
            SELECT id, name, description, price, condition, category, status, images
            FROM products
        "#,
    );
    // Zdefiniuj na nowo flagę i domknięcie dla tej sekcji, aby uniknąć problemów z pożyczaniem
    let mut data_added_where = false;
    let mut append_where_or_and_data = |builder: &mut QueryBuilder<Postgres>| {
        if !data_added_where {
            builder.push(" WHERE ");
            data_added_where = true;
        } else {
            builder.push(" AND ");
        }
    };

    if let Some(category) = params.category() {
        append_where_or_and_data(&mut data_builder);
        data_builder.push("category = ").push_bind(category);
    }
    if let Some(condition) = params.condition() {
        append_where_or_and_data(&mut data_builder);
        data_builder.push("condition = ").push_bind(condition);
    }
    append_where_or_and_data(&mut data_builder);
    data_builder.push("status = ").push_bind(&status_to_filter);

    if let Some(price_min) = params.price_min() {
        append_where_or_and_data(&mut data_builder);
        data_builder.push("price >= ").push_bind(price_min);
    }
    if let Some(price_max) = params.price_max() {
        append_where_or_and_data(&mut data_builder);
        data_builder.push("price <= ").push_bind(price_max);
    }

    let sort_by_column = match params.sort_by() {
        "price" => "price",
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
) -> Result<(StatusCode, Json<Product>), AppError> {
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

    let new_product_db = sqlx::query_as::<_, Product>(
        r#"
            INSERT INTO products (id, name, description, price, condition, category, status, images)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING id, name, description, price, condition , category, status , images
        "#,
    )
    .bind(new_product_id)
    .bind(&name)
    .bind(&description)
    .bind(price)
    .bind(condition)
    .bind(category)
    .bind(product_status)
    .bind(&cloudinary_urls)
    .fetch_one(&app_state.db_pool)
    .await?;

    tracing::info!("Utworzono produkt o ID: {}", new_product_id);

    Ok((StatusCode::CREATED, Json(new_product_db)))
}

pub async fn update_product_partial_handler(
    State(app_state): State<AppState>,
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
        "Obsłużono zapytanie PATCH /api/products/{} - aktualizacja: (multipart)",
        product_id,
    );

    // Pobierz istniejący produkt z bazy
    let mut existing_product = sqlx::query_as::<_, Product>(
        r#"
            SELECT * FROM products WHERE id = $1 FOR UPDATE"#,
    )
    .bind(product_id)
    .fetch_one(&app_state.db_pool)
    .await
    .map_err(|err| match err {
        sqlx::Error::RowNotFound => {
            tracing::warn!("PATCH: Nie znaleziono produktu o ID: {}", product_id);
            AppError::NotFound
        }
        _ => {
            tracing::error!(
                "PATCH: Błąd bazy danych podczas pobierania produktu {}: {:?}",
                product_id,
                err
            );
            AppError::SqlxError(err)
        }
    })?;

    // Przetwarzanie danych multipart
    let mut text_fields: HashMap<String, String> = HashMap::new();
    let mut new_image_uploads: Vec<(String, Vec<u8>)> = Vec::new();
    let mut urls_to_delete_str: Option<String> = None;

    while let Some(field) = multipart.next_field().await? {
        let field_name = match field.name() {
            Some(name) => name.to_string(),
            None => {
                tracing::warn!("Odebrano pole multipart bez nazwy w update, pomijam.");
                continue;
            }
        };

        let original_filename_opt = field.file_name().map(|s| s.to_string());
        tracing::info!(
            "Aktualizacja produktu - Przetwarzanie pola: name='{}', filename='{:?}'",
            field_name,
            original_filename_opt
        );

        if field_name.starts_with("image_file_") {
            // Nowe pliki do wgrania
            let filename = original_filename_opt.unwrap_or_else(|| format!("{}.jpg", field_name));
            let bytes = field.bytes().await?; // Używamy '?' - wymaga From<MultipartError>
            if !bytes.is_empty() {
                new_image_uploads.push((filename.clone(), bytes.to_vec()));
                tracing::info!(
                    "Aktualizacja produktu - Dodano plik do wgrania: {}, rozmiar: {} bajtów",
                    filename,
                    bytes.len()
                );
            } else {
                tracing::warn!(
                    "Aktualizacja produktu - Odebrano puste pole pliku: {}",
                    filename
                );
            }
        } else if field_name == "urls_to_delete" {
            // Lista URL-i do usunięcia (jako JSON string)
            urls_to_delete_str = Some(field.text().await?); // Używamy '?'
            tracing::info!("Aktualizacja produktu - Odebrano urls_to_delete");
        } else {
            // Inne pola tekstowe (name, description, etc.)
            let value = field.text().await?; // Używamy '?'
            text_fields.insert(field_name.clone(), value);
            tracing::info!(
                "Aktualizacja produktu - Odebrano pole tekstowe: name='{}', value='{}'",
                field_name,
                text_fields.get(&field_name).unwrap_or(&"".to_string())
            );
        }
    }

    if let Some(name) = text_fields.get("name") {
        existing_product.name = name.clone();
    }
    if let Some(description) = text_fields.get("description") {
        existing_product.description = description.clone();
    }
    if let Some(price) = text_fields.get("price") {
        existing_product.price = price
            .parse()
            .map_err(|_| AppError::UnprocessableEntity("Nieprawidłowy format ceny".to_string()))?;
    }
    if let Some(condition) = text_fields.get("condition") {
        existing_product.condition = ProductCondition::from_str(&condition).map_err(|_| {
            AppError::UnprocessableEntity("Nieprawidłowy stan produktu".to_string())
        })?;
    }
    if let Some(category) = text_fields.get("category") {
        existing_product.category = Category::from_str(&category).map_err(|_| {
            AppError::UnprocessableEntity("Nieprawidłowa kategoria produktu".to_string())
        })?;
    }
    if let Some(status) = text_fields.get("status") {
        existing_product.status = ProductStatus::from_str(&status).map_err(|_| {
            AppError::UnprocessableEntity("Nieprawidłowy status produktu".to_string())
        })?;
    }

    // Przetwarzanie obrazków do usunięcia
    let mut current_image_urls = existing_product.images.clone(); // Klonujemy, aby móc modyfikować
    if let Some(json_str) = urls_to_delete_str {
        match serde_json::from_str::<Vec<String>>(&json_str) {
            Ok(parsed_urls_to_delete) => {
                let mut delete_futures = Vec::new();
                tracing::debug!(
                    "URL-e do usunięcia (po parsowaniu JSON): {:?}",
                    parsed_urls_to_delete,
                );
                for url_to_delete in parsed_urls_to_delete {
                    let url_to_delete_clone = url_to_delete.clone();
                    if let Some(public_id) = extract_public_id_from_url(
                        &url_to_delete,
                        &app_state.cloudinary_config.cloud_name,
                    ) {
                        let config_clone = app_state.cloudinary_config.clone();
                        delete_futures.push(async move {
                            tracing::info!("Próba usunięcia obrazka z Cloudinary, public_id: '{}' (z URL: '{}')", public_id, url_to_delete_clone);
                            delete_image_from_cloudinary(&public_id, &config_clone).await
                        });
                        // Usuń URL z listy bieżących obrazków produktu
                        current_image_urls.retain(|url| url != &url_to_delete);
                    } else {
                        tracing::warn!(
                            "Nie można wyodrębnić public_id z URL do usunięcia: {}",
                            url_to_delete
                        );
                    }
                }
                // Czekanie na zakończenie operacji usuwania z Cloudinary
                if !delete_futures.is_empty() {
                    try_join_all(delete_futures).await.map_err(|e| {
                        tracing::error!("Błąd podczas usuwania obrazków z Cloudinary: {:?}", e);
                        AppError::InternalServerError(format!(
                            "Częściowy błąd usuwania obrazków. Oryginalny błąd: {:?}",
                            e
                        ))
                    })?;
                }
            }
            Err(e) => {
                tracing::error!("Błąd parsowania JSON dla urls_to_delete: {}", e);
                return Err(AppError::UnprocessableEntity(
                    "Nieprawidłowy format listy URL-i do usunięcia.".to_string(),
                ));
            }
        }
    }

    // Wgrywanie nowych obrazków i dodawanie ich URL
    if !new_image_uploads.is_empty() {
        let mut upload_futures = Vec::new();
        for (filename, bytes) in new_image_uploads {
            let config_clone = app_state.cloudinary_config.clone();
            upload_futures.push(async move {
                upload_image_to_cloudinary(bytes, filename, &config_clone).await
            });
        }
        let new_cloudinary_urls: Vec<String> = try_join_all(upload_futures).await?;
        current_image_urls.extend(new_cloudinary_urls);
    }

    // Walidacja
    if current_image_urls.is_empty() {
        return Err(AppError::UnprocessableEntity(
            "Produkt musi mieć przynajmniej jeden obrazek".to_string(),
        ));
    }
    existing_product.images = current_image_urls;

    let updated_product = sqlx::query_as::<_, Product>(r#"
            UPDATE products
            SET name = $1, description = $2, price = $3, condition = $4, category = $5, status = $6, images = $7
            WHERE id = $8
            RETURNING *
        "#).bind(&existing_product.name)
        .bind(&existing_product.description)
        .bind(&existing_product.price)
        .bind(&existing_product.condition)
        .bind(&existing_product.category)
        .bind(&existing_product.status)
        .bind(&existing_product.images)
        .bind(product_id)
        .fetch_one(&app_state.db_pool)
        .await?;

    tracing::info!("Zaktualizowano produkt o ID: {}", product_id);

    Ok(Json(updated_product))
}

pub async fn delete_product_handler(
    State(app_state): State<AppState>,
    Path(product_id): Path<Uuid>,
    claims: TokenClaims,
) -> Result<StatusCode, AppError> {
    tracing::info!("Obsłużono zapytanie DELETE /api/products/{}", product_id);

    if claims.role != Role::Admin {
        return Err(AppError::UnauthorizedAccess(
            "Tylko administrator może usuwać produkty".to_string(),
        ));
    }

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
            // Sprawdzanie ile wierszy zostało usuniętych
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

pub async fn register_handler(
    State(app_state): State<AppState>,
    Json(payload): Json<RegistrationPayload>,
) -> Result<(StatusCode, Json<UserPublic>), AppError> {
    payload.validate()?;

    // Sprawdzanie czy użytkownik istnieje
    let existing_user: Option<User> = sqlx::query_as(
        r#"
            SELECT id, email, password_hash, role, created_at, updated_at
            FROM users
            WHERE email = $1
            "#,
    )
    .bind(&payload.email)
    .fetch_optional(&app_state.db_pool)
    .await?;

    if existing_user.is_some() {
        return Err(AppError::EmailAlreadyExists(
            "Email już istnieje".to_string(),
        ));
    }

    // Hash hasła
    let password_hash = hash_password(&payload.password)?;

    // Wstawianie nowego użytkownika (domyślnie rola Customer)
    let new_user = sqlx::query_as::<_, User>(
        r#"INSERT INTO users (email, password_hash)
                VALUES ($1, $2)
                RETURNING id, email, password_hash, role, created_at, updated_at"#,
    )
    .bind(&payload.email)
    .bind(&password_hash)
    .fetch_one(&app_state.db_pool)
    .await?;

    tracing::info!("Zarejestrowano nowego użytkownika: {}", new_user.email);

    Ok((StatusCode::CREATED, Json(new_user.into())))
}

pub async fn login_handler(
    State(app_state): State<AppState>,
    Json(payload): Json<LoginPayload>,
) -> Result<Json<serde_json::Value>, AppError> {
    payload.validate()?;

    // Znajdywanie użytkownika po emailu
    let user = sqlx::query_as::<_, User>(
        r#"
            SELECT id, email, password_hash, role, created_at, updated_at
            FROM users
            WHERE email = $1
        "#,
    )
    .bind(&payload.email)
    .fetch_optional(&app_state.db_pool)
    .await?
    .ok_or(AppError::InvalidLoginCredentials)?;

    // Weryfikacja hasła
    if !verify_password(&user.password_hash, &payload.password)? {
        return Err(AppError::InvalidLoginCredentials);
    }

    // Wygeneruj token JWT
    let token = create_jwt(
        user.id,
        user.role,
        &app_state.jwt_secret,
        app_state.jwt_expiration_hours,
    )?;

    tracing::info!("Zalogowano użytkownika: {}", user.email);

    // Zwróć token w odpowiedzi JSON
    Ok(Json(serde_json::json!({ "token": token })))
}

pub async fn protected_route_handler(claims: TokenClaims) -> Result<Json<Value>, AppError> {
    Ok(Json(
        json!({ "message": "Gratulacje! Masz dostęp do chronionego zasobu.",
            "user_id": claims.sub,
            "user_role": claims.role,
            "expires_at": claims.exp }),
    ))
}

pub async fn create_order_handler(
    State(app_state): State<AppState>,
    claims: TokenClaims,
    Json(payload): Json<CreateOrderPayload>,
) -> Result<(StatusCode, Json<Order>), AppError> {
    payload.validate()?;

    // Pobieranie ID użytkownika
    let user_id = claims.sub;

    // Rozpoczęcie transakcji bazodanowej
    let mut tx = app_state.db_pool.begin().await.map_err(|e| {
        tracing::error!("Nie można rozpocząć transakcji: {}", e);
        AppError::InternalServerError("Nie można przetworzyć zamówienia".to_string())
    })?;

    // Walidacja i pobranie szczegółów produktów z BLOKADĄ ('FOR UPDATE)
    // Zapobiega to sytuacji, w której dwóch użytkowników jednocześnie
    // próbuje kupić ten sam unikalny produkt.
    let mut total_price: i64 = 0;
    let mut products_to_order: Vec<(Uuid, i64)> = Vec::with_capacity(payload.product_ids.len());

    // Sprawdzanie duplikatów ID produktów w zamówieniu
    let unique_product_ids = payload
        .product_ids
        .iter()
        .cloned()
        .collect::<std::collections::HashSet<_>>();

    if unique_product_ids.len() != payload.product_ids.len() {
        return Err(AppError::UnprocessableEntity(
            "Zamówienie zawiera zduplikowane produkty".to_string(),
        ));
    }

    for product_id in unique_product_ids {
        // Używamy `tx` zamiast `app_state.db_pool` do operacji wewnątrz transakcji
        // Dodajemy `FOR UPDATE` aby zablokować wiersz na czas transakcji
        let product = sqlx::query_as::<_, Product>(
            r#"
            SELECT id, name, description, price, condition, category, status, images
            FROM products
            WHERE id = $1 FOR UPDATE
        "#,
        )
        .bind(product_id)
        .fetch_optional(&mut *tx)
        .await?;

        // Sprawdzanie czy produkt jest dostępny
        match product {
            Some(p) => {
                if p.status != ProductStatus::Available {
                    tracing::warn!(
                        "Próba zamówienia niedostępnego produktu: product_id={}, status={:?}",
                        product_id,
                        p.status
                    );
                    return Err(AppError::NotFound);
                }

                products_to_order.push((p.id, p.price));
                total_price += p.price;
            }
            None => {
                tracing::warn!(
                    "Próba zamówienia nieistniejącego produktu: product_id={}",
                    product_id
                );
                return Err(AppError::NotFound);
            }
        }
    }

    // Obliczanie czy cena zamówienia nie jest ujemna
    if total_price < 0 {
        tracing::error!("Obliczono ujemną cenę całkowitą: {}", total_price);
        return Err(AppError::InternalServerError(
            "Błąd podczas obliczania ceny zamówienia".to_string(),
        ));
    }

    // Wstawianie rekordu do tabeli 'orders'
    let initial_status = OrderStatus::Pending;
    let order = sqlx::query_as::<_, Order>(
        r#"
            INSERT INTO orders (user_id, status, total_price, shipping_address_line1, shipping_address_line2, shipping_city, shipping_postal_code, shipping_country)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING id, user_id, order_date, status, total_price, shipping_address_line1, shipping_address_line2, shipping_city, shipping_postal_code, shipping_country, created_at, updated_at
            "#,
    ).bind(user_id)
    .bind(initial_status)
    .bind(total_price)
    .bind(&payload.shipping_address_line1)
    .bind(payload.shipping_address_line2.as_deref())
    .bind(&payload.shipping_city)
    .bind(&payload.shipping_postal_code)
    .bind(&payload.shipping_country)
    .fetch_one(&mut *tx)
    .await?;

    //Wstawianie rekordów do tabeli 'order_items'
    for (product_id, price_at_purchase) in &products_to_order {
        sqlx::query(
            r#"
                INSERT INTO order_items (order_id, product_id, price_at_purchase)
                VALUES ($1, $2, $3)
        "#,
        )
        .bind(order.id)
        .bind(product_id)
        .bind(price_at_purchase)
        .execute(&mut *tx)
        .await?;
    }

    // Aktualizacja statusu zamówionych produktów na 'Sold'
    // Przygotowanie listy ID do użycia w klauzuli WHERE IN
    let product_ids_to_update: Vec<Uuid> =
        products_to_order.iter().map(|(id, _price)| *id).collect();

    sqlx::query(
        r#"
            UPDATE products
            SET status = $1
            WHERE id = ANY($2)
        "#,
    )
    .bind(ProductStatus::Sold)
    .bind(&product_ids_to_update)
    .execute(&mut *tx)
    .await?;

    // Zatwierdzenie transakcji
    tx.commit().await.map_err(|e| {
        tracing::error!("Nie można zatwierdzić transakcji: {}", e);
        AppError::InternalServerError("Nie można sfinalizować zamówienia.".to_string())
    })?;

    tracing::info!(
        "Utworzono nowe zamówienie: order_id={}, user_id={}",
        order.id,
        user_id
    );

    Ok((StatusCode::CREATED, Json(order)))
}

pub async fn list_orders_handler(
    State(app_state): State<AppState>,
    claims: TokenClaims,
) -> Result<Json<Vec<Order>>, AppError> {
    let user_id = claims.sub;
    let user_role = claims.role;

    let orders: Vec<Order>;

    if user_role == Role::Admin {
        //Admin widzi wszystkie zamówienia
        orders = sqlx::query_as::<_, Order>(
            r#"
                SELECT id, user_id, order_date, status, total_price, shipping_address_line1, shipping_address_line2, shipping_city, shipping_postal_code, shipping_country, created_at, updated_at
                FROM orders
                ORDER BY order_date DESC
                "#
        ).fetch_all(&app_state.db_pool).await?;
        tracing::info!("Admin {} pobrał listę wszystkich zamówień", user_id);
    } else {
        //Customer widzi tylko swoje zamówienia {
        orders = sqlx::query_as::<_, Order>(
            r#"
                SELECT id, user_id, order_date, status, total_price, shipping_address_line1, shipping_address_line2, shipping_city, shipping_postal_code, shipping_country, created_at, updated_at
                FROM orders
                WHERE user_id = $1
                ORDER BY order_date DESC
            "#,
        )
        .bind(user_id)
        .fetch_all(&app_state.db_pool).await?;
        tracing::info!("Użytkownik {} pobrał listę swoich zamówień", user_id);
    }

    Ok(Json(orders))
}

pub async fn get_order_details_handler(
    State(app_state): State<AppState>,
    claims: TokenClaims,
    Path(order_id): Path<Uuid>,
) -> Result<Json<OrderDetailsResponse>, AppError> {
    let user_id = claims.sub;
    let user_role = claims.role;

    // Pobieranie zamówienia
    let order = sqlx::query_as::<_, Order>(r#"
            SELECT id, user_id, order_date, status, total_price, shipping_address_line1, shipping_address_line2, shipping_city, shipping_postal_code, shipping_country, created_at, updated_at
            FROM orders
            WHERE id = $1
        "#).bind(order_id).fetch_optional(&app_state.db_pool).await?;

    // Sprawdzanie czy zamówienie istnieje
    let order = match order {
        Some(o) => o,
        None => {
            tracing::warn!(
                "Nie znaleziono zamówienia o ID: {} dla użytkownika {}",
                order_id,
                user_id
            );
            return Err(AppError::NotFound);
        }
    };

    // Autoryzacja
    if user_role != Role::Admin && order.user_id != user_id {
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

    // Pobierz pozycję zamówienia
    let items = sqlx::query_as::<_, OrderItem>(
        r#"
            SELECT id, order_id, product_id, price_at_purchase
            FROM order_items
            WHERE order_id = $1
        "#,
    )
    .bind(order_id)
    .fetch_all(&app_state.db_pool)
    .await?;

    // Skonstruuj odpowiedź
    let response = OrderDetailsResponse { order, items };

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
    Json(payload): Json<UpdateOrderStatusPayload>,
) -> Result<Json<Order>, AppError> {
    let user_id = claims.sub;
    let user_role = claims.role;

    if user_role != Role::Admin {
        tracing::warn!(
            "Nieautoryzowana prośba zmiany statusu zamówienia: order_id={}, user_id={}, user_role={:?}",
            order_id,
            user_id,
            user_role
        );
        return Err(AppError::UnauthorizedAccess(
            "Tylko administrator może zmieniać status zamówienia".to_string(),
        ));
    }

    // Aktualizacja statusu w bazie danych
    let updated_order = sqlx::query_as::<_, Order>(r#"
            UPDATE orders
            SET status = $1, updated_at = CURRENT_TIMESTAMP
            WHERE id = $2
            RETURNING id, user_id, order_date, status, total_price, shipping_address_line1, shipping_address_line2, shipping_city, shipping_postal_code, shipping_country, created_at, updated_at
        "#).bind(&payload.status)
        .bind(order_id)
        .fetch_optional(&app_state.db_pool)
        .await?;

    // Sprawdzenie czy zamówienie zostało znalezione i zaktualizowane
    match updated_order {
        Some(order) => {
            tracing::info!(
                "Zaktualizowano status zamówienia: order_id={}, nowy_status={:?}, admin_id={}",
                order_id,
                payload.status,
                user_id
            );
            Ok(Json(order))
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

    // Rozpocznij trasnakcje
    let mut tx = app_state.db_pool.begin().await.map_err(|e| {
        tracing::error!("Nie można rozpocząć transakcji (koszyk): {}", e);
        AppError::InternalServerError("Błąd serwera przy dodawaniu do koszyka".to_string())
    })?;

    // Znajdź lub utwórz koszyk dla użytkownika
    let cart = match sqlx::query_as::<_, ShoppingCart>(
        r#"
            SELECT *
            FROM shopping_carts
            WHERE user_id = $1
            FOR UPDATE
        "#,
    )
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await?
    {
        Some(existing_product) => existing_product,
        None => {
            sqlx::query_as::<_, ShoppingCart>(
                r#"
                    INSERT INTO shopping_carts (user_id)
                    VALUES ($1)
                    RETURNING *
                "#,
            )
            .bind(user_id)
            .fetch_one(&mut *tx)
            .await?
        }
    };

    // Sprawdź czy produkt istnieje i jest dostępny (z blokadą FOR UPDATE)
    let product_to_add = sqlx::query_as::<_, Product>(
        r#"
            SELECT *
            FROM products
            WHERE id = $1
            FOR UPDATE
        "#,
    )
    .bind(payload.product_id)
    .fetch_optional(&mut *tx)
    .await?;

    match product_to_add {
        Some(product) => {
            if product.status != ProductStatus::Available {
                tracing::warn!(
                    "Użytkownik {} próbował dodać niedostępny produkt {} do koszyka {}",
                    user_id,
                    payload.product_id,
                    cart.id
                );
                return Err(AppError::NotFound);
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
            .bind(payload.product_id)
            .execute(&mut *tx)
            .await?;
            tracing::info!(
                "Product {} został dodany do koszyka {} dla użytkownika {}",
                payload.product_id,
                cart.id,
                user_id
            );
        }
        None => {
            tracing::warn!(
                "Użytkownik {}, próbował dodać nieistniejący produkt {} do koszyka {}",
                user_id,
                payload.product_id,
                cart.id
            );
            return Err(AppError::NotFound);
        }
    }

    // Pobierz zaaktualizowaną zawartość koszyka do zwrócenia (już w ramach transkacji)
    let items_db = sqlx::query_as::<_, CartItem>(
        r#"
            SELECT *
            FROM cart_items
            WHERE cart_id = $1
            ORDER BY added_at DESC
        "#,
    )
    .bind(cart.id)
    .fetch_all(&mut *tx)
    .await?;

    let mut cart_items_public: Vec<CartItemPublic> = Vec::new();
    let mut current_total_price: i64 = 0;

    for item_db in items_db {
        let product = sqlx::query_as::<_, Product>(
            r#"
                SELECT *
                FROM products
                WHERE id = $1
            "#,
        )
        .bind(item_db.product_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| {
            tracing::error!(
                "Błąd pobierania produktu dla pozycji koszyka: {:?}, produkt_id: {:?}",
                e,
                item_db.product_id
            );
            AppError::InternalServerError("Błąd przy konstruowaniu odpowiedzi koszyka".to_string())
        })?;

        current_total_price += product.price;
        cart_items_public.push(CartItemPublic {
            cart_item_id: item_db.id,
            product,
            added_at: item_db.added_at,
        });
    }

    // Zatwierdź tranksację
    tx.commit().await.map_err(|e| {
        tracing::error!("Nie można zatwierdzić transakcji (koszyk): {}", e);
        AppError::InternalServerError("Błąd serwera przy zapisywaniu koszyka.".to_string())
    })?;

    let mut conn_after_commit = app_state.db_pool.acquire().await.map_err(|e| {
        tracing::error!("Nie można uzyskać połączenia z puli po transakcji: {}", e);
        AppError::InternalServerError("Błąd serwera".to_string())
    })?;

    let final_cart = sqlx::query_as::<_, ShoppingCart>(
        r#"
            SELECT *
            FROM shopping_carts
            WHERE user_id = $1
        "#,
    )
    .bind(user_id)
    .fetch_one(&mut *conn_after_commit)
    .await?;

    let response_cart_details =
        build_cart_details_response(&final_cart, &mut conn_after_commit).await?;

    Ok((StatusCode::OK, Json(response_cart_details)))
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
