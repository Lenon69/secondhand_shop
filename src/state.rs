// src/state.rs

use sqlx::PgPool;

#[derive(Clone)]
pub struct AppState {
    pub db_pool: PgPool,
    pub jwt_secret: String,
    pub jwt_expiration_hours: i64,
}
