// src/services.rs

use crate::errors::AppError;
use crate::models::{Category, ProductGender, ProductStatus};
use crate::state::AppState;

/// Pobiera listę unikalnych, dostępnych kategorii dla danej płci.
///
/// Funkcja jest zoptymalizowana pod kątem wydajności:
/// 1. Najpierw sprawdza cache, aby uniknąć zbędnych zapytań do bazy danych.
/// 2. Jeśli danych nie ma w cache'u, wykonuje szybkie zapytanie `SELECT DISTINCT`.
/// 3. Wynik zapytania jest zapisywany w cache'u na przyszłe żądania.
pub async fn get_available_categories_for_gender(
    app_state: &AppState,
    gender: ProductGender,
) -> Result<Vec<Category>, AppError> {
    // Krok 1: Sprawdzenie cache'u
    if let Some(cached_categories) = app_state.category_list_cache.get(&gender).await {
        tracing::info!("Cache HIT dla listy kategorii dla płci: {:?}", gender);
        return Ok(cached_categories);
    }

    // Krok 2: Pobranie danych z bazy w przypadku "cache miss"
    tracing::info!(
        "Cache MISS dla listy kategorii dla płci: {:?}. Pobieranie z bazy.",
        gender
    );

    let available_categories = sqlx::query_scalar::<_, Category>(
        r#"
        SELECT DISTINCT category
        FROM products
        WHERE gender = $1 AND status = $2
        ORDER BY category ASC
        "#,
    )
    .bind(gender)
    .bind(ProductStatus::Available)
    .fetch_all(&app_state.db_pool)
    .await?;

    // Krok 3: Zapisanie wyniku w cache'u
    app_state
        .category_list_cache
        .insert(gender, available_categories.clone())
        .await;

    // Krok 4: Zwrócenie wyniku
    Ok(available_categories)
}
