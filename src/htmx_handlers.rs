// src/htmx_handlers.rs
//

#[allow(unused_imports)]
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    response::IntoResponse,
};
use axum_extra::TypedHeader;
#[allow(unused_imports)]
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
    models::{CartDetailsResponse, Category, Order, OrderStatus, Product, ShoppingCart},
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
        query_parts.push(format!("category={}", c.as_url_param()));
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
                                        a href="/dla-niej" "hx-get"="/htmx/dla/niej" "hx-target"="#content" "hx-swap"="innerHTML" "hx-push-url"="/dla-niej" class="text-sm text-blue-600 hover:text-blue-800 hover:underline transition-colors" {
                                            "← Wróć do " (product.gender.to_string())
                                        }
                                    } @else if product.gender == crate::models::ProductGender::Meskie {
                                        a href="/dla-niego" "hx-get"="/htmx/dla/niego" "hx-target"="#content" "hx-swap"="innerHTML" "hx-push-url"="/dla-niego" class="text-sm text-blue-600 hover:text-blue-800 hover:underline transition-colors" {
                                            "← Wróć do " (product.gender.to_string())
                                        }
                                    } @else {
                                        a href="/" "hx-get"="/htmx/products?limit=9" "hx-target"="#content" "hx-swap"="innerHTML" "hx-push-url"="/" class="text-sm text-blue-600 hover:text-blue-800 hover:underline transition-colors" {
                                            "← Wróć na stronę główną"
                                        }
                                    }
                                }
                            } @else {
                                // Fallback dla None
                                @if product.gender == crate::models::ProductGender::Damskie {
                                    a href="/dla-niej" "hx-get"="/htmx/dla/niej" "hx-target"="#content" "hx-swap"="innerHTML" "hx-push-url"="/dla-niej" class="text-sm text-blue-600 hover:text-blue-800 hover:underline transition-colors" {
                                        "← Wróć do " (product.gender.to_string())
                                    }
                                } @else if product.gender == crate::models::ProductGender::Meskie {
                                    a href="/dla-niego" "hx-get"="/htmx/dla/niego" "hx-target"="#content" "hx-swap"="innerHTML" "hx-push-url"="/dla-niego" class="text-sm text-blue-600 hover:text-blue-800 hover:underline transition-colors" {
                                        "← Wróć do " (product.gender.to_string())
                                    }
                                } @else {
                                    a href="/" "hx-get"="/htmx/products?limit=9" "hx-target"="#content" "hx-swap"="innerHTML" "hx-push-url"="/" class="text-sm text-blue-600 hover:text-blue-800 hover:underline transition-colors" {
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
        @for item in &items { // lub &items, zależnie od nazwy zmiennej
            li ."flex py-4 px-4 sm:px-0" {
                // --- Obrazek jako link ---
                a href=(format!("/produkty/{}", item.product.id)) // Fallback URL
                   "hx-get"=(format!("/htmx/produkt/{}", item.product.id)) // Endpoint HTMX
                   "hx-target"="#content"                                 // Cel podmiany
                   "hx-swap"="innerHTML"
                   "hx-push-url"=(format!("/produkty/{}", item.product.id)) // Aktualizacja URL w przeglądarce
                   // Opcjonalnie: wskaźnik ładowania, jeśli masz globalny np. .page-load-spinner
                   // "hx-indicator"=".page-load-spinner"
                   "@click"="if(typeof cartOpen !== 'undefined') cartOpen = false" // Zamknij koszyk (Alpine.js)
                   class="h-20 w-20 flex-shrink-0 overflow-hidden rounded-md border border-gray-200 block group"
                   aria-label={"Zobacz szczegóły produktu " (item.product.name)} {
                    @if !item.product.images.is_empty() {
                        img src=(item.product.images[0]) alt=(item.product.name) class="h-full w-full object-cover object-center group-hover:opacity-85 transition-opacity" loading="lazy";
                    } @else {
                        div ."h-full w-full bg-gray-100 flex items-center justify-center text-xs text-gray-400 group-hover:opacity-85 transition-opacity" { "Brak foto" }
                    }
                }

                div ."ml-4 flex flex-1 flex-col" {
                    div {
                        div ."flex justify-between text-sm font-medium text-gray-800" {
                            h3 ."group" { // Dodajemy 'group' dla efektu hover na linku wewnątrz
                                // --- Nazwa produktu jako link ---
                                a href=(format!("/produkty/{}", item.product.id)) // Fallback URL
                                   "hx-get"=(format!("/htmx/produkt/{}", item.product.id))
                                   "hx-target"="#content"
                                   "hx-swap"="innerHTML"
                                   "hx-push-url"=(format!("/produkty/{}", item.product.id))
                                   // "hx-indicator"=".page-load-spinner"
                                   "@click"="if(typeof cartOpen !== 'undefined') cartOpen = false" // Zamknij koszyk (Alpine.js)
                                  class="hover:text-pink-600 transition-colors group-hover:underline" {
                                    (item.product.name)
                                }
                            }
                            p ."ml-4 whitespace-nowrap" { (format_price_maud(item.product.price)) }
                        }
                        // Można tu dodać np. kategorię, jeśli jest potrzebna w skróconym widoku koszyka
                        // p ."mt-1 text-xs text-gray-500" { (item.product.category.to_string()) }
                    }
                    div ."flex flex-1 items-end justify-between text-xs mt-2" { // Dodano mt-2 dla odstępu
                        div ."flex" {
                            button type="button"
                                "hx-post"=(format!("/htmx/cart/remove/{}", item.product.id))
                                "hx-target"="#cart-content-target" // Celuje w listę itemów w koszyku
                                "hx-swap"="innerHTML"
                                class="text-sm font-medium text-pink-600 px-3 py-1 rounded-md hover:bg-pink-100 hover:text-pink-700 focus:outline-none focus:ring-2 focus:ring-pink-500 focus:ring-opacity-50 transition-all duration-150 ease-in-out" {
                                "Usuń"
                            }
                        }
                    }
                }
            }
        }
    }            // Suma częściowa jest teraz zarządzana przez Alpine.js w index.html na podstawie danych z HX-Trigger
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
        @for item in &cart_details.items { // lub &items, zależnie od nazwy zmiennej
            li ."flex py-4 px-4 sm:px-0" {
                // --- Obrazek jako link ---
                a href=(format!("/produkty/{}", item.product.id)) // Fallback URL
                   "hx-get"=(format!("/htmx/produkt/{}", item.product.id)) // Endpoint HTMX
                   "hx-target"="#content"                                 // Cel podmiany
                   "hx-swap"="innerHTML"
                   "hx-push-url"=(format!("/produkty/{}", item.product.id)) // Aktualizacja URL w przeglądarce
                   // Opcjonalnie: wskaźnik ładowania, jeśli masz globalny np. .page-load-spinner
                   // "hx-indicator"=".page-load-spinner"
                   "@click"="if(typeof cartOpen !== 'undefined') cartOpen = false" // Zamknij koszyk (Alpine.js)
                   class="h-20 w-20 flex-shrink-0 overflow-hidden rounded-md border border-gray-200 block group"
                   aria-label={"Zobacz szczegóły produktu " (item.product.name)} {
                    @if !item.product.images.is_empty() {
                        img src=(item.product.images[0]) alt=(item.product.name) class="h-full w-full object-cover object-center group-hover:opacity-85 transition-opacity" loading="lazy";
                    } @else {
                        div ."h-full w-full bg-gray-100 flex items-center justify-center text-xs text-gray-400 group-hover:opacity-85 transition-opacity" { "Brak foto" }
                    }
                }

                div ."ml-4 flex flex-1 flex-col" {
                    div {
                        div ."flex justify-between text-sm font-medium text-gray-800" {
                            h3 ."group" { // Dodajemy 'group' dla efektu hover na linku wewnątrz
                                // --- Nazwa produktu jako link ---
                                a href=(format!("/produkty/{}", item.product.id)) // Fallback URL
                                   "hx-get"=(format!("/htmx/produkt/{}", item.product.id))
                                   "hx-target"="#content"
                                   "hx-swap"="innerHTML"
                                   "hx-push-url"=(format!("/produkty/{}", item.product.id))
                                   // "hx-indicator"=".page-load-spinner"
                                   "@click"="if(typeof cartOpen !== 'undefined') cartOpen = false" // Zamknij koszyk (Alpine.js)
                                   class="hover:text-pink-600 transition-colors group-hover:underline" {
                                    (item.product.name)
                                }
                            }
                            p ."ml-4 whitespace-nowrap" { (format_price_maud(item.product.price)) }
                        }
                        // Można tu dodać np. kategorię, jeśli jest potrzebna w skróconym widoku koszyka
                        // p ."mt-1 text-xs text-gray-500" { (item.product.category.to_string()) }
                    }
                    div ."flex flex-1 items-end justify-between text-xs mt-2" { // Dodano mt-2 dla odstępu
                        div ."flex" {
                            button type="button"
                                "hx-post"=(format!("/htmx/cart/remove/{}", item.product.id))
                                "hx-target"="#cart-content-target" // Celuje w listę itemów w koszyku
                                "hx-swap"="innerHTML"
                                class="text-sm font-medium text-pink-600 px-3 py-1 rounded-md hover:bg-pink-100 hover:text-pink-700 focus:outline-none focus:ring-2 focus:ring-pink-500 focus:ring-opacity-50 transition-all duration-150 ease-in-out" {
                                "Usuń"
                            }
                        }
                    }
                }
            }
        }
    }        }
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

