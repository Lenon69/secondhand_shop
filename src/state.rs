// src/state.rs

use sqlx::PgPool;

#[derive(Clone)]
pub struct AppState {
    pub db_pool: PgPool,
    pub jwt_secret: String,
    pub jwt_expiration_hours: i64,
    pub cloudinary_config: CloudinaryConfig,
    pub resend_api_key: String,
}

#[derive(Clone)]
pub struct CloudinaryConfig {
    pub cloud_name: String,
    pub api_key: String,
    pub api_secret: String,
}
