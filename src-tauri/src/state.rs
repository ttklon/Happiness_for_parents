//! Shared application state (Tauri `manage`).

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use serde::Serialize;

use crate::config::AppConfig;

#[derive(Debug, Clone, Serialize)]
pub struct LogEvent {
    pub time: String,
    pub kind: String,
    pub text: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ScanStats {
    pub running: bool,
    pub workers: usize,
    pub total_checked: u64,
    pub hits: u64,
    pub empty: u64,
    pub received_but_empty: u64,
    pub cache_size: usize,
    pub seen_size: usize,
    pub checks_per_hour: f64,
    pub uptime_seconds: u64,
    pub errors: u64,
    pub kill_switch_active: bool,
    pub last_address: String,
    pub last_coin: String,
    pub last_phrase: String,
    pub last_balance: u64,
}

pub struct AppState {
    pub stats: Mutex<ScanStats>,
    pub hits: Mutex<Vec<crate::results::HitRecord>>,
    /// Fingerprints of everything already generated — we never scan twice.
    pub seen: Mutex<HashSet<String>>,
    /// address -> last check time (TTL cache, saves API quota).
    pub cache: Mutex<HashMap<String, Instant>>,
    pub stop_tx: Mutex<Option<tokio::sync::broadcast::Sender<()>>>,
    pub started_at: Mutex<Option<Instant>>,
    pub config: Mutex<AppConfig>,
    pub events: Mutex<VecDeque<LogEvent>>,
    pub history: Mutex<VecDeque<u64>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            stats: Mutex::new(ScanStats::default()),
            hits: Mutex::new(Vec::new()),
            seen: Mutex::new(HashSet::new()),
            cache: Mutex::new(HashMap::new()),
            stop_tx: Mutex::new(None),
            started_at: Mutex::new(None),
            config: Mutex::new(AppConfig::default()),
            events: Mutex::new(VecDeque::with_capacity(100)),
            history: Mutex::new(VecDeque::with_capacity(60)),
        }
    }
}

impl AppState {
    pub fn log(&self, kind: &str, text: impl Into<String>) {
        let text = text.into();
        let event = LogEvent {
            time: chrono::Local::now().format("%H:%M:%S").to_string(),
            kind: kind.to_string(),
            text: text.clone(),
        };
        if let Ok(mut events) = self.events.lock() {
            events.push_back(event);
            while events.len() > 100 {
                events.pop_front();
            }
        }
        if kind == "error" {
            log::warn!("{}", text);
        } else {
            log::info!("{}", text);
        }
    }
}

pub type SharedState = Arc<AppState>;
