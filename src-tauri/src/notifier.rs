//! Optional Telegram and Discord notifications (non-blocking, best-effort).

use crate::config::AppConfig;
use crate::results::HitRecord;

fn format_units(coin: &str, amount: u64) -> String {
    let decimals: u32 = match coin {
        "ETH" | "MATIC" | "BSC" | "ARB" | "OPT" | "BASE" | "AVAX" | "BNB" => 18,
        "SOL" | "TON" | "BCH" | "LTC" | "DOGE" | "BTC" => {
            if coin == "TON" { 9 } else { 8 }
        }
        _ => 8,
    };
    let divisor = 10u128.pow(decimals);
    let whole = amount as u128 / divisor;
    let frac = (amount as u128 % divisor).to_string();
    let frac = frac.trim_end_matches('0');
    if frac.is_empty() {
        format!("{whole}")
    } else {
        format!("{whole}.{frac}")
    }
}

fn telegram_text(hit: &HitRecord) -> String {
    format!(
        "🎯 Найден кошелёк!\nМонета: {}\nАдрес: <code>{}</code>\nБаланс: {}\nИсточник: {}\nФраза: <code>{}</code>\nВремя: {}",
        hit.coin,
        hit.address,
        format_units(&hit.coin, hit.balance),
        hit.source,
        hit.phrase,
        hit.time
    )
}

fn discord_payload(hit: &HitRecord) -> serde_json::Value {
    serde_json::json!({
        "username": "Happiness Scanner",
        "embeds": [{
            "title": "🎯 Найден кошелёк с балансом!",
            "color": 0x22c55e,
            "fields": [
                { "name": "Монета", "value": hit.coin, "inline": true },
                { "name": "Баланс", "value": format_units(&hit.coin, hit.balance), "inline": true },
                { "name": "Адрес", "value": format!("`{}`", hit.address), "inline": false },
                { "name": "Фраза / ключ", "value": format!("`{}`", hit.phrase), "inline": false },
                { "name": "Режим", "value": hit.source, "inline": true }
            ],
            "timestamp": chrono::Utc::now().to_rfc3339()
        }]
    })
}

pub async fn notify(hit: &HitRecord, config: &AppConfig) {
    let http = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
    {
        Ok(client) => client,
        Err(_) => return,
    };

    if config.telegram_enabled && !config.telegram_token.is_empty() && !config.telegram_chat_id.is_empty() {
        let url = format!(
            "https://api.telegram.org/bot{}/sendMessage",
            config.telegram_token
        );
        let body = serde_json::json!({
            "chat_id": config.telegram_chat_id,
            "text": telegram_text(hit),
            "parse_mode": "HTML",
            "disable_web_page_preview": true,
        });
        let _ = http.post(&url).json(&body).send().await;
    }

    if config.discord_enabled && !config.discord_webhook.trim().is_empty() {
        let _ = http
            .post(config.discord_webhook.trim())
            .json(&discord_payload(hit))
            .send()
            .await;
    }
}
