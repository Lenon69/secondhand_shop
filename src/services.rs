// src/services.rs

use crate::errors::AppError;
use crate::models::{Category, ProductGender, ProductStatus};
use crate::state::AppState;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, sqlx::FromRow)] // <<< DODAJ `sqlx::FromRow`
pub struct CategoryWithCount {
    // Nazwa pola musi pasować do nazwy zwracanej przez SQL, czyli `category`
    pub category: Category,
    // Nazwa pola musi pasować do aliasu w SQL, czyli `product_count`
    #[sqlx(rename = "product_count")]
    pub count: i64,
}

/// (Zoptymalizowana) Pobiera wszystkie kategorie wraz z liczbą dostępnych produktów dla danej płci.
/// Wynik działania tej funkcji jest cachowany.
pub async fn get_categories_with_counts(
    app_state: &AppState,
    gender: ProductGender,
) -> Result<Vec<CategoryWithCount>, AppError> {
    let cache_key = format!("category_counts_{}", gender.as_ref());

    if let Some(cached_data) = app_state.dynamic_html_cache.get(&cache_key).await {
        if let Ok(data) = serde_json::from_str::<Vec<CategoryWithCount>>(&cached_data) {
            tracing::info!(
                "Cache HIT: Zwracam listę kategorii z licznikiem dla: {:?}",
                gender
            );
            return Ok(data);
        }
    }

    tracing::info!(
        "Cache MISS: Pobieram listę kategorii z licznikiem dla: {:?}",
        gender
    );

    // --- NOWE, JEDNO ZAPYTANIE Z GROUP BY ---
    // Pobieramy wszystkie kategorie i ich liczności za jednym razem.
    let categories_with_counts = sqlx::query_as::<_, CategoryWithCount>(
        r#"
        SELECT category, COUNT(*) as product_count
        FROM products
        WHERE gender = $1 AND status = $2
        GROUP BY category
        HAVING COUNT(*) > 0
        ORDER BY category ASC
        "#,
    )
    .bind(gender)
    .bind(ProductStatus::Available)
    .fetch_all(&app_state.db_pool)
    .await?;

    // Zapisujemy świeżo pobrane dane do cache'u
    if let Ok(json_data) = serde_json::to_string(&categories_with_counts) {
        app_state
            .dynamic_html_cache
            .insert(cache_key, json_data)
            .await;
    }

    Ok(categories_with_counts)
}
