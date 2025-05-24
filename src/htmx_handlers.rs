// src/htmx_handlers.rs

#[allow(unused_imports)]
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    response::IntoResponse,
};
#[allow(unused_imports)]
use axum_extra::TypedHeader;
use maud::{Markup, PreEscaped, html};
use serde::Deserialize;
use serde_json;
use strum::IntoEnumIterator;
#[allow(unused_imports)]
use urlencoding::encode;
use uuid::Uuid;

use crate::models::{ProductGender, ProductStatus};
#[allow(unused_imports)]
use crate::{
    auth_models::TokenClaims,
    cart_utils,
    errors::AppError,
    filters::ListingParams,
    handlers::XGuestCartId,
    models::{CartDetailsResponse, Category, Product, ShoppingCart},
    pagination::PaginatedProductsResponse,
    state::AppState,
};

// Funkcje pomocnicze do budowania URL-i (przeniesione lub zaadaptowane z poprzedniej wersji)
// Te funkcje nadal będą bardzo przydatne do generowania linków w Maud.
// Pamiętaj, aby użyć urlencoding::encode dla parametrów URL, jeśli tego wymagają.
fn build_full_query_string_from_params(params: &ListingParams) -> String {
    let mut query_parts = Vec::new();
    query_parts.push(format!("limit={}", params.limit()));
    query_parts.push(format!("offset={}", params.offset()));
    if let Some(g) = params.gender() {
        query_parts.push(format!("gender={}", g.to_string()));
    }
    if let Some(c) = params.category() {
        query_parts.push(format!("category={}", c.to_string()));
    }
    if let Some(cond) = params.condition() {
        query_parts.push(format!("condition={}", cond.to_string()));
    }
    if let Some(stat) = params.status() {
        query_parts.push(format!("status={}", stat.to_string()));
    }
    if let Some(p_min) = params.price_min() {
        query_parts.push(format!("price_min={}", p_min));
    }
    if let Some(p_max) = params.price_max() {
        query_parts.push(format!("price_max={}", p_max));
    }
    if let Some(s) = params.search() {
        if !s.is_empty() {
            query_parts.push(format!("search={}", urlencoding::encode(&s)));
        }
    }
    query_parts.push(format!("sort_by={}", params.sort_by()));
    query_parts.push(format!("order={}", params.order()));
    query_parts.join("&")
}

fn build_filter_only_query_string(params: &ListingParams) -> String {
    let mut filter_parts = Vec::new();
    if let Some(g) = params.gender() {
        filter_parts.push(format!("gender={}", g.to_string()));
    }
    if let Some(c) = params.category() {
        filter_parts.push(format!("category={}", c.to_string()));
    }
    if let Some(cond) = params.condition() {
        filter_parts.push(format!("condition={}", cond.to_string()));
    }
    if let Some(stat) = params.status() {
        filter_parts.push(format!("status={}", stat.to_string()));
    }
    if let Some(p_min) = params.price_min() {
        filter_parts.push(format!("price_min={}", p_min));
    }
    if let Some(p_max) = params.price_max() {
        filter_parts.push(format!("price_max={}", p_max));
    }
    if let Some(s) = params.search() {
        if !s.is_empty() {
            filter_parts.push(format!("search={}", urlencoding::encode(&s)));
        }
    }
    filter_parts.push(format!("sort_by={}", params.sort_by()));
    filter_parts.push(format!("order={}", params.order()));

    if filter_parts.is_empty() {
        String::new()
    } else {
        format!("&{}", filter_parts.join("&")) // Zaczyna się od &
    }
}

// Handler generujący siatkę produktów za pomocą Maud
// pub async fn list_products_htmx_handler(
//     State(app_state): State<AppState>,
//     Query(params): Query<ListingParams>,
// ) -> Result<Markup, AppError> {
//     tracing::info!("MAUD: /htmx/products z parametrami: {:?}", params);

//     // 1. Pobierz dane produktów - ta logika pozostaje taka sama
//     //    Zakładamy, że crate::handlers::list_products zwraca Result<Json<PaginatedProductsResponse>, AppError>
//     let paginated_response_axum_json =
//         crate::handlers::list_products(State(app_state.clone()), Query(params.clone())).await?;
//     let paginated_response: PaginatedProductsResponse = paginated_response_axum_json.0; // Rozpakowujemy z Json

//     let products = paginated_response.data;
//     let current_page = paginated_response.current_page;
//     let total_pages = paginated_response.total_pages;
//     let per_page = paginated_response.per_page;

//     // 2. Wygeneruj stringi zapytań dla paginacji i linków "powrotu"
//     let filter_query_string = build_filter_only_query_string(&params);
//     let current_listing_params_qs = build_full_query_string_from_params(&params);

//     // 3. Wygeneruj HTML za pomocą Maud
//     Ok(html! {
//         div #products-grid-container { // Odpowiednik głównego diva z product_grid.html
//             div #products-container .grid.grid-cols-1.sm:grid-cols-2.lg:grid-cols-3.xl:grid-cols-4.gap-6 {
//                 @if products.is_empty() {
//                     p ."col-span-full text-center text-gray-500 py-8" {
//                         "Brak produktów spełniających kryteria."
//                     }
//                 } @else {
//                     @for product in &products {
//                         // Odpowiednik product_card.html
//                         div ."border rounded-lg p-4 shadow-lg flex flex-col" {
//                             a
//                                 href=(format!("/products/{}", product.id)) // Link do pełnej strony produktu (jeśli istnieje)
//                                 hx-get=(format!("/htmx/product/{}?return_params={}", product.id, urlencoding::encode(&current_listing_params_qs)))
//                                 hx-target="#content" // Lub inny główny target
//                                 hx-swap="innerHTML"
//                                 hx-push-url=(format!("/products/{}", product.id))
//                                 class="block mb-2 group"
//                             {
//                                 @if product.images.len() > 0 {
//                                     img
//                                         src=(product.images[0])
//                                         alt=(product.name)
//                                         class="w-full h-48 sm:h-56 object-cover rounded-md group-hover:opacity-85 transition-opacity duration-200"
//                                         loading="lazy";
//                                 } @else {
//                                     div ."w-full h-48 sm:h-56 bg-gray-200 rounded-md flex items-center justify-center group-hover:opacity-85 transition-opacity duration-200" {
//                                         span ."text-gray-500 text-sm" { "Brak zdjęcia" }
//                                     }
//                                 }
//                             }
//                             div ."flex-grow" { // Aby opis i cena wypełniły przestrzeń
//                                 h2 ."text-lg font-semibold mb-1 text-gray-800 group-hover:text-pink-600 transition-colors duration-200" {
//                                     a // Ten sam link co na obrazku dla tytułu
//                                         href=(format!("/products/{}", product.id))
//                                         hx-get=(format!("/htmx/product/{}?return_params={}", product.id, urlencoding::encode(&current_listing_params_qs)))
//                                         hx-target="#content"
//                                         hx-swap="innerHTML"
//                                         hx-push-url=(format!("/products/{}", product.id))
//                                     {
//                                         (product.name)
//                                     }
//                                 }
//                                 p ."text-gray-700 mb-1" { (product.price / 100) " zł" } // Zakładając cenę w groszach
//                                 p ."text-xs text-gray-500 mb-1" { "Stan: " (product.condition.to_string()) }
//                                 p ."text-xs text-gray-500 mb-2" { "Kategoria: " (product.category.to_string()) }
//                             }
//                             div ."mt-auto" { // Button na dole karty
//                                 button
//                                     hx-post=(format!("/htmx/cart/add/{}", product.id))
//                                     hx-swap="none" // Zakładając, że licznik koszyka aktualizuje się globalnie
//                                     class="w-full mt-2 bg-pink-600 hover:bg-pink-700 text-white font-medium py-2 px-4 rounded-lg transition-colors duration-200 ease-in-out focus:outline-none focus:ring-2 focus:ring-pink-500 focus:ring-opacity-70 transform active:scale-95"
//                                     title=(format!("Dodaj {} do koszyka", product.name))
//                                 {
//                                     "Dodaj do koszyka"
//                                 }
//                             }
//                         }
//                     }
//                 }
//             }