pub async fn about_us_page_handler() -> Result<Markup, AppError> {
    Ok(html! {
        div ."max-w-4xl mx-auto px-4 sm:px-6 lg:px-8 py-12 sm:py-16" {
            // Baner lub główny nagłówek strony
            div ."text-center mb-12" {
                h1 ."text-4xl sm:text-5xl font-bold tracking-tight text-gray-900" { "Nasza Pasja, Twój Styl" }
                p ."mt-4 text-xl text-gray-600" { "Poznaj historię i filozofię MEG JONI." }
            }

            // Sekcja wprowadzająca
            div ."prose prose-lg lg:prose-xl max-w-none text-gray-700 leading-relaxed space-y-6" {
                // Użycie prose-lg i prose-xl dla lepszej czytelności większych bloków tekstu
                // space-y-6 dla odstępów między paragrafami

                p ."text-xl font-semibold text-pink-600" { // Lekkie wyróżnienie pierwszego zdania
                    "Witaj w świecie MEG JONI!"
                }
                p {
                    "Jesteśmy grupą prawdziwych entuzjastów mody, dla których ubrania to coś znacznie więcej niż tylko okrycie. To forma sztuki, sposób na wyrażenie siebie i opowieść, którą każde z nas pisze na nowo każdego dnia."
                }

                // Możemy dodać zdjęcie zespołu lub inspirujące zdjęcie modowe tutaj, jeśli chcesz
                // Dla przykładu, placeholder na zdjęcie:
                /*
                div ."my-8 rounded-lg shadow-xl overflow-hidden aspect-w-16 aspect-h-9" {
                    img src="/static/images/team_placeholder.jpg" alt="Zespół MEG JONI lub inspiracja modowa" class="object-cover w-full h-full";
                }
                */

                h2 ."text-2xl sm:text-3xl font-semibold text-gray-800 mt-10 mb-4 border-b-2 border-pink-500 pb-2" {
                    "Miłość do Vintage i Zrównoważonego Stylu"
                }
                p {
                    "Naszą największą inspiracją jest moda z duszą – starannie wyszukane perełki vintage i odzież z drugiej ręki, która niesie ze sobą niepowtarzalne historie i ponadczasową jakość. Wierzymy, że moda powinna być zrównoważona, a dawanie ubraniom drugiego życia to najpiękniejszy sposób na dbanie o naszą planetę i podkreślanie własnej indywidualności. Przeszukujemy niezliczone miejsca, aby znaleźć te wyjątkowe egzemplarze, które wniosą do Twojej szafy niepowtarzalny charakter."
                }

                h2 ."text-2xl sm:text-3xl font-semibold text-gray-800 mt-10 mb-4 border-b-2 border-pink-500 pb-2" {
                    "Misja MEG JONI"
                }
                p {
                    "MEG JONI narodziło się z pragnienia dzielenia się tymi odkryciami. Chcemy stworzyć miejsce, gdzie każda i każdy z Was znajdzie coś wyjątkowego – ubrania, które nie tylko świetnie wyglądają, ale też mają charakter i pozwalają wyróżnić się z tłumu. Selekcjonujemy nasze kolekcje z największą starannością, dbając o jakość, unikalność i autentyczny styl."
                }

                // Sekcja z wyróżnionym cytatem lub wartościami
                div ."my-10 p-6 bg-pink-50 rounded-xl shadow-md border-l-4 border-pink-500" {
                    p ."text-lg italic text-pink-700 leading-relaxed" {
                        "„Moda przemija, styl pozostaje. W MEG JONI celebrujemy ten ponadczasowy styl, dając drugie życie wyjątkowym ubraniom.”"
                    }
                }

                h2 ."text-2xl sm:text-3xl font-semibold text-gray-800 mt-10 mb-4 border-b-2 border-pink-500 pb-2" {
                    "Co u nas znajdziesz?"
                }
                p {
                    "W naszych kolekcjach dla Niej i dla Niego znajdziesz ubrania, które opowiadają historie, dodatki z duszą i klasyki, które nigdy nie wychodzą z mody. Dbamy o to, by każdy produkt był dokładnie sprawdzony i opisany, gotowy na nowy rozdział w Twojej garderobie."
                }

                // Zaproszenie
                div ."mt-12 text-center" {
                    p ."text-xl text-gray-700 mb-4" {
                        "Dziękujemy, że jesteś z nami! Rozejrzyj się, zainspiruj i znajdź coś, co idealnie odda Twój styl."
                    }
                    a href="/" "hx-get"="/htmx/products?limit=9" "hx-target"="#content" "hx-swap"="innerHTML" "hx-push-url"="/"
                       class="inline-block bg-pink-600 text-white font-semibold py-3 px-8 rounded-lg shadow-md hover:bg-pink-700 transition-all duration-200 ease-in-out text-lg" {
                        "Odkrywaj nasze kolekcje"
                    }
                }
            }
        }
    })
}

pub async fn privacy_policy_page_handler() -> Result<Markup, AppError> {
    let effective_date = "25 maja 2025";
    let shop_name = "MEG JONI";
    let shop_url = "www.megjoni.com";
    let company_full_name = "MEG JONI Jan Kowalski";
    let company_address = "ul. Modna 1, 00-001 Warszawa";
    let company_nip = "123-456-78-90";
    let company_regon = "123456789";
    let contact_email_privacy = "prywatnosc@megjoni.com";
    let link_do_polityki_cookies = "/htmx/page/polityka-cookies";

    // Definicje tekstów jako zmienne Rusta
    let heading_main_text = format!("Polityka Prywatności {}", shop_name);
    let last_update_text = format!("Ostatnia aktualizacja: {}", effective_date);

    let intro_heading_text = "1. Wprowadzenie";
    let intro_paragraph_text = format!(
        "Witamy w {} (dalej jako \"Sklep\", \"my\", \"nas\"). Cenimy Twoją prywatność i zobowiązujemy się \
        do ochrony Twoich danych osobowych. Niniejsza Polityka Prywatności wyjaśnia, jakie dane osobowe \
        zbieramy, w jaki sposób je wykorzystujemy, udostępniamy i chronimy w związku z korzystaniem \
        z naszego sklepu internetowego dostępnego pod adresem {}.",
        shop_name, shop_url
    );

    let admin_heading_text = "2. Administrator Danych Osobowych";
    let admin_details_text = format!(
        "Administratorem Twoich danych osobowych jest {}, z siedzibą w {}, NIP: {}, REGON: {}.",
        company_full_name, company_address, company_nip, company_regon
    );
    let admin_contact_text = format!(
        "W sprawach dotyczących przetwarzania danych osobowych możesz skontaktować się z nami pod adresem e-mail: {}.",
        contact_email_privacy
    );

    let data_collected_heading_text = "3. Jakie dane zbieramy?";
    let data_collected_intro_text =
        "Podczas korzystania z naszego Sklepu możemy zbierać następujące rodzaje danych:";
    let data_voluntary_text = format!(
        "{} imię i nazwisko, adres e-mail, adres dostawy, numer telefonu, dane do faktury, dane logowania do konta użytkownika, treści wiadomości przesyłanych przez formularz kontaktowy.",
        "Dane podawane dobrowolnie przez Ciebie:"
    );
    let data_automatic_text = format!(
        "{} adres IP, typ i wersja przeglądarki, system operacyjny, odwołujący URL, strony odwiedzane w naszym Sklepie, czas spędzony na stronie, informacje zbierane za pomocą plików cookies i podobnych technologii.",
        "Dane zbierane automatycznie:"
    );

    let purpose_heading_text = "4. W jakim celu przetwarzamy Twoje dane?";
    let purpose_intro_text = "Twoje dane osobowe przetwarzamy w następujących celach:";
    let purposes_list_items = [
        "Realizacji i obsługi zamówień (podstawa prawna: art. 6 ust. 1 lit. b RODO - wykonanie umowy).",
        "Założenia i prowadzenia konta użytkownika w Sklepie (podstawa prawna: art. 6 ust. 1 lit. b RODO).",
        "Komunikacji z Tobą, w tym odpowiedzi na zapytania (podstawa prawna: art. 6 ust. 1 lit. f RODO - nasz prawnie uzasadniony interes).",
        "Rozpatrywania reklamacji i roszczeń (podstawa prawna: art. 6 ust. 1 lit. b, c, f RODO).",
        "Marketingu bezpośredniego naszych produktów i usług, w tym wysyłki newslettera, wyłącznie za Twoją zgodą (podstawa prawna: art. 6 ust. 1 lit. a RODO).",
        "Analizy statystycznej i ulepszania działania Sklepu (podstawa prawna: art. 6 ust. 1 lit. f RODO - nasz prawnie uzasadniony interes).",
        "Wypełnienia obowiązków prawnych ciążących na nas, np. podatkowych (podstawa prawna: art. 6 ust. 1 lit. c RODO).",
    ];

    let sharing_heading_text = "5. Komu udostępniamy Twoje dane?";
    let sharing_intro_text =
        "Twoje dane osobowe mogą być udostępniane następującym kategoriom odbiorców:";
    let shared_with_list_items = [
        "Dostawcom usług płatniczych w celu realizacji płatności.",
        "Firmom kurierskim i pocztowym w celu dostarczenia zamówień.",
        "Dostawcom usług IT (np. hosting, systemy mailingowe), którzy przetwarzają dane w naszym imieniu.",
        "Organom państwowym, jeśli wynika to z obowiązujących przepisów prawa.",
    ];
    let sharing_assurance_text = "Zapewniamy, że wszyscy nasi partnerzy przetwarzają Twoje dane zgodnie z obowiązującymi przepisami o ochronie danych i na podstawie odpowiednich umów powierzenia przetwarzania.";

    let storage_duration_heading_text = "6. Jak długo przechowujemy Twoje dane?";
    let storage_duration_text = "Twoje dane osobowe będą przechowywane przez okres niezbędny do realizacji celów, dla których zostały zebrane, a po tym czasie przez okres wymagany przepisami prawa (np. dla celów podatkowych, przedawnienia roszczeń) lub do momentu wycofania przez Ciebie zgody (jeśli przetwarzanie odbywało się na jej podstawie).";

    let user_rights_heading_text = "7. Twoje prawa";
    let user_rights_intro_text =
        "W związku z przetwarzaniem Twoich danych osobowych przysługują Ci następujące prawa:";
    let user_rights_list_items = [
        "Prawo dostępu do swoich danych.",
        "Prawo do sprostowania (poprawiania) swoich danych.",
        "Prawo do usunięcia danych (tzw. \"prawo do bycia zapomnianym\").", // Użyto standardowych cudzysłowów ASCII
        "Prawo do ograniczenia przetwarzania danych.",
        "Prawo do przenoszenia danych.",
        "Prawo do wniesienia sprzeciwu wobec przetwarzania danych (w szczególności wobec marketingu bezpośredniego).",
        "Prawo do cofnięcia zgody w dowolnym momencie, jeśli przetwarzanie odbywa się na podstawie zgody (cofnięcie zgody nie wpływa na zgodność z prawem przetwarzania, którego dokonano na podstawie zgody przed jej wycofaniem).",
        "Prawo do wniesienia skargi do organu nadzorczego, tj. Prezesa Urzędu Ochrony Danych Osobowych (ul. Stawki 2, 00-193 Warszawa).",
    ];
    let user_rights_contact_text = format!(
        "Aby skorzystać ze swoich praw, skontaktuj się z nami pod adresem e-mail podanym w punkcie 2 ({}) lub listownie.",
        contact_email_privacy
    );

    let cookies_heading_text = "8. Pliki Cookies";
    let cookies_paragraph1_text = format!(
        "Nasz Sklep wykorzystuje pliki cookies (ciasteczka). Są to małe pliki tekstowe przechowywane na Twoim urządzeniu \
        końcowym. Używamy ich m.in. do zapewnienia prawidłowego działania Sklepu, zapamiętywania Twoich preferencji, \
        analizy ruchu oraz w celach marketingowych. Szczegółowe informacje na temat plików cookies oraz możliwości \
        zarządzania nimi znajdziesz w naszej [LINK DO POLITYKI COOKIES - jeśli masz osobną, lub rozwiń ten punkt, np. {}].",
        link_do_polityki_cookies // Przykładowe użycie linku
    );
    let cookies_paragraph2_text =
        "Możesz zarządzać ustawieniami cookies z poziomu swojej przeglądarki internetowej.";

    let security_heading_text = "9. Bezpieczeństwo danych";
    let security_text = "Przykładamy dużą wagę do bezpieczeństwa Twoich danych osobowych. Stosujemy odpowiednie środki techniczne i organizacyjne, aby chronić Twoje dane przed nieuprawnionym dostępem, utratą, zniszczeniem czy modyfikacją.";

    let changes_heading_text = "10. Zmiany w Polityce Prywatności";
    let changes_text = "Zastrzegamy sobie prawo do wprowadzania zmian w niniejszej Polityce Prywatności. Wszelkie zmiany będą publikowane na tej stronie i wchodzą w życie z dniem publikacji. Zachęcamy do regularnego zapoznawania się z treścią Polityki Prywatności.";

    let contact_heading_text = "11. Kontakt";
    let contact_text_final_paragraph = format!(
        // Poprawiono problematyczny string
        "W przypadku pytań dotyczących niniejszej Polityki Prywatności lub przetwarzania Twoich danych osobowych, {} \
        prosimy o kontakt pod adresem e-mail: {} lub listownie na adres naszej siedziby podany w punkcie 2.",
        "", // Pusty string, jeśli nie ma nic do dodania na początku, lub dodaj jakiś tekst.
        contact_email_privacy
    );

    Ok(html! {
        div ."max-w-3xl mx-auto px-4 sm:px-6 lg:px-8 py-12 sm:py-16" {
            div ."text-center mb-10" {
                h1 ."text-3xl sm:text-4xl font-bold tracking-tight text-gray-900" { (heading_main_text) }
                p ."mt-2 text-sm text-gray-500" { (last_update_text) }
            }

            article ."prose prose-lg lg:prose-xl max-w-none text-gray-700 leading-relaxed space-y-6" {

                h2 { (intro_heading_text) }
                p { (intro_paragraph_text) }

                h2 { (admin_heading_text) }
                p { (admin_details_text) }
                p { (admin_contact_text) }


                h2 { (data_collected_heading_text) }
                p { (data_collected_intro_text) }
                ul {
                    li { (PreEscaped(data_voluntary_text.replace("Dane podawane dobrowolnie przez Ciebie:", "<strong>Dane podawane dobrowolnie przez Ciebie:</strong>"))) }
                    li { (PreEscaped(data_automatic_text.replace("Dane zbierane automatycznie:", "<strong>Dane zbierane automatycznie:</strong>"))) }
                }

                h2 { (purpose_heading_text) }
                p { (purpose_intro_text) }
                ul {
                    @for purpose_item in &purposes_list_items {
                        // Zamieniono półpauzy na myślniki
                        li { (purpose_item.replace(" – ", " - ")) }
                    }
                }

                h2 { (sharing_heading_text) }
                p { (sharing_intro_text) }
                ul {
                    @for shared_item in &shared_with_list_items {
                        li { (shared_item) }
                    }
                }
                p { (sharing_assurance_text) }

                h2 { (storage_duration_heading_text) }
                p { (storage_duration_text) }

                h2 { (user_rights_heading_text) }
                p { (user_rights_intro_text) }
                ul {
                    @for right_item in &user_rights_list_items {
                        // Zamieniono cudzysłowy typograficzne
                        li { (right_item.replace("„", "\"").replace("”", "\"")) }
                    }
                }
                p { (user_rights_contact_text) }

                h2 { (cookies_heading_text) }
                p { (cookies_paragraph1_text) }
                p { (cookies_paragraph2_text) }

                h2 { (security_heading_text) }
                p { (security_text) }

                h2 { (changes_heading_text) }
                p { (changes_text) }

                h2 { (contact_heading_text) }
                p { (contact_text_final_paragraph) } // Użycie poprawionego stringa
            }
        }
    })
}

