//! Infinite, never-repeating background scan engine.

use rand::{Rng, SeedableRng};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::broadcast;

use crate::balance;
use crate::config::{AppConfig, COINS, MODES};
use crate::results::{self, EmptyRecord, HitRecord};
use crate::state::{AppState, ScanStats, SharedState};
use crate::wallet::{self, Family};

struct Candidate {
    phrase: String,
    secret: [u8; 32],
    seed: Option<[u8; 64]>,
    coin: String,
    address: String,
    address_type: String,
    source: String,
    balance: u64,
}

fn log(state: &SharedState, kind: &str, message: impl Into<String>) {
    state.log(kind, message);
}

fn effective_choice(config: &AppConfig, cycle: u64) -> (String, String) {
    let coin = if config.coin == "ALL" {
        COINS[(cycle as usize) % COINS.len()].to_string()
    } else {
        config.coin.clone()
    };
    let mode = if config.scan_mode == "ALL_COINS_AUTO" {
        // Cycle through every interesting strategy, never repeating the same
        // (coin, mode) pair until the whole matrix has been covered.
        MODES[1 + ((cycle / COINS.len() as u64) as usize) % (MODES.len() - 1)].to_string()
    } else {
        config.scan_mode.clone()
    };
    (coin, mode)
}

fn pick_type(config: &AppConfig, rng: &mut impl Rng, coin: &str, seq: u64) -> String {
    if config.address_type != "auto" {
        return config.address_type.clone();
    }
    match wallet::coin_spec(coin).family {
        // Cover BIP44/49/84/86 in a deterministic cycle: every phrase gets all
        // four address formats across consecutive candidates (4x more chances).
        Family::Utxo if config.derive_all_paths => {
            ["legacy", "segwit", "native", "taproot"][(seq % 4) as usize].to_string()
        }
        Family::Utxo => ["legacy", "native", "segwit", "taproot"][rng.gen_range(0..4)].to_string(),
        _ => "auto".to_string(),
    }
}

fn mutation(rng: &mut impl Rng, word: &str) -> String {
    let mut text = word.to_string();
    match rng.gen_range(0..10) {
        0 => {
            text = text
                .chars()
                .map(|c| match c {
                    'a' => '4', 'e' => '3', 'i' => '1', 'o' => '0', 's' => '5', _ => c,
                })
                .collect()
        }
        1 => text = text.to_uppercase(),
        2 => {
            let year = 1950 + rng.gen_range(0..76);
            text = format!("{text}{year}")
        }
        3 => text = format!("{}{text}", text.chars().rev().collect::<String>()),
        4 => text = text.chars().filter(|c| !"aeiou".contains(*c)).collect(),
        5 => text = format!("{text}2026!"),
        6 => text = format!("{text}{}", rng.gen_range(0..9999)),
        7 => text = text.repeat(2),
        8 => text = format!("{}_{}", text, text.chars().rev().collect::<String>()),
        _ => {}
    }
    text
}

fn dictionary_words(config: &AppConfig, language: &str) -> Vec<String> {
    if !config.dictionary_file.trim().is_empty() {
        if let Ok(raw) = std::fs::read_to_string(config.dictionary_file.trim()) {
            let words: Vec<String> = raw
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty() && line.len() < 64)
                .map(str::to_string)
                .collect();
            if !words.is_empty() {
                return words;
            }
        }
    }
    wallet::wordlist(language)
        .into_iter()
        .map(str::to_string)
        .collect()
}

