//! Minimal self-updater. Replaces Velopack's role in `ScreenForge/Updates/AutoUpdateService.cs`.
//!
//! Velopack packages a delta installer and swaps it in; there is no Rust port of
//! it, so this implements the smallest correct version: fetch a version manifest
//! from a URL, compare with the running version, and (on request) download the
//! new executable next to the current one so the caller can relaunch it. It never
//! overwrites a running binary in place — Windows locks it — so the swap happens
//! via a `.new` file the next launch picks up.

use reqwest::blocking::Client;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// A release manifest, matching the shape Velopack's `releases.json` exposes.
#[derive(Clone, Debug, Deserialize)]
pub struct Release {
    /// Semantic version, e.g. "1.3.0".
    #[serde(alias = "Version", alias = "version")]
    pub version: String,
    /// Direct download URL for the platform package.
    #[serde(alias = "download_url", alias = "url")]
    pub url: String,
}

/// Compares two dotted numeric versions. Returns true when `remote` is newer.
pub fn is_newer(remote: &str, current: &str) -> bool {
    let parse = |v: &str| -> Vec<u32> {
        v.trim_start_matches('v')
            .split(['.', '-'])
            .map(|p| p.parse().unwrap_or(0))
            .collect()
    };
    let r = parse(remote);
    let c = parse(current);
    for i in 0..r.len().max(c.len()) {
        let a = r.get(i).copied().unwrap_or(0);
        let b = c.get(i).copied().unwrap_or(0);
        if a != b {
            return a > b;
        }
    }
    false
}

/// Checks the manifest URL for a newer release. `Ok(None)` means up to date.
pub fn check(manifest_url: &str, current_version: &str) -> Result<Option<Release>, String> {
    // A local path or file:// URL is read directly; anything else goes over HTTP.
    let body = if let Some(local) = local_path(manifest_url) {
        std::fs::read_to_string(&local).map_err(|e| format!("manifest okunamadı ({local}): {e}"))?
    } else {
        let client = Client::builder()
            .timeout(Duration::from_secs(20))
            .build()
            .map_err(|e| format!("HTTP istemcisi: {e}"))?;
        let response = client
            .get(manifest_url)
            .send()
            .map_err(|e| format!("güncelleme kontrolü: {e}"))?;
        if !response.status().is_success() {
            return Err(format!("güncelleme sunucusu HTTP {}", response.status().as_u16()));
        }
        response.text().map_err(|e| format!("yanıt: {e}"))?
    };

    // The manifest may be a single object or a list; accept both.
    let release = if let Ok(list) = serde_json::from_str::<Vec<Release>>(&body) {
        list.into_iter().max_by(|a, b| {
            // Newest by our own comparison.
            if is_newer(&a.version, &b.version) {
                std::cmp::Ordering::Greater
            } else if is_newer(&b.version, &a.version) {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Equal
            }
        })
    } else {
        serde_json::from_str::<Release>(&body).ok()
    };

    match release {
        Some(r) if is_newer(&r.version, current_version) => Ok(Some(r)),
        _ => Ok(None),
    }
}

/// Interprets `source` as a local path when it is not an http(s) URL.
fn local_path(source: &str) -> Option<String> {
    if source.starts_with("http://") || source.starts_with("https://") {
        None
    } else if let Some(rest) = source.strip_prefix("file://") {
        Some(rest.to_string())
    } else {
        Some(source.to_string())
    }
}

/// Downloads `url` to `<current_exe>.new`. Returns the path written.
pub fn download_to_new(url: &str) -> Result<PathBuf, String> {
    let current = std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?;
    let target = PathBuf::from(format!("{}.new", current.display()));

    let client = Client::builder()
        .timeout(Duration::from_secs(600))
        .build()
        .map_err(|e| format!("HTTP istemcisi: {e}"))?;
    let mut response = client.get(url).send().map_err(|e| format!("indirme: {e}"))?;
    if !response.status().is_success() {
        return Err(format!("indirme HTTP {}", response.status().as_u16()));
    }

    let mut file = std::fs::File::create(&target).map_err(|e| format!("dosya: {e}"))?;
    std::io::copy(&mut response, &mut file).map_err(|e| format!("yazma: {e}"))?;
    Ok(target)
}

/// If a `<exe>.new` sits next to the running binary, swaps it in and returns the
/// path the caller should relaunch — the update-applied step of a self-updater.
pub fn apply_pending() -> Result<Option<PathBuf>, String> {
    let current = std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?;
    let pending = PathBuf::from(format!("{}.new", current.display()));
    if !pending.exists() {
        return Ok(None);
    }

    // The running exe is locked; move it aside and put the new one in place.
    let backup = PathBuf::from(format!("{}.old", current.display()));
    let _ = std::fs::remove_file(&backup);
    std::fs::rename(&current, &backup).map_err(|e| format!("eski sürüm taşınamadı: {e}"))?;
    std::fs::rename(&pending, &current).map_err(|e| format!("yeni sürüm yerleştirilemedi: {e}"))?;
    let _ = std::fs::remove_file(&backup);

    Ok(Some(current))
}

/// True when `path` looks like a downloaded update ready to apply.
pub fn has_pending(path: &Path) -> bool {
    PathBuf::from(format!("{}.new", path.display())).exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_comparison() {
        assert!(is_newer("1.3.0", "1.2.8"));
        assert!(is_newer("2.0.0", "1.9.9"));
        assert!(is_newer("1.2.10", "1.2.9"));
        assert!(!is_newer("1.2.8", "1.2.8"));
        assert!(!is_newer("1.2.7", "1.2.8"));
        assert!(is_newer("v1.3.0", "1.2.8"));
    }
}