pub async fn terms_of_service_page_handler() -> Result<Markup, AppError> {
    let effective_date = "25 maja 2025";
    let shop_name = "MEG JONI";
    let shop_url = "www.megjoni.com";
    let company_full_name = "Meg Joni Sp. z o.o.";
    let company_address = "ul. Przykładowa 1, 00-001 Miasto";
    let company_nip = "123-456-78-90";
    let company_regon = "123456789";
    let contact_email = "kontakt@megjoni.com";
    let complaint_address = "ul. Przykładowa 1, 00-001 Miasto (Dział Reklamacji)";
    let bank_account_for_returns = "[NUMER KONTA BANKOWEGO DO ZWROTÓW]";

    // --- Definicje tekstów jako zmienne Rusta ---

    let heading_main_text = format!("Regulamin Sklepu Internetowego {}", shop_name);
    let last_update_text = format!("Obowiązuje od: {}", effective_date);

    // §1 Postanowienia ogólne
    let s1_title = "§1 Postanowienia ogólne";
    let s1_p1 = format!(
        "Sklep internetowy działający pod adresem {} (zwany dalej \"Sklepem\") prowadzony jest przez {}, \
        z siedzibą w {}, NIP: {}, REGON: {} (zwany dalej \"Sprzedawcą\").",
        shop_url, company_full_name, company_address, company_nip, company_regon
    );
    let s1_p2 = "Niniejszy regulamin (zwany dalej \"Regulaminem\") określa zasady i warunki korzystania ze Sklepu, \
        składania zamówień na produkty dostępne w Sklepie, dostarczania zamówionych produktów Klientowi, \
        uiszczania przez Klienta ceny sprzedaży produktów, uprawnienia Klienta do odstąpienia od umowy \
        oraz zasady składania i rozpatrywania reklamacji.";
    let s1_p3_intro = "Do korzystania ze Sklepu, w tym przeglądania asortymentu Sklepu oraz składania zamówień na Produkty, niezbędne jest:";
    let s1_p3_reqs = [
        "Urządzenie końcowe (np. komputer, tablet, smartfon) z dostępem do sieci Internet i przeglądarką internetową typu np. Chrome, Firefox, Safari, Edge.",
        "Aktywne konto poczty elektronicznej (e-mail).",
        "Włączona obsługa plików cookies oraz JavaScript w przeglądarce internetowej.",
    ];
    let s1_p4 = "Klient zobowiązany jest do korzystania ze Sklepu w sposób zgodny z prawem i dobrymi obyczajami, \
        mając na uwadze poszanowanie dóbr osobistych oraz praw autorskich i własności intelektualnej Sprzedawcy \
        oraz osób trzecich.";
    let s1_p5 = "Klienta obowiązuje zakaz dostarczania treści o charakterze bezprawnym.";
    let s1_p6_intro = "Definicje użyte w Regulaminie:";
    let s1_p6_defs = [
        ("Sprzedawca", "podmiot wskazany w ust. 1."),
        (
            "Klient",
            "osoba fizyczna posiadająca pełną zdolność do czynności prawnych, osoba prawna lub jednostka organizacyjna nieposiadająca osobowości prawnej, której ustawa przyznaje zdolność prawną, dokonująca Zamówienia w Sklepie.",
        ),
        (
            "Konsument",
            "Klient będący osobą fizyczną dokonujący ze Sprzedawcą czynności prawnej niezwiązanej bezpośrednio z jej działalnością gospodarczą lub zawodową.",
        ),
        (
            "Produkt",
            "dostępna w Sklepie rzecz ruchoma będąca przedmiotem Umowy Sprzedaży. Produkty w Sklepie są towarami używanymi (vintage), chyba że wyraźnie wskazano inaczej. Ich stan jest opisany na karcie produktu.",
        ),
        (
            "Umowa Sprzedaży",
            "umowa sprzedaży Produktu zawierana albo zawarta między Klientem a Sprzedawcą za pośrednictwem Sklepu internetowego.",
        ),
        (
            "Zamówienie",
            "oświadczenie woli Klienta składane za pomocą Formularza Zamówienia i zmierzające bezpośrednio do zawarcia Umowy Sprzedaży Produktu ze Sprzedawcą.",
        ),
    ];

    // §2 Składanie Zamówień
    let s2_title = "§2 Składanie Zamówień";
    let s2_p1 = "Informacje o Produktach podane na stronach internetowych Sklepu, w szczególności ich opisy, \
        parametry techniczne i użytkowe oraz ceny, stanowią zaproszenie do zawarcia umowy, w rozumieniu art. 71 Kodeksu Cywilnego.";
    let s2_p2 = "Wszystkie Produkty dostępne w Sklepie są używane, pochodzą z \"drugiej ręki\" (są towarem typu vintage), \
        chyba że w opisie Produktu wyraźnie zaznaczono inaczej. Sprzedawca dokłada wszelkich starań, aby stan Produktów \
        był dokładnie opisany i sfotografowany. Klient akceptuje, że Produkty mogą nosić ślady normalnego użytkowania, \
        które nie stanowią wady produktu, jeśli są zgodne z opisem.";
    let s2_p3_intro =
        "W celu złożenia Zamówienia Klient powinien wykonać co najmniej następujące czynności:";
    let s2_p3_steps = [
        "Dodać wybrany Produkt (lub Produkty) do koszyka.",
        "Podać dane niezbędne do realizacji Zamówienia, takie jak: imię i nazwisko, adres dostawy, adres e-mail, numer telefonu, a w przypadku firm dodatkowo NIP i nazwę firmy.",
        "Wybrać jeden z dostępnych sposobów dostawy.",
        "Wybrać jeden z dostępnych sposobów płatności.",
        "Zapoznać się z Regulaminem i zaakceptować jego postanowienia.",
        "Kliknąć przycisk \"Zamawiam i płacę\" lub inny równoznaczny.",
    ];
    let s2_p4 = "Złożenie Zamówienia stanowi złożenie Sprzedawcy przez Klienta oferty zawarcia Umowy Sprzedaży Produktów będących przedmiotem Zamówienia.";
    let s2_p5 = "Po złożeniu Zamówienia, Klient otrzymuje wiadomość e-mail zawierającą ostateczne potwierdzenie wszystkich \
        istotnych elementów Zamówienia. Z chwilą otrzymania przez Klienta powyższej wiadomości e-mail zostaje zawarta \
        Umowa Sprzedaży między Klientem a Sprzedawcą.";

    // §3 Ceny i Metody Płatności
    let s3_title = "§3 Ceny i Metody Płatności";
    let s3_p1 = "Ceny Produktów podawane są w polskich złotych (PLN) i są cenami brutto (zawierają podatek VAT, jeśli dotyczy).";
    let s3_p2 = "Ceny Produktów nie zawierają kosztów dostawy. Koszty dostawy są wskazywane w trakcie składania Zamówienia \
        i są doliczane do całkowitej wartości Zamówienia.";
    let s3_p3_intro =
        "Klient może wybrać następujące metody płatności: [LISTA METOD PŁATNOŚCI, np.:]";
    let s3_p3_methods = [
        "Przelew tradycyjny na konto bankowe Sprzedawcy.",
        "Płatność za pośrednictwem systemu płatności online [NAZWA SYSTEMU PŁATNOŚCI np. Przelewy24, PayU, Stripe].",
        "[Inne dostępne metody].",
    ];
    let s3_p4 = "Klient zobowiązany jest do dokonania płatności w terminie [np. 7] dni kalendarzowych od dnia zawarcia \
        Umowy Sprzedaży. W przypadku braku płatności we wskazanym terminie, Zamówienie może zostać anulowane.";

    // §4 Dostawa
    let s4_title = "§4 Dostawa";
    let s4_p1 = "Zamówione Produkty są dostarczane na terytorium Rzeczypospolitej Polskiej. W przypadku chęci zamówienia \
        dostawy poza terytorium Polski, prosimy o indywidualny kontakt.";
    let s4_p2_intro = "Dostawa Produktów odbywa się za pośrednictwem [LISTA DOSTAWCÓW, np.:]";
    let s4_p2_methods = ["Firmy kurierskiej [Nazwa firmy].", "Paczkomatów InPost."];
    let s4_p3 = "Termin realizacji Zamówienia (przygotowanie do wysyłki) wynosi zazwyczaj [np. 1-3] dni robocze od dnia \
        zaksięgowania wpłaty na koncie Sprzedawcy lub od dnia potwierdzenia Zamówienia w przypadku wyboru płatności \
        za pobraniem (jeśli dostępna).";
    let s4_p4 = "Czas dostawy przez przewoźnika zależy od wybranej metody dostawy i wynosi zazwyczaj [np. 1-2] dni robocze.";

    // §5 Prawo odstąpienia od umowy
    let s5_title = "§5 Prawo odstąpienia od umowy (dotyczy Konsumentów)";
    let s5_p1 = "Konsument, który zawarł umowę na odległość, może w terminie 14 dni odstąpić od niej bez podawania \
        przyczyny i bez ponoszenia kosztów, z wyjątkiem kosztów określonych w ustawie o prawach konsumenta.";
    let s5_p2 = "Bieg terminu do odstąpienia od umowy rozpoczyna się od objęcia Produktu w posiadanie przez Konsumenta \
        lub wskazaną przez niego osobę trzecią inną niż przewoźnik.";
    let s5_p3_text = format!(
        "Konsument może odstąpić od umowy, składając Sprzedawcy oświadczenie o odstąpieniu od umowy. Oświadczenie można \
        złożyć na formularzu, którego wzór stanowi załącznik nr 2 do Ustawy o Prawach Konsumenta, lub w innej formie \
        pisemnej, bądź drogą elektroniczną na adres e-mail: {}.",
        contact_email
    );
    let s5_p3_form_intro = "Przykładowy wzór formularza odstąpienia od umowy (nieobowiązkowy):";
    let s5_p3_form_content = format!(
        "Miejscowość, data\n\n\
        Imię i nazwisko konsumenta\n\
        Adres konsumenta\n\n\
        {}\n\
        {}\n\n\
        OŚWIADCZENIE O ODSTĄPIENIU OD UMOWY ZAWARTEJ NA ODLEGŁOŚĆ\n\n\
        Oświadczam, że zgodnie z art. 27 ustawy z dnia 30 maja 2014 r. o prawach konsumenta (Dz. U. 2014 poz. 827 ze zm.) \
        odstępuję od umowy sprzedaży następujących rzeczy: [nazwa produktu/produktów], numer zamówienia [numer zamówienia], \
        zawartej dnia [data zawarcia umowy], odebranej dnia [data odbioru produktu].\n\n\
        Proszę o zwrot kwoty [kwota] zł na rachunek bankowy numer: [numer rachunku bankowego, np. {}].\n\n\
        Podpis konsumenta (tylko jeżeli formularz jest przesyłany w wersji papierowej)",
        company_full_name, company_address, bank_account_for_returns
    );
    let s5_p4 = "Konsument ma obowiązek zwrócić Produkt Sprzedawcy lub przekazać go osobie upoważnionej przez Sprzedawcę \
        do odbioru niezwłocznie, jednak nie później niż 14 dni od dnia, w którym odstąpił od umowy. Do zachowania \
        terminu wystarczy odesłanie Produktu przed jego upływem. Konsument ponosi bezpośrednie koszty zwrotu Produktu.";
    let s5_p5 = format!(
        "Produkt należy zwrócić na adres: {} (lub adres siedziby, jeśli taki sam).",
        complaint_address
    );
    let s5_p6 = "Sprzedawca ma obowiązek niezwłocznie, nie później niż w terminie 14 dni od dnia otrzymania oświadczenia \
        Konsumenta o odstąpieniu od umowy, zwrócić Konsumentowi wszystkie dokonane przez niego płatności, w tym koszty \
        dostarczenia Produktu (z wyjątkiem dodatkowych kosztów wynikających z wybranego przez Konsumenta sposobu \
        dostarczenia innego niż najtańszy zwykły sposób dostarczenia oferowany przez Sprzedawcę).";
    let s5_p7 = "Sprzedawca dokonuje zwrotu płatności przy użyciu takiego samego sposobu płatności, jakiego użył Konsument, \
        chyba że Konsument wyraźnie zgodził się na inny sposób zwrotu, który nie wiąże się dla niego z żadnymi kosztami. \
        Sprzedawca może wstrzymać się ze zwrotem płatności otrzymanych od Konsumenta do chwili otrzymania Produktu z \
        powrotem lub dostarczenia przez Konsumenta dowodu jego odesłania, w zależności od tego, które zdarzenie nastąpi wcześniej.";
    let s5_p8 = "Konsument ponosi odpowiedzialność za zmniejszenie wartości Produktu będące wynikiem korzystania z niego \
        w sposób wykraczający poza konieczny do stwierdzenia charakteru, cech i funkcjonowania Produktu.";

    // §6 Reklamacje
    let s6_title = "§6 Reklamacje";
    let s6_p1 = "Sprzedawca jest zobowiązany dostarczyć Klientowi Produkt wolny od wad fizycznych i prawnych (rękojmia), \
        z uwzględnieniem, że oferowane Produkty są towarami używanymi, a ich stan (w tym ewentualne ślady użytkowania \
        niebędące wadami) jest opisany indywidualnie dla każdego Produktu.";
    let s6_p2 = format!(
        "Reklamację można złożyć pisemnie na adres: {} lub drogą elektroniczną na adres e-mail: {}.",
        complaint_address, contact_email
    );
    let s6_p3 = "Zaleca się, aby zgłoszenie reklamacyjne zawierało co najmniej: imię i nazwisko Klienta, adres do korespondencji, \
        adres e-mail, datę nabycia towaru, rodzaj reklamowanego towaru, dokładny opis wady oraz datę jej stwierdzenia, \
        żądanie Klienta, a także preferowany przez Klienta sposób poinformowania o sposobie rozpatrzenia reklamacji. \
        Dołączenie dowodu zakupu może przyspieszyć proces.";
    let s6_p4 = "Sprzedawca rozpatrzy reklamację w terminie 14 dni od dnia jej otrzymania i poinformuje Klienta o sposobie jej załatwienia.";
    let s6_p5 = "W przypadku uznania reklamacji, Produkt wadliwy zostanie naprawiony lub wymieniony na inny, wolny od wad. \
        Jeśli naprawa lub wymiana okażą się niemożliwe lub wymagałyby nadmiernych kosztów, Klient może żądać stosownego \
        obniżenia ceny albo odstąpić od umowy (o ile wada jest istotna). Zwrot środków nastąpi na wskazany przez Klienta \
        numer konta bankowego.";

    // §7 Ochrona Danych Osobowych
    let s7_title = "§7 Ochrona Danych Osobowych";
    let s7_p1 = format!(
        // Dodaj link do Polityki Prywatności
        "Administratorem danych osobowych Klientów zbieranych za pośrednictwem Sklepu internetowego jest Sprzedawca. \
        Szczegółowe informacje dotyczące przetwarzania danych osobowych oraz praw przysługujących Klientom znajdują się \
        w Polityce Prywatności dostępnej na stronie Sklepu pod adresem: {}/htmx/page/polityka-prywatnosci.", // Użyj dynamicznego linku lub stałego
        shop_url // Lub bezpośrednio "/htmx/page/polityka-prywatnosci", jeśli URL jest względny
    );

    // §8 Postanowienia końcowe
    let s8_title = "§8 Postanowienia końcowe";
    let s8_p1 = "W sprawach nieuregulowanych w niniejszym Regulaminie mają zastosowanie powszechnie obowiązujące przepisy \
        prawa polskiego, w szczególności Kodeksu cywilnego oraz ustawy o prawach konsumenta.";
    let s8_p2 = "Sprzedawca zastrzega sobie prawo do dokonywania zmian Regulaminu z ważnych przyczyn, np. zmiany przepisów prawa, \
        zmiany sposobów płatności i dostaw - w zakresie, w jakim te zmiany wpływają na realizację postanowień niniejszego Regulaminu. \
        O każdej zmianie Sprzedawca poinformuje Klienta z co najmniej 7-dniowym wyprzedzeniem, publikując zmieniony Regulamin \
        na stronie Sklepu. Zamówienia złożone przed datą wejścia w życie zmian Regulaminu są realizowane na podstawie \
        zapisów obowiązujących w dniu złożenia zamówienia.";
    let s8_p3 = "Ewentualne spory powstałe pomiędzy Sprzedawcą a Klientem będącym Konsumentem zostają poddane sądom \
        właściwym zgodnie z postanowieniami właściwych przepisów Kodeksu postępowania cywilnego.";
    let s8_p4 = "Konsument ma możliwość skorzystania z pozasądowych sposobów rozpatrywania reklamacji i dochodzenia roszczeń. \
        Szczegółowe informacje dotyczące możliwości skorzystania przez Konsumenta z pozasądowych sposobów rozpatrywania \
        reklamacji i dochodzenia roszczeń oraz zasady dostępu do tych procedur dostępne są w siedzibach oraz na stronach \
        internetowych powiatowych (miejskich) rzeczników konsumentów, organizacji społecznych, do których zadań statutowych \
        należy ochrona konsumentów, Wojewódzkich Inspektoratów Inspekcji Handlowej oraz pod następującymi adresami \
        internetowymi Urzędu Ochrony Konkurencji i Konsumentów: [wstaw odpowiednie linki do UOKiK, platformy ODR itp.].";
    let s8_p5 = format!("Regulamin wchodzi w życie z dniem {}.", effective_date);

    Ok(html! {
        div ."max-w-3xl mx-auto px-4 sm:px-6 lg:px-8 py-12 sm:py-16" {
            div ."text-center mb-10" {
                h1 ."text-3xl sm:text-4xl font-bold tracking-tight text-gray-900" { (heading_main_text) }
                p ."mt-2 text-sm text-gray-500" { (last_update_text) }
            }

            article ."prose prose-lg lg:prose-xl max-w-none text-gray-700 leading-relaxed space-y-6" {

                h2 { (s1_title) }
                ol ."list-decimal list-inside space-y-2" {
                    li { (s1_p1) }
                    li { (s1_p2) }
                    li { (s1_p3_intro)
                        ul ."list-disc list-inside pl-6 space-y-1 mt-1" {
                            @for req_item in &s1_p3_reqs {
                                li { (req_item) }
                            }
                        }
                    }
                    li { (s1_p4) }
                    li { (s1_p5) }
                    li { (s1_p6_intro)
                        ul ."list-disc list-inside pl-6 space-y-1 mt-1" {
                            @for (term, def) in &s1_p6_defs {
                                li { strong {(term)} " - " (def) }
                            }
                        }
                    }
                }

                h2 { (s2_title) }
                ol ."list-decimal list-inside space-y-2" {
                    li { (s2_p1) }
                    li { (s2_p2) }
                    li { (s2_p3_intro)
                        ul ."list-disc list-inside pl-6 space-y-1 mt-1" {
                            @for step_item in &s2_p3_steps {
                                li { (step_item) }
                            }
                        }
                    }
                    li { (s2_p4) }
                    li { (s2_p5) }
                }

                h2 { (s3_title) }
                ol ."list-decimal list-inside space-y-2" {
                    li { (s3_p1) }
                    li { (s3_p2) }
                    li { (s3_p3_intro)
                        ul ."list-disc list-inside pl-6 space-y-1 mt-1" {
                            @for method_item in &s3_p3_methods {
                                li { (method_item) }
                            }
                        }
                    }
                    li { (s3_p4) }
                }

                h2 { (s4_title) }
                ol ."list-decimal list-inside space-y-2" {
                    li { (s4_p1) }
                    li { (s4_p2_intro)
                        ul ."list-disc list-inside pl-6 space-y-1 mt-1" {
                            @for method_item in &s4_p2_methods {
                                li { (method_item) }
                            }
                        }
                    }
                    li { (s4_p3) }
                    li { (s4_p4) }
                }

                h2 { (s5_title) }
                ol ."list-decimal list-inside space-y-2" {
                    li { (s5_p1) }
                    li { (s5_p2) }
                    li {
                        (s5_p3_text)
                        br;
                        (s5_p3_form_intro)
                        pre ."bg-gray-100 p-3 rounded text-sm whitespace-pre-wrap mt-2" {
                            (s5_p3_form_content)
                        }
                    }
                    li { (s5_p4) }
                    li { (s5_p5) }
                    li { (s5_p6) }
                    li { (s5_p7) }
                    li { (s5_p8) }
                }

                h2 { (s6_title) }
                ol ."list-decimal list-inside space-y-2" {
                    li { (s6_p1) }
                    li { (s6_p2) }
                    li { (s6_p3) }
                    li { (s6_p4) }
                    li { (s6_p5) }
                }

                h2 { (s7_title) }
                p {
                    (PreEscaped(s7_p1.replace("[LINK DO POLITYKI PRYWATNOŚCI]", &format!("<a href=\"/htmx/page/polityka-prywatnosci\" class=\"text-pink-600 hover:underline\">{}</a>", "Polityce Prywatności"))))
                }

                h2 { (s8_title) }
                ol ."list-decimal list-inside space-y-2" {
                    li { (s8_p1) }
                    li { (s8_p2) }
                    li { (s8_p3) }
                    li { (s8_p4) } // Pamiętaj o uzupełnieniu linków w tej zmiennej
                    li { (s8_p5) }
                }
            }
        }
    })
}

