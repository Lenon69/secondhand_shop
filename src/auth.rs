// src/auth.rs
pub use crate::auth_models::{Role, TokenClaims};
use crate::errors::AppError;
use argon2::Argon2;
use argon2::password_hash::{
    PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng,
};
use chrono::{Duration, Utc};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, TokenData, Validation, decode, encode};
use uuid::Uuid;

/// Funkcja do hashowania hasła
pub fn hash_password(password: &str) -> Result<String, AppError> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|_| AppError::InternalServerError("Błąd podczas hashowania hasła".to_string()))?
        .to_string();

    Ok(password_hash)
}

// Funkcja do weryfikacji hasła
pub fn verify_password(hashed_password: &str, password: &str) -> Result<bool, AppError> {
    let parsed_hash =
        PasswordHash::new(hashed_password).map_err(|_| AppError::InvalidCredentials)?;

    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok())
}

//Funkcja do generowania JWT
pub fn create_jwt(
    user_id: Uuid,
    role: Role,
    secret: &str,
    expiration_hours: i64,
) -> Result<String, AppError> {
    let now = Utc::now();
    let expiration_time = now + Duration::hours(expiration_hours);

    let claims = TokenClaims {
        sub: user_id,
        role,
        exp: expiration_time.timestamp(),
        iat: now.timestamp(),
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_ref()),
    )
    .map_err(|e| AppError::InternalServerError(format!("Błąd podczas tworzenia JWT: {}", e)))
}
// Funkcja do weryfikacji JWT

// Funkcja do weryfikacji JWT (przyda się później w middleware)
pub fn verify_jwt(token: &str, secret: &str) -> Result<TokenData<TokenClaims>, AppError> {
    decode::<TokenClaims>(
        token,
        &DecodingKey::from_secret(secret.as_ref()),
        &Validation::default(), // Domyślna walidacja sprawdza m.in. 'exp'
    )
    .map_err(|e| match e.kind() {
        jsonwebtoken::errors::ErrorKind::ExpiredSignature => AppError::TokenExpired,
        _ => AppError::InvalidToken,
    })
}
