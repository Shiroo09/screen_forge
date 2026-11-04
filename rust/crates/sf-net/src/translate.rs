//! Text translation. Port of `ScreenForge/Translate/GoogleTextTranslateClient.cs`.
//!
//! The C# client tried several endpoints in order. This uses the simplest that
//! returns JSON: `clients5.google.com/translate_a/t` with `client=dict-chrome-ex`,
//! which answers with `[["translated","source",null,null,10]]` or a bare string
//! for a single word. The cached endpoint and batching in the original are left to
//! the caller.

use reqwest::blocking::Client;
use serde_json::Value;
use std::time::Duration;

const ENDPOINT: &str = "https://clients5.google.com/translate_a/t";
const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

/// A translation result: the text and the detected source language (if reported).
#[derive(Clone, PartialEq, Debug)]
pub struct Translation {
    pub text: String,
    pub detected_language: Option<String>,
}

/// Translates `text` from auto-detected source into `target_lang` (e.g. "tr").
pub fn translate(text: &str, target_lang: &str) -> Result<Translation, String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("çevrilecek metin boş".to_string());
    }
    if target_lang.trim().is_empty() {
        return Err("hedef dil boş".to_string());
    }

    let client = Client::builder()
        .timeout(Duration::from_secs(20))
        .user_agent(UA)
        .build()
        .map_err(|e| format!("HTTP istemcisi: {e}"))?;

    let response = client
        .get(ENDPOINT)
        .query(&[
            ("client", "dict-chrome-ex"),
            ("sl", "auto"),
            ("tl", target_lang),
            ("q", text),
        ])
        .header("Accept", "*/*")
        .header("Referer", "https://translate.google.com/")
        .send()
        .map_err(|e| format!("çeviri isteği: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("çeviri sunucusu HTTP {} döndürdü.", response.status().as_u16()));
    }

    let body = response.text().map_err(|e| format!("yanıt okunamadı: {e}"))?;
    parse_response(&body).ok_or_else(|| format!("çeviri yanıtı anlaşılamadı: {}", truncate(&body, 120)))
}

/// Parses the `dict-chrome-ex` response. Exposed for tests.
pub fn parse_response(body: &str) -> Option<Translation> {
    let value: Value = serde_json::from_str(body.trim()).ok()?;

    match value {
        // Single sentence: [["çeviri","kaynak",null,null,10]]
        Value::Array(items) => {
            let first = items.first()?;
            match first {
                Value::Array(parts) => {
                    let text = parts.first()?.as_str()?.to_string();
                    let detected = parts.get(1).and_then(|v| v.as_str()).map(str::to_string);
                    Some(Translation { text, detected_language: detected })
                }
                // A bare string result (single word).
                Value::String(s) => Some(Translation {
                    text: s.clone(),
                    detected_language: None,
                }),
                _ => None,
            }
        }
        _ => None,
    }
}

fn truncate(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}