pub async fn contact_page_handler() -> Result<Markup, AppError> {
    // Dane kontaktowe - UZUPEŁNIJ WŁASNYMI DANYMI!
    let shop_name = "MEG JONI";
    let contact_email = "kontakt@megjoni.com";
    let contact_phone = Some("+48 123 456 789");
    let company_full_name = "MEG JONI Jan Kowalski";
    let company_address_line1 = "ul. Modna 1";
    let company_address_line2 = "00-001 Warszawa";
    // Możesz dodać linki do mediów społecznościowych
    let social_facebook_url = Some("https://www.facebook.com/megjoni"); // Opcjonalnie
    let social_instagram_url = Some("https://www.instagram.com/meg.joni"); // Opcjonalnie

    // --- Definicje tekstów jako zmienne Rusta ---
    let heading_main_text = "Skontaktuj się z nami";
    let intro_text = format!(
        "Masz pytania dotyczące naszych produktów, zamówienia, a może chcesz po prostu porozmawiać o modzie vintage? \
        Jesteśmy tutaj, aby Ci pomóc! W {} cenimy każdego klienta i staramy się odpowiadać na wszystkie wiadomości \
        tak szybko, jak to tylko możliwe.",
        shop_name
    );

    let email_heading_text = "Napisz do nas";
    let email_description_text =
        format!("Najlepszym sposobem na kontakt jest wysłanie wiadomości e-mail na adres:");

    let phone_heading_text = "Zadzwoń do nas";
    let phone_description_text = if contact_phone.is_some() {
        "Jeśli wolisz rozmowę telefoniczną, jesteśmy dostępni pod numerem:"
    } else {
        "" // Pusty, jeśli nie ma telefonu
    };
    let phone_hours_text = "Pn. - Pt. w godzinach 9:00 - 17:00"; // Przykładowe godziny

    let address_heading_text = "Adres korespondencyjny";
    // let address_note_text = "(Uwaga: nie prowadzimy sprzedaży stacjonarnej pod tym adresem)"; // Jeśli dotyczy

    let social_media_heading_text = "Znajdź nas w sieci";

    let response_time_text =
        "Staramy się odpowiadać na wszystkie zapytania w ciągu 24 godzin w dni robocze.";

    Ok(html! {
        div ."max-w-3xl mx-auto px-4 sm:px-6 lg:px-8 py-12 sm:py-16" {
            div ."text-center mb-12" {
                h1 ."text-4xl sm:text-5xl font-bold tracking-tight text-gray-900" { (heading_main_text) }
                p ."mt-4 text-lg text-gray-600" { (intro_text) }
            }

            div ."space-y-10" {
                // Sekcja Email
                section ."p-6 bg-white rounded-lg shadow-lg border border-gray-200" {
                    h2 ."text-2xl font-semibold text-pink-600 mb-3" { (email_heading_text) }
                    p ."text-gray-700 mb-2" { (email_description_text) }
                    a href=(format!("mailto:{}", contact_email)) class="text-lg text-gray-900 font-medium hover:underline break-all" { (contact_email) }
                }

                // Sekcja Telefon (opcjonalna)
                @if let Some(phone) = contact_phone {
                    section ."p-6 bg-white rounded-lg shadow-lg border border-gray-200" {
                        h2 ."text-2xl font-semibold text-pink-600 mb-3" { (phone_heading_text) }
                        @if !phone_description_text.is_empty() {
                            p ."text-gray-700 mb-2" { (phone_description_text) }
                        }
                        a href=(format!("tel:{}", phone.replace(" ", ""))) class="text-lg text-gray-900 font-medium hover:underline" { (phone) }
                        p ."text-sm text-gray-500 mt-1" { (phone_hours_text) }
                    }
                }

                // Sekcja Adres Korespondencyjny
                section ."p-6 bg-white rounded-lg shadow-lg border border-gray-200" {
                    h2 ."text-2xl font-semibold text-pink-600 mb-3" { (address_heading_text) }
                    p ."text-gray-700 leading-relaxed" {
                        (company_full_name) br;
                        (company_address_line1) br;
                        (company_address_line2)
                    }
                    // @if let Some(note) = address_note_text {
                    //     p ."text-sm text-gray-500 mt-2" { (note) }
                    // }
                }

                // Sekcja Media Społecznościowe (opcjonalna)
                @if social_facebook_url.is_some() || social_instagram_url.is_some() {
                    section ."p-6 bg-white rounded-lg shadow-lg border border-gray-200" {
                        h2 ."text-2xl font-semibold text-pink-600 mb-4" { (social_media_heading_text) }
                        div ."flex space-x-6" {
                            @if let Some(fb_url) = social_facebook_url {
                                a href=(fb_url) target="_blank" rel="noopener noreferrer" class="text-gray-600 hover:text-blue-600 transition-colors" {
                                    // Prosty tekst lub SVG ikona
                                    span class="text-lg font-medium" {"Facebook"}
                                    // Dla SVG np.:
                                    // svg."w-8 h-8" fill="currentColor" viewBox="0 0 24 24" { path d="..." /}
                                }
                            }
                            @if let Some(ig_url) = social_instagram_url {
                                a href=(ig_url) target="_blank" rel="noopener noreferrer" class="text-gray-600 hover:text-pink-500 transition-colors" {
                                    span class="text-lg font-medium" {"Instagram"}
                                }
                            }
                        }
                    }
                }

                // Czas odpowiedzi
                div ."text-center mt-10 pt-6 border-t border-gray-200" {
                    p ."text-md text-gray-600" { (response_time_text) }
                }
            }
        }
    })
}

