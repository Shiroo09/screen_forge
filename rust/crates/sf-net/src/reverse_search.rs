//! Reverse-image-search helpers. Port of `ScreenForge/Search/ReverseImageSearch.cs`.
//!
//! Only the pure parts: the Lens upload HTML and the Yandex CBIR response parser.
//! The HTTP calls live with the app shell.

use base64::Engine as _;
use serde_json::Value;

pub const CHROME_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

/// Self-submitting HTML that posts the image to Google Lens. Written to a temp
/// file and opened in the browser. `timestamp_ms` is passed in (C# used `UtcNow`).
pub fn build_lens_upload_html(png_bytes: &[u8], timestamp_ms: i64) -> String {
    let b64 = base64::engine::general_purpose::STANDARD.encode(png_bytes);
    let mut sb = String::with_capacity(b64.len() + 512);
    sb.push_str("<!DOCTYPE html><html><head><meta charset=\"utf-8\"><title>Google Lens</title></head><body>");
    sb.push_str("<form id=\"f\" method=\"POST\" enctype=\"multipart/form-data\" action=\"https://lens.google.com/v3/upload?ep=ccm&amp;s=&amp;st=");
    sb.push_str(&timestamp_ms.to_string());
    sb.push_str("\"><input id=\"img\" type=\"file\" name=\"encoded_image\"></form>");
    sb.push_str("<script>");
    sb.push_str("const b64=\"");
    sb.push_str(&b64);
    sb.push_str("\";const bin=atob(b64);const bytes=new Uint8Array(bin.length);");
    sb.push_str("for(let i=0;i<bin.length;i++)bytes[i]=bin.charCodeAt(i);");
    sb.push_str("const file=new File([bytes],\"image.png\",{type:\"image/png\"});");
    sb.push_str("const dt=new DataTransfer();dt.items.add(file);");
    sb.push_str("document.getElementById(\"img\").files=dt.files;");
    sb.push_str("document.getElementById(\"f\").submit();");
    sb.push_str("</script></body></html>");
    sb
}

/// Parses the Yandex CBIR JSON to extract the image id and (optional) source URL.
/// Returns `None` when the payload is not the expected shape.
pub fn try_parse_yandex_cbir(json: &str) -> Option<(String, Option<String>)> {
    if json.trim().is_empty() {
        return None;
    }

    let root: Value = serde_json::from_str(json).ok()?;

    let blocks = root.get("blocks")?.as_array()?;
    let first = blocks.first()?;
    let params = first.get("params")?;

    let id = params.get("cbirId")?.as_str()?;
    if id.trim().is_empty() {
        return None;
    }

    let original = params
        .get("originalImageUrl")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.to_string());

    Some((id.to_string(), original))
}
