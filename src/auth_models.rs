// src/auth_models.rs
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

pub use crate::models::Role;

#[derive(Debug, Deserialize, Validate)]
pub struct RegistrationPayload {
    #[validate(email(message = "Niepoprawny format adresu email"))]
    pub email: String,

    #[validate(length(min = 8, message = "Hasło musi mieć conajmniej 8 znaków"))]
    pub password: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct LoginPayload {
    #[validate(email(message = "Niepoprawny format adresu email"))]
    pub email: String,

    #[validate(length(min = 1, message = "Hasło jest wymagane"))]
    pub password: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TokenClaims {
    pub sub: Uuid,
    pub role: Role,
    pub exp: i64,
    pub iat: i64,
}
