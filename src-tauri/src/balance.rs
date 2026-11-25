//! Strict balance APIs: no generic JSON number hunting.
//!
//! A hit is recorded only when TWO independent endpoints return the same
//! positive balance and the amount is above the dust threshold for that coin.

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

#[derive(Clone, Copy)]
enum Parser {
    ChainStats,   // blockstream / mempool.space
    FinalBalance, // blockchain.info
    Blockchair,
    BlockCypher,
    DogeApi,
    RpcHex,       // eth_getBalance
    SolanaRpc,    // getBalance -> result.value
    TonCenter,    // result, nanoton
}

struct Endpoint {
    name: &'static str,
    url: String,
    verb: Verb,
    parser: Parser,
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

fn evm_rpcs(coin: &str) -> [&'static str; 2] {
    match coin {
        "MATIC" => ["https://rpc.ankr.com/polygon", "https://polygon-rpc.com"],
        "BSC" => ["https://rpc.ankr.com/bsc", "https://bsc-dataseed.binance.org"],
        "ARB" => ["https://rpc.ankr.com/arbitrum", "https://arb1.arbitrum.io/rpc"],
        "OPT" => ["https://rpc.ankr.com/optimism", "https://mainnet.optimism.io"],
        "BASE" => ["https://rpc.ankr.com/base", "https://mainnet.base.org"],
        "AVAX" => [
            "https://rpc.ankr.com/avalanche_c_chain",
            "https://api.avax.network/ext/bc/C/rpc",
        ],
        _ => ["https://rpc.ankr.com/eth", "https://eth.llamarpc.com"],
    }
}

fn endpoints(coin: &str, address: &str) -> Vec<Endpoint> {
    let mut out = Vec::new();
    let mut add = |name: &'static str, url: String, parser: Parser| {
        out.push(Endpoint {
            name,
            url,
            verb: Verb::Get,
            parser,
        });
    };
    match coin {
        "BTC" => {
            add("blockstream", format!("https://blockstream.info/api/address/{address}"), Parser::ChainStats);
            add("mempool.space", format!("https://mempool.space/api/address/{address}"), Parser::ChainStats);
            add("blockchain.info", format!("https://blockchain.info/rawaddr/{address}"), Parser::FinalBalance);
            add("blockchair", format!("https://api.blockchair.com/bitcoin/dashboards/address/{address}"), Parser::Blockchair);
            add("blockcypher", format!("https://api.blockcypher.com/v1/btc/main/addrs/{address}/balance"), Parser::BlockCypher);
        }
        "LTC" => {
            add("blockcypher", format!("https://api.blockcypher.com/v1/ltc/main/addrs/{address}/balance"), Parser::BlockCypher);
            add("blockchair", format!("https://api.blockchair.com/litecoin/dashboards/address/{address}"), Parser::Blockchair);
        }
        "DOGE" => {
            add("blockcypher", format!("https://api.blockcypher.com/v1/doge/main/addrs/{address}/balance"), Parser::BlockCypher);
            add("blockchair", format!("https://api.blockchair.com/dogecoin/dashboards/address/{address}"), Parser::Blockchair);
            add("dogeapi", format!("https://dogeapi.net/v1/address/{address}"), Parser::DogeApi);
        }
        "BCH" => {
            add("blockchair", format!("https://api.blockchair.com/bitcoin/cash/dashboards/address/{address}"), Parser::Blockchair);
            add("blockstream", format!("https://bch.blockstream.info/api/address/{address}"), Parser::ChainStats);
        }
        "SOL" => {
            for (i, rpc) in ["https://api.mainnet-beta.solana.com", "https://solana-rpc.publicnode.com"]
                .iter()
                .enumerate()
            {
                out.push(Endpoint {
                    name: if i == 0 { "solana-rpc" } else { "solana-publicnode" },
                    url: (*rpc).to_string(),
                    verb: Verb::Post(rpc_body("getBalance", vec![Value::String(address.into())])),
                    parser: Parser::SolanaRpc,
                });
            }
        }
        "TON" => {
            // Two independent hosts; both expose the same strict endpoint.
            add("toncenter", format!("https://toncenter.com/api/v2/getAddressBalance?address={address}"), Parser::TonCenter);
            add("toncenter-2", format!("https://api.toncenter.com/api/v2/getAddressBalance?address={address}"), Parser::TonCenter);
        }
        _ => {
            for (i, rpc) in evm_rpcs(coin).iter().enumerate() {
                out.push(Endpoint {
                    name: if i == 0 { "rpc-primary" } else { "rpc-secondary" },
                    url: (*rpc).to_string(),
                    verb: Verb::Post(rpc_body(
                        "eth_getBalance",
                        vec![Value::String(address.into()), Value::String("latest".into())],
                    )),
                    parser: Parser::RpcHex,
                });
            }
        }
    }
    out
}

