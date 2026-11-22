use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use serde::{Deserialize, Serialize};
use crate::logger;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AccountInventoryInfo {
    pub steam_id: u64,
    pub total_items: usize,
    pub card_count: usize,
    pub cs2_items: usize,
    pub tf2_items: usize,
    pub estimated_value_str: String,
    pub last_checked: String,
}

static INVENTORY_CACHE: OnceLock<Mutex<HashMap<u64, AccountInventoryInfo>>> = OnceLock::new();

fn get_cache() -> &'static Mutex<HashMap<u64, AccountInventoryInfo>> {
    INVENTORY_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn get_account_inventory(steam_id: u64) -> Option<AccountInventoryInfo> {
    get_cache().lock().ok().and_then(|c| c.get(&steam_id).cloned())
}

pub async fn check_inventory_tracker(
    username: String,
    steam_id: u64,
    proxy_url: Option<String>,
) -> Option<AccountInventoryInfo> {
    if steam_id == 0 {
        return None;
    }

    let mut builder = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .timeout(Duration::from_secs(12));

    if let Some(proxy_str) = proxy_url.filter(|p| !p.trim().is_empty()) {
        if let Ok(proxy) = reqwest::Proxy::all(&proxy_str) {
            builder = builder.proxy(proxy);
        }
    }

    let client = builder.build().unwrap_or_default();

    // 1. Steam Topluluk Envanteri (753/6 - Kartlar, Rozetler)
    let steam_cards_url = format!("https://steamcommunity.com/inventory/{}/753/6?l=turkish&count=500", steam_id);
    let mut card_count = 0;
    if let Ok(resp) = client.get(&steam_cards_url).send().await {
        if let Ok(json) = resp.json::<serde_json::Value>().await {
            if let Some(assets) = json.get("assets").and_then(|a| a.as_array()) {
                card_count = assets.len();
            }
        }
    }

    // 2. CS2 Envanteri (730/2)
    let cs2_url = format!("https://steamcommunity.com/inventory/{}/730/2?l=turkish&count=500", steam_id);
    let mut cs2_count = 0;
    if let Ok(resp) = client.get(&cs2_url).send().await {
        if let Ok(json) = resp.json::<serde_json::Value>().await {
            if let Some(assets) = json.get("assets").and_then(|a| a.as_array()) {
                cs2_count = assets.len();
            }
        }
    }

    // 3. TF2 Envanteri (440/2)
    let tf2_url = format!("https://steamcommunity.com/inventory/{}/440/2?l=turkish&count=500", steam_id);
    let mut tf2_count = 0;
    if let Ok(resp) = client.get(&tf2_url).send().await {
        if let Ok(json) = resp.json::<serde_json::Value>().await {
            if let Some(assets) = json.get("assets").and_then(|a| a.as_array()) {
                tf2_count = assets.len();
            }
        }
    }

    let total = card_count + cs2_count + tf2_count;
    let now_str = chrono::Local::now().format("%H:%M:%S").to_string();

    let old_info = get_account_inventory(steam_id);
    if let Some(ref prev) = old_info {
        if total > prev.total_items {
            let diff = total - prev.total_items;
            logger::log_success(&format!(
                "📦 [INVENTORY INCREASED] [{}] +{} new items detected! Total: {} items (🃏 {} cards, 🔫 {} CS2, 🎩 {} TF2)",
                username, diff, total, card_count, cs2_count, tf2_count
            ));
            crate::telegram::send_telegram_alert(&format!(
                "📦 *[Inventory Update]* [{}]\n+{} new items detected!\nTotal: {} items (🃏 {} cards, 🔫 {} CS2, 🎩 {} TF2)",
                username, diff, total, card_count, cs2_count, tf2_count
            ));
        }
    }

    let info = AccountInventoryInfo {
        steam_id,
        total_items: total,
        card_count,
        cs2_items: cs2_count,
        tf2_items: tf2_count,
        estimated_value_str: format!("{} items", total),
        last_checked: now_str,
    };

    if let Ok(mut c) = get_cache().lock() {
        c.insert(steam_id, info.clone());
    }

    Some(info)
}
