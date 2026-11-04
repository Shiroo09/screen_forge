//! Live reverse-image-search requests. Pairs with the pure helpers in
//! `reverse_search.rs`. Port of the network path in
//! `ScreenForge/Search/ReverseImageSearch.cs`.
//!
//! Yandex: POST the image to its CBIR uploader, read `cbirId` from the JSON, and
//! return the search URL. Google Lens: write the self-submitting HTML to a temp
//! file and hand back its path, which the caller opens in the browser.

use reqwest::blocking::{multipart, Client};
use std::time::Duration;

use crate::reverse_search::{build_lens_upload_html, try_parse_yandex_cbir, CHROME_UA};

const YANDEX_COM: &str = "https://yandex.com";
const YANDEX_RU: &str = "https://yandex.ru";
const REQUEST_JSON: &str = r#"{"blocks":[{"block":"cbir-uploader__get-cbir-id"}]}"#;

/// Which engine to search with.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Engine {
    #[default]
    Yandex,
    GoogleLens,
}

/// Returns a URL the browser can open for the reverse image search.
pub fn search_url(png_bytes: &[u8], engine: Engine) -> Result<String, String> {
    if png_bytes.is_empty() {
        return Err("Empty image.".to_string());
    }

    match engine {
        Engine::GoogleLens => {
            let html = build_lens_upload_html(png_bytes, now_ms());
            let path = std::env::temp_dir().join(format!("ScreenForge-lens-{}.html", std::process::id()));
            std::fs::write(&path, html).map_err(|e| format!("temp file: {e}"))?;
            Ok(path.to_string_lossy().into_owned())
        }
        Engine::Yandex => {
            let client = Client::builder()
                .timeout(Duration::from_secs(45))
                .user_agent(CHROME_UA)
                .build()
                .map_err(|e| format!("HTTP istemcisi: {e}"))?;

            for origin in [YANDEX_COM, YANDEX_RU] {
                if let Some(url) = try_yandex_origin(&client, origin, png_bytes)? {
                    return Ok(url);
                }
            }
            Err("Yandex görsel araması başarısız.".to_string())
        }
    }
}

fn try_yandex_origin(
    client: &Client,
    origin: &str,
    png_bytes: &[u8],
) -> Result<Option<String>, String> {
    let encoded: String = REQUEST_JSON
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect();
    let url = format!("{origin}/images/touch/search?rpt=imageview&format=json&request={encoded}");

    let part = multipart::Part::bytes(png_bytes.to_vec())
        .file_name("image.png")
        .mime_str("image/png")
        .map_err(|e| e.to_string())?;
    let form = multipart::Form::new().part("upfile", part);

    let response = match client
        .post(&url)
        .header("X-Requested-With", "XMLHttpRequest")
        .multipart(form)
        .send()
    {
        Ok(r) => r,
        Err(_) => return Ok(None),
    };

    if !response.status().is_success() {
        return Ok(None);
    }

    let body = response.text().map_err(|e| e.to_string())?;
    let Some((cbir_id, original)) = try_parse_yandex_cbir(&body) else {
        return Ok(None);
    };

    let mut result = format!("{origin}/images/search?cbir_id={}&rpt=imageview", encode(&cbir_id));
    if let Some(orig) = original {
        result.push_str("&url=");
        result.push_str(&encode(&orig));
    }
    Ok(Some(result))
}

fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
