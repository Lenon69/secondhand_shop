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

// src/cart_utils.rs
pub async fn build_cart_details_response(
    cart: &ShoppingCart,
    conn: &mut PgConnection,
) -> Result<CartDetailsResponse, AppError> {
    let items_with_products = sqlx::query_as::<_, CartItemWithProduct>(
        r#"
            SELECT
                ci.id AS cart_item_id,
                ci.cart_id,      -- Dodane dla kompletności CartItemWithProduct
                ci.added_at,
                p.id AS product_id, -- Alias dla id produktu, aby uniknąć konfliktu z ci.id
                p.name,
                p.description,
                p.price,
                p.gender,
                p.condition,
                p.category,
                p.status,
                p.on_sale,
                p.images
            FROM cart_items ci
            JOIN products p ON ci.product_id = p.id
            WHERE ci.cart_id = $1
            ORDER BY ci.added_at ASC
        "#,
    )
    .bind(cart.id)
    .fetch_all(&mut *conn) // Używamy &mut *conn zamiast conn bezpośrednio
    .await?;

    let mut cart_items_public: Vec<CartItemPublic> = Vec::with_capacity(items_with_products.len());
    let mut current_total_price: i64 = 0;

    for row in items_with_products {
        if row.status != ProductStatus::Available {
            tracing::warn!(
                "Produkt '{}' (ID: {}) w koszyku (ID koszyka: {}) ma status inny niż 'Available': {:?}. Pomijam.",
                row.name,
                row.product_id,
                cart.id,
                row.status
            );
            // === KLUCZOWA ZMIANA: AKTYWNE USUWANIE "PRODUKTU-WIDMO" ===
            // Zamiast tylko pomijać, wykonujemy zapytanie DELETE.
            sqlx::query("DELETE FROM cart_items WHERE id = $1")
                .bind(row.cart_item_id)
                .execute(&mut *conn)
                .await
                .map_err(|e| {
                    tracing::error!(
                        "Nie udało się usunąć niedostępnej pozycji ({}) z koszyka ({}): {}",
                        row.cart_item_id,
                        cart.id,
                        e
                    );
                    // Mimo błędu, kontynuujemy, aby nie zablokować całego widoku koszyka
                    e
                })?; // Używamy ? aby propagować błąd, jeśli usunięcie się nie powiedzie.

            continue; // Kontynuujemy pętlę, nie dodając tego produktu do widoku.
        }

        current_total_price += row.price;
        cart_items_public.push(CartItemPublic {
            cart_item_id: row.cart_item_id,
            product: Product {
                // Ręczne tworzenie obiektu Product z danych w CartItemWithProduct
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
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
            added_at: row.added_at,
        });
    }

    // Aktualizacja timestampu koszyka
    // To zapytanie jest wykonywane nawet jeśli koszyk jest pusty (po odfiltrowaniu niedostępnych produktów),
    // aby odzwierciedlić fakt "przeglądania" koszyka.
    let updated_cart_timestamp = sqlx::query_scalar::<_, DateTime<Utc>>(
        r#"
            UPDATE shopping_carts
            SET updated_at = CURRENT_TIMESTAMP
            WHERE id = $1
            RETURNING updated_at
        "#,
    )
    .bind(cart.id)
    .fetch_one(conn) // conn jest &mut PgConnection
    .await
    .unwrap_or_else(|err| {
        tracing::error!("Nie udało się zaktualizować updated_at dla koszyka {}: {}. Używam istniejącej wartości.", cart.id, err);
        cart.updated_at // Fallback do wartości z obiektu 'cart'
    });

    Ok(CartDetailsResponse {
        cart_id: cart.id,
        user_id: cart.user_id,
        total_items: cart_items_public.len(),
        items: cart_items_public,
        total_price: current_total_price,
        updated_at: updated_cart_timestamp,
    })
}

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
