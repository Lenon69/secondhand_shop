use axum::{
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};

use serde_json::json;
use thiserror::Error;
use validator::ValidationErrors;

#[allow(dead_code)]
#[derive(Debug, Error)]
pub enum AppError {
    #[error("Błąd SQLx: {0}")]
    SqlxError(#[from] sqlx::Error),

    #[error("Nie znaleziono zasobu")]
    NotFound,

    #[error("Błędy walidacji")]
    ValidationError(#[from] ValidationErrors),

    #[error("Nieprawidłowe dane wejściowe: {0}")]
    UnprocessableEntity(String),

    #[error("Email już istnieje: {0}")]
    EmailAlreadyExists(String),

    #[error("Nieprawidłowe dane logowania")]
    InvalidLoginCredentials,

    #[error("Brak wymaganego tokenu: {0}")]
    MissingToken(String),

    #[error("Token wygasł")]
    TokenExpired,

    #[error("Nieprawidłowy token: {0}")]
    InvalidToken(String),

    #[error("Błąd generowania hasła")]
    PasswordHashingError,

    #[error("Nieautoryzowany dostęp: {0}")]
    UnauthorizedAccess(String),

    #[error("Wewnętrzny błąd serwera")]
    InternalServerError(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            AppError::SqlxError(sqlx_error) => {
                tracing::error!("Błąd SQLx: {:?}", sqlx_error);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Wystąpił wewnętrzny błąd serwera (baza danych)".to_string(),
                )
            }
            AppError::NotFound => (StatusCode::NOT_FOUND, "Nie znaleziono zasobu".to_string()),
            AppError::ValidationError(errors) => {
                let mut messages = Vec::new();
                for (field, field_errors) in errors.field_errors() {
                    for error in field_errors {
                        let msg = error.message.as_ref().map_or_else(
                            || format!("Pole '{}' jest nieprawidłowe", field),
                            |m| format!("Pole '{}': {}", field, m),
                        );
                        messages.push(msg);
                    }
                }
                (StatusCode::UNPROCESSABLE_ENTITY, messages.join("; "))
            }
            AppError::UnprocessableEntity(message) => (StatusCode::UNPROCESSABLE_ENTITY, message),
            AppError::EmailAlreadyExists(message) => (StatusCode::CONFLICT, message),
            AppError::InvalidLoginCredentials => (
                StatusCode::UNAUTHORIZED,
                "Nieprawidłowe dane logowania".to_string(),
            ),
            AppError::MissingToken(message) => (StatusCode::UNAUTHORIZED, message), // NOWA obsługa
            AppError::TokenExpired => (StatusCode::UNAUTHORIZED, "Token wygasł".to_string()),
            AppError::InvalidToken(message) => (StatusCode::UNAUTHORIZED, message), // ZAKTUALIZOWANA obsługa
            AppError::PasswordHashingError => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Błąd podczas przetwarzania hasła".to_string(),
            ),
            AppError::UnauthorizedAccess(message) => (StatusCode::FORBIDDEN, message),
            AppError::InternalServerError(message) => (StatusCode::INTERNAL_SERVER_ERROR, message),
        };

        let body = Json(json!({ "error": error_message }));
        (status, body).into_response()
    }
}

// Konwersja dla operatora '?'
// impl From<sqlx::Error> for AppError {
//     fn from(err: sqlx::Error) -> Self {
//         // Można tu dodać logikę mapowania konkretnych błędów sqlx
//         // np. unikalności na EmailAlreadyExists, jeśli to możliwe i sensowne
//         match err {
//             sqlx::Error::Database(db_err) if db_err.is_unique_violation() => {
//                 // To może być zbyt ogólne, jeśli masz inne ograniczenia UNIQUE
//                 // Lepiej sprawdzać konkretny constraint jeśli to konieczne
//                 tracing::warn!("Naruszenie ograniczenia unikalności: {:?}", db_err);
//                 // Zwracamy bardziej ogólny błąd, bo nie wiemy, które pole naruszyło unikalność
//                 // W handlerze register sprawdzamy email jawnie, co jest lepsze
//                 AppError::InternalServerError("Naruszenie unikalności danych".to_string())
//             }
//             sqlx::Error::RowNotFound => AppError::NotFound, // Mapuj RowNotFound na NotFound
//             _ => AppError::SqlxError(err),
//         }
//     }
// }

// impl From<ValidationErrors> for AppError {
//     fn from(err: ValidationErrors) -> Self {
//         AppError::ValidationError(err)
//     }
// }

impl From<jsonwebtoken::errors::Error> for AppError {
    fn from(err: jsonwebtoken::errors::Error) -> Self {
        match err.kind() {
            jsonwebtoken::errors::ErrorKind::ExpiredSignature => AppError::TokenExpired,
            _ => AppError::InvalidToken("Token JWT jest nieprawidłowy lub uszkodzony".to_string()),
        }
    }
}
