// src/cart_utils.rs

use chrono::{DateTime, Utc};
use sqlx::PgConnection;

use crate::{
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
        // Sprawdź, czy produkt nie został np. usunięty lub jego status się nie zmienił
        // Chociaż JOIN powinien zwrócić tylko istniejące produkty, dodatkowa walidacja statusu może być przydatna
        if row.status != ProductStatus::Available {
            tracing::warn!(
                "Produkt '{}' (ID: {}) w koszyku (ID koszyka: {}) ma status inny niż 'Available': {:?}. Pomijam.",
                row.name,
                row.product_id,
                cart.id,
                row.status
            );
            // Można rozważyć usunięcie tej pozycji z koszyka w tym miejscu, jeśli to pożądane zachowanie
            // np. sqlx::query("DELETE FROM cart_items WHERE id = $1").bind(row.cart_item_id).execute(&mut *conn).await?;
            continue;
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
