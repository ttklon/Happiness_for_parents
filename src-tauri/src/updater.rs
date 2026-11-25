//! Self-update system.
//!
//! Every minute the UI asks GitHub for the latest release, compares versions
//! and, on request, downloads the installer and verifies its SHA-256 checksum
//! (published as a release asset) before launching it. Settings and results
//! live in %APPDATA% and are never touched by an update.

use serde::{Deserialize, Serialize};

const RELEASES_API: &str =
    "https://api.github.com/repos/ttklon/Happiness_for_parents/releases/latest";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateInfo {
    pub available: bool,
    pub version: String,
    pub current_version: String,
    pub notes: String,
    pub url: String,
    pub asset_name: String,
    pub size: u64,
    pub checksum: String,
    pub published_at: String,
}

fn version_parts(version: &str) -> Vec<u64> {
    version
        .trim_start_matches('v')
        .split(['.', '-', '+'])
        .filter_map(|part| part.parse::<u64>().ok())
        .collect()
}

fn version_newer(candidate: &str, current: &str) -> bool {
    let mut a = version_parts(candidate);
    let mut b = version_parts(current);
    let len = a.len().max(b.len());
    a.resize(len, 0);
    b.resize(len, 0);
    a > b
}

/// Update traffic follows the same proxy/Tor settings as the scanner, so
/// checking a release never reveals the real IP to GitHub either.
fn http_client(proxy: Option<String>) -> Result<reqwest::Client, String> {
    let mut builder = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .user_agent("HappinessScanner-Updater/0.6");
    if let Some(proxy) = proxy {
        builder = builder.proxy(reqwest::Proxy::all(&proxy).map_err(|e| e.to_string())?);
    }
    builder.build().map_err(|e| e.to_string())
}

async fn checksum_for(client: &reqwest::Client, assets: &[serde_json::Value], filename: &str) -> String {
    for asset in assets {
        if asset.get("name").and_then(|n| n.as_str()) != Some("checksums.txt") {
            continue;
        }
        if let Some(url) = asset.get("browser_download_url").and_then(|u| u.as_str()) {
            if let Ok(response) = client.get(url).send().await {
                if let Ok(text) = response.text().await {
                    for line in text.lines() {
                        let mut parts = line.split_whitespace();
                        if let (Some(hash), Some(name)) = (parts.next(), parts.next()) {
                            if name == filename || name.ends_with(&format!("/{filename}")) {
                                return hash.to_lowercase();
                            }
                        }
                    }
                }
            }
        }
    }
    String::new()
}

/// Fetch the newest public release and compare it with the compiled version.
pub async fn check(proxy: Option<String>) -> Result<UpdateInfo, String> {
    let current = env!("CARGO_PKG_VERSION").to_string();
    let client = http_client(proxy)?;
    let release: serde_json::Value = client
        .get(RELEASES_API)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;

    let version = release
        .get("tag_name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let assets = release
        .get("assets")
        .and_then(|a| a.as_array())
        .cloned()
        .unwrap_or_default();

    // Prefer a Windows .exe installer, fall back to the MSI package.
    let picked = assets
        .iter()
        .find(|a| {
            a.get("name")
                .and_then(|n| n.as_str())
                .map(|n| n.to_lowercase().ends_with("setup.exe"))
                .unwrap_or(false)
        })
        .or_else(|| {
            assets.iter().find(|a| {
                a.get("name")
                    .and_then(|n| n.as_str())
                    .map(|n| n.to_lowercase().ends_with(".msi"))
                    .unwrap_or(false)
            })
        });

    let (url, asset_name, size, checksum) = match picked {
        Some(asset) => {
            let name = asset
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or("installer")
                .to_string();
            let url = asset
                .get("browser_download_url")
                .and_then(|u| u.as_str())
                .ok_or("release asset has no download URL")?
                .to_string();
            if !(url.starts_with("https://github.com/")
                || url.starts_with("https://objects.githubusercontent.com/"))
            {
                return Err("отказ: ссылка обновления вне github.com".into());
            }
            let size = asset.get("size").and_then(|s| s.as_u64()).unwrap_or(0);
            let checksum = checksum_for(&client, &assets, &name).await;
            (url, name, size, checksum)
        }
        None => (String::new(), String::new(), 0, String::new()),
    };

    Ok(UpdateInfo {
        available: version_newer(&version, &current) && !url.is_empty(),
        version,
        current_version: current,
        notes: release
            .get("body")
            .and_then(|b| b.as_str())
            .unwrap_or("")
            .chars()
            .take(2000)
            .collect(),
        url,
        asset_name,
        size,
        checksum,
        published_at: release
            .get("published_at")
            .and_then(|p| p.as_str())
            .unwrap_or("")
            .to_string(),
    })
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::Digest;
    let digest = sha2::Sha256::digest(bytes);
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

/// Download, verify and launch the installer. The running app is not killed
/// and no application data is modified by this function.
pub async fn download_and_install(info: &UpdateInfo, proxy: Option<String>) -> Result<String, String> {
    if info.url.is_empty() || !(info.url.starts_with("https://github.com/")
        || info.url.starts_with("https://objects.githubusercontent.com/"))
    {
        return Err("небезопасная ссылка на обновление".into());
    }
    let client = http_client(proxy)?;
    let bytes = client
        .get(&info.url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .bytes()
        .await
        .map_err(|e| e.to_string())?
        .to_vec();

    if !info.checksum.is_empty() {
        let actual = sha256_hex(&bytes);
        if actual != info.checksum.trim().to_lowercase() {
            return Err(format!(
                "SHA-256 не совпал: ожидался {}, получен {} — установка отменена",
                info.checksum, actual
            ));
        }
    }

    let filename = if info.asset_name.ends_with(".msi") {
        "happiness-scanner-update.msi"
    } else {
        "happiness-scanner-update.exe"
    };
    let path = std::env::temp_dir().join(filename);
    std::fs::write(&path, &bytes).map_err(|e| e.to_string())?;

    #[cfg(target_os = "windows")]
    {
        if filename.ends_with(".msi") {
            std::process::Command::new("msiexec")
                .args(["/i"])
                .arg(&path)
                .args(["/qr", "/norestart"])
                .spawn()
                .map_err(|e| e.to_string())?;
        } else {
            std::process::Command::new(&path)
                .arg("/S")
                .spawn()
                .map_err(|e| e.to_string())?;
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = path;
    }

    Ok(format!(
        "Установщик {} проверен (SHA-256) и запущен. Настройки и результаты не сбрасываются.",
        info.asset_name
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_comparison() {
        assert!(version_newer("v0.6.1", "0.6.0"));
        assert!(version_newer("0.7.0", "0.6.9"));
        assert!(!version_newer("0.6.0", "0.6.0"));
        assert!(!version_newer("v0.5.9", "0.6.0"));
    }
}
