// src/handlers.rs
use axum::Json;
use axum::response::IntoResponse;
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
use crate::filters::ListingParams;
use crate::models::Product;
use crate::models::*;
use crate::pagination::PaginatedProductsResponse;
use crate::{
    auth::{create_jwt, hash_password, verify_password},
    cloudinary::upload_image_to_cloudinary,
    state::AppState,
};
use crate::{
    auth_models::{LoginPayload, RegistrationPayload, TokenClaims},
    models::{CreateOrderFromCartPayload, Order, OrderStatus, ProductGender, ProductStatus},
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
        // To jest OK, jeśli Product ma FromRow
        r#"SELECT id, name, description, price, gender, condition, category, status, images
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
            SELECT id, name, description, price, gender, condition, category, status, images
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

    let new_product_db = sqlx::query_as::<_, Product>(
        r#"
            INSERT INTO products (id, name, description, price, gender, condition, category, status, images)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING id, name, description, price, gender, condition , category, status , images
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

    if let Some(gender) = text_fields.get("gender") {
        existing_product.gender = ProductGender::from_str(&gender).map_err(|_| {
            AppError::UnprocessableEntity("Nieprawidłowa sub-kategoria produktu".to_string())
        })?;
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
            SET name = $1, description = $2, price = $3, gender = $4, condition = $5, category = $6, status = $7, images = $8
            WHERE id = $9
            RETURNING *
        "#).bind(&existing_product.name)
        .bind(&existing_product.description)
        .bind(&existing_product.price)
        .bind(&existing_product.gender)
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

    // Pobierz produkt, aby uzyskać listę obrazów
    let product_to_delete = sqlx::query_as::<_, Product>(
        r#"
            SELECT id, name, description, price, gender, condition, category, status, images
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
            if query_result.rows_affected() == 0 {
                // To nie powinno się zdarzyć, jeśli produkt był pobrany powyżej,
                // ale dla pewności zostawiamy.
                tracing::warn!(
                    "DELETE: Nie znaleziono produktu do usunięcia o ID {} (mimo wcześniejszego pobrania)",
                    product_id
                );
                Err(AppError::NotFound)
            } else {
                tracing::info!("Usunięto produkt o ID: {} z bazy danych", product_id);
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
    Json(payload): Json<CreateOrderFromCartPayload>, // <-- ZMIENIONY PAYLOAD
) -> Result<(StatusCode, Json<OrderDetailsResponse>), AppError> {
    // Zwracamy OrderDetailsResponse dla spójności
    payload.validate()?; // Walidacja danych adresowych

    let user_id = claims.sub;
    tracing::info!(
        "Użytkownik {} próbuje złożyć zamówienie na podstawie koszyka.",
        user_id
    );

    // Rozpocznij transakcję
    let mut tx = app_state.db_pool.begin().await.map_err(|e| {
        tracing::error!(
            "Nie można rozpocząć transakcji (tworzenie zamówienia): {}",
            e
        );
        AppError::InternalServerError("Błąd serwera podczas tworzenia zamówienia.".to_string())
    })?;

    // 1. Znajdź koszyk użytkownika i jego pozycje (z blokadą)
    let cart = match sqlx::query_as::<_, ShoppingCart>(
        "SELECT * FROM shopping_carts WHERE user_id = $1 FOR UPDATE",
    )
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await?
    {
        Some(c) => c,
        None => {
            tracing::warn!(
                "Użytkownik {} próbował złożyć zamówienie, ale nie ma koszyka.",
                user_id
            );
            return Err(AppError::UnprocessableEntity(
                "Twój koszyk nie został znaleziony lub jest pusty.".to_string(),
            ));
        }
    };

    let cart_items_db = sqlx::query_as::<_, CartItem>(
        "SELECT * FROM cart_items WHERE cart_id = $1 FOR UPDATE", // Blokujemy pozycje koszyka
    )
    .bind(cart.id)
    .fetch_all(&mut *tx)
    .await?;

    if cart_items_db.is_empty() {
        tracing::warn!(
            "Użytkownik {} próbował złożyć zamówienie z pustym koszykiem (cart_id: {}).",
            user_id,
            cart.id
        );
        return Err(AppError::UnprocessableEntity(
            "Twój koszyk jest pusty.".to_string(),
        ));
    }

    // 2. Przetwórz pozycje z koszyka: sprawdź produkty, zbierz dane do OrderItem
    let mut order_items_to_create: Vec<(Uuid, i64)> = Vec::with_capacity(cart_items_db.len()); // (product_id, price_at_purchase)
    let mut total_price: i64 = 0;
    let mut product_ids_to_mark_sold: Vec<Uuid> = Vec::new();

    for cart_item in &cart_items_db {
        // Iterujemy po referencjach
        let product = sqlx::query_as::<_, Product>(
            "SELECT * FROM products WHERE id = $1 FOR UPDATE", // Blokujemy produkt
        )
        .bind(cart_item.product_id)
        .fetch_optional(&mut *tx) // Produkt mógł zostać usunięty przez admina w międzyczasie
        .await?;

        match product {
            Some(p) => {
                if p.status != ProductStatus::Available {
                    tracing::warn!(
                        "Produkt {} (ID: {}) w koszyku użytkownika {} jest już niedostępny (status: {:?}).",
                        p.name,
                        p.id,
                        user_id,
                        p.status
                    );
                    // Można rozważyć usunięcie tej pozycji z koszyka i poinformowanie użytkownika,
                    // lub po prostu przerwanie tworzenia zamówienia.
                    return Err(AppError::UnprocessableEntity(format!(
                        "Produkt '{}' w Twoim koszyku stał się niedostępny. Usuń go z koszyka i spróbuj ponownie.",
                        p.name
                    )));
                }
                order_items_to_create.push((p.id, p.price)); // Zapisujemy aktualną cenę jako price_at_purchase
                total_price += p.price;
                product_ids_to_mark_sold.push(p.id);
            }
            None => {
                // Produkt został usunięty z bazy, podczas gdy był w koszyku.
                // Usuń tę pozycję z koszyka (dla czystości) i poinformuj użytkownika.
                tracing::error!(
                    "Produkt o ID {} (z koszyka użytkownika {}) nie został znaleziony w bazie. Usuwam z koszyka.",
                    cart_item.product_id,
                    user_id
                );
                sqlx::query("DELETE FROM cart_items WHERE id = $1")
                    .bind(cart_item.id)
                    .execute(&mut *tx)
                    .await?; // Ignorujemy błąd, jeśli usunięcie się nie powiedzie, główny problem to brak produktu
                return Err(AppError::UnprocessableEntity(
                    "Jeden z produktów w Twoim koszyku został usunięty ze sklepu. Odśwież koszyk i spróbuj ponownie.".to_string()
                ));
            }
        }
    }

    // 3. Wstaw rekord do tabeli `orders`
    let initial_status = OrderStatus::Pending; // Początkowy status
    let order = sqlx::query_as::<_, Order>(
        // Zwracamy pełny obiekt Order
        r#"
            INSERT INTO orders (user_id, status, total_price,
                                shipping_address_line1, shipping_address_line2,
                                shipping_city, shipping_postal_code, shipping_country)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING id, user_id, order_date, status, total_price,
                      shipping_address_line1, shipping_address_line2,
                      shipping_city, shipping_postal_code, shipping_country,
                      created_at, updated_at
        "#,
    )
    .bind(user_id)
    .bind(initial_status)
    .bind(total_price)
    .bind(&payload.shipping_address_line1)
    .bind(payload.shipping_address_line2.as_deref())
    .bind(&payload.shipping_city)
    .bind(&payload.shipping_postal_code)
    .bind(&payload.shipping_country)
    .fetch_one(&mut *tx)
    .await?;

    // 4. Wstaw rekordy do tabeli `order_items`
    let mut created_order_items_db: Vec<OrderItem> =
        Vec::with_capacity(order_items_to_create.len());
    for (product_id, price_at_purchase) in order_items_to_create {
        let oi = sqlx::query_as::<_, OrderItem>(
            r#"
                INSERT INTO order_items (order_id, product_id, price_at_purchase)
                VALUES ($1, $2, $3)
                RETURNING id, order_id, product_id, price_at_purchase 
            "#, // W strukturze OrderItem nie ma added_at, jeśli tak zdefiniowałeś
        )
        .bind(order.id)
        .bind(product_id)
        .bind(price_at_purchase)
        .fetch_one(&mut *tx)
        .await?;
        created_order_items_db.push(oi);
    }

    // 5. Wyczyść koszyk użytkownika (usuń wszystkie pozycje z cart_items dla tego cart_id)
    let deleted_cart_items = sqlx::query("DELETE FROM cart_items WHERE cart_id = $1")
        .bind(cart.id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
    tracing::info!(
        "Wyczyszczono {} pozycji z koszyka {} dla użytkownika {}",
        deleted_cart_items,
        cart.id,
        user_id
    );
    // Można też zaktualizować `updated_at` w `shopping_carts` lub poczekać na trigger, jeśli jest

    // 6. Zaktualizuj status zamówionych produktów na 'Sold'
    if !product_ids_to_mark_sold.is_empty() {
        sqlx::query(
            r#"
                UPDATE products
                SET status = $1
                WHERE id = ANY($2)
            "#,
        )
        .bind(ProductStatus::Sold)
        .bind(&product_ids_to_mark_sold)
        .execute(&mut *tx)
        .await?;
    }

    // 7. Zatwierdź transakcję
    tx.commit().await.map_err(|e| {
        tracing::error!(
            "Nie można zatwierdzić transakcji (tworzenie zamówienia): {}",
            e
        );
        AppError::InternalServerError("Błąd serwera podczas finalizowania zamówienia.".to_string())
    })?;

    tracing::info!(
        "Utworzono nowe zamówienie ID: {} na podstawie koszyka dla użytkownika {}",
        order.id,
        user_id
    );

    // 8. Przygotuj odpowiedź OrderDetailsResponse
    let mut order_items_details_public: Vec<OrderItemDetailsPublic> =
        Vec::with_capacity(created_order_items_db.len());
    for item_db in created_order_items_db {
        let product = sqlx::query_as::<_, Product>(
            // Potrzebujemy produktu do CartItemPublic
            "SELECT * FROM products WHERE id = $1",
        )
        .bind(item_db.product_id)
        .fetch_one(&app_state.db_pool) // Poza transakcją, bo już zatwierdzona
        .await?;

        order_items_details_public.push(OrderItemDetailsPublic {
            order_item_id: item_db.id,
            product,
            price_at_purchase: item_db.price_at_purchase,
        });
    }

    let response = OrderDetailsResponse {
        order, // To jest obiekt Order zwrócony z INSERT INTO orders
        items: order_items_details_public,
    };

    Ok((StatusCode::CREATED, Json(response)))
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

    // 1. Pobieranie zamówienia
    let order_optional = sqlx::query_as::<_, Order>(
        r#"
            SELECT id, user_id, order_date, status, total_price,
                   shipping_address_line1, shipping_address_line2,
                   shipping_city, shipping_postal_code, shipping_country,
                   created_at, updated_at
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
                SELECT id, name, description, price, gender, condition, category, status, images
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
            SELECT id, name, description, price, condition, category, status, images
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
                SELECT id, name, description, price, gender, condition, category, status, images
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