#[derive(Debug)]
struct FaqItem {
    question: String,
    answer: String,
}

pub async fn faq_page_handler() -> Result<Markup, AppError> {
    let faq_items = vec![
        FaqItem {
            question: "Jakie są dostępne metody płatności?".to_string(),
            answer: "W naszym sklepie MEG JONI akceptujemy następujące metody płatności: szybkie przelewy online (Przelewy24, BLIK) oraz przelew tradycyjny. Wszystkie transakcje są bezpieczne i szyfrowane.".to_string(),
        },
        FaqItem {
            question: "Jaki jest czas realizacji zamówienia?".to_string(),
            answer: "Standardowo, zamówienia przygotowujemy do wysyłki w ciągu 1-2 dni roboczych od momentu zaksięgowania wpłaty. Czas dostawy przez przewoźnika to zazwyczaj dodatkowe 1-2 dni robocze.".to_string(),
        },
        FaqItem {
            question: "Jakie są koszty i opcje dostawy?".to_string(),
            answer: "Oferujemy dostawę za pośrednictwem Paczkomatów InPost oraz Poczta Polska. Koszt dostawy jest widoczny podczas składania zamówienia i zależy od wybranej opcji. Dla zamówień powyżej [np. 300 zł] dostawa jest darmowa!".to_string(),
        },
        FaqItem {
            question: "Czy wysyłacie za granicę?".to_string(),
            answer: "Obecnie realizujemy wysyłki wyłącznie na terenie Polski. Pracujemy nad rozszerzeniem naszej oferty o wysyłki międzynarodowe.".to_string(),
        },
        FaqItem {
            question: "W jakim stanie są oferowane ubrania?".to_string(),
            answer: "W MEG JONI specjalizujemy się w odzieży vintage i używanej w doskonałym lub bardzo dobrym stanie. Każdy produkt jest starannie sprawdzany, a jego stan (wraz z ewentualnymi minimalnymi śladami użytkowania, które dodają charakteru) jest dokładnie opisany na karcie produktu. Stawiamy na jakość i unikatowość.".to_string(),
        },
        FaqItem {
            question: "Jak dbać o odzież vintage?".to_string(),
            answer: "Pielęgnacja odzieży vintage zależy od materiału. Zawsze sprawdzaj metki, jeśli są dostępne. Generalnie zalecamy delikatne pranie ręczne lub w niskich temperaturach, a dla szczególnie cennych materiałów (jak jedwab czy wełna) czyszczenie chemiczne. Unikaj suszenia w suszarce bębnowej.".to_string(),
        },
        FaqItem {
            question: "Czy produkty są unikatowe?".to_string(),
            answer: "Tak, większość naszej oferty to pojedyncze, unikatowe egzemplarze. To właśnie czyni zakupy w MEG JONI wyjątkowym doświadczeniem - masz szansę zdobyć coś, czego nie będzie miał nikt inny!".to_string(),
        },
        FaqItem {
            question: "Czy mogę zwrócić zakupiony produkt?".to_string(),
            answer: "Oczywiście. Masz 14 dni na zwrot towaru bez podania przyczyny od momentu otrzymania przesyłki. Produkt musi być w stanie nienaruszonym, z oryginalnymi metkami (jeśli były). Szczegóły procedury zwrotu znajdziesz w naszym Regulaminie Sklepu.".to_string(),
        },
        FaqItem {
            question: "Jak złożyć reklamację?".to_string(),
            answer: "Jeśli otrzymany produkt posiada wadę, która nie była opisana, skontaktuj się z nami mailowo, dołączając zdjęcia i opis problemu. Każdą reklamację rozpatrujemy indywidualnie. Więcej informacji znajdziesz w Regulaminie Sklepu.".to_string(),
        },
    ];

    Ok(html! {
        div ."max-w-3xl mx-auto px-4 sm:px-6 lg:px-8 py-12 sm:py-16" {
            div ."text-center mb-12" {
                h1 ."text-4xl sm:text-5xl font-bold tracking-tight text-gray-900" { "Najczęściej Zadawane Pytania (FAQ)" }
                p ."mt-3 text-lg text-gray-600" { "Masz pytanie? Sprawdź, czy nie ma tutaj odpowiedzi!" }
            }

            div ."space-y-6" { // Kontener na wszystkie pytania i odpowiedzi
                @for (index, item) in faq_items.iter().enumerate() {
                    div ."bg-white rounded-lg shadow-md border border-gray-200 overflow-hidden"
                        "x-data"=(format!("{{ open: {} }}", if index == 0 { "true" } else { "false" })) // Pierwsze pytanie domyślnie otwarte
                        {
                        // Pytanie - klikalny nagłówek
                        h3 ."cursor-pointer p-5 sm:p-6 border-b border-gray-200 hover:bg-gray-50 transition-colors duration-150"
                           "@click"="open = !open"
                           class="flex justify-between items-center w-full" {
                            span ."text-lg font-semibold text-gray-800" { (item.question) }
                            span ."text-pink-500" { // Kontener na ikonkę
                                svg ."w-6 h-6 transform transition-transform duration-200 ease-in-out"
                                    "x-bind:class"="open ? 'rotate-180' : ''" // Obrót ikonki
                                    fill="none" stroke="currentColor" "viewBox"="0 0 24 24" "xmlns"="http://www.w3.org/2000/svg" {
                                    path "stroke-linecap"="round" "stroke-linejoin"="round" "stroke-width"="2" d="M19 9l-7 7-7-7";
                                }
                            }
                        }
                        // Odpowiedź - rozwijana sekcja
                        div ."p-5 sm:p-6 text-gray-700 leading-relaxed prose max-w-none" // prose dla formatowania tekstu
                            "x-show"="open"
                            "x-cloak"
                            "x-transition:enter"="transition ease-out duration-300"
                            "x-transition:enter-start"="opacity-0 max-h-0"
                            "x-transition:enter-end"="opacity-100 max-h-screen"
                            "x-transition:leave"="transition ease-in duration-200"
                            "x-transition:leave-start"="opacity-100 max-h-screen"
                            "x-transition:leave-end"="opacity-0 max-h-0"
                            style="overflow: hidden;" {

                            @for line in item.answer.lines() {
                                (line) br;
                            }
                        }
                    }
                }
        }
            }
    })
}

