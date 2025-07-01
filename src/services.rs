// src/services.rs

use crate::errors::AppError;
use crate::models::{Category, ProductGender, ProductStatus};
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use strum::IntoEnumIterator;

// Struktura, która będzie przechowywać kategorię razem z liczbą produktów.
// Musi mieć `Serialize` i `Deserialize`, aby można ją było zapisać w cache jako JSON.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CategoryWithCount {
    pub category: Category,
    pub count: i64,
}

/// Pobiera wszystkie kategorie wraz z liczbą dostępnych produktów dla danej płci.
/// Wynik działania tej funkcji jest cachowany.
pub async fn get_categories_with_counts(
    app_state: &AppState,
    gender: ProductGender,
) -> Result<Vec<CategoryWithCount>, AppError> {
    // Tworzymy unikalny klucz do cache'u dla każdej płci.
    let cache_key = format!("category_counts_{}", gender.as_ref());

    // 1. Sprawdzamy, czy dane istnieją w cache'u.
    if let Some(cached_data) = app_state.dynamic_html_cache.get(&cache_key).await {
        // Próbujemy zdeserializować dane z cache'u.
        if let Ok(data) = serde_json::from_str::<Vec<CategoryWithCount>>(&cached_data) {
            tracing::info!(
                "Cache HIT: Zwracam listę kategorii z licznikiem dla: {:?}",
                gender
            );
            return Ok(data);
        }
    }

    // 2. Jeśli nie ma w cache'u, pobieramy dane z bazy.
    tracing::info!(
        "Cache MISS: Pobieram listę kategorii z licznikiem dla: {:?}",
        gender
    );
    let mut categories_with_counts = Vec::new();

    // Iterujemy po wszystkich wariantach enuma `Category`.
    for category in Category::iter() {
        let count_result: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM products WHERE gender = $1 AND category = $2 AND status = $3",
        )
        .bind(gender)
        .bind(category)
        .bind(ProductStatus::Available) // Liczymy tylko dostępne produkty
        .fetch_one(&app_state.db_pool)
        .await?;

        // Dodajemy do wektora tylko kategorie, które mają co najmniej jeden produkt.
        if count_result.0 > 0 {
            categories_with_counts.push(CategoryWithCount {
                category,
                count: count_result.0,
            });
        }
    }

    // 3. Zapisujemy świeżo pobrane dane do cache'u na przyszłość.
    if let Ok(json_data) = serde_json::to_string(&categories_with_counts) {
        app_state
            .dynamic_html_cache
            .insert(cache_key, json_data)
            .await;
    }

    Ok(categories_with_counts)
}
