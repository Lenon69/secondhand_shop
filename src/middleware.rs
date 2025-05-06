use axum::{
    RequestPartsExt,
    extract::FromRequestParts,
    http::{HeaderMap, HeaderValue, StatusCode, request::Parts},
};
use axum_extra::TypedHeader;
use axum_extra::headers::{Authorization, authorization::Bearer};

use crate::{
    auth::verify_jwt,
    auth_models::TokenClaims,
    errors::AppError,
    state::{self, AppState},
};

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
                tracing::warn!("Failed to extract Bearer token: {:?}", e);
                match e.reason() {
                    // Tutaj można by dodać bardziej szczegółowe mapowanie błędów z TypedHeaderRejection
                    // na razie ogólny błąd, jeśli nagłówek jest źle sformatowany lub go nie ma.
                    _ => {
                        AppError::MissingToken("Brak lub niepoprawny nagłówek Authorization".into())
                    }
                }
            })?;

        let token = bearer.token();

        // Zweryfikuj token
        // verify_jwt powinno przyjmować sekret JWT z AppState
        let claims = verify_jwt(token, &state.jwt_secret).map_err(|e| {
            tracing::error!("Invalid token: {:?}", e);
            // Tutaj mapujemy błąd z jsonwebtoken na nasz AppError
            // Można by to rozbudować, aby rozróżniać np. wygaśnięty token od nieprawidłowej sygnatury
            AppError::InvalidToken("Token jest nieprawidłowy lub wygasł".into())
        })?;

        Ok(claims.claims)
    }
}
