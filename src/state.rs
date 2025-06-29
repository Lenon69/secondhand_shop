// src/state.rs

use moka::future::Cache;
use sqlx::PgPool;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub db_pool: PgPool,
    pub jwt_secret: String,
    pub jwt_expiration_hours: i64,
    pub cloudinary_config: CloudinaryConfig,
    pub resend_api_key: String,
    pub html_cache: Arc<Cache<String, String>>,
}

#[derive(Clone)]
pub struct CloudinaryConfig {
    pub cloud_name: String,
    pub api_key: String,
    pub api_secret: String,
}
