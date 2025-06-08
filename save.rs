fn render_product_form_maud(product_opt: Option<&Product>) -> Result<Markup, AppError> {
    let is_new = product_opt.is_none();
    let default_product = Product {
        id: Uuid::new_v4(),
        name: "".to_string(),
        description: "".to_string(),
        price: 0,
        gender: ProductGender::Damskie,
        condition: ProductCondition::VeryGood,
        category: Category::Inne,
        status: ProductStatus::Available,
        images: vec![],
        on_sale: false,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    let product = product_opt.unwrap_or(&default_product);

    let form_title = if is_new {
        "Dodaj Nowy Produkt"
    } else {
        "Edytuj Produkt"
    };
    let form_action = if is_new {
        "/api/products".to_string()
    } else {
        format!("/api/products/{}", product.id)
    };
    let button_text = if is_new {
        "Dodaj Produkt"
    } else {
        "Zapisz Zmiany"
    };

    let initial_images_json =
        serde_json::to_string(&product.images).unwrap_or_else(|_| "[]".to_string());
    let current_status_str = product.status.as_ref().to_string();

    let form_body = html! {
        // Wszystkie pola formularza idą tutaj
        input type="hidden" name="urls_to_delete" id="urls_to_delete_hidden_input";
        section {
            h3 ."text-xl font-semibold text-gray-700 mb-4 pb-2 border-b border-gray-200" { "Dane Podstawowe" }
            div ."space-y-5" {
                div {
                    label for="name" ."block text-sm font-medium text-gray-700 mb-1" { "Nazwa produktu *" }
                    input type="text" name="name" id="name" required value=(product.name) class="admin-filter-input";
                }
                div {
                    label for="description" ."block text-sm font-medium text-gray-700 mb-1" { "Opis produktu *" }
                    textarea name="description" id="description" rows="6" required class="admin-filter-input" { (product.description) }
                }
                div {
                    label for="price" ."block text-sm font-medium text-gray-700 mb-1" { "Cena (w groszach) *" }
                    input type="number" name="price" id="price" required min="0" step="1" value=(product.price) class="admin-filter-input";
                }
            }
        }

        section {
            h3 ."text-xl font-semibold text-gray-700 mb-4 pb-2 border-b border-gray-200" { "Klasyfikacja i Status" }
            div ."grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-x-6 gap-y-5" {
                div {
                    label for="gender" ."block text-sm font-medium text-gray-700 mb-1" { "Płeć *" }
                    select name="gender" id="gender" required class="admin-filter-select" {
                        @for v in ProductGender::iter() { option value=(v.as_ref()) selected[product.gender == v] { (v.to_string()) } }
                    }
                }
                div {
                    label for="condition" ."block text-sm font-medium text-gray-700 mb-1" { "Stan *" }
                    select name="condition" id="condition" required class="admin-filter-select" {
                        @for v in ProductCondition::iter() { option value=(v.as_ref()) selected[product.condition == v] { (v.to_string()) } }
                    }
                }
                div {
                    label for="category" ."block text-sm font-medium text-gray-700 mb-1" { "Kategoria *" }
                    select name="category" id="category" required class="admin-filter-select" {
                        @for v in Category::iter() { option value=(v.as_ref()) selected[product.category == v] { (v.to_string()) } }
                    }
                }
                div {
                    label for="status" ."block text-sm font-medium text-gray-700 mb-1" { "Status *" }
                    select name="status" id="status" required x-model="productStatus" class="admin-filter-select" {
                        @for v in ProductStatus::iter() { option value=(v.as_ref()) { (v.to_string()) } }
                    }
                }
            }
        }

        section ."mt-6 pt-6 border-t border-gray-200" {
             h3 ."text-xl font-semibold text-gray-700 mb-4 pb-2 border-b border-gray-200" { "Opcje Sprzedaży" }
            div class="relative flex items-start" {
                div class="flex h-6 items-center" {
                    input id="on_sale" name="on_sale" type="checkbox" checked[product.on_sale] class="h-4 w-4 rounded border-gray-300 text-pink-600 focus:ring-pink-500";
                }
                div class="ml-3 text-sm leading-6" {
                    label for="on_sale" class="font-medium text-gray-700" { "Produkt na wyprzedaży" }
                    p class="text-xs text-gray-500" { "Zaznacz, jeśli produkt ma być częścią wyprzedaży." }
                }
            }
        }

        // Sekcja: Zdjęcia Produktu (TA SAMA LOGIKA HTML CO W EDYCJI)
        section {
            // input type="hidden" name="urls_to_delete" id="urls_to_delete_hidden_input_new_form"; // Już dodane na początku formularza
            h3 ."text-xl font-semibold text-gray-700 mb-2 pb-2 border-b border-gray-200" { "Zdjęcia Produktu" }
            p ."text-xs text-gray-500 mb-4" { "Dodaj od 1 do 8 zdjęć. Pierwsze zdjęcie będzie zdjęciem głównym." }
            div ."grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 gap-4" {
                @for i in 0..8 {
                    @let slot_input_id = format!("new_form_image_file_slot_{}", i); // Unikalne ID dla inputu pliku
                    @let input_name = format!("image_file_{}", i + 1); // Nazwa pola dla backendu

                    div class="relative aspect-square border-2 border-dashed border-gray-300 rounded-lg flex flex-col items-center justify-center text-gray-400 hover:border-pink-400 transition-colors group"
                        "x-bind:class"=(format!("{{ \
                            '!border-solid !border-pink-500 shadow-lg': imagePreviews[{}], \
                            'hover:bg-pink-50/20': !imagePreviews[{}] \
                        }}", i, i)) { // Uproszczony x-bind:class, bo nie ma 'isMarkedForDeletion'

                        // --- Podgląd obrazka i przycisk "X" (tylko dla nowych podglądów) ---
                        template "x-if"=(format!("imagePreviews[{}]", i)) { // isSlotFilled jest true, jeśli imagePreviews[i] nie jest null
                            div ."absolute inset-0 w-full h-full" {
                                img "x-bind:src"=(format!("imagePreviews[{}]", i)) // getSlotImageSrc zwróci imagePreviews[i]
                                     alt=(format!("Podgląd zdjęcia {}", i + 1))
                                     class="w-full h-full object-cover rounded-md";
                                button type="button"
                                       "@click.prevent"=(format!("removeImage({}, '{}')", i, slot_input_id))
                                       class="absolute top-1 right-1 p-0.5 bg-red-600 text-white rounded-full opacity-0 group-hover:opacity-100 hover:bg-red-700 transition-all text-xs w-5 h-5 flex items-center justify-center shadow-md z-10"
                                       title="Usuń ten podgląd" {
                                    svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor" class="w-3 h-3" {
                                        path "fill-rule"="evenodd" d="M8.75 1A2.75 2.75 0 006 3.75v.443c-.795.077-1.584.176-2.365.298a.75.75 0 10.23 1.482l.149-.022.841 10.518A2.75 2.75 0 007.596 19h4.807a2.75 2.75 0 002.742-2.53l.841-10.52.149.023a.75.75 0 00.23-1.482A41.03 41.03 0 0014 4.193v-.443A2.75 2.75 0 0011.25 1h-2.5zM10 4c.84 0 1.673.025 2.5.075V3.75c0-.69-.56-1.25-1.25-1.25h-2.5c-.69 0-1.25.56-1.25 1.25v.325C8.327 4.025 9.16 4 10 4zM8.58 7.72a.75.75 0 00-1.5.06l.3 7.5a.75.75 0 101.5-.06l-.3-7.5zm4.34.06a.75.75 0 10-1.5-.06l-.3 7.5a.75.75 0 101.5.06l.3-7.5z" "clip-rule"="evenodd" {}
                                    }
                                }
                            }
                        }

                        // --- Labelka do dodawania pliku (widoczna, gdy nie ma podglądu) ---
                        // Nie ma potrzeby używania isMarkedForDeletion, bo to nowy produkt
                        template "x-if"=(format!("!imagePreviews[{}]", i)) {
                            label for=(slot_input_id) class="cursor-pointer p-2 text-center w-full h-full flex flex-col items-center justify-center hover:bg-pink-50/50 transition-colors rounded-md" {
                                svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor" class="w-8 h-8 text-gray-400 group-hover:text-pink-500 transition-colors" {
                                    path d="M9.25 13.25a.75.75 0 001.5 0V4.793l2.97 2.97a.75.75 0 001.06-1.06l-4.25-4.25a.75.75 0 00-1.06 0L5.22 6.704a.75.75 0 001.06 1.06L9.25 4.793v8.457z" {}
                                    path d="M3.5 12.75a.75.75 0 00-1.5 0v2.5A2.75 2.75 0 004.75 18h10.5A2.75 2.75 0 0018 15.25v-2.5a.75.75 0 00-1.5 0v2.5c0 .69-.56 1.25-1.25 1.25H4.75c-.69 0-1.25-.56-1.25-1.25v-2.5z" {}
                                }
                                div ."text-xs mt-1 text-gray-500 group-hover:text-pink-600 transition-colors" {
                                     @if i == 0 { "Dodaj główne *" } @else { "Dodaj zdjęcie" }
                                }
                            }
                        }
                        input type="file" name=(input_name) id=(slot_input_id)
                               accept="image/jpeg,image/png,image/webp"
                               "@change"=(format!("handleFileChange($event, {})", i))
                               class="opacity-0 absolute inset-0 w-full h-full cursor-pointer z-0"
                               required[i == 0];
                    }
                }
            }
        }

        // Przyciski Akcji
        section ."pt-8 border-t border-gray-200 mt-8" {
            div ."flex flex-col sm:flex-row justify-end items-center gap-3" {
                a href="/htmx/admin/products"
                   hx-get="/htmx/admin/products" hx-target="#admin-content" hx-swap="innerHTML"
                   class="px-6 py-2.5 text-sm font-medium text-gray-700 bg-white border border-gray-300 rounded-lg hover:bg-gray-50 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-gray-400 transition-all w-full sm:w-auto text-center" {
                    "Anuluj"
                }
                button type="submit"
                       class="w-full sm:w-auto inline-flex justify-center items-center px-8 py-2.5 border border-transparent text-sm font-medium rounded-lg shadow-sm text-white bg-pink-600 hover:bg-pink-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-pink-500 transition-transform transform hover:scale-105" {
                    span { "Zapisz" }
                }
            }
        }
    };

    Ok(html! {
        div #admin-product-form-container ."p-4 sm:p-6 lg:p-8 bg-gray-50 min-h-screen" {
            div ."max-w-4xl mx-auto" {
                div ."flex justify-between items-center mb-6 pb-3 border-b border-gray-300" {
                    h2 ."text-2xl sm:text-3xl font-semibold text-gray-800" { (form_title)
                        @if !is_new { ": " span."text-pink-600"{(product.name)} }
                    }
                    a href="/htmx/admin/products" hx-get="/htmx/admin/products" hx-target="#admin-content" hx-swap="innerHTML"
                       class="text-sm text-pink-600 hover:text-pink-700 hover:underline font-medium transition-colors" {
                        "← Wróć do listy"
                    }
                }
                div #product-form-messages ."mb-4 min-h-[2em]" {}

                // KROK 2: Użyj @if do wyrenderowania całego, kompletnego tagu <form>
                // z odpowiednim atrybutem, a wewnątrz wstaw zdefiniowany wcześniej `form_body`.
                @if is_new {
                    form hx-encoding="multipart/form-data" hx-post=(form_action)
                         hx-target="#product-form-messages"
                         class="space-y-8 bg-white p-6 sm:p-8 rounded-xl shadow-xl border border-gray-200"
                         x-data="adminProductEditForm()"
                         "data-initial-images"=(initial_images_json)
                         "data-current-status"=(current_status_str)
                         x-init="initAlpineComponent($el.dataset.initialImages, $el.dataset.currentStatus)" {

                        (form_body)
                    }
                } @else {
                    form hx-encoding="multipart/form-data" hx-patch=(form_action)
                         hx-target="#product-form-messages"
                         class="space-y-8 bg-white p-6 sm:p-8 rounded-xl shadow-xl border border-gray-200"
                         x-data="adminProductEditForm()"
                         "data-initial-images"=(initial_images_json)
                         "data-current-status"=(current_status_str)
                         x-init="initAlpineComponent($el.dataset.initialImages, $el.dataset.currentStatus)" {

                        (form_body)
                    }
                }
            }
        }
    })
}
