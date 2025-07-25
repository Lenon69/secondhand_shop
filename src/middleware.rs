use std::sync::Arc;

use axum::extract::FromRef;
use axum::{RequestPartsExt, extract::FromRequestParts, http::request::Parts};
use axum_extra::TypedHeader;
use axum_extra::extract::cookie::CookieJar;
use axum_extra::headers::{Authorization, authorization::Bearer};
use uuid::Uuid;

use crate::handlers::XGuestCartId;
use crate::{auth::verify_jwt, auth_models::TokenClaims, errors::AppError, state::AppState};

impl FromRequestParts<AppState> for TokenClaims {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        // Metoda 1: Spróbuj z nagłówka 'Authorization' (dla HTMX i API)
        if let Ok(TypedHeader(Authorization(bearer))) =
            parts.extract::<TypedHeader<Authorization<Bearer>>>().await
        {
            let token_data = verify_jwt(bearer.token(), &state.jwt_secret)?;
            return Ok(token_data.claims);
        }

        // Metoda 2: Jeśli nie ma nagłówka, spróbuj z ciasteczka (dla F5)
        let cookies = parts.extract::<CookieJar>().await.unwrap();
        if let Some(cookie) = cookies.get("token") {
            let token_data = verify_jwt(cookie.value(), &state.jwt_secret)?;
            return Ok(token_data.claims);
        }

        // Jeśli obie metody zawiodą, sprawdź czy to żądanie HTML i przekieruj
        if let Some(accept_header) = parts.headers.get(axum::http::header::ACCEPT) {
            if let Ok(accept_str) = accept_header.to_str() {
                if accept_str.contains("text/html") {
                    return Err(AppError::RedirectToLogin);
                }
            }
        }

        // Jeśli to nie było żądanie HTML, zwróć standardowy błąd braku tokenu
        Err(AppError::MissingToken(
            "Brak tokenu autoryzacji w nagłowku lub ciasteczku.".into(),
        ))
    }
}

#[derive(Debug, Clone)]
pub struct OptionalTokenClaims(pub Option<TokenClaims>);

// NOWA, POPRAWIONA IMPLEMENTACJA
impl FromRequestParts<AppState> for OptionalTokenClaims {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        tracing::debug!(
            "Próba ekstrakcji OPCJONALNYCH danych uwierzytelniających z nagłówka lub ciasteczka."
        );

        // --- Metoda 1: Spróbuj z nagłówka 'Authorization' (dla HTMX i API) ---
        if let Ok(TypedHeader(Authorization(bearer))) =
            parts.extract::<TypedHeader<Authorization<Bearer>>>().await
        {
            let token_str = bearer.token();
            match verify_jwt(token_str, &state.jwt_secret) {
                Ok(claims_data) => {
                    tracing::debug!("Znaleziono poprawny token w nagłówku Authorization.");
                    // Zwracamy poprawny typ: OptionalTokenClaims
                    return Ok(OptionalTokenClaims(Some(claims_data.claims)));
                }
                Err(e) => {
                    tracing::warn!(
                        "Nieprawidłowy token w nagłówku Authorization (sprawdzam ciasteczko): {:?}",
                        e
                    );
                }
            }
        }

        // --- Metoda 2: Spróbuj z ciasteczka 'token' (dla odświeżenia strony F5) ---
        let cookies = CookieJar::from_headers(&parts.headers);
        if let Some(cookie) = cookies.get("token") {
            let token_str = cookie.value();
            match verify_jwt(token_str, &state.jwt_secret) {
                Ok(claims_data) => {
                    tracing::debug!("Znaleziono poprawny token w ciasteczku 'token'.");
                    // Zwracamy poprawny typ: OptionalTokenClaims
                    return Ok(OptionalTokenClaims(Some(claims_data.claims)));
                }
                Err(e) => {
                    tracing::warn!(
                        "Nieprawidłowy token w ciasteczku 'token' (traktuję jako brak tokenu): {:?}",
                        e
                    );
                }
            }
        }

