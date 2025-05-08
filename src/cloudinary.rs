// src.cloudinary

use crate::{errors::AppError, state::CloudinaryConfig};
use reqwest::{Client, multipart};
use serde::Deserialize;
use sha1::{Digest, Sha1};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Deserialize)]
struct CloudinaryUploadResponse {
    secure_url: String,
}

pub async fn upload_image_to_cloudinary(
    image_bytes: Vec<u8>,
    filename: String,
    config: &CloudinaryConfig,
) -> Result<String, AppError> {
    // Generowanie timestampu
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| AppError::InternalServerError("Błąd czasu systemowego".to_string()))?
        .as_secs();

    // Przygotowanie parametrów do podpisu
    let params_to_sign = format!("timestamp={}", timestamp);

    // Dodanie sekretu API do stringu do podpisania
    let string_to_sign = format!("{}{}", params_to_sign, config.api_secret);

    // Obliczenie SHA-1
    let mut hasher = Sha1::new();
    hasher.update(string_to_sign.as_bytes());
    let signature_bytes = hasher.finalize();

    // Konwersja wyniku SHA-1 na string heksadecymalny
    let signature = hex::encode(signature_bytes);

    // Przygotowanie danych mulitpart
    let part = multipart::Part::bytes(image_bytes)
        .file_name(filename)
        .mime_str("image/*")
        .map_err(|e| {
            tracing::error!("Błąd ustawiania typu MIME: {}", e);
            AppError::InternalServerError("Wewnętrzny błąd podczas przygotowania pliku".to_string())
        })?;

    let form = reqwest::multipart::Form::new()
        .part("file", part)
        .text("api_key", config.api_key.clone())
        .text("timestamp", timestamp.to_string())
        .text("signature", signature);

    // URL API Cloudinar
    let url = format!(
        "https://api.cloudinary.com/v1_1/{}/image/upload",
        config.cloud_name
    );

    // Wysyłanie żądania
    let client = Client::new();
    let resposne = client.post(&url).multipart(form).send().await;

    match resposne {
        Ok(resp) => {
            if resp.status().is_success() {
                let upload_result = resp.json::<CloudinaryUploadResponse>().await;
                match upload_result {
                    Ok(result) => Ok(result.secure_url),
                    Err(e) => {
                        tracing::error!("Błąd deserializacji odpowiedzi Cloudinary: {}", e);
                        Err(AppError::InternalServerError(
                            "Nie można przetworzyć odpowiedzi z serwera obrazów".to_string(),
                        ))
                    }
                }
            } else {
                let status = resp.status();
                let error_text = resp
                    .text()
                    .await
                    .unwrap_or_else(|_| "Brak treści błędu".to_string());
                tracing::error!(
                    "Błąd uploadu do Cloudinary: Status={}, Treść={}",
                    status,
                    error_text
                );
                Err(AppError::InternalServerError(format!(
                    "Błąd podczas wysyłania obrazu (status: {})",
                    status
                )))
            }
        }
        Err(e) => {
            tracing::error!("Błąd sieci podczas komunikacji z Cloudinary: {}", e);
            Err(AppError::InternalServerError(
                "Błąd połączenia z serwerem obrazów".to_string(),
            ))
        }
    }
}
