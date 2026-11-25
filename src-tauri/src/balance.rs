//! Free public balance APIs with automatic failover and endpoint health tracking.

use once_cell::sync::Lazy;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::config::AppConfig;

#[derive(Debug, Clone, Serialize)]
pub struct EndpointHealth {
    pub name: String,
    pub coin: String,
    pub ok: u64,
    pub fail: u64,
    pub quarantined_for_seconds: u64,
    pub last_error: String,
}

#[derive(Debug, Clone, Default)]
struct HealthEntry {
    ok: u64,
    fail: u64,
    last_error: String,
    quarantined_until: Option<Instant>,
}

static HEALTH: Lazy<Mutex<HashMap<String, HealthEntry>>> = Lazy::new(|| Mutex::new(HashMap::new()));

fn health_key(coin: &str, name: &str) -> String {
    format!("{coin}:{name}")
}

fn record(coin: &str, name: &str, success: bool, error: &str) {
    if let Ok(mut map) = HEALTH.lock() {
        let entry = map.entry(health_key(coin, name)).or_default();
        if success {
            entry.ok += 1;
            entry.fail = 0;
            entry.last_error.clear();
            entry.quarantined_until = None;
        } else {
            entry.fail += 1;
            entry.last_error = error.chars().take(160).collect();
            if entry.fail >= 3 {
                let secs = 2u64.saturating_pow(entry.fail.min(12) as u32).min(3600);
                entry.quarantined_until = Some(Instant::now() + Duration::from_secs(secs));
            }
        }
    }
}

pub fn health_snapshot() -> Vec<EndpointHealth> {
    let map = match HEALTH.lock() {
        Ok(map) => map,
        Err(_) => return Vec::new(),
    };
    map.iter()
        .map(|(key, entry)| {
            let (coin, name) = key.split_once(':').unwrap_or((key.as_str(), ""));
            EndpointHealth {
                name: name.to_string(),
                coin: coin.to_string(),
                ok: entry.ok,
                fail: entry.fail,
                quarantined_for_seconds: entry
                    .quarantined_until
                    .map(|until| until.saturating_duration_since(Instant::now()).as_secs())
                    .unwrap_or(0),
                last_error: entry.last_error.clone(),
            }
        })
        .collect()
}

fn is_quarantined(coin: &str, name: &str) -> bool {
    HEALTH.lock()
        .ok()
        .and_then(|map| map.get(&health_key(coin, name)).cloned())
        .and_then(|entry| entry.quarantined_until)
        .map(|until| until > Instant::now())
        .unwrap_or(false)
}

#[derive(Clone)]
enum Verb {
    Get,
    Post(String),
}

struct Endpoint {
    name: &'static str,
    url: String,
    verb: Verb,
}

fn rpc_body(method: &str, params: Vec<Value>) -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": method,
        "params": params,
    })
    .to_string()
}

fn endpoints(coin: &str, address: &str) -> Vec<Endpoint> {
    let mut out = Vec::new();
    let mut add = |name: &'static str, url: String| {
        out.push(Endpoint { name, url, verb: Verb::Get });
    };
    match coin {
        "BTC" => {
            add("blockstream", format!("https://blockstream.info/api/address/{address}"));
            add("mempool.space", format!("https://mempool.space/api/address/{address}"));
            add("blockchain.info", format!("https://blockchain.info/rawaddr/{address}"));
            add("blockchair", format!("https://api.blockchair.com/bitcoin/dashboards/address/{address}"));
        }
        "LTC" => {
            add("blockcypher", format!("https://api.blockcypher.com/v1/ltc/main/addrs/{address}/balance"));
            add("blockchair", format!("https://api.blockchair.com/litecoin/dashboards/address/{address}"));
        }
        "DOGE" => {
            add("blockcypher", format!("https://api.blockcypher.com/v1/doge/main/addrs/{address}/balance"));
            add("dogeapi", format!("https://dogeapi.net/v1/address/{address}"));
        }
        "BCH" => {
            add("blockchair", format!("https://api.blockchair.com/bitcoin/cash/dashboards/address/{address}"));
            add("blockstream", format!("https://bch.blockstream.info/api/address/{address}"));
        }
        "TRX" => {
            add("trongrid", format!("https://api.trongrid.io/v1/accounts/{address}"));
        }
        "SOL" => {
            out.push(Endpoint {
                name: "solana-rpc",
                url: "https://api.mainnet-beta.solana.com".into(),
                verb: Verb::Post(rpc_body("getBalance", vec![Value::String(address.into())])),
            });
        }
        "TON" => {
            add("toncenter", format!("https://toncenter.com/api/v2/getAddressBalance?address={address}"));
        }
        _ => {
            // All EVM chains expose the same eth_getBalance JSON-RPC call.
            let rpc = match coin {
                "MATIC" => "https://rpc.ankr.com/polygon",
                "BSC" => "https://rpc.ankr.com/bsc",
                "ARB" => "https://rpc.ankr.com/arbitrum",
                "OPT" => "https://rpc.ankr.com/optimism",
                "BASE" => "https://rpc.ankr.com/base",
                "AVAX" => "https://rpc.ankr.com/avalanche_c_chain",
                _ => "https://rpc.ankr.com/eth",
            };
            out.push(Endpoint {
                name: "rpc",
                url: rpc.to_string(),
                verb: Verb::Post(rpc_body("eth_getBalance", vec![Value::String(address.into()), Value::String("latest".into())])),
            });
            add("blockscout", format!("https://eth.blockscout.com/api?module=account&action=balance&address={address}"));
        }
    }
    out
}

