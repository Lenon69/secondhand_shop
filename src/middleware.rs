use axum::{RequestPartsExt, extract::FromRequestParts, http::request::Parts};
use axum_extra::TypedHeader;
use axum_extra::headers::{Authorization, authorization::Bearer};

use crate::{auth::verify_jwt, auth_models::TokenClaims, errors::AppError, state::AppState};

impl FromRequestParts<AppState> for TokenClaims {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        // Wyciągnij TypedHeader<Authorization<Bearer>>
        let bearer_result = parts.extract::<TypedHeader<Authorization<Bearer>>>().await;

        let bearer = match bearer_result {
            Ok(TypedHeader(Authorization(bearer))) => bearer,
            Err(e) => {
                // Błąd ekstrakcji nagłówka - to tutaj dodamy naszą logikę
                tracing::warn!("Nie udało się wyekstrahować nagłówka Bearer: {:?}", e);

                // Sprawdź, czy przeglądarka oczekuje strony HTML
                if let Some(accept_header) = parts.headers.get(axum::http::header::ACCEPT) {
                    if let Ok(accept_str) = accept_header.to_str() {
                        if accept_str.contains("text/html") {
                            // TAK, to jest pełne żądanie strony. Przekieruj na logowanie.
                            tracing::info!(
                                "Wykryto żądanie HTML bez tokenu. Przekierowuję na /logowanie."
                            );
                            return Err(AppError::RedirectToLogin);
                        }
                    }
                }

                // Jeśli to nie jest żądanie HTML (np. API, HTMX), zwróć standardowy błąd JSON.
                return Err(AppError::MissingToken(
                    "Brak lub niepoprawny nagłówek Authorization".into(),
                ));
            }
        };

        let token = bearer.token();

        // Weryfikacja tokenu pozostaje bez zmian
        let claims = verify_jwt(token, &state.jwt_secret).map_err(|e| {
            tracing::error!("Nieprawidłowy token: {:?}", e);
            // Tutaj również moglibyśmy dodać logikę przekierowania dla wygasłych tokenów
            // jeśli zażądanoby strony HTML, ale na razie zostawmy to prostsze.
            AppError::InvalidToken("Token jest nieprawidłowy lub wygasł".into())
        })?;

        Ok(claims.claims)
    }
}

#[derive(Debug, Clone)]
pub struct OptionalTokenClaims(pub Option<TokenClaims>);

impl FromRequestParts<AppState> for OptionalTokenClaims {
    type Rejection = AppError; // Można też użyć Infallible, jeśli nie ma ścieżki błędu

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        tracing::debug!("Attempting to extract OPTIONAL TokenClaims.");
        // Spróbuj wyekstrahować nagłówek Authorization
        match parts.extract::<TypedHeader<Authorization<Bearer>>>().await {
            Ok(TypedHeader(Authorization(bearer))) => {
                // Nagłówek jest obecny, spróbuj zweryfikować token
                let token_str = bearer.token();
                // tracing::debug!("Extracted token string for optional TokenClaims: {}", token_str);
                match verify_jwt(token_str, &state.jwt_secret) {
                    Ok(claims_data) => {
                        // tracing::debug!("Token verified successfully for optional TokenClaims, sub: {}", claims_data.claims.sub);
                        Ok(OptionalTokenClaims(Some(claims_data.claims)))
                    }
                    Err(e) => {
                        // Token jest obecny, ale nieprawidłowy (np. wygasł, zły podpis)
                        tracing::warn!(
                            "Invalid token provided for optional TokenClaims (will treat as None): {:?}",
                            e
                        );
                        // Zamiast zwracać błąd, traktujemy to jako brak ważnego tokenu
                        Ok(OptionalTokenClaims(None))
                    }
                }
            }
            Err(_) => {
                // Nagłówek Authorization nie jest obecny lub ma zły format Bearer
                tracing::debug!(
                    "No valid Authorization Bearer header found for optional TokenClaims."
                );
                Ok(OptionalTokenClaims(None)) // Traktujemy to jako brak tokenu
            }
        }
    }
}