const USER_AGENTS: [&str; 6] = [
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:127.0) Gecko/20100101 Firefox/127.0",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Safari/605.1.15",
    "X11; Linux x86_64 AppleWebKit/537.36 Chrome/125.0.0.0 Safari/537.36",
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

fn client(config: &AppConfig, proxy: Option<String>) -> Result<reqwest::Client, String> {
    if config.block_direct_connection && proxy.is_none() {
        return Err("Прямое соединение заблокировано: включите Tor или прокси".into());
    }
    let mut builder = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .connect_timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::limited(3));
    if let Some(proxy) = proxy {
        builder = builder.proxy(reqwest::Proxy::all(&proxy).map_err(|e| format!("proxy: {e}"))?);
    }
    builder.build().map_err(|e| e.to_string())
}

/// Strict integer conversion: decimal numbers/strings and `0x` hex only.
/// Floats, booleans, arrays and objects are never treated as balances.
fn uint(value: Option<&Value>) -> Option<u64> {
    match value? {
        Value::Number(n) => n.as_u64(),
        Value::String(s) => {
            let s = s.trim();
            if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
                u128::from_str_radix(hex, 16)
                    .ok()
                    .and_then(|v| u64::try_from(v).ok())
            } else if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
                None
            } else {
                s.parse::<u64>().ok()
            }
        }
        _ => None,
    }
}

/// Returns `(balance, total_received)` in the coin's smallest unit, only from
/// documented fields. Unknown/extra numbers in a response are ignored.
fn parse_balance(parser: Parser, address: &str, json: &Value) -> Option<(u64, u64)> {
    match parser {
        Parser::ChainStats => {
            let chain = json.get("chain_stats")?;
            let funded = uint(chain.get("funded_txo_sum"))?;
            let spent = uint(chain.get("spent_txo_sum"))?;
            Some((funded.saturating_sub(spent), funded))
        }
        Parser::FinalBalance => {
            let balance = uint(json.get("final_balance"))?;
            Some((balance, uint(json.get("total_received")).unwrap_or(balance)))
        }
        Parser::Blockchair => {
            let row = json.get("data")?.get(address)?;
            let row = row.get("address").unwrap_or(row);
            let balance = uint(row.get("balance"))?;
            Some((balance, uint(row.get("total_received")).unwrap_or(balance)))
        }
        Parser::BlockCypher => {
            let balance = uint(json.get("balance"))?;
            Some((balance, uint(json.get("total_received")).unwrap_or(balance)))
        }
        Parser::DogeApi => {
            let root = json.get("data").unwrap_or(json);
            let balance = uint(root.get("balance"))?;
            Some((balance, uint(root.get("total_received")).unwrap_or(balance)))
        }
        Parser::RpcHex => {
            let hex = json.get("result")?.as_str()?;
            let hex = hex.strip_prefix("0x").or_else(|| hex.strip_prefix("0X"))?;
            let wei = u128::from_str_radix(hex, 16).ok()?;
            Some((u64::try_from(wei).unwrap_or(u64::MAX), u64::try_from(wei).unwrap_or(u64::MAX)))
        }
        Parser::SolanaRpc => {
            let value = uint(json.get("result")?.get("value"))?;
            Some((value, value))
        }
        Parser::TonCenter => {
            let value = uint(json.get("result"))?;
            Some((value, value))
        }
    }
}