//             @if total_pages > 1 { // Pokaż paginację tylko jeśli jest więcej niż 1 strona
//                 div #pagination-controls ."mt-8 flex justify-center items-center space-x-1 sm:space-x-2" {
//                     @if current_page > 1 {
//                         button
//                             hx-get=(format!("/htmx/products?offset={}&limit={}{}", (current_page - 2) * per_page, per_page, filter_query_string)) // offset to (page-1)*limit
//                             hx-target="#products-grid-container" // Celujemy w kontener siatki + paginacji
//                             hx-swap="outerHTML" // Zastępujemy cały kontener
//                             class="px-3 sm:px-4 py-2 border rounded-md text-sm font-medium text-gray-700 bg-white hover:bg-gray-50 focus:outline-none focus:ring-2 focus:ring-pink-500"
//                         {
//                             "Poprzednia"
//                         }
//                     } @else {
//                         span class="px-3 sm:px-4 py-2 border rounded-md text-sm font-medium text-gray-400 bg-gray-50 cursor-not-allowed" { "Poprzednia" }
//                     }

//                     // Opcjonalnie: Numery stron
//                     @for page_num in 1..=total_pages {
//                         @if page_num == current_page {
//                             span class="px-3 sm:px-4 py-2 border rounded-md text-sm font-medium text-white bg-pink-600 z-10" { (page_num) }
//                         } @else if page_num == 1 || page_num == total_pages || (page_num >= current_page - 1 && page_num <= current_page + 1) {
//                             button
//                                 hx-get=(format!("/htmx/products?offset={}&limit={}{}", (page_num - 1) * per_page, per_page, filter_query_string))
//                                 hx-target="#products-grid-container"
//                                 hx-swap="outerHTML"
//                                 class="px-3 sm:px-4 py-2 border rounded-md text-sm font-medium text-gray-700 bg-white hover:bg-gray-50 focus:outline-none focus:ring-2 focus:ring-pink-500"
//                             {
//                                 (page_num)
//                             }
//                         } @else if page_num == current_page - 2 || page_num == current_page + 2 {
//                              span class="px-3 sm:px-4 py-2 text-sm text-gray-500" { "..." }
//                         }
//                     }

//                     @if current_page < total_pages {
//                         button
//                             hx-get=(format!("/htmx/products?offset={}&limit={}{}", current_page * per_page, per_page, filter_query_string))
//                             hx-target="#products-grid-container"
//                             hx-swap="outerHTML"
//                             class="px-3 sm:px-4 py-2 border rounded-md text-sm font-medium text-gray-700 bg-white hover:bg-gray-50 focus:outline-none focus:ring-2 focus:ring-pink-500"
//                         {
//                             "Następna"
//                         }
//                     } @else {
//                         span class="px-3 sm:px-4 py-2 border rounded-md text-sm font-medium text-gray-400 bg-gray-50 cursor-not-allowed" { "Następna" }
//                     }
//                 }
//             }
//         }
//     })
// }

#[derive(Deserialize, Debug)]
pub struct DetailViewParams {
    #[serde(default)]
    pub return_params: Option<String>,
}

// Funkcja pomocnicza do formatowania ceny (można ją umieścić gdzieś indziej, np. w utils)
fn format_price_maud(price: i64) -> String {
    format!("{:.2}", (price as f64) / 100.0).replace('.', ",") + " zł"
}