        // --- Metoda 3: Jeśli obie metody zawiodły ---
        tracing::debug!(
            "Nie znaleziono poprawnego tokenu w nagłówku ani ciasteczku. Zwracam None."
        );
        // Zwracamy poprawny typ: OptionalTokenClaims z wartością None
        Ok(OptionalTokenClaims(None))
    }
}

#[derive(Debug, Clone)]
pub struct OptionalGuestCartId(pub Option<Uuid>);

impl<S> FromRequestParts<S> for TokenClaims
where
    Arc<AppState>: FromRef<S>, // Ten warunek pozwala Axumowi wyciągnąć Arc<AppState> ze stanu routera
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        // Wyciągamy nasz stan z generycznego stanu routera
        let state = Arc::<AppState>::from_ref(state);

        // Reszta kodu pozostaje bez zmian!
        if let Ok(TypedHeader(Authorization(bearer))) =
            parts.extract::<TypedHeader<Authorization<Bearer>>>().await
        {
            let token_data = verify_jwt(bearer.token(), &state.jwt_secret)?;
            return Ok(token_data.claims);
        }

        let cookies = CookieJar::from_headers(&parts.headers);
        if let Some(cookie) = cookies.get("token") {
            let token_data = verify_jwt(cookie.value(), &state.jwt_secret)?;
            return Ok(token_data.claims);
        }

        if let Some(accept_header) = parts.headers.get(axum::http::header::ACCEPT) {
            if let Ok(accept_str) = accept_header.to_str() {
                if accept_str.contains("text/html") {
                    return Err(AppError::RedirectToLogin);
                }
            }
        }

        Err(AppError::MissingToken(
            "Brak tokenu autoryzacji w nagłówku lub ciasteczku.".into(),
        ))
    }
}

impl<S> FromRequestParts<S> for OptionalTokenClaims
where
    Arc<AppState>: FromRef<S>, // Ten sam warunek co wyżej
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let state = Arc::<AppState>::from_ref(state);
        // Reszta logiki funkcji pozostaje identyczna...
        tracing::debug!("Próba ekstrakcji OPCJONALNYCH danych uwierzytelniających...");

        if let Ok(TypedHeader(Authorization(bearer))) =
            parts.extract::<TypedHeader<Authorization<Bearer>>>().await
        {
            if let Ok(claims_data) = verify_jwt(bearer.token(), &state.jwt_secret) {
                return Ok(OptionalTokenClaims(Some(claims_data.claims)));
            }
        }

        let cookies = CookieJar::from_headers(&parts.headers);
        if let Some(cookie) = cookies.get("token") {
            if let Ok(claims_data) = verify_jwt(cookie.value(), &state.jwt_secret) {
                return Ok(OptionalTokenClaims(Some(claims_data.claims)));
            }
        }

        Ok(OptionalTokenClaims(None))
    }
}

// Ten ekstraktor nie używa stanu, więc możemy go uprościć
impl<S> FromRequestParts<S> for OptionalGuestCartId
where
    S: Send + Sync, // Wystarczy tylko to
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &S, // Nie używamy stanu, więc _state
    ) -> Result<Self, Self::Rejection> {
        // Logika pozostaje bez zmian
        if let Ok(TypedHeader(XGuestCartId(guest_id))) =
            parts.extract::<TypedHeader<XGuestCartId>>().await
        {
            return Ok(OptionalGuestCartId(Some(guest_id)));
        }

        let cookies = CookieJar::from_headers(&parts.headers);
        if let Some(cookie) = cookies.get("guest_cart_id") {
            if let Ok(guest_id) = Uuid::parse_str(cookie.value()) {
                return Ok(OptionalGuestCartId(Some(guest_id)));
            }
        }

        Ok(OptionalGuestCartId(None))
    }
}