pub async fn shipping_returns_page_handler() -> Result<Markup, AppError> {
    let shop_name = "MEG JONI";
    let processing_time = "1-2 dni robocze";
    let delivery_time = "1-2 dni robocze";
    let free_shipping_threshold = "300 zł";
    let contact_email_returns = "zwroty@megjoni.com";
    let return_address_line1 = "MEG JONI - Zwroty";
    let return_address_line2 = "ul. Magazynowa 5";
    let return_address_line3 = "00-002 Miasto";
    let link_to_terms = "/htmx/page/regulamin";

    let page_title = "Wysyłka i Zwroty";
    let page_subtitle = format!(
        "Wszystko, co musisz wiedzieć o dostawie i zwrotach w {}",
        shop_name
    );

    let shipping_section_title = "Informacje o Wysyłce";
    let shipping_area = "Realizujemy wysyłki na terenie całej Polski.".to_string();
    let shipping_carriers_intro = "Korzystamy z usług zaufanych partnerów logistycznych, aby Twoje zamówienie dotarło bezpiecznie i na czas. Dostępne opcje to:".to_string();
    let shipping_carriers_list = ["Paczkomaty InPost 24/7".to_string()];
    let shipping_costs_text = format!(
        "Koszty wysyłki są obliczane automatycznie podczas składania zamówienia i zależą od wybranej metody dostawy \
        oraz wagi/gabarytów paczki. Dokładny koszt zobaczysz przed finalizacją zakupu. \
        Pamiętaj, że dla wszystkich zamówień powyżej {} dostawa jest całkowicie darmowa!",
        free_shipping_threshold
    );
    let processing_time_text = format!(
        "Staramy się, aby każde zamówienie zostało przygotowane i wysłane jak najszybciej. \
        Standardowy czas realizacji (przygotowanie paczki do nadania) wynosi {}.",
        processing_time
    );
    let delivery_time_text = format!(
        "Po nadaniu przesyłki, przewidywany czas dostawy przez naszych partnerów logistycznych to zwykle {}.",
        delivery_time
    );
    let tracking_text =
        "Gdy tylko Twoje zamówienie zostanie wysłane, otrzymasz od nas wiadomość e-mail, bądź poinformujemy Cie na komunikatorze WhatsApp/Messenger/Instagram".to_string();
    let packaging_text = "Każde vintage cudo pakujemy z najwyższą starannością, używając (tam gdzie to możliwe) \
        materiałów przyjaznych środowisku, aby Twoje nowe nabytki dotarły do Ciebie w nienaruszonym stanie.".to_string();

    let returns_section_title = "Zwroty i Odstąpienie od Umowy";
    let right_to_return_text = format!(
        "Rozumiemy, że czasem coś może nie pasować idealnie. Zgodnie z obowiązującym prawem, jako Konsument masz \
        14 dni kalendarzowych na odstąpienie od umowy sprzedaży (zwrot towaru) bez podawania przyczyny, licząc od dnia, \
        w którym otrzymałeś/aś przesyłkę. Pełne informacje na ten temat znajdziesz w naszym Regulaminie Sklepu (link poniżej)."
    );
    let return_conditions_heading = "Warunki Zwrotu:";
    let return_conditions_list = [
        "Produkt nie może nosić żadnych nowych śladów użytkowania poza tymi, które wynikały z jego charakteru vintage i były jasno opisane na stronie produktu.".to_string(),
        "Produkt powinien posiadać wszystkie oryginalne metki i oznaczenia (jeśli były dołączone).".to_string(),
        "Produkt musi być kompletny i zwrócony w stanie umożliwiającym jego dalszą odsprzedaż.".to_string(),
        "Prosimy o staranne zapakowanie zwracanego towaru, aby nie uległ uszkodzeniu podczas transportu.".to_string()
    ];
    let return_procedure_heading = "Procedura Zwrotu - krok po kroku:";
    let return_procedure_steps = [
        format!("1. Poinformuj nas: Skontaktuj się z nami mailowo na adres {} w ciągu 14 dni od otrzymania towaru, informując o chęci dokonania zwrotu. Podaj numer zamówienia i zwracane produkty. Możesz skorzystać ze wzoru formularza odstąpienia od umowy dostępnego w Regulaminie Sklepu, ale nie jest to obowiązkowe.", contact_email_returns),
        "2. Przygotuj paczkę: Starannie zapakuj zwracane produkty wraz z dowodem zakupu lub jego kopią oraz (opcjonalnie) wypełnionym formularzem zwrotu.".to_string(),
        format!("3. Odeślij produkt: Wyślij paczkę na adres: {}, {}, {}. Pamiętaj, że bezpośredni koszt odesłania produktu ponosi Klient. Nie przyjmujemy przesyłek za pobraniem.", return_address_line1, return_address_line2, return_address_line3),
        "4. Oczekuj na zwrot środków: Po otrzymaniu i pozytywnym zweryfikowaniu przesyłki zwrotnej, niezwłocznie (nie później niż w ciągu 14 dni) zwrócimy Ci należność za produkty oraz pierwotne koszty najtańszej oferowanej przez nas formy dostawy. Zwrot nastąpi tą samą metodą płatności, jakiej użyłeś/aś przy zakupie, chyba że wspólnie ustalimy inaczej.".to_string()
    ];
    let non_returnable_heading = "Produkty niepodlegające zwrotowi:";
    let non_returnable_text = "Ze względu na charakter naszych produktów (odzież używana/vintage), większość z nich podlega standardowej procedurze zwrotu. Wyjątki mogą dotyczyć np. bielizny ze względów higienicznych, jeśli została rozpakowana z zapieczętowanego opakowania – o takich sytuacjach zawsze informujemy w opisie produktu.".to_string();

    let complaints_section_title = "Reklamacje";
    let complaints_text = format!(
        "W MEG JONI przykładamy ogromną wagę do jakości i dokładności opisów naszych unikatowych produktów. \
        Jeśli jednak zdarzy się, że otrzymany towar posiada wadę, która nie została ujawniona w opisie, lub jest \
        niezgodny z zamówieniem, masz pełne prawo do złożenia reklamacji. Szczegółowe informacje dotyczące procedury \
        reklamacyjnej, Twoich praw oraz naszych obowiązków znajdziesz w §6 naszego Regulaminu Sklepu, dostępnego tutaj: \
        <a href='{0}' class='text-pink-600 hover:text-pink-700 hover:underline' \"hx-get\"='{0}' \"hx-target\"='#content' \"hx-swap\"='innerHTML' \"hx-push-url\"='{0}'>Regulamin Sklepu</a>.",
        link_to_terms
    );

    Ok(html! {
        div ."max-w-4xl mx-auto px-4 sm:px-6 lg:px-8 py-12 sm:py-16" {
            div ."text-center mb-12" {
                h1 ."text-4xl sm:text-5xl font-bold tracking-tight text-gray-900" { (page_title) }
                p ."mt-3 text-lg text-gray-600" { (page_subtitle) }
            }

            div ."space-y-8" {
                // Sekcja Wysyłka
                div "x-data"="{ open: true }" ."bg-white rounded-xl shadow-lg border border-gray-200 overflow-hidden" {
                    button type="button" "@click"="open = !open" class="w-full flex justify-between items-center p-5 sm:p-6 text-left hover:bg-gray-50 focus:outline-none" {
                        h2 ."text-2xl sm:text-3xl font-semibold text-pink-600" { (shipping_section_title) }
                        svg ."w-6 h-6 text-pink-500 transform transition-transform duration-200" "x-bind:class"="open ? 'rotate-180' : ''" fill="none" stroke="currentColor" "viewBox"="0 0 24 24" "xmlns"="http://www.w3.org/2000/svg" {
                            path "stroke-linecap"="round" "stroke-linejoin"="round" "stroke-width"="2" d="M19 9l-7 7-7-7";
                        }
                    }
                    div ."px-5 sm:px-6 pb-6 pt-3 prose prose-lg max-w-none text-gray-700 leading-relaxed"
                        "x-show"="open" "x-cloak"
                        "x-transition:enter"="transition ease-out duration-300" "x-transition:enter-start"="opacity-0 max-h-0" "x-transition:enter-end"="opacity-100 max-h-[1000px]"
                        "x-transition:leave"="transition ease-in duration-200" "x-transition:leave-start"="opacity-100 max-h-[1000px]" "x-transition:leave-end"="opacity-0 max-h-0"
                        style="overflow: hidden;" {

                        h3 ."text-xl font-semibold text-gray-800 mt-4 mb-2" { "Obszar dostawy" }
                        p { (shipping_area) }
                        h3 ."text-xl font-semibold text-gray-800 mt-4 mb-2" { "Dostępni przewoźnicy" }
                        p { (shipping_carriers_intro) }
                        ul ."list-disc pl-5 space-y-1" {
                            @for carrier in &shipping_carriers_list {
                                li { (carrier) }
                            }
                        }
                        h3 ."text-xl font-semibold text-gray-800 mt-4 mb-2" { "Koszty wysyłki" }
                        p { (shipping_costs_text) }
                        h3 ."text-xl font-semibold text-gray-800 mt-4 mb-2" { "Czas realizacji i dostawy" }
                        p { (processing_time_text) }
                        p { (delivery_time_text) }
                        h3 ."text-xl font-semibold text-gray-800 mt-4 mb-2" { "Śledzenie przesyłki" }
                        p { (tracking_text) }
                        h3 ."text-xl font-semibold text-gray-800 mt-4 mb-2" { "Pakowanie" }
                        p { (packaging_text) }
                    }
                }

                // Sekcja Zwroty
                div "x-data"="{ open: false }" ."bg-white rounded-xl shadow-lg border border-gray-200 overflow-hidden" {
                    button type="button" "@click"="open = !open" class="w-full flex justify-between items-center p-5 sm:p-6 text-left hover:bg-gray-50 focus:outline-none" {
                        h2 ."text-2xl sm:text-3xl font-semibold text-pink-600" { (returns_section_title) }
                        svg ."w-6 h-6 text-pink-500 transform transition-transform duration-200" "x-bind:class"="open ? 'rotate-180' : ''" fill="none" stroke="currentColor" "viewBox"="0 0 24 24" "xmlns"="http://www.w3.org/2000/svg" {
                            path "stroke-linecap"="round" "stroke-linejoin"="round" "stroke-width"="2" d="M19 9l-7 7-7-7";
                        }
                    }
                    div ."px-5 sm:px-6 pb-6 pt-3 prose prose-lg max-w-none text-gray-700 leading-relaxed"
                        "x-show"="open" "x-cloak"
                        "x-transition:enter"="transition ease-out duration-300" "x-transition:enter-start"="opacity-0 max-h-0" "x-transition:enter-end"="opacity-100 max-h-[1500px]"
                        "x-transition:leave"="transition ease-in duration-200" "x-transition:leave-start"="opacity-100 max-h-[1500px]" "x-transition:leave-end"="opacity-0 max-h-0"
                        style="overflow: hidden;" {

                        p { (right_to_return_text) }
                        h3 ."text-xl font-semibold text-gray-800 mt-4 mb-2" { (return_conditions_heading) }
                        ul ."list-disc pl-5 space-y-1" {
                            @for condition in &return_conditions_list {
                                li { (condition) }
                            }
                        }
                        h3 ."text-xl font-semibold text-gray-800 mt-4 mb-2" { (return_procedure_heading) }
                        ol ."list-decimal pl-5 space-y-2" {
                            @for step in &return_procedure_steps {
                                li { (step) }
                            }
                        }
                        h3 ."text-xl font-semibold text-gray-800 mt-4 mb-2" { (non_returnable_heading) }
                        p { (non_returnable_text) }
                    }
                }

                // Sekcja Reklamacje
                div ."p-6 bg-white rounded-lg shadow-lg border border-gray-200" {
                    h2 ."text-2xl sm:text-3xl font-semibold text-pink-600 mb-3" { (complaints_section_title) }
                    p ."text-gray-700 leading-relaxed" { (PreEscaped(&complaints_text)) }
                }
            }
        }
    })
}

