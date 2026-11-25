//! Settings persistence and validation.
//!
//! The config lives in `%APPDATA%\Happiness Scanner\config.json` on Windows.
//! `#[serde(default)]` means older configs keep working when new fields appear.

use serde::{Deserialize, Serialize};
use tauri::Manager;

pub const COINS: [&str; 13] = [
    "BTC", "ETH", "LTC", "DOGE", "BCH", "MATIC", "BSC", "ARB", "OPT", "BASE", "AVAX",
    "SOL", "TON",
];

pub const MODES: [&str; 15] = [
    "ALL_COINS_AUTO",
    "RANDOM",
    "BRAIN_WALLET",
    "BRAIN_COMBOS",
    "MULTI_WORD_COMBO",
    "BIP39_PASSPHRASE",
    "TARGET_ADDRESSES",
    "MULTI_COIN_BRAIN",
    "TRANSACTION_HISTORY",
    "BLOOM_FILTER",
    "VANITY_PATTERN",
    "SEQUENTIAL_KEYS",
    "RICH_LIST",
    "PUZZLE",
    "DICTIONARY",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    // --- load level -------------------------------------------------------
    pub workers: usize,
    pub batch_size: usize,
    pub delay_ms: u64,

    // --- what to search ---------------------------------------------------
    pub coin: String,
    pub scan_mode: String,
    pub address_type: String,
    pub bip39_language: String,
    pub vanity_prefix: String,
    pub dictionary_file: String,
    pub save_empty: bool,
    /// Check all four derivation paths (BIP44/49/84/86) for every phrase.
    pub derive_all_paths: bool,
    /// File with one target address per line (TARGET_ADDRESSES mode).
    pub target_addresses_file: String,
    /// Never write recovery phrases into the UI log.
    pub privacy_log_phrases: bool,
    /// Check GitHub releases once a minute for a new version.
    pub update_check_enabled: bool,

    // --- cache ------------------------------------------------------------
    pub address_cache_enabled: bool,
    pub address_cache_ttl_minutes: u64,

    // --- notifications ----------------------------------------------------
    pub telegram_enabled: bool,
    pub telegram_token: String,
    pub telegram_chat_id: String,
    pub discord_enabled: bool,
    pub discord_webhook: String,

    // --- anonymity --------------------------------------------------------
    /// Master switch: one click applies every protection below.
    pub full_anonymity_mode: bool,
    pub anonymity_mode: bool,
    pub use_tor: bool,
    pub tor_proxy: String,
    pub proxy_url: String,
    /// Additional proxies, one per line, rotated between API requests.
    pub proxy_list: String,
    pub proxy_rotation: bool,
    /// Random pause added to the configured delay (load / fingerprint shaping).
    pub jitter_min_ms: u64,
    pub jitter_max_ms: u64,
    pub random_user_agent: bool,
    pub random_headers: bool,
    pub stealth_mode: bool,
    pub stealth_window_title: bool,
    pub encryption_enabled: bool,
    pub encryption_password: String,
    pub memory_only_mode: bool,
    pub kill_switch: bool,
    pub kill_switch_threshold: u32,
    pub block_direct_connection: bool,

    // --- system -----------------------------------------------------------
    pub autostart: bool,
    pub start_minimized: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            workers: 2,
            batch_size: 25,
            delay_ms: 1200,
            coin: "ALL".to_string(),
            scan_mode: "ALL_COINS_AUTO".to_string(),
            address_type: "auto".to_string(),
            bip39_language: "english".to_string(),
            vanity_prefix: String::new(),
            dictionary_file: String::new(),
            save_empty: false,
            derive_all_paths: true,
            target_addresses_file: String::new(),
            privacy_log_phrases: true,
            update_check_enabled: true,
            address_cache_enabled: true,
            address_cache_ttl_minutes: 60,
            telegram_enabled: false,
            telegram_token: String::new(),
            telegram_chat_id: String::new(),
            discord_enabled: false,
            discord_webhook: String::new(),
            full_anonymity_mode: true,
            anonymity_mode: true,
            use_tor: false,
            tor_proxy: "socks5h://127.0.0.1:9050".to_string(),
            proxy_url: String::new(),
            proxy_list: String::new(),
            proxy_rotation: false,
            jitter_min_ms: 200,
            jitter_max_ms: 1500,
            random_user_agent: true,
            random_headers: true,
            stealth_mode: true,
            stealth_window_title: false,
            encryption_enabled: false,
            encryption_password: String::new(),
            memory_only_mode: false,
            kill_switch: true,
            kill_switch_threshold: 10,
            block_direct_connection: false,
            autostart: false,
            start_minimized: false,
        }
    }
}

impl AppConfig {
    /// Clamp user input to safe ranges and fill generated secrets.
    pub fn normalized(mut self) -> Self {
        self.workers = self.workers.clamp(1, 32);
        self.batch_size = self.batch_size.clamp(1, 500);
        self.delay_ms = self.delay_ms.min(600_000);
        self.kill_switch_threshold = self.kill_switch_threshold.clamp(1, 1000);
        self.address_cache_ttl_minutes = self.address_cache_ttl_minutes.clamp(1, 100_800);
        // socks5h resolves DNS at the proxy: without this, Tor users leak
        // every API hostname to their local/ISP resolver.
        if self.tor_proxy.contains("9050") && !self.tor_proxy.starts_with("socks5h://") {
            self.tor_proxy = "socks5h://127.0.0.1:9050".to_string();
        }
        self.jitter_min_ms = self.jitter_min_ms.min(60_000);
        self.jitter_max_ms = self.jitter_max_ms.clamp(self.jitter_min_ms, 60_000);

        if !COINS.contains(&self.coin.as_str()) && self.coin != "ALL" {
            self.coin = "ALL".to_string();
        }
        if !MODES.contains(&self.scan_mode.as_str()) {
            self.scan_mode = "ALL_COINS_AUTO".to_string();
        }
        if !["auto", "legacy", "segwit", "native", "taproot"].contains(&self.address_type.as_str())
        {
            self.address_type = "auto".to_string();
        }
        if !["english", "spanish", "french", "italian", "japanese", "korean", "chinese_simplified", "chinese_traditional"].contains(&self.bip39_language.as_str()) {
            self.bip39_language = "english".to_string();
        }
        self.vanity_prefix = self
            .vanity_prefix
            .trim()
            .chars()
            .take(12)
            .collect::<String>()
            .to_lowercase();
        if self.full_anonymity_mode {
            self.apply_full_anonymity();
        }
        self
    }

