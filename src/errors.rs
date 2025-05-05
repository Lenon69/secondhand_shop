use axum::{
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};

use jsonwebtoken::Validation;
use serde_json::json;
use sqlx::types::Json;
use validator::{Validate, ValidationErrors};

#[derive(Debug)]
pub enum AppError {
    SqlxError(sqlx::Error),
    NotFound,
    ValidationError(ValidationErrors),
    InvalidCredentials,
    EmailAlreadyExists,
    InvalidToken,
    MissingToken,
    TokenExpired,
    UnauthorizedAccess,
    InterenalServerError(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_body) = match self {
            AppError::SqlxError(sqlx_error) => {
                tracing::error!("Błąd SQLx: {:?}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    json!({"error": "Wystąpił wewnętrzny błąd serwera (baza danych)"}),
                )
            }
            AppError::NotFound => (
                StatusCode::NOT_FOUND,
                json!({"error": "Zasób nie został znaleziony"}),
            ),
            AppError::ValidationError(validation_errors) => {
                tracing::warn!("Błąd walidacji danych: {:?}", validation_errors);
                (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    json!({"error": validation_errors}),
                )
            }
            AppError::InvalidCredentials => (
                StatusCode::UNAUTHORIZED,
                json!({"error": "Niepoprawny email lub hasło"}),
            ),
            AppError::EmailAlreadyExists => (
                StatusCode::CONFLICT,
                json!({"error": "Użytkownik o podanym adresie email już istnieje"}),
            ),
            AppError::InvalidToken | AppError::MissingToken => (
                StatusCode::UNAUTHORIZED,
                json!({"error": "Nieprawidłowy lub brakujący token uwierzytelniający"}),
            ),
            AppError::TokenExpired => (
                StatusCode::UNAUTHORIZED,
                json!({"error": "Token uwierzytelniający wygasł"}),
            ),
            AppError::UnauthorizedAccess => (
                StatusCode::FORBIDDEN,
                json!({"error": "Brak uprawnień do wykonania tej akcji"}),
            ),
            AppError::InterenalServerError(_) => {
                tracing::error!("Wewnętrzny błąd serwera: {}", message);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    json!({"error": "Wystąpił wewnętrzny błąd serwera"}),
                )
            }
        };

        (status, Json(error_body)).into_response()
    }
}

// Konwersja dla operatora '?'
impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        match err {
            sqlx::Error::Database(database_error) if database_error.is_unique_violation() =>
                tracing::warn!("Naruszenie ograniczenia unikalności: {:?}", database_error);
                AppError::InterenalServerError("Naruszenie unikalności danych".to_string())
            } 
            sqlx::Error::RowNotFound => AppError::NotFound,
            _ => AppError::SqlxError(err)
        }
    }
}

impl From<ValidationErrors> for AppError {
    fn from(err: ValidationErrors) -> Self {
        AppError::ValidationError(err)
        
    }
}

impl From<jsonwebtoken::errors::Error> for AppError {
    fn from(err: jsonwebtoken::errors::Error) -> Self {
        match err.kind() {
            jsonwebtoken::errors::ErrorKind::ExpiredSignature => AppError::TokenExpired,
            _ => AppError::InvalidToken, 
        }
    }
}