pub async fn my_account_page_handler(claims: TokenClaims) -> Result<Markup, AppError> {
    tracing::info!(
        "MAUD: Użytkownik ID {} wszedł na stronę Moje Konto",
        claims.sub
    );

    let sidebar_links = vec![
        (
            "Moje Zamówienia",
            "/htmx/moje-konto/zamowienia",
            "/moje-konto/zamowienia",
        ),
        ("Moje Dane", "/htmx/moje-konto/dane", "/moje-konto/dane"), // Placeholder, do zaimplementowania
        ("Adresy", "/htmx/moje-konto/adresy", "/moje-konto/adresy"), // Placeholder, do zaimplementowania
    ];
    let default_section_url = "/htmx/moje-konto/zamowienia";

    Ok(html! {
        div ."max-w-7xl mx-auto px-2 sm:px-4 lg:px-8 py-8 sm:py-10" {
            h1 ."text-3xl sm:text-4xl font-bold tracking-tight text-gray-900 mb-8 text-center md:text-left" { "Moje Konto" }
            div ."flex flex-col md:flex-row gap-6 lg:gap-8" {
                aside ."w-full md:w-1/4 lg:w-1/5 bg-white p-4 sm:p-6 rounded-lg shadow-md md:sticky md:top-20 md:self-start" {
                    nav {
                        ul ."space-y-2" {
                            @for (label, hx_get_url, push_url) in sidebar_links {
                                li {
                                    a href=(push_url)
                                       "hx-get"=(hx_get_url)
                                       "hx-target"="#my-account-content"
                                       "hx-swap"="innerHTML"
                                       "hx-push-url"=(push_url)
                                       "hx-indicator"="#my-account-content-spinner"
                                       class="block px-3 py-2 rounded-md text-gray-700 hover:bg-pink-50 hover:text-pink-600 transition-colors duration-150 ease-in-out focus:outline-none focus:ring-2 focus:ring-pink-500" {
                                        (label)
                                    }
                                }
                            }
                            li ."pt-4 mt-4 border-t border-gray-200" {
                                // --- POPRAWIONY LINK WYLOGOWANIA ---
                                a href="#" // href nie jest już istotny dla nawigacji
                                   "@click.prevent"="$dispatch('trigger-alpine-logout')" // Wywołuje zdarzenie dla Alpine.js
                                   class="block px-3 py-2 rounded-md text-red-600 hover:bg-red-50 hover:text-red-700 font-medium transition-colors duration-150 ease-in-out focus:outline-none focus:ring-2 focus:ring-red-500" {
                                    "Wyloguj"
                                }
                            }
                        }
                    }
                }
                main #my-account-content ."w-full md:w-3/4 lg:w-4/5 bg-white p-4 sm:p-6 rounded-lg shadow-md min-h-[300px]"
                     "hx-get"=(default_section_url)
                     "hx-trigger"="load"
                     "hx-swap"="innerHTML" {
                    div #my-account-content-spinner .flex.justify-center.items-center.h-40 {
                        svg class="animate-spin h-8 w-8 text-pink-600" xmlns="http://www.w3.org/2000/svg" fill="none" "viewBox"="0 0 24 24" {
                            circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" "stroke-width"="4";
                            path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z";
                        }
                    }
                }
            }
        }
    })
}