    /// One-button anonymity: every protective layer is switched on at once.
    /// Tor and "block direct" stay opt-in because they require a local Tor
    /// daemon; the master switch in the UI turns them on explicitly.
    pub fn apply_full_anonymity(&mut self) {
        self.full_anonymity_mode = true;
        self.anonymity_mode = true;
        self.random_user_agent = true;
        self.random_headers = true;
        self.proxy_rotation = self.proxy_rotation || !self.proxy_list.trim().is_empty();
        self.stealth_mode = true;
        // The title mask is cosmetic: hiding the window behind it made a fresh
        // install look as if the app never started. Leave it opt-in.
        self.encryption_enabled = true;
        // Results must survive restarts/updates; memory-only stays opt-in.
        self.kill_switch = true;
    }

    /// All configured proxies (Tor first, then the primary and the rotation pool).
    pub fn proxies(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.use_tor && !self.tor_proxy.trim().is_empty() {
            out.push(self.tor_proxy.trim().to_string());
        }
        if !self.proxy_url.trim().is_empty() {
            out.push(self.proxy_url.trim().to_string());
        }
        if self.proxy_rotation {
            for line in self.proxy_list.lines() {
                let line = line.trim();
                if !line.is_empty() && !out.iter().any(|p| p == line) {
                    out.push(line.to_string());
                }
            }
        }
        out
    }

    /// Effective proxy for HTTP requests (first configured one).
    pub fn proxy(&self) -> Option<String> {
        self.proxies().into_iter().next()
    }

    /// Validation messages for the UI (informational, not fatal).
    pub fn warnings(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.block_direct_connection && self.proxy().is_none() {
            out.push("Блокировка прямого соединения включена, но прокси не задан — запросы к API будут отклонены.".into());
        }
        if self.use_tor && self.proxy().as_deref() == Some(self.tor_proxy.as_str()) {
            out.push("Tor включён: убедитесь, что Tor Browser/daemon запущен на 127.0.0.1:9050.".into());
        }
        if self.encryption_enabled && !self.memory_only_mode && self.encryption_password.is_empty() {
            out.push("Шифрование включено без пароля — задайте пароль или включите режим «только в памяти».".into());
        }
        if self.full_anonymity_mode && !self.anonymity_mode {
            out.push("Мастер анонимности включён, но режим анонимности выключен вручную.".into());
        }
        if self.workers > 8 {
            out.push("Большое число воркеров повышает риск блокировки API. Увеличьте задержку между батчами.".into());
        }
        out
    }
}

pub fn data_dir(handle: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("app data dir: {e}"))?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

pub fn config_path(handle: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    Ok(data_dir(handle)?.join("config.json"))
}

pub fn results_dir(handle: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = data_dir(handle)?.join("results");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

pub fn load(handle: &tauri::AppHandle) -> Result<AppConfig, String> {
    let path = config_path(handle)?;
    if !path.exists() {
        return Ok(AppConfig::default().normalized());
    }
    let raw = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let cfg: AppConfig = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    Ok(cfg.normalized())
}

pub fn save(handle: &tauri::AppHandle, config: &AppConfig) -> Result<Vec<String>, String> {
    let cfg = config.clone().normalized();
    let path = config_path(handle)?;
    let json = serde_json::to_string_pretty(&cfg).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())?;
    Ok(cfg.warnings())
}

/// Requires Windows: adds a HKCU Run entry for the current executable.
#[cfg(target_os = "windows")]
pub fn set_autostart(enabled: bool) -> Result<(), String> {
    use std::process::Command;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let exe = exe.to_string_lossy();
    let ps = if enabled {
        format!(
            "New-ItemProperty -Path 'HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Run' -Name 'HappinessScanner' -Value '\"{exe}\" --minimized' -PropertyType String -Force | Out-Null"
        )
    } else {
        "Remove-ItemProperty -Path 'HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Run' -Name 'HappinessScanner' -ErrorAction SilentlyContinue | Out-Null".to_string()
    };
    let status = Command::new("powershell")
        .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &ps])
        .status()
        .map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err("powershell returned an error".into())
    }
}

#[cfg(not(target_os = "windows"))]
pub fn set_autostart(_enabled: bool) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod launch_tests {
    use super::AppConfig;

    #[test]
    fn first_launch_keeps_the_window_visible() {
        let cfg = AppConfig::default().normalized();
        assert!(!cfg.start_minimized, "a new install must show its window");
        assert!(
            !cfg.stealth_window_title,
            "cosmetic title masking must be opt-in"
        );
    }

    #[test]
    fn anonymity_does_not_disable_result_persistence() {
        let cfg = AppConfig::default().normalized();
        assert!(
            !cfg.memory_only_mode,
            "hits must survive an update unless memory-only is chosen explicitly"
        );
    }
}
