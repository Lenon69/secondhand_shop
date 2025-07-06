// src/extractor.rs

use axum::{
    RequestPartsExt,
    extract::{FromRef, FromRequestParts},
    http::request::Parts,
};
use axum_extra::{
    TypedHeader,
    headers::{Authorization, authorization::Bearer},
};
use jsonwebtoken::{DecodingKey, Validation, decode};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::sync::Arc;
use uuid::Uuid;

use crate::state::AppState;

/// Definicja tego, co znajduje się w tokenie JWT.
/// Prawdopodobnie masz już tę lub podobną strukturę.
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: Uuid,  // Subject (user_id)
    pub exp: usize, // Expiration time
}

/// Nasz nowy ekstraktor. Będzie zawierał `Some(Claims)`, jeśli
/// token jest obecny i poprawny, lub `None` w każdym innym przypadku.
pub struct OptionalTokenClaims(pub Option<Claims>);

impl<S> FromRequestParts<S> for OptionalTokenClaims
where
    Arc<AppState>: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let typed_header_result = parts.extract::<TypedHeader<Authorization<Bearer>>>().await;

        let token = match typed_header_result {
            Ok(TypedHeader(Authorization(bearer))) => bearer.token().to_owned(),
            Err(_) => return Ok(OptionalTokenClaims(None)),
        };

        // Ta linia jest teraz w pełni wykorzystywana!
        let app_state = Arc::<AppState>::from_ref(state);

        // Pobieramy sekret z pamięci (ze stanu), a nie ze środowiska
        let claims = decode::<Claims>(
            &token,
            &DecodingKey::from_secret(app_state.jwt_secret.as_ref()), // <-- POPRAWIONE
            &Validation::default(),
        );

        match claims {
            Ok(token_data) => Ok(OptionalTokenClaims(Some(token_data.claims))),
            Err(_) => Ok(OptionalTokenClaims(None)),
        }
    }
}
