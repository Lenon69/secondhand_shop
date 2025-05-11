// src.cloudinary

use crate::{errors::AppError, state::CloudinaryConfig};
use reqwest::{Client, multipart};
use serde::Deserialize;
use sha1::{Digest, Sha1};
use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Deserialize)]
struct CloudinaryUploadResponse {
    secure_url: String,
}

// Struktura odpowiedzi z API usuwania (może nie być potrzebna, jeśli tylko sprawdzamy status)
#[derive(Debug, Deserialize)]
struct CloudinaryDeleteResponse {
    result: String,
}

// Funkcja do ekstrakcji public_id z URL-a Cloudinary
pub fn extract_public_id_from_url(url: &str, cloud_name: &str) -> Option<String> {
    let base = format!("https://res.cloudinary.com/{}/image/upload/", cloud_name);
    if url.starts_with(&base) {
        let remainder = &url[base.len()..]; // v1746734489/gz9tincppshabnuqlt4e.jpg
        // Usuwanie wersji
        let path_after_version;
        if remainder.starts_with("v") && remainder.contains('/') {
            path_after_version = remainder.split_once('/').map_or(remainder, |(_, p)| p);
        } else {
            path_after_version = remainder;
        }

        // Usuwanie rozszerzenia pliku
        let public_id_option = path_after_version
            .rsplit_once('.')
            .map(|(id, _)| id.to_string());
        tracing::debug!("Wyodrębniony public_id: {:?}", public_id_option);
        public_id_option
    } else {
        tracing::warn!("URL '{}' nie pasuje do oczekiwanej bazy Cloudinary", url);
        None
    }
}

pub async fn delete_image_from_cloudinary(
    public_id: &str,
    config: &CloudinaryConfig,
) -> Result<(), AppError> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| AppError::InternalServerError("Błąd czasu systemowego".to_string()))?
        .as_secs();

    // Parametry do podpisu dla żadania typu 'destroy'
    let mut params_to_sign = BTreeMap::new();
    params_to_sign.insert("public_id".to_string(), public_id.to_string());
    params_to_sign.insert("timestamp".to_string(), timestamp.to_string());

    let mut signature_string = params_to_sign
        .iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect::<Vec<String>>()
        .join("&");
    signature_string.push_str(&config.api_secret);

    let mut hasher = Sha1::new();
    hasher.update(signature_string.as_bytes());
    let signature = hex::encode(hasher.finalize());

    // Przygotowanie danych formularza
    let mut form_params = params_to_sign;
    form_params.insert("api_key".to_string(), config.api_key.clone());
    form_params.insert("signature".to_string(), signature);

    tracing::debug!(
        "Cloudinary config w delete_image: cloud_name='{}', api_key='{}'",
        config.cloud_name,
        config.api_key
    );
    let url = format!(
        "https://api.cloudinary.com/v1_1/{}/image/destroy",
        config.cloud_name,
    );
    tracing::debug!("URL do usunięcia obrazka: {}", url);

    let client = Client::new();
    let request_builder = client.post(&url).form(&form_params);

    tracing::debug!("Zabudowano żądanie usunięcia dla public_id: {}", public_id);

    let response_result = request_builder.send().await;

    match response_result {
        Ok(resp) => {
            if resp.status().is_success() {
                let delete_api_response =
                    resp.json::<CloudinaryDeleteResponse>().await.map_err(|e| {
                        tracing::error!(
                            "Błąd deserializacji odpowiedzi usuwania z Cloudinary: {}",
                            e
                        );
                        AppError::InternalServerError(
                            "Nie można przetworzyć odpowiedzi usunięcia z serwera obrazów."
                                .to_string(),
                        )
                    })?;

                if delete_api_response.result == "ok" || delete_api_response.result == "not found" {
                    tracing::info!(
                        "Obraz o public_id '{}' pomyślnie usunięty z Cloudinary (lub nie znaleziono). Wynik: {}",
                        public_id,
                        delete_api_response.result
                    );
                    Ok(()) // Jawny zwrot
                } else {
                    tracing::error!(
                        "Cloudinary zwróciło nieoczekiwany wynik przy usuwaniu obrazu '{}': {}",
                        public_id,
                        delete_api_response.result
                    );
                    Err(AppError::InternalServerError(format!(
                        "Serwer obrazów zwrócił nieoczekiwany wynik: {}",
                        delete_api_response.result
                    ))) // Jawny zwrot
                }
            } else {
                let status = resp.status();
                let error_text = resp
                    .text()
                    .await
                    .unwrap_or_else(|_| "Brak treści błędu".to_string());
                tracing::error!(
                    "Błąd usuwania obrazu z Cloudinary (public_id: {}): Status={}, Treść={}",
                    public_id,
                    status,
                    error_text
                );
                Err(AppError::InternalServerError(format!(
                    "Błąd podczas usuwania obrazu z serwera (status: {}).",
                    status
                ))) // Jawny zwrot
            }
        }
        Err(e) => {
            tracing::error!(
                "Błąd sieci (lub budowania) podczas usuwania obrazu z Cloudinary (public_id: {}): {:?}",
                public_id,
                e
            );
            if e.is_builder() {
                tracing::error!("Szczegóły błędu budowania Reqwest (usuwanie): {:?}", e);
            }
            Err(AppError::InternalServerError(format!(
                "Błąd połączenia/budowania żądania do serwera obrazów przy usuwaniu: {}",
                e
            ))) // Jawny zwrot
        }
    }
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