fn generate_candidate(
    config: &AppConfig,
    rng: &mut impl Rng,
    cycle: u64,
    seq: u64,
) -> Candidate {
    let words = dictionary_words(config, &config.bip39_language);
    let (coin, mode) = effective_choice(config, cycle);
    let use_bip39 = matches!(
        mode.as_str(),
        "RANDOM"
            | "BIP39_PASSPHRASE"
            | "VANITY_PATTERN"
            | "BLOOM_FILTER"
            | "TRANSACTION_HISTORY"
    );

    let (phrase, secret, seed): (String, [u8; 32], Option<[u8; 64]>) = if use_bip39 || mode == "ALL_COINS_AUTO" {
        let phrase = wallet::random_mnemonic(rng, &config.bip39_language, if seq % 4 == 0 { 24 } else { 12 });
        let passphrase = if mode == "BIP39_PASSPHRASE" {
            format!("{}", rng.gen_range(0..1_000_000))
        } else {
            String::new()
        };
        let seed = wallet::mnemonic_to_seed(&phrase, &passphrase);
        (phrase, [0u8; 32], Some(seed))
    } else if mode == "PUZZLE" {
        let mut bytes = [0u8; 32];
        let value = (seq + 1) as u32;
        bytes[28..32].copy_from_slice(&value.to_be_bytes());
        (format!("PUZZLE-{value}"), bytes, None)
    } else if mode == "SEQUENTIAL_KEYS" {
        let phrase = format!("seq-{seq:012x}");
        let key = wallet::brain_key(&phrase);
        (phrase, key, None)
    } else if mode == "DICTIONARY" || mode == "RICH_LIST" || mode == "TARGET_ADDRESSES" {
        let word = words[rng.gen_range(0..words.len())].clone();
        let phrase = mutation(rng, &word);
        let key = wallet::brain_key(&phrase);
        (phrase, key, None)
    } else {
        // Brain wallet, combos, multi-coin brain: 1–4 mutated words.
        let count = match mode.as_str() {
            "MULTI_WORD_COMBO" => rng.gen_range(3..7),
            "BRAIN_COMBOS" => rng.gen_range(2..5),
            "MULTI_COIN_BRAIN" => rng.gen_range(1..4),
            _ => 1,
        };
        let mut parts = Vec::with_capacity(count);
        for _ in 0..count {
            let word = words[rng.gen_range(0..words.len())].clone();
            parts.push(mutation(rng, &word));
        }
        let phrase = parts.join(if rng.gen_bool(0.3) { " " } else { "" });
        let key = wallet::brain_key(&phrase);
        (phrase, key, None)
    };

    let address_type = pick_type(config, rng, &coin, seq);
    let key = wallet::key_for_coin(&secret, seed.as_ref(), &coin, &address_type);
    let address = wallet::secret_to_address(&key, &coin, &address_type)
        .unwrap_or_else(|_| format!("invalid-{coin}-{seq}"));

    Candidate {
        phrase,
        secret: key,
        seed: None,
        coin,
        address,
        address_type,
        source: mode,
        balance: 0,
    }
}

/// Start N independent, endless workers. Each one generates fresh candidates,
/// filters duplicates with a fingerprint set and a TTL cache, then asks the
/// free APIs for balances.
pub fn start(app: tauri::AppHandle, state: SharedState, config: AppConfig) -> Result<String, String> {
    {
        let mut stats = state.stats.lock().map_err(|e| e.to_string())?;
        if stats.running {
            return Ok("Сканер уже запущен".into());
        }
        *stats = ScanStats {
            running: true,
            workers: config.workers,
            ..Default::default()
        };
    }
    let config = config.normalized();
    if config.block_direct_connection && config.proxy().is_none() {
        let mut stats = state.stats.lock().map_err(|e| e.to_string())?;
        stats.running = false;
        return Err("Включена блокировка прямого соединения, но прокси/Tor не настроен.".into());
    }
    *state.config.lock().map_err(|e| e.to_string())? = config.clone();
    *state.started_at.lock().map_err(|e| e.to_string())? = Some(Instant::now());
    state.seen.lock().map_err(|e| e.to_string())?.clear();
    state.cache.lock().map_err(|e| e.to_string())?.clear();

    let (tx, _) = broadcast::channel::<()>(8);
    *state.stop_tx.lock().map_err(|e| e.to_string())? = Some(tx.clone());

    let results_path: PathBuf = crate::config::results_dir(&app)?;
    let hits_path = results_path.join("hits.jsonl");
    let empty_path = results_path.join("empty.jsonl");

    let mut enabled = Vec::new();
    if config.anonymity_mode {
        enabled.push("анонимный режим");
    }
    if config.use_tor || config.proxy().is_some() {
        enabled.push("прокси/Tor");
    }
    if config.random_user_agent && config.random_headers {
        enabled.push("рандомные заголовки");
    }
    if config.encryption_enabled {
        enabled.push("AES-256-GCM");
    }
    if config.memory_only_mode {
        enabled.push("только в памяти");
    }
    if config.kill_switch {
        enabled.push("килл-свитч");
    }
    if config.block_direct_connection {
        enabled.push("блок direct HTTP");
    }
    log(
        &state,
        "info",
        format!(
            "Старт: {} воркеров, режим {}, монеты {}, защита: {}",
            config.workers,
            config.scan_mode,
            config.coin,
            enabled.join(", ")
        ),
    );

    for worker_id in 0..config.workers {
        let state = Arc::clone(&state);
        let app = app.clone();
        let cfg = config.clone();
        let hits_path = hits_path.clone();
        let empty_path = empty_path.clone();
        let mut stop = tx.subscribe();
        tauri::async_runtime::spawn(async move {
            worker_loop(
                worker_id,
                app,
                state,
                cfg,
                hits_path,
                empty_path,
                &mut stop,
            )
            .await;
        });
    }

    Ok(format!("Запущено воркеров: {}", config.workers))
}