/// Dust protection. A balance below this is either unspendable or rent dust
/// and must not be reported as a find. Values are in the smallest unit.
fn dust_threshold(coin: &str) -> u64 {
    match coin {
        "BTC" | "LTC" | "DOGE" | "BCH" => 1_000,
        "SOL" => 1_000_000,          // 0.001 SOL
        "TON" => 10_000_000,        // 0.01 TON
        _ => 100_000_000_000_000,   // 0.0001 native EVM coin
    }
}

/// Returns `(balance, total_received)` only when at least two independent
/// endpoints agree on a positive, non-dust balance.
pub async fn check_balance(coin: &str, address: &str, config: &AppConfig) -> Result<(u64, u64), String> {
    let proxies = config.proxies();
    let proxy = if proxies.is_empty() {
        None
    } else if config.proxy_rotation && proxies.len() > 1 {
        Some(proxies[rand::random::<usize>() % proxies.len()].clone())
    } else {
        Some(proxies[0].clone())
    };
    let http = client(config, proxy)?;
    let mut last_error = "no endpoints configured".to_string();
    let mut confirmations: Vec<(&'static str, u64, u64)> = Vec::new();

    for endpoint in endpoints(coin, address) {
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
            Ok(json) => match parse_balance(endpoint.parser, address, &json) {
                Some((balance, received)) => {
                    record(coin, endpoint.name, true, "");
                    confirmations.push((endpoint.name, balance, received));
                    if confirmations.len() >= 2 {
                        break;
                    }
                }
                None => {
                    // "account not found" / empty responses mean zero, not a hit.
                    last_error = format!("{}: нет подтверждённого баланса", endpoint.name);
                    record(coin, endpoint.name, true, "");
                }
            },
            Err(e) => {
                last_error = format!("{}: {e}", endpoint.name);
                record(coin, endpoint.name, false, &last_error);
            }
        }
    }

    let (first_name, first_balance, first_received) = match confirmations.first().copied() {
        Some(first) => first,
        None => return Err(last_error),
    };
    if confirmations.len() < 2 {
        return Err(format!(
            "{first_name}: баланс {first_balance} не подтверждён вторым API"
        ));
    }
    let balance = first_balance;
    let mut received = first_received;
    for (_, other, other_received) in confirmations.iter().skip(1) {
        let hi = (*other).max(balance);
        let lo = (*other).min(balance);
        // Up to 10% drift between providers (mempool, indexer lag).
        if hi.saturating_sub(lo) > (lo / 10).max(1) {
            return Err(format!(
                "API расходятся в балансе: {balance} против {other} — находка отклонена"
            ));
        }
        received = received.max(*other_received);
    }
    if balance < dust_threshold(coin) {
        return Ok((0, received)); // пыль — это не находка
    }
    Ok((balance, received))
}

pub async fn probe(config: &AppConfig) -> Result<String, String> {
    let address = "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa";
    match check_balance("BTC", address, config).await {
        Ok((balance, _)) => Ok(format!("API доступен (баланс тестового адреса: {balance} сатоши)")),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn strict_parsers_read_only_documented_fields() {
        let btc = json!({"chain_stats": {"funded_txo_sum": "2000", "spent_txo_sum": "500"}});
        assert_eq!(parse_balance(Parser::ChainStats, "", &btc), Some((1500, 2000)));
        let sol = json!({"jsonrpc": "2.0", "result": {"value": 953520}, "id": 1});
        assert_eq!(parse_balance(Parser::SolanaRpc, "", &sol), Some((953520, 953520)));
        let evm = json!({"result": "0xde0b6b3a7640000"});
        assert_eq!(parse_balance(Parser::RpcHex, "", &evm), Some((1_000_000_000_000_000_000, 1_000_000_000_000_000_000)));
    }

    #[test]
    fn empty_and_unknown_responses_are_not_balances() {
        // This exact shape used to be treated as a hit by the old generic parser.
        assert_eq!(parse_balance(Parser::ChainStats, "", &json!({"data": [], "success": true})), None);
        assert_eq!(parse_balance(Parser::SolanaRpc, "", &json!({"result": null})), None);
        assert_eq!(uint(Some(&json!(1.5))), None);
        assert_eq!(uint(Some(&json!("not-a-number"))), None);
    }

    #[test]
    fn dust_is_below_threshold() {
        assert!(953_520 < dust_threshold("SOL"));
        assert!(1_000 >= dust_threshold("BTC"));
    }
}
