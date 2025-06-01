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
        // To automatycznie sprawdzi, czy nagłówek istnieje i czy jest poprawnym Bearer tokenem.
        let TypedHeader(Authorization(bearer)) = parts
            .extract::<TypedHeader<Authorization<Bearer>>>()
            .await
            .map_err(|e| {
                // `axum_extra::extract::typed_header::TypedHeaderRejection` ma różne warianty
                // Możemy je zmapować na bardziej szczegółowe błędy AppError
                tracing::warn!("Failed to extract Bearer token header: {:?}", e);
                AppError::MissingToken("Brak lub niepoprawny nagłówek Authorization".into())
            })?;

        let token = bearer.token();

        // Zweryfikuj token
        // verify_jwt powinno przyjmować sekret JWT z AppState
        let claims = verify_jwt(token, &state.jwt_secret).map_err(|e| {
            tracing::error!("Invalid token: {:?}", e);
            AppError::InvalidToken("Token jest nieprawidłowy lub wygasł".into())
        })?;

        Ok(claims.claims)
    }
}

#[derive(Debug, Clone)]
pub struct OptionalTokenClaims(pub Option<TokenClaims>);

impl FromRequestParts<AppState> for OptionalTokenClaims {
    // Dla opcjonalnego ekstraktora, odrzucenie (Rejection) zazwyczaj nie powinno się zdarzyć,
    // chyba że wystąpi jakiś wewnętrzny błąd. Jeśli token jest nieobecny lub nieprawidłowy,
    // po prostu zwracamy Ok(OptionalTokenClaims(None)).
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