pub fn stop(state: SharedState) -> Result<String, String> {
    if let Some(tx) = state.stop_tx.lock().map_err(|e| e.to_string())?.take() {
        let _ = tx.send(());
    }
    if let Ok(mut stats) = state.stats.lock() {
        stats.running = false;
    }
    log(&state, "info", "Сканер остановлен");
    Ok("Сканер останавливается".into())
}

async fn worker_loop(
    worker_id: usize,
    app: tauri::AppHandle,
    state: SharedState,
    config: AppConfig,
    hits_path: PathBuf,
    empty_path: PathBuf,
    stop: &mut broadcast::Receiver<()>,
) {
    let mut seq = worker_id as u64 * 1_000_000;
    let mut consecutive_errors = 0u32;
    let mut last_history = Instant::now();

    // TARGET_ADDRESSES mode: check addresses from a local file instead of
    // generating new ones. Nothing is ever sent except the public address.
    let targets: Vec<String> = if config.scan_mode == "TARGET_ADDRESSES"
        && !config.target_addresses_file.trim().is_empty()
    {
        std::fs::read_to_string(config.target_addresses_file.trim())
            .map(|raw| {
                raw.lines()
                    .map(str::trim)
                    .filter(|line| !line.is_empty() && line.len() < 128)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    loop {
        if stop.try_recv().is_ok() || state.stats.lock().map(|s| s.kill_switch_active).unwrap_or(false) {
            break;
        }

        // StdRng is Send, unlike ThreadRng, so the worker future can cross awaits.
        let mut rng = rand::rngs::StdRng::from_entropy();
        let mut batch: Vec<Candidate> = Vec::with_capacity(config.batch_size);
        let mut attempts = 0usize;
        while batch.len() < config.batch_size && attempts < config.batch_size * 20 {
            attempts += 1;
            seq += 1;
            let mut candidate = if targets.is_empty() {
                generate_candidate(&config, &mut rng, seq, seq)
            } else {
                let (coin, _) = effective_choice(&config, seq);
                Candidate {
                    phrase: "TARGET".to_string(),
                    secret: [0u8; 32],
                    seed: None,
                    coin,
                    address: targets[(seq as usize) % targets.len()].clone(),
                    address_type: "auto".to_string(),
                    source: "TARGET_ADDRESSES".to_string(),
                    balance: 0,
                }
            };

            // Vanity mode: keep drawing until the address has the prefix.
            if config.scan_mode == "VANITY_PATTERN" && !config.vanity_prefix.is_empty() {
                let wanted = config.vanity_prefix.as_str();
                if !candidate.address.to_lowercase().starts_with(wanted) {
                    continue;
                }
            }

            let fingerprint = format!("{}:{}", candidate.coin, candidate.address);
            if !state
                .seen
                .lock()
                .map(|mut set| set.insert(fingerprint.clone()))
                .unwrap_or(false)
            {
                continue;
            }

            if config.address_cache_enabled {
                let cached = state
                    .cache
                    .lock()
                    .ok()
                    .and_then(|cache| cache.get(&fingerprint).copied())
                    .map(|at| at.elapsed() < Duration::from_secs(config.address_cache_ttl_minutes * 60))
                    .unwrap_or(false);
                if cached {
                    continue;
                }
            }
            batch.push(candidate);
        }

        for mut candidate in batch {
            if stop.try_recv().is_ok() {
                break;
            }
            match balance::check_balance(&candidate.coin, &candidate.address, &config).await {
                Ok((balance, received)) => {
                    consecutive_errors = 0;
                    candidate.balance = balance;
                    process_result(&app, &state, &config, &candidate, received, &hits_path, &empty_path);
                }
                Err(error) => {
                    consecutive_errors += 1;
                    if let Ok(mut stats) = state.stats.lock() {
                        stats.errors += 1;
                    }
                    log(&state, "error", format!("[w{worker_id}] {error}"));
                    if config.kill_switch && consecutive_errors >= config.kill_switch_threshold {
                        if let Ok(mut stats) = state.stats.lock() {
                            stats.kill_switch_active = true;
                            stats.running = false;
                        }
                        log(
                            &state,
                            "error",
                            "Килл-свитч сработал: слишком много ошибок API. Сканирование остановлено.",
                        );
                        let _ = stop.try_recv();
                        break;
                    }
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
        }

        if state.stats.lock().map(|s| s.kill_switch_active).unwrap_or(false) {
            break;
        }

        update_stats(&state);
        if last_history.elapsed() >= Duration::from_secs(2) {
            let rate = state
                .stats
                .lock()
                .map(|s| s.checks_per_hour as u64)
                .unwrap_or(0);
            if let Ok(mut history) = state.history.lock() {
                history.push_back(rate);
                while history.len() > 60 {
                    history.pop_front();
                }
            }
            last_history = Instant::now();
        }
        if config.delay_ms > 0 {
            tokio::time::sleep(Duration::from_millis(config.delay_ms)).await;
        }
        // Jitter makes request timing irregular — harder to fingerprint/ban.
        if config.jitter_max_ms > config.jitter_min_ms {
            let jitter = config.jitter_min_ms
                + rand::random::<u64>() % (config.jitter_max_ms - config.jitter_min_ms + 1);
            tokio::time::sleep(Duration::from_millis(jitter)).await;
        }
    }
}

fn process_result(
    app: &tauri::AppHandle,
    state: &SharedState,
    config: &AppConfig,
    candidate: &Candidate,
    received: u64,
    hits_path: &PathBuf,
    empty_path: &PathBuf,
) {
    let fingerprint = format!("{}:{}", candidate.coin, candidate.address);
    if config.address_cache_enabled {
        if let Ok(mut cache) = state.cache.lock() {
            cache.insert(fingerprint, Instant::now());
            if cache.len() > 200_000 {
                let now = Instant::now();
                let ttl = Duration::from_secs(config.address_cache_ttl_minutes * 60);
                cache.retain(|_, time| now.duration_since(*time) < ttl);
            }
        }
    }

    let mut stats = match state.stats.lock() {
        Ok(stats) => stats,
        Err(_) => return,
    };
    stats.total_checked += 1;
    stats.last_address = candidate.address.clone();
    stats.last_coin = candidate.coin.clone();
    stats.last_phrase = candidate.phrase.clone();
    stats.last_balance = candidate.balance;

    if candidate.balance > 0 {
        stats.hits += 1;
        let record = HitRecord {
            id: uuid::Uuid::new_v4().to_string(),
            time: chrono::Utc::now().to_rfc3339(),
            coin: candidate.coin.clone(),
            address: candidate.address.clone(),
            phrase: candidate.phrase.chars().take(300).collect(),
            source: candidate.source.clone(),
            balance: candidate.balance,
            received,
        };
        if let Ok(mut hits) = state.hits.lock() {
            hits.insert(0, record.clone());
            while hits.len() > 1000 {
                hits.pop();
            }
        }
        if !config.memory_only_mode {
            let password = if config.encryption_enabled {
                Some(config.encryption_password.as_str())
            } else {
                None
            };
            let _ = results::append_hit(hits_path, &record, password);
        }
        let log_text = if config.privacy_log_phrases {
            format!("🎯 {} — {} ({})", candidate.coin, candidate.address, candidate.phrase)
        } else {
            format!("🎯 {} — {}", candidate.coin, candidate.address)
        };
        log(state, "hit", log_text);
        let cfg = config.clone();
        let hit = record.clone();
        let handle = app.clone();
        tauri::async_runtime::spawn(async move {
            crate::notifier::notify(&hit, &cfg).await;
            let _ = handle;
        });
    } else if received > 0 {
        stats.received_but_empty += 1;
    } else {
        stats.empty += 1;
        if config.save_empty && !config.memory_only_mode {
            let record = EmptyRecord {
                time: chrono::Utc::now().to_rfc3339(),
                coin: candidate.coin.clone(),
                address: candidate.address.clone(),
                phrase: candidate.phrase.chars().take(300).collect(),
            };
            let password = if config.encryption_enabled {
                Some(config.encryption_password.as_str())
            } else {
                None
            };
            let _ = results::append_empty(empty_path, &record, password);
        }
    }
}

fn update_stats(state: &SharedState) {
    let elapsed = state
        .started_at
        .lock()
        .ok()
        .and_then(|started| started.map(|at| at.elapsed().as_secs_f64()))
        .unwrap_or(0.0);
    if elapsed <= 0.0 {
        return;
    }
    if let Ok(mut stats) = state.stats.lock() {
        stats.checks_per_hour = stats.total_checked as f64 / elapsed * 3600.0;
        stats.uptime_seconds = elapsed as u64;
        stats.cache_size = state.cache.lock().map(|c| c.len()).unwrap_or(0);
        stats.seen_size = state.seen.lock().map(|s| s.len()).unwrap_or(0);
    }
}

/// Convenience used by the UI "generate example" button (no network calls).
pub fn preview_candidates(config: &AppConfig, count: usize) -> Vec<(String, String, String)> {
    let mut rng = rand::thread_rng();
    (0..count)
        .map(|i| {
            let candidate = generate_candidate(config, &mut rng, i as u64, i as u64);
            (candidate.coin, candidate.address, candidate.phrase)
        })
        .collect()
}
