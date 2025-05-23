// src/cart_utils.rs

use chrono::{DateTime, Utc};
use sqlx::PgConnection;

use crate::{
    errors::AppError,
    models::{CartDetailsResponse, CartItem, CartItemPublic, Product, ShoppingCart},
};

pub async fn build_cart_details_response(
    cart: &ShoppingCart,
    conn: &mut PgConnection,
) -> Result<CartDetailsResponse, AppError> {
    let items_db = sqlx::query_as::<_, CartItem>(
        r#"
            SELECT *
            FROM cart_items
            WHERE cart_id = $1
            ORDER BY added_at ASC
            "#,
    )
    .bind(cart.id)
    .fetch_all(&mut *conn)
    .await?;

    let mut cart_items_public: Vec<CartItemPublic> = Vec::with_capacity(items_db.len());
    let mut current_total_price: i64 = 0;

    for item_db in items_db {
        let product = sqlx::query_as::<_, Product>("SELECT * FROM products WHERE id = $1")
            .bind(item_db.product_id)
            .fetch_one(&mut *conn) // Ponownie używamy &mut *conn dla PgPool lub &mut Transaction
            .await
            .map_err(|e| {
                tracing::error!(
                    "Błąd pobierania produktu (ID: {}) dla pozycji koszyka (ID: {}): {:?}",
                    item_db.product_id,
                    item_db.id,
                    e
                );
                // Jeśli produkt nie został znaleziony, można go po cichu usunąć z koszyka
                // lub zwrócić błąd. Na razie zwracamy błąd.
                AppError::InternalServerError(
                    "Błąd przy konstruowaniu szczegółów koszyka (produkt nie znaleziony)."
                        .to_string(),
                )
            })?;

        current_total_price += product.price;
        cart_items_public.push(CartItemPublic {
            cart_item_id: item_db.id,
            product,
            added_at: item_db.added_at,
        });
    }

    let updated_cart_timestamp = sqlx::query_scalar::<_, DateTime<Utc>>(
        r#"
            UPDATE shopping_carts
            SET updated_at = CURRENT_TIMESTAMP
            WHERE id = $1
            RETURNING updated_at
        "#,
    )
    .bind(cart.id)
    .fetch_one(conn)
    .await
    .unwrap_or(cart.updated_at);

    Ok(CartDetailsResponse {
        cart_id: cart.id,
        user_id: cart.user_id,
        total_items: cart_items_public.len(),
        items: cart_items_public,
        total_price: current_total_price,
        updated_at: updated_cart_timestamp,
    })
}
