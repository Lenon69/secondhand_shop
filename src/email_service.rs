// src/email_service.rs

use std::env;

use crate::{
    errors::AppError,
    models::{OrderDetailsResponse, PaymentMethod, User},
    state::AppState,
};
use maud::{Markup, PreEscaped, html};
use resend_rs::{Resend, types::CreateEmailBaseOptions};

// Pomocnicza funkcja do formatowania ceny, tak jak w htmx_handlers
fn format_price_maud(price: i64) -> String {
    format!("{:.2}", (price as f64) / 100.0).replace('.', ",") + " zł"
}

// Funkcja, którą będziemy wywoływać z handlera
pub async fn send_order_confirmation_email(
    app_state: &AppState,
    order_details: &OrderDetailsResponse,
) -> Result<(), AppError> {
    let recipient_email: String;

    // === NOWA, POPRAWNA LOGIKA POBIERANIA E-MAILA ===
    if let Some(guest_email_val) = &order_details.order.guest_email {
        // Przypadek 1: Zamówienie gościa, e-mail jest w zamówieniu.
        recipient_email = guest_email_val.clone();
        tracing::info!("Wysyłka e-maila do gościa na adres: {}", recipient_email);
    } else if let Some(user_id_val) = order_details.order.user_id {
        // Przypadek 2: Zamówienie zalogowanego użytkownika, pobierz e-mail z tabeli `users`.
        let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
            .bind(user_id_val)
            .fetch_optional(&app_state.db_pool)
            .await?
            .ok_or_else(|| AppError::NotFound)?; // Zwróć błąd, jeśli użytkownik nie istnieje

        recipient_email = user.email;
        tracing::info!(
            "Wysyłka e-maila do zalogowanego użytkownika na adres: {}",
            recipient_email
        );
    } else {
        // Przypadek 3: Błąd - zamówienie nie ma ani e-maila gościa, ani ID użytkownika.
        tracing::error!(
            "Nie można ustalić adresu e-mail odbiorcy dla zamówienia ID: {}",
            order_details.order.id
        );
        return Err(AppError::InternalServerError(
            "Brak adresu e-mail do wysyłki potwierdzenia.".to_string(),
        ));
    }

    // Inicjalizacja klienta Resend
    let resend = Resend::new(&app_state.resend_api_key);

    // Wyrenderuj treść HTML e-maila
    let email_html_content = render_order_confirmation_email_html(order_details);

    // Pobierz e-mail administratora/nadawcy ze zmiennej środowiskowej
    let sender_display_name = "mess - all that vintage";
    let sender_email_address =
        env::var("ADMIN_EMAIL").unwrap_or_else(|_| "noreply@mess.com".to_string());
    let sender_formatted = format!("{} <{}>", sender_display_name, sender_email_address);

    let subject = format!(
        "Potwierdzenie zamówienia nr #{}",
        &order_details.order.id.to_string()[..8]
    );

    // Używamy .builder() do stworzenia zapytania
    let params = CreateEmailBaseOptions::new(
        &sender_formatted,
        vec![recipient_email.clone()], // Używamy sklonowanego e-maila
        &subject,
    )
    .with_html(&email_html_content.into_string());

    tracing::info!(
        "Wysyłanie e-maila z potwierdzeniem zamówienia do: {}",
        recipient_email
    );

    // Wyślij e-mail
    resend.emails.send(params).await.map_err(|e| {
        tracing::error!("Błąd API Resend: {:?}", e);
        AppError::InternalServerError("Błąd podczas wysyłania e-maila.".to_string())
    })?;

    tracing::info!("E-mail z potwierdzeniem zamówienia został wysłany pomyślnie.");
    Ok(())
}

// Funkcja renderująca szablon HTML e-maila
fn render_order_confirmation_email_html(order_details: &OrderDetailsResponse) -> Markup {
    let order = &order_details.order;
    let order_id_short = &order.id.to_string()[..8];
    let payment_method_details = match order.payment_method.as_ref() {
        Some(PaymentMethod::Blik) => {
            "Płatność BLIK na numer telefonu: <strong>603 117 793</strong>. W tytule przelewu prosimy podać numer zamówienia."
        }
        Some(PaymentMethod::Transfer) => {
            "Prosimy o dokonanie przelewu na numer konta: <strong>XX XXXX XXXX XXXX XXXX XXXX XXXX</strong>. W tytule przelewu prosimy podać numer zamówienia."
        }
        None => "Metoda płatności nie została określona. Skontaktuj się z nami.",
    };

    html! {
        (PreEscaped("<!DOCTYPE html>"))
        html lang="pl" {
            head {
                meta charset="UTF-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { "Potwierdzenie zamówienia" }
                style {
                    (PreEscaped(r#"
                        body { font-family: Arial, sans-serif; color: #333; }
                        .container { max-width: 600px; margin: auto; padding: 20px; border: 1px solid #ddd; }
                        .header { background-color: #fce4ec; padding: 10px; text-align: center; }
                        .header h1 { color: #e91e63; }
                        .item { border-bottom: 1px solid #eee; padding: 10px 0; display: flex; }
                        .item img { width: 80px; height: 80px; object-fit: cover; margin-right: 15px; }
                        .item-details { flex-grow: 1; }
                        .total { text-align: right; font-weight: bold; font-size: 1.2em; margin-top: 20px; }
                        .payment-info { background-color: #fff9c4; border: 1px solid #fdd835; padding: 15px; margin-top: 20px; }
                    "#))
                }
            }
            body {
                div class="container" {
                    div class="header" {
                        h1 { "mess - all that vintage" }
                        h2 { "Dziękujemy za Twoje zamówienie!" }
                    }
                    h3 { "Hej, " (order.shipping_first_name) "!" }
                    p { "Twoje zamówienie nr #" (order_id_short) " zostało pomyślnie złożone. Poniżej znajdziesz jego podsumowanie." }

                    h4 style="border-bottom: 2px solid #eee; padding-bottom: 5px;" { "Szczegóły zamówienia" }

                    @for item in &order_details.items {
                        div class="item" {
                            @if let Some(img) = item.product.images.get(0) {
                                img src=(img) alt=(item.product.name);
                            }
                            div class="item-details" {
                                strong { (item.product.name) }
                                br;
                                span { "Cena: " (format_price_maud(item.price_at_purchase)) }
                            }
                        }
                    }

                    p class="total" {
                        "Suma do zapłaty: " strong { (format_price_maud(order.total_price)) }
                    }

                    div class="payment-info" {
                        h4 { "Dane do płatności" }
                        p { (PreEscaped(payment_method_details)) }
                    }

                    div {
                        h4 { "Adres dostawy" }
                        p {
                            (order.shipping_first_name) " " (order.shipping_last_name) br;
                            (order.shipping_address_line1) br;
                            @if let Some(line2) = &order.shipping_address_line2 { (line2) br; }
                            (order.shipping_postal_code) " " (order.shipping_city)
                        }
                    }

                    p { "Dziękujemy za zakupy i zapraszamy ponownie!" }
                    p { "Zespół mess - all that vintage" }
                }
            }
        }
    }
}
