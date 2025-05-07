// src/handlers.rs
use axum::Json;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
};
use serde_json::{Value, json};
use sqlx::{Postgres, QueryBuilder};

use crate::errors::AppError;
use crate::filters::ListingParams;
use crate::models::{CreateProductPayload, Product, UpdateProductPayload, User, UserPublic};
use crate::pagination::PaginatedProductsResponse;
use crate::{
    auth::{create_jwt, hash_password, verify_password},
    state::AppState,
};
use crate::{
    auth_models::{LoginPayload, RegistrationPayload, Role, TokenClaims},
    models::ProductStatus,
};
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
        // Poprawiony brak spacji
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
        // Poprawiona literówka
        data_builder.push("condition = ").push_bind(condition);
    }
    append_where_or_and_data(&mut data_builder);
    // Przekazanie przez referencję
    data_builder.push("status = ").push_bind(&status_to_filter);

    if let Some(price_min) = params.price_min() {
        append_where_or_and_data(&mut data_builder);
        data_builder.push("price >= ").push_bind(price_min);
    }
    if let Some(price_max) = params.price_max() {
        append_where_or_and_data(&mut data_builder);
        // Poprawiony brak spacji
        data_builder.push("price <= ").push_bind(price_max);
    }

    // Reszta bez zmian (sortowanie, limit, offset, wykonanie, metadane, odpowiedź)
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
    Json(payload): Json<CreateProductPayload>,
) -> Result<(StatusCode, Json<Product>), AppError> {
    payload.validate()?;

    if claims.role != Role::Admin {
        return Err(AppError::UnauthorizedAccess(
            "Tylko administrator może dodawać produkty".to_string(),
        ));
    }

    tracing::info!("Obsłużono zapytanie POST /api/products - tworzenie produktu");

    let new_id = Uuid::new_v4();
    let default_status = ProductStatus::Available;

    let created_product = sqlx::query_as::<_, Product>(
        r#"
            INSERT INTO products (id, name, description, price, condition, category, status, images)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING id, name, description, price, condition , category, status , images
        "#,
    )
    .bind(new_id)
    .bind(&payload.name)
    .bind(&payload.description)
    .bind(&payload.price)
    .bind(&payload.condition)
    .bind(&payload.category)
    .bind(default_status)
    .bind(&payload.images)
    .fetch_one(&app_state.db_pool)
    .await?;

    tracing::info!("Utworzono produkt o ID: {}", new_id);

    Ok((StatusCode::CREATED, Json(created_product)))
}

pub async fn update_product_partial_handler(
    State(app_state): State<AppState>,
    Path(product_id): Path<Uuid>,
    claims: TokenClaims,
    Json(payload): Json<UpdateProductPayload>,
) -> Result<Json<Product>, AppError> {
    payload.validate()?;

    if claims.role != Role::Admin {
        return Err(AppError::UnauthorizedAccess(
            "Tylko administrator może aktualizować produkty".to_string(),
        ));
    }

    tracing::info!(
        "Obsłużono zapytanie PATCH /api/products/{} - aktualizacja: {:?}",
        product_id,
        payload
    );

    let mut existing_product = sqlx::query_as::<_, Product>(
        r#"
            SELECT id, name, description, price, condition, category, status, images
            FROM products
            WHERE id = $1"#,
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

    if let Some(name) = payload.name {
        existing_product.name = name;
    }
    if let Some(description) = payload.description {
        existing_product.description = description;
    }
    if let Some(price) = payload.price {
        existing_product.price = price;
    }
    if let Some(condition) = payload.condition {
        existing_product.condition = condition;
    }
    if let Some(category) = payload.category {
        existing_product.category = category;
    }
    if let Some(status) = payload.status {
        existing_product.status = status;
    }
    if let Some(images) = payload.images {
        existing_product.images = images;
    }

    let updated_product = sqlx::query_as::<_, Product>(r#"
            UPDATE products
            SET name = $1, description = $2, price = $3, condition = $4, category = $5, status = $6, images = $7
            WHERE id = $8
            RETURNING id, name, description, price, condition, category, status, images
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
            //Sprawdź ile wierszy zostało usuniętych
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

    //Sprawdź czy użytkownik istnieje
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

    // Hashuj hasło
    let password_hash = hash_password(&payload.password)?;

    // Wstaw nowego użytkownika (domyślnie rola Customer)
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

    //Zwróć dane publiczne użytkownika
    Ok((StatusCode::CREATED, Json(new_user.into())))
}

pub async fn login_handler(
    State(app_state): State<AppState>,
    Json(payload): Json<LoginPayload>,
) -> Result<Json<serde_json::Value>, AppError> {
    payload.validate()?;

    // Znajdź użytkownika po emailu
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

    // Zweryfikuj hasło
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