pub async fn login_page_htmx_handler() -> Result<Markup, AppError> {
    tracing::info!("MAUD: Żądanie strony logowania HTMX");

    let page_title = "Logowanie do MEG JONI";
    let form_id = "login-form";
    let messages_id = "login-messages";
    let api_login_endpoint = "/api/auth/login";
    let registration_htmx_endpoint = "/htmx/rejestracja"; // Bez /page/
    let registration_url = "/rejestracja";

    Ok(html! {
        div ."min-h-[calc(100vh-var(--header-height,10rem))] w-full flex items-center justify-center p-4 bg-gradient-to-br from-pink-50 via-purple-50 to-indigo-100" {
            div ."w-full max-w-md" { // Ten div centruje kartę
                div ."bg-white/80 backdrop-blur-md py-8 px-6 sm:px-10 shadow-2xl rounded-xl border border-gray-200" {
                    div ."mb-8 text-center" {
                        h2 ."text-3xl font-bold text-gray-900" { (page_title) }
                    }

                    div #(messages_id) ."mb-4 text-sm min-h-[1.25em]"; // min-h-[1.25em] aby uniknąć skoku layoutu

                    form id="login-form"
                        "hx-post"=(api_login_endpoint)
                        "hx-ext"="json-enc"
                        "hx-target"=(format!("#{}", messages_id))
                        "hx-swap"="innerHTML"
                        class="space-y-6" {

                        div {
                            label for="email" ."block text-sm font-medium text-gray-700" { "Adres e-mail" }
                            div ."mt-1" {
                                input #email name="email" type="email" autocomplete="email" required
                                       class="appearance-none block w-full px-4 py-3 border border-gray-300 rounded-lg shadow-sm placeholder-gray-400
                                              focus:outline-none focus:ring-2 focus:ring-pink-500 focus:border-pink-500 
                                              transition duration-150 ease-in-out sm:text-sm";
                            }
                        }

                        div {
                            label for="password" ."block text-sm font-medium text-gray-700" { "Hasło" }
                            div ."mt-1" {
                                input #password name="password" type="password" autocomplete="current-password" required
                                       class="appearance-none block w-full px-4 py-3 border border-gray-300 rounded-lg shadow-sm placeholder-gray-400
                                              focus:outline-none focus:ring-2 focus:ring-pink-500 focus:border-pink-500 
                                              transition duration-150 ease-in-out sm:text-sm";
                            }
                        }

                        div {
                            button type="submit"
                                   class="w-full flex justify-center py-3 px-4 border border-transparent rounded-lg shadow-sm text-sm font-medium text-white
                                          bg-pink-600 hover:bg-pink-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-pink-500 
                                          transition-all duration-150 ease-in-out transform hover:scale-105" {
                                "Zaloguj się"
                            }
                        }
                    }

                    div ."mt-6 pt-6 border-t border-gray-200" {
                        div ."text-center" {
                            p ."text-sm text-gray-600" {
                                "Nie masz jeszcze konta? "
                                a href=(registration_url)
                                   "hx-get"=(registration_htmx_endpoint)
                                   "hx-target"="#content"
                                   "hx-swap"="innerHTML"
                                   "hx-push-url"=(registration_url)
                                   class="font-medium text-pink-600 hover:text-pink-500 hover:underline" {
                                    "Zarejestruj się"
                                }
                            }
                        }
                    }
                }
            }
        }
    })
}

pub async fn registration_page_htmx_handler() -> Result<Markup, AppError> {
    tracing::info!("MAUD: Żądanie strony rejestracji HTMX");

    let page_title = "Załóż konto w MEG JONI";
    let form_id = "registration-form";
    let messages_id = "registration-messages";
    let api_register_endpoint = "/api/auth/register";
    let login_htmx_endpoint = "/htmx/logowanie"; // Bez /page/
    let login_url = "/logowanie";

    Ok(html! {
        div ."min-h-[calc(100vh-var(--header-height,10rem))] w-full flex items-center justify-center p-4 bg-gradient-to-br from-teal-50 via-cyan-50 to-sky-100" {
            div ."w-full max-w-md" {
                div ."bg-white/80 backdrop-blur-md py-8 px-6 sm:px-10 shadow-2xl rounded-xl border border-gray-200" {
                    div ."mb-8 text-center" {
                        h2 ."text-3xl font-bold text-gray-900" { (page_title) }
                    }

                    div #(messages_id) ."mb-4 text-sm min-h-[1.25em]"; // Na komunikaty (sukces/błąd)

                    form #(form_id)
                        "hx-post"=(api_register_endpoint)
                        "hx-ext"="json-enc"
                        "hx-target"=(format!("#{}", messages_id))
                        "hx-swap"="innerHTML"
                        class="space-y-6" {

                        div {
                            label for="reg-email" ."block text-sm font-medium text-gray-700" { "Adres e-mail" }
                            div ."mt-1" {
                                input #reg-email name="email" type="email" autocomplete="email" required
                                       class="appearance-none block w-full px-4 py-3 border border-gray-300 rounded-lg shadow-sm placeholder-gray-400 focus:outline-none focus:ring-2 focus:ring-teal-500 focus:border-teal-500 transition duration-150 ease-in-out sm:text-sm";
                            }
                        }

                        div {
                            label for="reg-password" ."block text-sm font-medium text-gray-700" { "Hasło" }
                            div ."mt-1" {
                                input #reg-password name="password" type="password" autocomplete="new-password" required minlength="8"
                                       class="appearance-none block w-full px-4 py-3 border border-gray-300 rounded-lg shadow-sm placeholder-gray-400 focus:outline-none focus:ring-2 focus:ring-teal-500 focus:border-teal-500 transition duration-150 ease-in-out sm:text-sm";
                                p ."mt-1 text-xs text-gray-500" { "Minimum 8 znaków." }
                            }
                        }

                        // TODO: Dodaj pole do potwierdzenia hasła (ważne!)
                        // div {
                        //     label for="confirm_password" ."block text-sm font-medium text-gray-700" { "Potwierdź hasło" }
                        //     div ."mt-1" {
                        //         input #confirm_password name="confirm_password" type="password" autocomplete="new-password" required minlength="8"
                        //                class="appearance-none block w-full px-4 py-3 border border-gray-300 rounded-lg shadow-sm ...";
                        //     }
                        // }

                        // TODO: Dodaj checkboxy ze zgodami (Regulamin, Polityka Prywatności) - są one prawnie wymagane.
                        // div ."pt-2 space-y-2" {
                        //    ... przykładowy checkbox ...
                        // }

                        div {
                            button type="submit"
                                   class="w-full flex justify-center py-3 px-4 border border-transparent rounded-lg shadow-sm text-sm font-medium text-white
                                          bg-teal-600 hover:bg-teal-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-teal-500 
                                          transition-all duration-150 ease-in-out transform hover:scale-105" {
                                "Zarejestruj się"
                            }
                        }
                    }

                    div ."mt-6 pt-6 border-t border-gray-200" {
                        div ."text-center" {
                            p ."text-sm text-gray-600" {
                                "Masz już konto? "
                                a href=(login_url)
                                   "hx-get"=(login_htmx_endpoint)
                                   "hx-target"="#content"
                                   "hx-swap"="innerHTML"
                                   "hx-push-url"=(login_url)
                                   class="font-medium text-teal-600 hover:text-teal-500 hover:underline" {
                                    "Zaloguj się"
                                }
                            }
                        }
                    }
                }
            }
        }
    })
}

pub async fn my_orders_htmx_handler(
    State(app_state): State<AppState>,
    claims: TokenClaims, // Wymagane zalogowanie, Axum zajmie się autoryzacją
) -> Result<Markup, AppError> {
    let user_id = claims.sub;
    tracing::info!("MAUD: Użytkownik ID {} żąda listy swoich zamówień", user_id);

    // 1. Pobierz zamówienia użytkownika z bazy danych
    let orders: Vec<Order> = sqlx::query_as::<_, Order>(
        r#"
            SELECT id, user_id, order_date, status, total_price, 
                   shipping_address_line1, shipping_address_line2, shipping_city, 
                   shipping_postal_code, shipping_country, created_at, updated_at
            FROM orders
            WHERE user_id = $1
            ORDER BY order_date DESC
        "#,
    )
    .bind(user_id)
    .fetch_all(&app_state.db_pool)
    .await?; // Znak zapytania skonwertuje błąd sqlx::Error na AppError (jeśli masz implementację From<sqlx::Error> for AppError)

    // 2. Wyrenderuj HTML za pomocą Maud
    Ok(html! {
        div {
            h2 ."text-2xl sm:text-3xl font-semibold text-gray-800 mb-6" { "Moje Zamówienia" }
            @if orders.is_empty() {
                p ."text-gray-600 py-4" { "Nie złożyłeś/aś jeszcze żadnych zamówień." }
            } @else {
                div ."space-y-6" {
                    @for order_item in &orders {
                        // Przygotowanie wartości do wyświetlenia, aby uniknąć problemów z Option wprost w Maud
                        @let order_id_display = order_item.id.to_string().chars().take(8).collect::<String>();
                        // Używamy to_string() na wyniku format(), aby zapewnić, że to String
                        @let order_date_display = order_item.order_date.format("%d-%m-%Y %H:%M").to_string();
                        @let order_status_display = order_item.status.to_string(); // Zakłada, że OrderStatus implementuje Display
                        @let order_total_display = format_price_maud(order_item.total_price);

                        @let status_classes = match order_item.status {
                            // Użyj pełnej ścieżki, jeśli OrderStatus jest w innym module i niezaimportowany bezpośrednio
                            // np. crate::models::OrderStatus::Pending
                            OrderStatus::Pending => "bg-yellow-100 text-yellow-800",
                            OrderStatus::Processing => "bg-blue-100 text-blue-800",
                            OrderStatus::Shipped => "bg-green-100 text-green-800",
                            OrderStatus::Delivered => "bg-emerald-100 text-emerald-800",
                            OrderStatus::Cancelled => "bg-red-100 text-red-800",
                        };

                        div ."border border-gray-200 rounded-lg p-4 sm:p-6 hover:shadow-lg transition-shadow duration-200 ease-in-out bg-white" {
                            div ."flex flex-col sm:flex-row justify-between sm:items-center mb-3 pb-3 border-b border-gray-100" {
                                div {
                                    h3 ."text-lg font-semibold text-pink-600" {
                                        "Zamówienie #" (order_id_display)
                                    }
                                    p ."text-sm text-gray-500" { "Data złożenia: " (order_date_display) }
                                }
                                div ."mt-2 sm:mt-0 text-left sm:text-right" {
                                    span class=(format!("px-3 py-1 text-xs font-semibold rounded-full {}", status_classes)) {
                                        (order_status_display)
                                    }
                                }
                            }
                            div ."flex flex-col sm:flex-row justify-between items-start sm:items-center" {
                                div ."text-sm text-gray-700" {
                                    p { "Suma: " strong { (order_total_display) } }
                                }
                                div ."mt-3 sm:mt-0" {
                                    // TODO: Zaimplementuj handler /htmx/moje-konto/zamowienie-szczegoly/{order_id}
                                    // Ten handler powinien być chroniony i renderować szczegóły zamówienia za pomocą Maud
                                    a href=(format!("/moje-konto/zamowienia/{}", order_item.id)) // URL dla paska adresu
                                       "hx-get"=(format!("/htmx/moje-konto/zamowienie-szczegoly/{}", order_item.id))
                                       "hx-target"="#my-account-content" // Podmienia zawartość w głównym obszarze "Moje Konto"
                                       "hx-swap"="innerHTML"
                                       "hx-push-url"=(format!("/moje-konto/zamowienia/{}", order_item.id))
                                       class="text-sm text-pink-600 hover:text-pink-700 hover:underline font-medium py-2 px-3 rounded-md hover:bg-pink-50 transition-colors" {
                                        "Zobacz szczegóły"
                                    }
                                }
                            }
                        }
                    }
                    // TODO: Rozważ dodanie paginacji dla zamówień, jeśli lista może być bardzo długa
                    // Paginacja również używałaby HTMX, celując w ten sam div lub jego rodzica.
                }
            }
        }
    })
}