const USER_AGENTS: [&str; 6] = [
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:127.0) Gecko/20100101 Firefox/127.0",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Safari/605.1.15",
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36",
    "HappinessScanner/0.6 (+local; no telemetry)",
    "curl/8.7.1",
];

const LANGUAGES: [&str; 5] = [
    "en-US,en;q=0.9",
    "de-DE,de;q=0.9,en;q=0.8",
    "fr-FR,fr;q=0.9,en;q=0.8",
    "ru-RU,ru;q=0.9,en;q=0.8",
    "ja-JP,ja;q=0.9,en;q=0.8",
];

fn client(config: &AppConfig) -> Result<reqwest::Client, String> {
    if config.block_direct_connection && config.proxy().is_none() {
        return Err("Прямое соединение заблокировано: включите Tor или прокси".into());
    }
    let mut builder = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .connect_timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::limited(3));
    if let Some(proxy) = config.proxy() {
        builder = builder.proxy(reqwest::Proxy::all(&proxy).map_err(|e| format!("proxy: {e}"))?);
    }
    builder.build().map_err(|e| e.to_string())
}

fn number(value: &Value) -> Option<u64> {
    match value {
        Value::Null => None,
        Value::Bool(_) => None,
        Value::Number(n) => n
            .as_u64()
            .or_else(|| n.as_f64().map(|f| f.max(0.0) as u64)),
        Value::String(s) => {
            let s = s.trim();
            if let Some(hex) = s.strip_prefix("0x") {
                u64::from_str_radix(hex, 16).ok()
            } else {
                s.parse::<u64>()
                    .ok()
                    .or_else(|| s.parse::<f64>().ok().map(|f| f.max(0.0) as u64))
            }
        }
        Value::Array(items) => items.first().and_then(number),
        Value::Object(_) => None,
    }
}

const PRIORITY_KEYS: [&str; 10] = [
    "final_balance",
    "balance",
    "confirmed_balance",
    "result",
    "value",
    "amount",
    "unconfirmed_balance",
    "total_received",
    "funded_txo_sum",
    "availableBalance",
];

fn find_number(value: &Value, depth: usize) -> Option<u64> {
    if depth > 8 {
        return None;
    }
    if let Some(n) = number(value) {
        return Some(n);
    }
    if let Value::Object(map) = value {
        for key in PRIORITY_KEYS {
            if let Some(inner) = map.get(key) {
                if let Some(n) = find_number(inner, depth + 1) {
                    return Some(n);
                }
            }
        }
        // Special case: BTC blockstream chain_stats (funded - spent).
        if let Some(chain) = map.get("chain_stats") {
            let funded = find_number(&serde_json::json!({ "x": chain.get("funded_txo_sum") }), depth + 1);
            let spent = find_number(&serde_json::json!({ "x": chain.get("spent_txo_sum") }), depth + 1);
            if let (Some(f), Some(s)) = (funded, spent) {
                return Some(f.saturating_sub(s));
            }
        }
        for inner in map.values() {
            if let Some(n) = find_number(inner, depth + 1) {
                return Some(n);
            }
        }
    }
    None
}

/// Returns `(balance, total_received)` in the coin's smallest unit.
pub async fn check_balance(coin: &str, address: &str, config: &AppConfig) -> Result<(u64, u64), String> {
    let http = client(config)?;
    let list = endpoints(coin, address);
    let mut last_error = "no endpoints configured".to_string();

    for endpoint in list {
        if is_quarantined(coin, endpoint.name) {
            continue;
        }
        let mut request = match endpoint.verb {
            Verb::Get => http.get(&endpoint.url),
            Verb::Post(body) => http
                .post(&endpoint.url)
                .header("content-type", "application/json")
                .body(body.to_string()),
        };
        if config.random_user_agent || config.random_headers {
            request = request.header(
                "user-agent",
                USER_AGENTS[rand::random::<usize>() % USER_AGENTS.len()],
            );
        }
        if config.random_headers {
            request = request
                .header("accept-language", LANGUAGES[rand::random::<usize>() % LANGUAGES.len()])
                .header("accept", "application/json")
                .header("dnt", "1");
        }
        let response = request.send().await;
        let result = match response {
            Ok(resp) if resp.status().is_success() => resp.json::<Value>().await.map_err(|e| e.to_string()),
            Ok(resp) => Err(format!("HTTP {}", resp.status())),
            Err(e) => Err(e.to_string()),
        };
        match result {
            Ok(json) => {
                if let Some(balance) = find_number(&json, 0) {
                    let received = find_number(
                        &json.get("chain_stats").and_then(|c| c.get("funded_txo_sum")).cloned()
                            .unwrap_or(Value::Null),
                        0,
                    ).unwrap_or(balance);
                    record(coin, endpoint.name, true, "");
                    return Ok((balance, received));
                }
                last_error = format!("{}: unexpected response", endpoint.name);
                record(coin, endpoint.name, false, &last_error);
            }
            Err(e) => {
                last_error = format!("{}: {e}", endpoint.name);
                record(coin, endpoint.name, false, &last_error);
            }
        }
    }
    Err(last_error)
}

pub async fn probe(config: &AppConfig) -> Result<String, String> {
    let address = "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa";
    match check_balance("BTC", address, config).await {
        Ok((balance, _)) => Ok(format!("API доступен (баланс тестового адреса: {balance} сатоши)")),
        Err(e) => Err(e),
    }
}
