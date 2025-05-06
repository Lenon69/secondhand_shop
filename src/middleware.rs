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
