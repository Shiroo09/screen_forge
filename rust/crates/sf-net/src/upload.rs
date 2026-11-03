//! Litterbox upload helpers. Port of `ScreenForge/Upload/LitterboxUploadProvider.cs`.
//!
//! The pure decisions only: what may be uploaded, what the server receives, and
//! how the returned URL is read out of the response body. The multipart request
//! itself belongs to the app shell.

pub const ENDPOINT: &str = "https://litterbox.catbox.moe/resources/internals/api.php";
pub const KEEP_FOR: &str = "72h";
pub const BROWSER_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";
pub const MAX_BYTES: u64 = 1024 * 1024 * 1024;

/// Checks a file path for upload eligibility. Returns `Ok(())` or the message
/// the UI shows.
pub fn check_allowed(path: &str) -> Result<(), &'static str> {
    let ext = extension_lower(path);
    if ext == ".exe"
        || ext == ".scr"
        || ext == ".cpl"
        || ext == ".jar"
        || ext.starts_with(".doc")
    {
        return Err("Bu dosya türü yüklenemiyor.");
    }

    match std::fs::metadata(path) {
        Ok(meta) if meta.len() > MAX_BYTES => Err("Dosya boyutu 1 GB sınırını aşıyor."),
        Ok(_) => Ok(()),
        Err(_) => Err("Dosya okunamadı."),
    }
}

/// The filename sent to the server: the caller's name cleaned of header-breaking
/// characters, or a MIME-derived default.
pub fn safe_upload_name(file_name: Option<&str>, mime_type: &str) -> String {
    let name = match file_name {
        Some(f) if !f.trim().is_empty() => f
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or("")
            .replace(['"', '\r', '\n'], "")
            .trim()
            .to_string(),
        _ => String::new(),
    };

    if !name.is_empty() && name != "." && name != ".." {
        return name;
    }

    let ext = match mime_type.to_lowercase().as_str() {
        "image/jpeg" => "jpg",
        "image/webp" => "webp",
        "image/gif" => "gif",
        _ => "png",
    };
    format!("screenforge.{ext}")
}

/// First `http(s)://` token in the response body — the uploaded file's URL.
pub fn extract_url(body: &str) -> Option<String> {
    let body = body.trim();
    let start = body.find("http://").or_else(|| body.find("https://"))?;
    let rest = &body[start..];
    let end = rest
        .find(|c: char| c.is_whitespace())
        .unwrap_or(rest.len());
    let url = &rest[..end];
    if url.is_empty() {
        None
    } else {
        Some(url.to_string())
    }
}

fn extension_lower(path: &str) -> String {
    match path.rfind('.') {
        Some(i) => path[i..].to_lowercase(),
        None => String::new(),
    }
}
