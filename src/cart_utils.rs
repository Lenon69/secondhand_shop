// src/cart_utils.rs

use chrono::{DateTime, Utc};
use sqlx::PgConnection;
use uuid::Uuid;

use crate::{
    auth::TokenClaims,
    errors::AppError,
    models::{
        CartDetailsResponse, CartItemPublic, CartItemWithProduct, Product, ProductStatus,
        ShoppingCart,
    },
};

// NOWA, DOCELOWA WERSJA FUNKCJI POMOCNICZEJ
/// Pobiera szczegóły koszyka na podstawie opcjonalnych danych użytkownika lub gościa.
/// Ta wersja współpracuje z nowymi, uniwersalnymi ekstraktorami.
pub async fn get_cart_details(
    conn: &mut PgConnection,
    user_claims_opt: Option<TokenClaims>,
    guest_cart_id_opt: Option<Uuid>,
) -> Result<Option<CartDetailsResponse>, AppError> {
    // Logika do znalezienia właściwego koszyka
    let cart_opt = if let Some(claims) = user_claims_opt {
        // Scenariusz 1: Użytkownik jest zalogowany. Szukamy koszyka po jego ID.
        tracing::debug!(
            "get_cart_details_v2: Szukanie koszyka dla zalogowanego użytkownika ID: {}",
            claims.sub
        );
        sqlx::query_as::<_, ShoppingCart>("SELECT * FROM shopping_carts WHERE user_id = $1")
            .bind(claims.sub)
            .fetch_optional(&mut *conn)
            .await?
    } else if let Some(guest_id) = guest_cart_id_opt {
        // Scenariusz 2: Użytkownik jest gościem. Szukamy koszyka po jego ID sesji.
        tracing::debug!(
            "get_cart_details_v2: Szukanie koszyka dla gościa o ID sesji: {}",
            guest_id
        );
        sqlx::query_as::<_, ShoppingCart>(
            "SELECT * FROM shopping_carts WHERE guest_session_id = $1",
        )
        .bind(guest_id)
        .fetch_optional(&mut *conn)
        .await?
    } else {
        // Scenariusz 3: Nie ma żadnej tożsamości (ani tokenu, ani ciasteczka). Użytkownik nie ma koszyka.
        tracing::debug!(
            "get_cart_details_v2: Brak tożsamości użytkownika lub gościa. Brak koszyka."
        );
        None
    };

    // Jeśli znaleźliśmy koszyk (w scenariuszu 1 lub 2), budujemy jego szczegóły.
    if let Some(cart) = cart_opt {
        tracing::debug!(
            "get_cart_details_v2: Znaleziono koszyk ID: {}. Budowanie szczegółów.",
            cart.id
        );
        // Używamy istniejącej funkcji, która robi całą "ciężką pracę"
        let details = build_cart_details_response(&cart, conn).await?;
        Ok(Some(details))
    } else {
        // Jeśli nie znaleziono koszyka, zwracamy Ok(None), co oznacza "brak koszyka".
        Ok(None)
    }
}

/// Buduje pełną odpowiedź ze szczegółami koszyka, weryfikując dostępność produktów.
pub async fn build_cart_details_response(
    cart: &ShoppingCart,
    conn: &mut PgConnection,
) -> Result<CartDetailsResponse, AppError> {
    // KROK 1: Zmodyfikowane zapytanie, które JAWNIE wybiera wszystkie pola.
    let items_with_products = sqlx::query_as::<_, CartItemWithProduct>(
        r#"
            SELECT
                ci.id AS cart_item_id,
                ci.cart_id,
                ci.product_id,
                ci.added_at,
                p.id,
                p.name,
                p.description,
                p.price,
                p.gender,
                p.condition,
                p.category,
                p.status,
                p.on_sale,
                p.images,
                p.created_at, 
                p.updated_at  
            FROM cart_items ci
            JOIN products p ON ci.product_id = p.id
            WHERE ci.cart_id = $1
            ORDER BY ci.added_at ASC
        "#,
    )
    .bind(cart.id)
    .fetch_all(&mut *conn)
    .await?;

    let mut cart_items_public: Vec<CartItemPublic> = Vec::with_capacity(items_with_products.len());
    let mut current_total_price: i64 = 0;

    for row in items_with_products {
        if row.status != ProductStatus::Available {
            tracing::warn!(
                "Produkt '{}' w koszyku ma status inny niż 'Available': {:?}. Usuwam.",
                row.name,
                row.status
            );
            sqlx::query("DELETE FROM cart_items WHERE id = $1")
                .bind(row.cart_item_id)
                .execute(&mut *conn)
                .await?;
            continue;
        }

        current_total_price += row.price;
        cart_items_public.push(CartItemPublic {
            cart_item_id: row.cart_item_id,
            product: Product {
                // Teraz wszystkie pola w `row` pasują do pól w `Product`
                id: row.product_id,
                name: row.name,
                description: row.description,
                price: row.price,
                gender: row.gender,
                condition: row.condition,
                category: row.category,
                status: row.status,
                images: row.images,
                on_sale: row.on_sale,
                created_at: row.created_at, // Teraz to pole istnieje
                updated_at: row.updated_at, // I to również
            },
            added_at: row.added_at,
        });
    }

    // Reszta funkcji pozostaje bez zmian (aktualizacja updated_at i zwrócenie odpowiedzi)
    let updated_cart_timestamp = sqlx::query_scalar::<_, DateTime<Utc>>(
        "UPDATE shopping_carts SET updated_at = CURRENT_TIMESTAMP WHERE id = $1 RETURNING updated_at",
    )
    .bind(cart.id)
    .fetch_one(conn)
    .await
    .unwrap_or_else(|_| cart.updated_at);

    Ok(CartDetailsResponse {
        cart_id: cart.id,
        user_id: cart.user_id,
        total_items: cart_items_public.len(),
        items: cart_items_public,
        total_price: current_total_price,
        updated_at: updated_cart_timestamp,
    })
}
