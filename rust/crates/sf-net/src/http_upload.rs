//! Live Litterbox upload over HTTP. Pairs with the pure helpers in `upload.rs`.
//! Port of the request path in `ScreenForge/Upload/LitterboxUploadProvider.cs`.
//!
//! Uses a blocking client so callers need no async runtime; the C# provider was
//! async, but the upload is a single request and blocking keeps the CLI simple.

use reqwest::blocking::{multipart, Client};
use std::time::Duration;

use crate::upload::{extract_url, safe_upload_name, ENDPOINT, KEEP_FOR, MAX_BYTES};

/// Uploads `bytes` and returns the public URL.
///
/// Retries once on an empty body, mirroring the C# double-post. Errors are
/// returned as strings the UI can show directly.
pub fn upload(
    bytes: &[u8],
    mime_type: &str,
    file_name: Option<&str>,
) -> Result<String, String> {
    if bytes.is_empty() {
        return Err("Geçersiz dosya verisi.".to_string());
    }
    if bytes.len() as u64 > MAX_BYTES {
        return Err("Dosya boyutu 1 GB sınırını aşıyor.".to_string());
    }

    let client = Client::builder()
        .timeout(Duration::from_secs(15 * 60))
        .user_agent(crate::upload::BROWSER_USER_AGENT)
        .build()
        .map_err(|e| format!("HTTP istemcisi: {e}"))?;

    match post_once(&client, bytes, mime_type, file_name)? {
        Some(url) => Ok(url),
        None => post_once(&client, bytes, mime_type, file_name)?
            .ok_or_else(|| "Yükleme reddedildi: Sunucu boş yanıt verdi.".to_string()),
    }
}

fn post_once(
    client: &Client,
    bytes: &[u8],
    mime_type: &str,
    file_name: Option<&str>,
) -> Result<Option<String>, String> {
    let mime = if mime_type.trim().is_empty() {
        "application/octet-stream"
    } else {
        mime_type
    };

    let part = multipart::Part::bytes(bytes.to_vec())
        .file_name(safe_upload_name(file_name, mime))
        .mime_str(mime)
        .map_err(|e| format!("multipart: {e}"))?;

    let form = multipart::Form::new()
        .text("reqtype", "fileupload")
        .text("time", KEEP_FOR)
        .part("fileToUpload", part);

    let response = client
        .post(ENDPOINT)
        .multipart(form)
        .send()
        .map_err(|e| format!("Yükleme isteği: {e}"))?;

    let status = response.status();
    let body = response.text().map_err(|e| format!("Yanıt okunamadı: {e}"))?;

    if !status.is_success() {
        return Err(format!("Yükleme sunucusu HTTP {} döndürdü.", status.as_u16()));
    }

    Ok(extract_url(&body))
}

/// Uploads a file from disk, guessing the MIME type from its extension.
pub fn upload_file(path: &str) -> Result<String, String> {
    crate::upload::check_allowed(path)?;
    let bytes = std::fs::read(path).map_err(|e| format!("Dosya okunamadı: {e}"))?;
    let mime = mime_for(path);
    let name = std::path::Path::new(path)
        .file_name()
        .and_then(|n| n.to_str());
    upload(&bytes, mime, name)
}

fn mime_for(path: &str) -> &'static str {
    let lower = path.to_lowercase();
    if lower.ends_with(".png") {
        "image/png"
    } else if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        "image/jpeg"
    } else if lower.ends_with(".webp") {
        "image/webp"
    } else if lower.ends_with(".gif") {
        "image/gif"
    } else if lower.ends_with(".mp4") {
        "video/mp4"
    } else {
        "application/octet-stream"
    }
}