pub async fn get_product_detail_htmx_handler(
    State(app_state): State<AppState>,
    Path(product_id): Path<Uuid>,
    Query(query_params): Query<DetailViewParams>,
) -> Result<Markup, AppError> {
    tracing::info!(
        "MAUD: /htmx/product/{} z parametrami: {:?}",
        product_id,
        query_params
    );

    let product = match sqlx::query_as::<_, Product>(
        r#"SELECT id, name, description, price, gender, condition, category, status, images
           FROM products
           WHERE id = $1"#,
    )
    .bind(product_id)
    .fetch_one(&app_state.db_pool)
    .await
    {
        Ok(p) => p,
        Err(sqlx::Error::RowNotFound) => {
            tracing::warn!("MAUD: Nie znaleziono produktu o ID: {}", product_id);
            return Err(AppError::NotFound);
        }
        Err(e) => {
            tracing::error!(
                "MAUD: Błąd bazy danych przy pobieraniu produktu {}: {:?}",
                product_id,
                e
            );
            return Err(AppError::from(e));
        }
    };

    let formatted_price = format_price_maud(product.price);

    let initial_image_url_str: &str = product.images.get(0).map_or("", |url| url.as_str());

    // 1. Te stringi są już poprawnymi literałami JS dzięki serde_json::to_string
    let initial_image_js_literal =
        serde_json::to_string(initial_image_url_str).unwrap_or_else(|_| String::from("\"\""));

    let all_images_js_array_literal: String =
        serde_json::to_string(&product.images).unwrap_or_else(|_| String::from("[]"));
    // 2. Zbuduj string dla x-data. Zauważ, że klucze obiektu JS nie potrzebują cudzysłowów, jeśli są prostymi identyfikatorami.
    let x_data_attribute_value = format!(
        "{{ currentMainImage: {val1}, allProductImages: {val2} }}", // Zmieniono na allProductImages (camelCase)
        val1 = initial_image_js_literal,
        val2 = all_images_js_array_literal
    );
    // x_data_attribute_value będzie teraz stringiem Rusta np:
    // "{ currentMainImage: \"url1\", allProductImages: [\"url1\",\"url2\"] }"

    let main_image_click_alpine_action = format!(
        // Użyj klucza imagesArray, a wartość to już gotowa tablica JS
        "if (currentMainImage && currentMainImage !== '') $dispatch('open-alpine-modal', {{ src: currentMainImage, imagesArray: {} }})",
        all_images_js_array_literal // Ten string zostanie wstawiony jako literał tablicy JS
    );

    let return_query_params_str_rust: Option<String> = query_params.return_params;

    Ok(html! {
    // Usunięto PreEscaped, pozwól Maud na standardowe HTML escaping.
    // Alpine.js odczytując atrybut, zinterpretuje &quot; jako ".
    div "x-data"=(x_data_attribute_value)
        class="bg-white p-4 sm:p-6 lg:p-8 rounded-lg shadow-xl" {
        div ."grid grid-cols-1 md:grid-cols-2 gap-8 lg:gap-12" {
            // --- Kolumna z obrazkami ---
            div ."space-y-4" {
                @if !product.images.is_empty() {
                    div ."aspect-w-4 aspect-h-3 sm:aspect-w-1 sm:aspect-h-1 rounded-lg overflow-hidden border border-gray-200 shadow-sm bg-gray-50 flex items-center justify-center" {
                        img
                            "x-bind:src"="currentMainImage && currentMainImage !== '' ? currentMainImage : '/static/placeholder.png'"
                            alt={"Zdjęcie główne: " (product.name)} // Maud domyślnie escapuje product.name
                            class="max-w-full max-h-[60vh] md:max-h-full object-contain cursor-pointer hover:opacity-90 transition-opacity duration-200"
                            loading="lazy"
                            "@click"=(main_image_click_alpine_action);
                    }

                    @if product.images.len() > 1 {
                        div .grid.grid-cols-3.sm:grid-cols-4.md:grid-cols-3.lg:grid-cols-5.gap-2.sm:gap-3 {
                            // Używamy allProductImages (camelCase) konsekwentnie
                            @for (image_url_loop_item, index) in product.images.iter().zip(0..) {
                                @let click_action_str = format!("currentMainImage = allProductImages[{}]", index);
                                @let class_binding_str = format!("currentMainImage === allProductImages[{}] ? 'border-pink-500 ring-2 ring-pink-500' : 'border-gray-200 hover:border-pink-400'", index);

                                button type="button"
                                    "@click"=(click_action_str)
                                    "x-bind:class"=(class_binding_str)
                                    class="aspect-w-1 aspect-h-1 block border-2 rounded-md overflow-hidden focus:outline-none focus:border-pink-500 transition-all duration-150 bg-gray-50"
                                    aria-label={"Zmień główne zdjęcie na miniaturkę " (index + 1)} {
                                    img src=(image_url_loop_item) alt={"Miniaturka " (index + 1) ": " (product.name)} class="w-full h-full object-cover object-center" loading="lazy";
                                }
                            }
                        }
                    }
                } @else {
                    div ."aspect-w-4 aspect-h-3 sm:aspect-w-1 sm:aspect-h-1 w-full bg-gray-100 rounded-lg flex items-center justify-center border border-gray-200" {
                        img src="/static/placeholder.png" alt={"Brak zdjęcia produktu " (product.name)} class="max-w-full max-h-full object-contain opacity-50";
                    }
                }
            }
                // --- Kolumna z informacjami o produkcie ---
                div ."flex flex-col" {
                    h1 ."text-2xl sm:text-3xl lg:text-4xl font-bold tracking-tight text-gray-900 mb-2" { (product.name) }
                    p ."text-3xl font-semibold text-pink-600 mb-5" { (formatted_price) }

                    div ."space-y-2 text-sm text-gray-700 mb-5" {
                        p { strong ."font-medium text-gray-900" { "Rodzaj:" } " " (product.gender.to_string()) }
                        p { strong ."font-medium text-gray-900" { "Kategoria:" } " " (product.category.to_string()) }
                        p { strong ."font-medium text-gray-900" { "Stan:" } " " (product.condition.to_string()) }
                        p {
                            strong ."font-medium text-gray-900" { "Status:" } " "
                            @let status_str = product.status.to_string();
                            @if status_str == "Dostępny" {
                                span ."px-2 py-0.5 inline-flex text-xs leading-5 font-semibold rounded-full bg-green-100 text-green-800" { "Dostępny" }
                            } @else if status_str == "Zarezerwowany" {
                                span ."px-2 py-0.5 inline-flex text-xs leading-5 font-semibold rounded-full bg-yellow-100 text-yellow-800" { "Zarezerwowany" }
                            } @else if status_str == "Sprzedany" {
                                span ."px-2 py-0.5 inline-flex text-xs leading-5 font-semibold rounded-full bg-red-100 text-red-800" { "Sprzedany" }
                            } @else {
                                span { (status_str) }
                            }
                        }
                    }

                    div ."prose prose-sm max-w-none text-gray-600 mb-6" {
                        h2 ."text-md font-semibold text-gray-800 mb-1 not-prose" { "Opis produktu:" }
                        @for line in product.description.lines() {
                            (line) br;
                        }
                    }

                    div ."mt-auto pt-6" {
                        @if product.status.to_string() == "Dostępny" {
                            button
                                "hx-post"=(format!("/htmx/cart/add/{}", product.id)) // Nazwy atrybutów HTMX też w cudzysłowy
                                "hx-swap"="none"
                                class="w-full bg-pink-600 hover:bg-pink-700 text-white font-semibold py-3 px-6 rounded-lg shadow-md transition-all duration-200 ease-in-out hover:shadow-lg focus:outline-none focus:ring-2 focus:ring-pink-500 focus:ring-opacity-70 cursor-pointer transform active:scale-95"
                                title=(format!("Dodaj {} do koszyka", product.name))
                            {
                                "Dodaj do koszyka"
                            }
                        } @else {
                            div ."w-full text-center py-3 px-6 rounded-lg bg-gray-100 text-gray-500 font-semibold" {
                                "Produkt obecnie niedostępny"
                            }
                        }

                        // --- Logika linku powrotnego ---
                        div ."mt-4 text-center sm:text-left" {
                             @if let Some(qs_val) = &return_query_params_str_rust {
                                @if !qs_val.is_empty() {
                                    a href=(format!("/kategoria?{}", qs_val))
                                       "hx-get"=(format!("/htmx/products?{}", qs_val))
                                       "hx-target"="#content" "hx-swap"="innerHTML"
                                       "hx-push-url"=(format!("/kategoria?{}", qs_val))
                                       class="text-sm text-blue-600 hover:text-blue-800 hover:underline transition-colors" {
                                        "← Wróć do poprzedniego widoku"
                                    }
                                } @else {
                                    // Fallback dla Some("")
                                    @if product.gender == crate::models::ProductGender::Damskie { // Bezpośrednie porównanie enumów
                                        a href="/dla-niej" "hx-get"="/htmx/strona-plec/Damskie" "hx-target"="#content" "hx-push-url"="/dla-niej" class="text-sm text-blue-600 hover:text-blue-800 hover:underline transition-colors" {
                                            "← Wróć do " (product.gender.to_string())
                                        }
                                    } @else if product.gender == crate::models::ProductGender::Meskie {
                                        a href="/dla-niego" "hx-get"="/htmx/strona-plec/Meskie" "hx-target"="#content" "hx-push-url"="/dla-niego" class="text-sm text-blue-600 hover:text-blue-800 hover:underline transition-colors" {
                                            "← Wróć do " (product.gender.to_string())
                                        }
                                    } @else {
                                        a href="/" "hx-get"="/htmx/products?limit=9" "hx-target"="#content" "hx-push-url"="/" class="text-sm text-blue-600 hover:text-blue-800 hover:underline transition-colors" {
                                            "← Wróć na stronę główną"
                                        }
                                    }
                                }
                            } @else {
                                // Fallback dla None
                                @if product.gender == crate::models::ProductGender::Damskie {
                                    a href="/dla-niej" "hx-get"="/htmx/strona-plec/Damskie" "hx-target"="#content" "hx-push-url"="/dla-niej" class="text-sm text-blue-600 hover:text-blue-800 hover:underline transition-colors" {
                                        "← Wróć do " (product.gender.to_string())
                                    }
                                } @else if product.gender == crate::models::ProductGender::Meskie {
                                    a href="/dla-niego" "hx-get"="/htmx/strona-plec/Meskie" "hx-target"="#content" "hx-push-url"="/dla-niego" class="text-sm text-blue-600 hover:text-blue-800 hover:underline transition-colors" {
                                        "← Wróć do " (product.gender.to_string())
                                    }
                                } @else {
                                    a href="/" "hx-get"="/htmx/products?limit=9" "hx-target"="#content" "hx-push-url"="/" class="text-sm text-blue-600 hover:text-blue-800 hover:underline transition-colors" {
                                        "← Wróć na stronę główną"
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    })
}

pub async fn get_cart_details_htmx_handler(
    State(app_state): State<AppState>,
    user_claims_result: Result<TokenClaims, AppError>, // Wynik ekstrakcji JWT (może być błąd, jeśli brak tokenu)
    guest_cart_id_header: Option<TypedHeader<XGuestCartId>>,
) -> Result<(HeaderMap, Markup), AppError> {
    // Zwracamy teraz krotkę (nagłówki, Markup)
    tracing::info!("MAUD: /htmx/cart/details - żądanie zawartości koszyka");

    let mut conn = app_state.db_pool.acquire().await.map_err(|e| {
        tracing::error!("MAUD Cart: Nie można uzyskać połączenia z puli: {}", e);
        AppError::InternalServerError("Błąd serwera przy ładowaniu koszyka".to_string())
    })?;

    let mut cart_details_response: Option<CartDetailsResponse> = None;
    let mut final_guest_cart_id_for_trigger: Option<Uuid> = None;

    if let Ok(claims) = user_claims_result {
        // Użytkownik jest zalogowany
        let user_id = claims.sub;
        if let Some(cart) =
            sqlx::query_as::<_, ShoppingCart>("SELECT * FROM shopping_carts WHERE user_id = $1")
                .bind(user_id)
                .fetch_optional(&mut *conn)
                .await?
        {
            cart_details_response =
                Some(cart_utils::build_cart_details_response(&cart, &mut conn).await?);
        }
    } else if let Some(TypedHeader(XGuestCartId(guest_id))) = guest_cart_id_header {
        // Użytkownik-gość z istniejącym ID koszyka
        final_guest_cart_id_for_trigger = Some(guest_id);
        if let Some(cart) = sqlx::query_as::<_, ShoppingCart>(
            "SELECT * FROM shopping_carts WHERE guest_session_id = $1",
        )
        .bind(guest_id)
        .fetch_optional(&mut *conn)
        .await?
        {
            cart_details_response =
                Some(cart_utils::build_cart_details_response(&cart, &mut conn).await?);
        }
    }
    // Jeśli ani zalogowany, ani gość z ID, cart_details_response pozostanie None (pusty koszyk)

    let items = cart_details_response
        .as_ref()
        .map_or_else(Vec::new, |cdr| cdr.items.clone()); // Klonujemy, bo cdr jest potrzebny niżej
    let total_items = cart_details_response
        .as_ref()
        .map_or(0, |cdr| cdr.total_items);
    let total_price = cart_details_response
        .as_ref()
        .map_or(0, |cdr| cdr.total_price);

    // Przygotuj nagłówek HX-Trigger
    let mut headers = HeaderMap::new();
    let trigger_payload = serde_json::json!({
        "updateCartCount": {
            "newCount": total_items,
            "newCartTotalPrice": total_price, // Przekazujemy sumę do aktualizacji w Alpine.js
            "newGuestCartId": final_guest_cart_id_for_trigger // Przekaż, jeśli jest (dla Alpine)
        }
    });
    if let Ok(trigger_value) = HeaderValue::from_str(&trigger_payload.to_string()) {
        headers.insert("HX-Trigger", trigger_value);
    } else {
        tracing::error!("MAUD Cart: Nie można utworzyć nagłówka HX-Trigger dla koszyka");
    }

    let markup = html! {
        @if items.is_empty() {
            p ."text-gray-600 py-6 text-center" { "Twój koszyk jest pusty." }
        } @else {
            // Ta informacja może być teraz zarządzana przez Alpine.js na podstawie danych z HX-Trigger
            // p ."text-sm text-gray-500" { "Masz " (total_items) " przedmiot(y) w koszyku." }

            ul role="list" ."my-6 divide-y divide-gray-200 border-t border-b" {
                @for item in &items {
                    li ."flex py-4 px-4 sm:px-0" { // Dodano padding dla lepszego wyglądu na mobilnych w panelu
                        div ."h-20 w-20 flex-shrink-0 overflow-hidden rounded-md border border-gray-200" {
                            @if !item.product.images.is_empty() {
                                img src=(item.product.images[0]) alt=(item.product.name) class="h-full w-full object-cover object-center" loading="lazy";
                            } @else {
                                div ."h-full w-full bg-gray-100 flex items-center justify-center text-xs text-gray-400" { "Brak foto" }
                            }
                        }
                        div ."ml-4 flex flex-1 flex-col" {
                            div {
                                div ."flex justify-between text-sm font-medium text-gray-800" {
                                    h3 {
                                        // Link do szczegółów produktu - można dodać później, na razie nazwa
                                        (item.product.name)
                                    }
                                    // Cena za sztukę
                                    p ."ml-4" { (format_price_maud(item.product.price)) }
                                }
                                // Można dodać kategorię lub inne detale, jeśli potrzebne
                                // p ."mt-1 text-xs text-gray-500" { "Kategoria: " (item.product.category.to_string()) }
                            }
                            div ."flex flex-1 items-end justify-between text-xs" {
                                div ."flex" {
                                    button type="button"
                                        "hx-post"=(format!("/htmx/cart/remove/{}", item.product.id)) // Endpoint do zaimplementowania
                                        "hx-target"="#cart-content-target" // Odświeża tę samą sekcję
                                        "hx-swap"="innerHTML"
                                        // Można dodać hx-confirm
                                        class="font-medium text-pink-600 hover:text-pink-500 transition-colors" {
                                        "Usuń"
                                    }
                                }
                            }
                        }
                    }
                }
            }
            // Suma częściowa jest teraz zarządzana przez Alpine.js w index.html na podstawie danych z HX-Trigger
        }
    };
    Ok((headers, markup))
}

pub async fn add_item_to_cart_htmx_handler(
    State(app_state): State<AppState>,
    Path(product_id): Path<Uuid>,
    user_claims_result: Result<TokenClaims, AppError>, // Rezultat ekstrakcji JWT
    guest_cart_id_header: Option<TypedHeader<XGuestCartId>>,
) -> Result<(HeaderMap, StatusCode), AppError> {
    tracing::info!(
        "MAUD HTMX: /htmx/cart/add/{} - próba dodania produktu",
        product_id
    );

    let mut tx = app_state.db_pool.begin().await.map_err(|e| {
        tracing::error!("MAUD AddToCart: Błąd rozpoczynania transakcji: {}", e);
        AppError::InternalServerError("Błąd serwera przy dodawaniu do koszyka".to_string())
    })?;

    let cart: ShoppingCart;
    let mut new_guest_cart_id_to_set: Option<Uuid> = None; // ID do odesłania w triggerze, jeśli nowy koszyk gościa

    // 1. Ustal koszyk (użytkownika lub gościa)
    if let Ok(claims) = user_claims_result {
        // Użytkownik zalogowany
        let user_id = claims.sub;
        cart = match sqlx::query_as("SELECT * FROM shopping_carts WHERE user_id = $1 FOR UPDATE")
            .bind(user_id)
            .fetch_optional(&mut *tx)
            .await?
        {
            Some(existing_cart) => existing_cart,
            None => {
                sqlx::query_as("INSERT INTO shopping_carts (user_id) VALUES ($1) RETURNING *")
                    .bind(user_id)
                    .fetch_one(&mut *tx)
                    .await?
            }
        };
        tracing::info!(
            "MAUD AddToCart: Użytkownik ID: {}, koszyk ID: {}",
            user_id,
            cart.id
        );
    } else if let Some(TypedHeader(XGuestCartId(guest_id))) = guest_cart_id_header {
        // Gość z istniejącym ID koszyka
        if let Some(existing_cart) =
            sqlx::query_as("SELECT * FROM shopping_carts WHERE guest_session_id = $1 FOR UPDATE")
                .bind(guest_id)
                .fetch_optional(&mut *tx)
                .await?
        {
            cart = existing_cart;
            new_guest_cart_id_to_set = Some(guest_id); // Prześlemy to samo ID gościa
            tracing::info!(
                "MAUD AddToCart: Gość z istniejącym koszykiem (Session ID: {}), koszyk ID: {}",
                guest_id,
                cart.id
            );
        } else {
            // ID gościa z nagłówka nie znaleziono w bazie - tworzymy nowy koszyk z NOWYM ID
            let new_id = Uuid::new_v4();
            new_guest_cart_id_to_set = Some(new_id);
            cart = sqlx::query_as(
                "INSERT INTO shopping_carts (guest_session_id) VALUES ($1) RETURNING *",
            )
            .bind(new_id)
            .fetch_one(&mut *tx)
            .await?;
            tracing::info!(
                "MAUD AddToCart: Nagłówek X-Guest-Cart-Id był, ale koszyk nie istniał. Stworzono nowy koszyk (Session ID: {}), koszyk ID: {}",
                new_id,
                cart.id
            );
        }
    } else {
        // Nowy gość, bez nagłówka X-Guest-Cart-Id
        let new_id = Uuid::new_v4();
        new_guest_cart_id_to_set = Some(new_id); // To ID zostanie wysłane do klienta
        cart =
            sqlx::query_as("INSERT INTO shopping_carts (guest_session_id) VALUES ($1) RETURNING *")
                .bind(new_id)
                .fetch_one(&mut *tx)
                .await?;
        tracing::info!(
            "MAUD AddToCart: Nowy gość, stworzono nowy koszyk (Session ID: {}), koszyk ID: {}",
            new_id,
            cart.id
        );
    }

    // 2. Sprawdź produkt i dodaj do koszyka
    let mut headers = HeaderMap::new();
    let product_opt =
        sqlx::query_as::<_, Product>("SELECT * FROM products WHERE id = $1 FOR UPDATE")
            .bind(product_id)
            .fetch_optional(&mut *tx)
            .await?;

    match product_opt {
        Some(product) => {
            if product.status != ProductStatus::Available {
                tracing::warn!(
                    "MAUD AddToCart: Produkt {} (ID: {}) niedostępny. Status: {:?}",
                    product.name,
                    product_id,
                    product.status
                );
                tx.rollback().await?; // Ważne: wycofaj transakcję, bo nic nie dodajemy

                let trigger_payload = serde_json::json!({
                    "showMessage": { "type": "warning", "message": format!("Produkt '{}' jest obecnie niedostępny.", product.name) }
                });
                if let Ok(val) = HeaderValue::from_str(&trigger_payload.to_string()) {
                    headers.insert("HX-Trigger", val);
                }
                return Ok((headers, StatusCode::OK)); // Zwracamy OK, ale z wiadomością o błędzie
            }

            // Dodaj produkt do cart_items (lub zignoruj, jeśli już istnieje)
            sqlx::query("INSERT INTO cart_items (cart_id, product_id) VALUES ($1, $2) ON CONFLICT (cart_id, product_id) DO NOTHING")
                .bind(cart.id)
                .bind(product_id)
                .execute(&mut *tx)
                .await?;
            tracing::info!(
                "MAUD AddToCart: Produkt ID {} dodany/istniał w koszyku ID {}",
                product_id,
                cart.id
            );
        }
        None => {
            tracing::warn!(
                "MAUD AddToCart: Produkt o ID {} nie znaleziony.",
                product_id
            );
            tx.rollback().await?;

            let trigger_payload = serde_json::json!({
                "showMessage": { "type": "error", "message": "Wybrany produkt nie został znaleziony." }
            });
            if let Ok(val) = HeaderValue::from_str(&trigger_payload.to_string()) {
                headers.insert("HX-Trigger", val);
            }
            return Ok((headers, StatusCode::NOT_FOUND)); // Można też OK z triggerem błędu
        }
    }

    // 3. Pobierz zaktualizowane szczegóły koszyka dla HX-Trigger
    // Funkcja build_cart_details_response aktualizuje też `updated_at` w tabeli `shopping_carts`
    let cart_details: CartDetailsResponse =
        cart_utils::build_cart_details_response(&cart, &mut tx).await?;

    // 4. Zatwierdź transakcję
    tx.commit().await.map_err(|e| {
        tracing::error!("MAUD AddToCart: Błąd przy zatwierdzaniu transakcji: {}", e);
        AppError::InternalServerError("Błąd serwera przy zapisie koszyka".to_string())
    })?;

    // 5. Przygotuj nagłówek HX-Trigger
    let trigger_payload = serde_json::json!({
        "updateCartCount": {
            "newCount": cart_details.total_items,
            "newCartTotalPrice": cart_details.total_price,
            "newGuestCartId": new_guest_cart_id_to_set // Przekaż nowe lub istniejące ID gościa
        },
        "showMessage": { // Opcjonalna wiadomość o sukcesie
            "type": "success",
            "message": "Dodano produkt do koszyka!"
        }
    });

    if let Ok(trigger_value) = HeaderValue::from_str(&trigger_payload.to_string()) {
        headers.insert("HX-Trigger", trigger_value);
    } else {
        tracing::error!("MAUD AddToCart: Nie można utworzyć nagłówka HX-Trigger.");
    }

    // Ponieważ przyciski "Dodaj do koszyka" mają hx-swap="none", nie zwracamy HTML.
    // Zwracamy tylko nagłówki (z HX-Trigger) i kod statusu. f
    Ok((headers, StatusCode::OK)) // Można też użyć StatusCode::NO_CONTENT (204), jeśli nie ma żadnej wiadomości w payloadzie
}

pub async fn remove_item_from_cart_htmx_handler(
    State(app_state): State<AppState>,
    Path(product_id_to_remove): Path<Uuid>,
    user_claims_result: Result<TokenClaims, AppError>,
    guest_cart_id_header: Option<TypedHeader<XGuestCartId>>,
) -> Result<(HeaderMap, Markup), AppError> {
    tracing::info!(
        "MAUD HTMX: /htmx/cart/remove/{} - próba usunięcia produktu",
        product_id_to_remove
    );

    let mut tx = app_state.db_pool.begin().await.map_err(|e| {
        tracing::error!("MAUD RemoveFromCart: Błąd rozpoczynania transakcji: {}", e);
        AppError::InternalServerError("Błąd serwera przy usuwaniu z koszyka".to_string())
    })?;

    let mut cart_for_response: Option<ShoppingCart> = None;
    let mut guest_cart_id_for_trigger: Option<Uuid> = None;

    // 1. Znajdź koszyk użytkownika lub gościa
    if let Ok(claims) = user_claims_result {
        // Użytkownik zalogowany
        let user_id = claims.sub;
        if let Some(cart) =
            sqlx::query_as("SELECT * FROM shopping_carts WHERE user_id = $1 FOR UPDATE")
                .bind(user_id)
                .fetch_optional(&mut *tx)
                .await?
        {
            cart_for_response = Some(cart);
        }
    } else if let Some(TypedHeader(XGuestCartId(guest_id))) = guest_cart_id_header {
        // Gość
        if let Some(cart) =
            sqlx::query_as("SELECT * FROM shopping_carts WHERE guest_session_id = $1 FOR UPDATE")
                .bind(guest_id)
                .fetch_optional(&mut *tx)
                .await?
        {
            cart_for_response = Some(cart);
            guest_cart_id_for_trigger = Some(guest_id);
        }
    }

    // 2. Usuń produkt z koszyka, jeśli koszyk istnieje
    if let Some(ref cart) = cart_for_response {
        let delete_result =
            sqlx::query("DELETE FROM cart_items WHERE cart_id = $1 AND product_id = $2")
                .bind(cart.id)
                .bind(product_id_to_remove)
                .execute(&mut *tx)
                .await?;

        if delete_result.rows_affected() > 0 {
            tracing::info!(
                "MAUD RemoveFromCart: Produkt ID {} usunięty z koszyka ID {}",
                product_id_to_remove,
                cart.id
            );
        } else {
            tracing::warn!(
                "MAUD RemoveFromCart: Produkt ID {} nie znaleziony w koszyku ID {} do usunięcia.",
                product_id_to_remove,
                cart.id
            );
        }
        // Nawet jeśli produkt nie został znaleziony do usunięcia (może już go nie było),
        // nadal chcemy zbudować i zwrócić aktualny stan koszyka.
    } else {
        // Koszyk nie istnieje, więc nie ma z czego usuwać.
        // Zwrócimy pusty stan koszyka, trigger z zerowymi wartościami.
        tracing::warn!("MAUD RemoveFromCart: Próba usunięcia produktu, ale koszyk nie istnieje.");
    }

    // 3. Pobierz zaktualizowane szczegóły koszyka (lub domyślne, jeśli koszyk nie istniał)
    //    build_cart_details_response aktualizuje też updated_at koszyka.
    let cart_details: CartDetailsResponse = if let Some(ref cart_ref) = cart_for_response {
        // Musimy odświeżyć stan koszyka, ponieważ build_cart_details_response może go zaktualizować
        let refreshed_cart =
            sqlx::query_as::<_, ShoppingCart>("SELECT * FROM shopping_carts WHERE id = $1")
                .bind(cart_ref.id)
                .fetch_one(&mut *tx) // Używamy tx, bo updated_at jest modyfikowane w build_cart_details_response
                .await?;
        cart_utils::build_cart_details_response(&refreshed_cart, &mut tx).await?
    } else {
        // Jeśli koszyk nie istniał, zwracamy "pustą" odpowiedź.
        // Można by tu zwrócić błąd, ale dla HTMX chcemy zazwyczaj zwrócić fragment HTML.
        // W tym przypadku, jeśli nie ma koszyka, `items` będzie puste.
        CartDetailsResponse {
            cart_id: Uuid::nil(), // Lub inne sensowne domyślne ID
            user_id: None,
            items: vec![],
            total_items: 0,
            total_price: 0,
            updated_at: chrono::Utc::now(),
        }
    };

    // 4. Zatwierdź transakcję
    tx.commit().await.map_err(|e| {
        tracing::error!(
            "MAUD RemoveFromCart: Błąd przy zatwierdzaniu transakcji: {}",
            e
        );
        AppError::InternalServerError("Błąd serwera przy aktualizacji koszyka".to_string())
    })?;

    // 5. Przygotuj nagłówek HX-Trigger
    let mut headers = HeaderMap::new();
    let trigger_payload = serde_json::json!({
        "updateCartCount": {
            "newCount": cart_details.total_items,
            "newCartTotalPrice": cart_details.total_price,
            "newGuestCartId": guest_cart_id_for_trigger // Przekazujemy ID gościa, jeśli było
        },
        "showMessage": {
            "type": "info",
            "message": "Produkt usunięty z koszyka."
        }
    });

    if let Ok(trigger_value) = HeaderValue::from_str(&trigger_payload.to_string()) {
        headers.insert("HX-Trigger", trigger_value);
    } else {
        tracing::error!("MAUD RemoveFromCart: Nie można utworzyć nagłówka HX-Trigger.");
    }

    // 6. Wyrenderuj HTML dla listy przedmiotów w koszyku (podobnie jak w get_cart_details_htmx_handler)
    let markup = html! {
        @if cart_details.items.is_empty() {
            p ."text-gray-600 py-6 text-center" { "Twój koszyk jest pusty." }
        } @else {
            ul role="list" ."my-6 divide-y divide-gray-200 border-t border-b" {
                @for item in &cart_details.items {
                    li ."flex py-4 px-4 sm:px-0" {
                        div ."h-20 w-20 flex-shrink-0 overflow-hidden rounded-md border border-gray-200" {
                            @if !item.product.images.is_empty() {
                                img src=(item.product.images[0]) alt=(item.product.name) class="h-full w-full object-cover object-center" loading="lazy";
                            } @else {
                                div ."h-full w-full bg-gray-100 flex items-center justify-center text-xs text-gray-400" { "Brak foto" }
                            }
                        }
                        div ."ml-4 flex flex-1 flex-col" {
                            div {
                                div ."flex justify-between text-sm font-medium text-gray-800" {
                                    h3 { (item.product.name) }
                                    p ."ml-4" { (format_price_maud(item.product.price)) }
                                }
                            }
                            div ."flex flex-1 items-end justify-between text-xs" {
                                div ."flex" {
                                    button type="button"
                                        "hx-post"=(format!("/htmx/cart/remove/{}", item.product.id))
                                        "hx-target"="#cart-content-target"
                                        "hx-swap"="innerHTML"
                                        class="font-medium text-pink-600 hover:text-pink-500 transition-colors" {
                                        "Usuń"
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    };

    Ok((headers, markup))
}

fn render_product_grid_maud(
    products: &[Product], // Przyjmujemy plaster (slice)
    current_page: i64,
    total_pages: i64,
    per_page: i64,
    filter_query_string: &str, // Dla linków paginacji
    current_listing_params_qs: &str, // Dla linków "Zobacz szczegóły"
                               // Opcjonalnie: można dodać target_div_id: &str, jeśli paginacja miałaby celować w różne kontenery
) -> Markup {
    html! {
        div #products-grid-container { // Ten ID jest ważny dla hx-target paginacji
            div #products-container .grid.grid-cols-1.sm:grid-cols-2.lg:grid-cols-3.xl:grid-cols-4.gap-6 {
                @if products.is_empty() {
                    p ."col-span-full text-center text-gray-500 py-8" {
                        "Brak produktów spełniających wybrane kryteria."
                    }
                } @else {
                    @for product in products { // Iterujemy po plasterku
                        div ."border rounded-lg p-4 shadow-lg flex flex-col bg-white" {
                            a  href=(format!("/produkty/{}", product.id)) // Link do "pełnej" strony produktu
                                "hx-get"=(format!("/htmx/produkt/{}?return_params={}", product.id, urlencoding::encode(current_listing_params_qs)))
                                "hx-target"="#content" // Główny cel dla szczegółów produktu
                                "hx-swap"="innerHTML"
                                "hx-push-url"=(format!("/produkty/{}", product.id)) // Aktualizuj URL na stronie produktu
                                class="block mb-2 group" {
                                @if !product.images.is_empty() {
                                    img src=(product.images[0]) alt=(product.name) class="w-full h-48 sm:h-56 object-cover rounded-md group-hover:opacity-85 transition-opacity duration-200" loading="lazy";
                                } @else {
                                    div ."w-full h-48 sm:h-56 bg-gray-200 rounded-md flex items-center justify-center group-hover:opacity-85 transition-opacity duration-200" {
                                        span ."text-gray-500 text-sm" { "Brak zdjęcia" }
                                    }
                                }
                            }
                            div ."flex-grow" {
                                h2 ."text-lg font-semibold mb-1 text-gray-800 group-hover:text-pink-600 transition-colors duration-200" {
                                    a href=(format!("/produkty/{}", product.id))
                                       "hx-get"=(format!("/htmx/produkt/{}?return_params={}", product.id, urlencoding::encode(current_listing_params_qs)))
                                       "hx-target"="#content" "hx-swap"="innerHTML"
                                       "hx-push-url"=(format!("/produkty/{}", product.id)) {
                                        (product.name)
                                    }
                                }
                                p ."text-gray-700 mb-1" { (format_price_maud(product.price)) } // Użyj funkcji format_price_maud
                                p ."text-xs text-gray-500 mb-1" { "Stan: " (product.condition.to_string()) }
                                p ."text-xs text-gray-500 mb-2" { "Kategoria: " (product.category.to_string()) }
                            }
                            div ."mt-auto" {
                                button "hx-post"=(format!("/htmx/cart/add/{}", product.id)) "hx-swap"="none"
                                        class="w-full mt-2 bg-pink-600 hover:bg-pink-700 text-white font-medium py-2 px-4 rounded-lg transition-colors duration-200 ease-in-out focus:outline-none focus:ring-2 focus:ring-pink-500 focus:ring-opacity-70 transform active:scale-95"
                                        title=(format!("Dodaj {} do koszyka", product.name)) {
                                    "Dodaj do koszyka"
                                }
                            }
                        }
                    }
                }
            }

            @if total_pages > 1 {
                div #pagination-controls ."mt-8 flex justify-center items-center space-x-1 sm:space-x-2" {
                    @if current_page > 1 {
                        button "hx-get"=(format!("/htmx/products?offset={}&limit={}{}", (current_page - 2) * per_page, per_page, filter_query_string))
                               "hx-target"="#products-grid-container" "hx-swap"="outerHTML" // Celujemy w kontener siatki + paginacji
                               class="px-3 sm:px-4 py-2 border rounded-md text-sm font-medium text-gray-700 bg-white hover:bg-gray-50 focus:outline-none focus:ring-2 focus:ring-pink-500" {
                            "Poprzednia"
                        }
                    } @else {
                        span class="px-3 sm:px-4 py-2 border rounded-md text-sm font-medium text-gray-400 bg-gray-50 cursor-not-allowed" { "Poprzednia" }
                    }
                    @for page_num in 1..=total_pages {
                        @if page_num == current_page {
                            span class="px-3 sm:px-4 py-2 border rounded-md text-sm font-medium text-white bg-pink-600 z-10" { (page_num) }
                        } @else if page_num == 1 || page_num == total_pages || (page_num >= current_page - 2 && page_num <= current_page + 2) { // Prostsza logika wyświetlania numerów
                            button "hx-get"=(format!("/htmx/products?offset={}&limit={}{}", (page_num - 1) * per_page, per_page, filter_query_string))
                                   "hx-target"="#products-grid-container" "hx-swap"="outerHTML"
                                   class="px-3 sm:px-4 py-2 border rounded-md text-sm font-medium text-gray-700 bg-white hover:bg-gray-50 focus:outline-none focus:ring-2 focus:ring-pink-500" {
                                (page_num)
                            }
                        } @else if page_num == current_page - 3 || page_num == current_page + 3 { // Dla kropek
                             span class="px-1 sm:px-2 py-2 text-sm text-gray-500" { "..." }
                        }
                    }
                    @if current_page < total_pages {
                        button "hx-get"=(format!("/htmx/products?offset={}&limit={}{}", current_page * per_page, per_page, filter_query_string))
                               "hx-target"="#products-grid-container" "hx-swap"="outerHTML"
                               class="px-3 sm:px-4 py-2 border rounded-md text-sm font-medium text-gray-700 bg-white hover:bg-gray-50 focus:outline-none focus:ring-2 focus:ring-pink-500" {
                            "Następna"
                        }
                    } @else {
                        span class="px-3 sm:px-4 py-2 border rounded-md text-sm font-medium text-gray-400 bg-gray-50 cursor-not-allowed" { "Następna" }
                    }
                }
            }
        }
    }
}

pub async fn list_products_htmx_handler(
    State(app_state): State<AppState>,
    Query(params): Query<ListingParams>,
) -> Result<Markup, AppError> {
    tracing::info!("MAUD: /htmx/products z parametrami: {:?}", params);
    let paginated_response_axum_json =
        crate::handlers::list_products(State(app_state.clone()), Query(params.clone())).await?;
    let paginated_response: PaginatedProductsResponse = paginated_response_axum_json.0;

    let filter_query_string = build_filter_only_query_string(&params);
    let current_listing_params_qs = build_full_query_string_from_params(&params);

    Ok(render_product_grid_maud(
        &paginated_response.data,
        paginated_response.current_page,
        paginated_response.total_pages,
        paginated_response.per_page,
        &filter_query_string,
        &current_listing_params_qs,
    ))
}

pub async fn gender_page_handler(
    State(app_state): State<AppState>,
    Path(gender_slug): Path<String>,
) -> Result<Markup, AppError> {
    tracing::info!("MAUD: /htmx/dla/{} - ładowanie strony płci", gender_slug);

    let (current_gender, current_gender_display_name) = match gender_slug.as_str() {
        "niej" => (ProductGender::Damskie, "Dla niej"),
        "niego" => (ProductGender::Meskie, "Dla niego"),
        _ => {
            tracing::warn!("MAUD: Nieznany slug płci: {}", gender_slug);
            return Err(AppError::NotFound);
        }
    };

    let categories: Vec<Category> = Category::iter().collect();

    let initial_listing_params = ListingParams {
        gender: Some(current_gender.clone()),
        limit: Some(9),
        offset: Some(0),
        category: None,
        condition: None,
        status: Some(ProductStatus::Available),
        price_min: None,
        price_max: None,
        sort_by: Some("name".to_string()),
        order: Some("asc".to_string()),
        search: None,
    };

    let paginated_response_axum_json = crate::handlers::list_products(
        State(app_state.clone()),
        Query(initial_listing_params.clone()),
    )
    .await?;
    let paginated_response: PaginatedProductsResponse = paginated_response_axum_json.0;

    let filter_query_string_for_initial_grid =
        build_filter_only_query_string(&initial_listing_params);
    let current_listing_params_qs_for_initial_grid =
        build_full_query_string_from_params(&initial_listing_params);

    Ok(html! {
            // Dodajemy x-data. Domyślnie kategorie na mobile są zwinięte (false), na desktopie logika x-show nie zadziała dzięki md:block
            div ."flex flex-col md:flex-row gap-6"
                "x-data"="{ showMobileCategories: false }"
                "x-init"="if (window.innerWidth >= 768) { showMobileCategories = true }" // Pokaż na desktop przy inicjalizacji
                {

                // --- Przycisk do rozwijania/zwijania kategorii na mobile ---
                div ."md:hidden p-4 border-b border-gray-200 bg-gray-50" { // Widoczny tylko na mobile
                    button type="button"
                           "@click"="showMobileCategories = !showMobileCategories"
                           class="w-full flex justify-between items-center px-3 py-2 rounded-md text-gray-700 hover:bg-gray-100 focus:outline-none font-semibold" {
                        span { (current_gender_display_name) ": Kategorie" } // Lub po prostu "Kategorie / Filtry"
    svg "x-show"="!showMobileCategories" class="w-5 h-5 transform transition-transform duration-200" fill="none" stroke="currentColor" "viewBox"="0 0 24 24" "xmlns"="http://www.w3.org/2000/svg" {
                        path "stroke-linecap"="round" "stroke-linejoin"="round" "stroke-width"="2" d="M19 9l-7 7-7-7";
                    }
                    svg "x-show"="showMobileCategories" "x-cloak" class="w-5 h-5 transform transition-transform duration-200 rotate-180" fill="none" stroke="currentColor" "viewBox"="0 0 24 24" "xmlns"="http://www.w3.org/2000/svg" {
                        path "stroke-linecap"="round" "stroke-linejoin"="round" "stroke-width"="2" d="M19 9l-7 7-7-7"; // Ta sama ścieżka, SVG jest obracane przez klasę CSS
                    }                }
                }

                // --- Panel boczny z kategoriami ---
                // Na mobile (poniżej md) będzie kontrolowany przez showMobileCategories
                // Na desktopie (md i w górę) będzie zawsze widoczny dzięki klasom Tailwind
                aside #category-sidebar
                      // Ukryty domyślnie na mobile, chyba że showMobileCategories jest true
                      // Zawsze widoczny i stylowany na desktopie
                      class="w-full md:w-1/4 lg:w-1/5 bg-gray-50 md:p-4 md:border md:border-gray-200 md:rounded-lg md:shadow-sm md:sticky md:top-20 md:self-start transition-all duration-300 ease-in-out"
                      style="max-height: calc(100vh - 100px); overflow-y: auto;"
                      x-show="showMobileCategories || window.innerWidth >= 768" // Pokaż jeśli stan LUB desktop
                      x-transition:enter="transition ease-out duration-200"
                      x-transition:enter-start="opacity-0 max-h-0" // Zaczynamy od zerowej wysokości i opacity
                      x-transition:enter-end="opacity-100 max-h-[500px]" // Rozwijamy do pewnej max wysokości (dostosuj)
                      x-transition:leave="transition ease-in duration-150"
                      x-transition:leave-start="opacity-100 max-h-[500px]"
                      x-transition:leave-end="opacity-0 max-h-0"
                      x-cloak { // Zapobiega mignięciu przed inicjalizacją Alpine

                    // Ten div zapewnia padding wewnątrz aside, który może być schowany przy zwijaniu
                    div class="p-4 md:p-0" {
                        h2 ."text-xl font-semibold mb-4 text-gray-800 hidden md:block" { "Kategorie " (current_gender_display_name) }
                        nav {
                            ul ."space-y-1" {
                                li {
                                    a href="#"
                                       "hx-get"=(format!("/htmx/products?gender={}", current_gender.to_string()))
                                       "hx-target"="#product-listing-area" "hx-swap"="innerHTML"
                                       "hx-push-url"=(format!("/dla/{}", gender_slug))
                                       "hx-indicator"=".product-load-spinner"
                                       "@click"="if (window.innerWidth < 768) showMobileCategories = false" // Zwiń po kliknięciu na mobile
                                       class="block px-3 py-2 rounded-md text-gray-700 hover:bg-pink-50 hover:text-pink-600 transition-colors"
                                       "_"="on htmx:afterSwap remove .font-bold .text-pink-700 from #category-sidebar a add .font-bold .text-pink-700 to me" {
                                        "Wszystkie"
                                    }
                                }
                                @for category_item in &categories {
                                    li {
                                        a href="#"
                                           "hx-get"=(format!("/htmx/products?gender={}&category={}", current_gender.to_string(), category_item.as_url_param()))
                                           "hx-target"="#product-listing-area" "hx-swap"="innerHTML"
                                           "hx-push-url"=(format!("/dla/{}/{}", gender_slug, category_item.to_string().to_lowercase().replace(' ', "-").replace("ł", "l").replace("ó", "o").replace("ż", "z").replace("ą", "a").replace("ę", "e").replace("ć", "c").replace("ń", "n").replace("ś", "s")))
                                           "hx-indicator"=".product-load-spinner"
                                           "@click"="if (window.innerWidth < 768) showMobileCategories = false" // Zwiń po kliknięciu na mobile
                                           class="block px-3 py-2 rounded-md text-gray-700 hover:bg-pink-50 hover:text-pink-600 transition-colors"
                                           "_"="on htmx:afterSwap remove .font-bold .text-pink-700 from #category-sidebar a add .font-bold .text-pink-700 to me" {
                                            (category_item.to_string())
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // --- Główny obszar na listę produktów ---
                section #product-listing-area ."w-full md:w-3/4 lg:w-4/5" { // Usunięto p-0 md:p-0, padding będzie w siatce lub globalnie
                    (render_product_grid_maud(
                        &paginated_response.data,
                        paginated_response.current_page,
                        paginated_response.total_pages,
                        paginated_response.per_page,
                        &filter_query_string_for_initial_grid,
                        &current_listing_params_qs_for_initial_grid,
                    ))
                }
            }
        })
}
