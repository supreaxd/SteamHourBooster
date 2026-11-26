use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use serde::{Deserialize, Serialize};
use crate::data::{ActiveDropGame, InventoryItem, SellStrategy};
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardDropEvent {
    pub timestamp: String,
    pub username: String,
    pub game_name: String,
    pub card_name: String,
    pub market_price: String,
}

static CARD_DROPS: OnceLock<Mutex<Vec<CardDropEvent>>> = OnceLock::new();

pub fn record_card_drop(event: CardDropEvent) {
    let drops = CARD_DROPS.get_or_init(|| Mutex::new(Vec::new()));
    if let Ok(mut lock) = drops.lock() {
        lock.push(event);
        if lock.len() > 100 {
            lock.remove(0);
        }
    }
}

pub fn get_recent_card_drops() -> Vec<CardDropEvent> {
    let drops = CARD_DROPS.get_or_init(|| Mutex::new(Vec::new()));
    drops.lock().map(|l| l.clone()).unwrap_or_default()
}

static INVENTORY_CACHE: OnceLock<Mutex<HashMap<u64, AccountInventoryInfo>>> = OnceLock::new();
static DETAILED_INVENTORY_CACHE: OnceLock<Mutex<HashMap<u64, Vec<InventoryItem>>>> = OnceLock::new();
static ACTIVE_DROPS_CACHE: OnceLock<Mutex<HashMap<u64, Vec<ActiveDropGame>>>> = OnceLock::new();

fn get_cache() -> &'static Mutex<HashMap<u64, AccountInventoryInfo>> {
    INVENTORY_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn get_account_inventory(steam_id: u64) -> Option<AccountInventoryInfo> {
    get_cache().lock().ok().and_then(|c| c.get(&steam_id).cloned())
}

fn get_detailed_cache() -> &'static Mutex<HashMap<u64, Vec<InventoryItem>>> {
    DETAILED_INVENTORY_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn get_cached_detailed_inventory(steam_id: u64) -> Vec<InventoryItem> {
    let cache = DETAILED_INVENTORY_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    cache.lock().ok().and_then(|c| c.get(&steam_id).cloned()).unwrap_or_default()
}

pub fn set_cached_detailed_inventory(steam_id: u64, items: Vec<InventoryItem>) {
    let cache = DETAILED_INVENTORY_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut c) = cache.lock() {
        c.insert(steam_id, items);
    }
}

pub fn get_cached_active_drops(steam_id: u64) -> Vec<ActiveDropGame> {
    let cache = ACTIVE_DROPS_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    cache.lock().ok().and_then(|c| c.get(&steam_id).cloned()).unwrap_or_default()
}

pub fn set_cached_active_drops(steam_id: u64, drops: Vec<ActiveDropGame>) {
    let cache = ACTIVE_DROPS_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut c) = cache.lock() {
        c.insert(steam_id, drops);
    }
}

/// Calculate seller received price from gross market price
/// Steam fee formula: 5% Steam fee + 10% publisher fee = 15% fee
pub fn calculate_seller_receives(gross: f64) -> f64 {
    if gross <= 0.02 {
        0.01
    } else {
        (gross / 1.15 * 100.0).round() / 100.0
    }
}

/// Helper to build HTTP client with optional proxy
fn build_http_client(proxy_url: Option<String>) -> reqwest::Client {
    let mut builder = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36")
        .timeout(Duration::from_secs(8));

    if let Some(proxy_str) = proxy_url.filter(|p| !p.trim().is_empty()) {
        if let Ok(proxy) = reqwest::Proxy::all(&proxy_str) {
            builder = builder.proxy(proxy);
        }
    }

    builder.build().unwrap_or_default()
}

/// Scans the Steam Badges page of the user to find all games with active card drops remaining
pub async fn fetch_active_drops(steam_id: u64, proxy_url: Option<String>) -> Vec<ActiveDropGame> {
    if steam_id == 0 {
        return Vec::new();
    }

    let client = build_http_client(proxy_url.clone());
    let direct_client = build_http_client(None);
    let badges_url = format!("https://steamcommunity.com/profiles/{}/badges/?l=english", steam_id);
    let referer = format!("https://steamcommunity.com/profiles/{}/", steam_id);

    let req1 = client.get(&badges_url).header("Referer", &referer);
    let req2 = direct_client.get(&badges_url).header("Referer", &referer);

    let resp_res = if proxy_url.is_some() {
        match req1.send().await {
            Ok(r) => Ok(r),
            Err(e) => {
                logger::log_warn(&format!("Proxy failed for badges ({}), falling back to direct connection", e));
                req2.send().await
            }
        }
    } else {
        req2.send().await
    };

    let mut result_drops = Vec::new();

    if let Ok(resp) = resp_res {
        if resp.status().is_success() {
            if let Ok(html) = resp.text().await {
                let rows: Vec<&str> = html.split("class=\"badge_row ").collect();
                for r in rows.iter().skip(1) {
                    let app_id = if let Some(idx) = r.find("/gamecards/") {
                        let sub = &r[idx + 11..];
                        if let Some(end_idx) = sub.find('/') {
                            sub[..end_idx].parse::<u32>().unwrap_or(0)
                        } else {
                            0
                        }
                    } else {
                        0
                    };

                    if app_id == 0 {
                        continue;
                    }

                    // Extract game name
                    let name = if let Some(title_idx) = r.find("badge_title") {
                        let sub = &r[title_idx..];
                        if let Some(start) = sub.find('>') {
                            let inner = &sub[start + 1..];
                            if let Some(end) = inner.find('<') {
                                inner[..end].trim().to_string()
                            } else {
                                format!("AppID {}", app_id)
                            }
                        } else {
                            format!("AppID {}", app_id)
                        }
                    } else {
                        format!("AppID {}", app_id)
                    };

                    // Extract remaining cards
                    let mut remaining: u32 = 0;
                    let patterns = [
                        "card drops remaining",
                        "card drop remaining",
                        "drops remaining",
                        "kart düşürme hakkınız kaldı",
                        "kart kaldı",
                    ];
                    for pat in &patterns {
                        if let Some(pos) = r.find(pat) {
                            let before = &r[..pos];
                            if let Some(span_start) = before.rfind('>') {
                                let num_str = before[span_start + 1..].trim();
                                if let Ok(n) = num_str.parse::<u32>() {
                                    remaining = n;
                                    break;
                                }
                            }
                        }
                    }

                    if remaining > 0 {
                        result_drops.push(ActiveDropGame {
                            app_id,
                            name: clean_html_entities(&name),
                            remaining_cards: remaining,
                            cards_dropped: 0,
                        });
                    }
                }
            }
        }
    }

    // Sort by remaining cards descending
    result_drops.sort_by(|a, b| b.remaining_cards.cmp(&a.remaining_cards));
    logger::log_info(&format!("Active drops scan for {}: found {} games with card drops", steam_id, result_drops.len()));
    set_cached_active_drops(steam_id, result_drops.clone());
    result_drops
}

fn clean_html_entities(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

/// Fetch complete inventory items for cards (753), CS2 (730), TF2 (440)
/// Includes proxy failover and rate-limit mitigation
pub async fn fetch_detailed_inventory(steam_id: u64, proxy_url: Option<String>) -> Vec<InventoryItem> {
    if steam_id == 0 {
        return Vec::new();
    }

    let client = build_http_client(proxy_url.clone());
    let direct_client = build_http_client(None);
    let mut items = Vec::new();
    let referer = format!("https://steamcommunity.com/profiles/{}/inventory", steam_id);

    // Contexts to fetch: 753/6 (Cards/Steam items), 730/2 (CS2), 440/2 (TF2)
    let endpoints = [
        (753, 6, "Steam Cards & Community"),
        (730, 2, "Counter-Strike 2"),
        (440, 2, "Team Fortress 2"),
    ];

    for (app_id, context_id, label) in endpoints {
        let url = format!(
            "https://steamcommunity.com/inventory/{}/{}/{}?l=english&count=75",
            steam_id, app_id, context_id
        );

        let req1 = client
            .get(&url)
            .header("Referer", &referer)
            .header("Accept", "application/json, text/javascript, */*; q=0.01");
        let req2 = direct_client
            .get(&url)
            .header("Referer", &referer)
            .header("Accept", "application/json, text/javascript, */*; q=0.01");

        let resp_res = if proxy_url.is_some() {
            match req1.send().await {
                Ok(r) => Ok(r),
                Err(e) => {
                    logger::log_warn(&format!("Proxy failed for {} inventory: {}, falling back to direct", label, e));
                    req2.send().await
                }
            }
        } else {
            req2.send().await
        };

        if let Ok(resp) = resp_res {
            if resp.status().is_success() {
                if let Ok(json) = resp.json::<serde_json::Value>().await {
                    parse_inventory_json(&json, &mut items, app_id);
                }
            } else {
                logger::log_warn(&format!("Steam inventory API returned HTTP {} for {}", resp.status(), label));
            }
        }

        tokio::time::sleep(Duration::from_millis(300)).await;
    }

    logger::log_success(&format!("Successfully loaded {} inventory items for SteamID {}", items.len(), steam_id));
    set_cached_detailed_inventory(steam_id, items.clone());

    // Spawn background task to fetch live Steam Community Market prices for all unique items
    let needed: Vec<(u32, String)> = items
        .iter()
        .filter(|i| i.marketable && !i.market_hash_name.is_empty())
        .map(|i| (i.app_id, i.market_hash_name.clone()))
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    if !needed.is_empty() {
        let sid = steam_id;
        let p_url = proxy_url.clone();
        tokio::spawn(async move {
            update_inventory_market_prices(sid, needed, p_url).await;
        });
    }

    items
}

fn parse_inventory_json(json: &serde_json::Value, items: &mut Vec<InventoryItem>, default_app_id: u32) {
    let assets = match json.get("assets").and_then(|a| a.as_array()) {
        Some(a) => a,
        None => return,
    };
    let descriptions = match json.get("descriptions").and_then(|d| d.as_array()) {
        Some(d) => d,
        None => return,
    };

    for asset in assets {
        let classid = asset.get("classid").and_then(|c| c.as_str()).unwrap_or("");
        let assetid = asset.get("assetid").and_then(|a| a.as_str()).unwrap_or("");
        let instanceid = asset.get("instanceid").and_then(|i| i.as_str()).unwrap_or("0");
        let amount = asset.get("amount").and_then(|a| a.as_str()).and_then(|a| a.parse::<u32>().ok()).unwrap_or(1);

        if let Some(desc) = descriptions.iter().find(|d| d.get("classid").and_then(|c| c.as_str()) == Some(classid)) {
            let name = desc.get("name").and_then(|n| n.as_str()).unwrap_or("Unknown").to_string();
            let market_hash_name = desc.get("market_hash_name").and_then(|n| n.as_str()).unwrap_or(&name).to_string();
            let icon_url_suffix = desc.get("icon_url").and_then(|u| u.as_str()).unwrap_or("");
            let icon_url = if icon_url_suffix.is_empty() {
                String::new()
            } else {
                format!("https://community.akamai.steamstatic.com/economy/image/{}", icon_url_suffix)
            };

            let marketable = desc.get("marketable").and_then(|m| m.as_i64()).unwrap_or(0) == 1;
            let tradable = desc.get("tradable").and_then(|t| t.as_i64()).unwrap_or(0) == 1;

            // Extract tags for game name, item type, and rarity
            let mut game_name = String::new();
            let mut item_type = "Item".to_string();
            let mut rarity_name = String::new();

            if let Some(tags) = desc.get("tags").and_then(|t| t.as_array()) {
                for tag in tags {
                    let cat = tag.get("category").and_then(|c| c.as_str()).unwrap_or("");
                    let loc_name = tag.get("localized_tag_name").and_then(|l| l.as_str()).unwrap_or("");
                    if cat == "Game" || cat == "game" {
                        game_name = loc_name.to_string();
                    } else if cat == "item_class" || cat == "Type" || cat == "cardborder" {
                        item_type = loc_name.to_string();
                    } else if cat == "Rarity" || cat == "rarity" {
                        rarity_name = loc_name.to_string();
                    }
                }
            }

            if game_name.is_empty() {
                game_name = match default_app_id {
                    730 => "Counter-Strike 2".to_string(),
                    440 => "Team Fortress 2".to_string(),
                    _ => "Steam".to_string(),
                };
            }

            // Check live price cache first, or assign realistic rarity/type estimate
            let (base_price, seller_receives) = if let Some((p, r)) = get_cached_item_price(default_app_id, &market_hash_name) {
                (p, r)
            } else {
                let name_lower = name.to_lowercase();
                let type_lower = item_type.to_lowercase();
                let rarity_lower = rarity_name.to_lowercase();
                let estimated = if name_lower.contains("knife") || name_lower.contains("karambit") || name_lower.contains("bayonet") || name_lower.contains("gloves") {
                    85.00
                } else if type_lower.contains("graffiti") || name_lower.contains("sealed graffiti") {
                    0.04
                } else if name_lower.contains("glitter") || name_lower.contains("holo") || name_lower.contains("foil") || name_lower.contains("gold") {
                    0.45
                } else if type_lower.contains("sticker") || type_lower.contains("çıkartma") {
                    0.10
                } else if type_lower.contains("case") || type_lower.contains("kasa") || type_lower.contains("container") {
                    0.65
                } else if type_lower.contains("parlak") || type_lower.contains("foil") {
                    0.30
                } else if type_lower.contains("kart") || type_lower.contains("card") {
                    0.06
                } else if default_app_id == 730 {
                    if rarity_lower.contains("covert") || name_lower.contains("covert") {
                        10.00
                    } else if rarity_lower.contains("classified") || name_lower.contains("classified") {
                        3.20
                    } else if rarity_lower.contains("restricted") || name_lower.contains("restricted") {
                        0.95
                    } else if rarity_lower.contains("mil-spec") || rarity_lower.contains("high grade") {
                        0.18
                    } else if rarity_lower.contains("industrial") {
                        0.07
                    } else if rarity_lower.contains("consumer") || rarity_lower.contains("base grade") {
                        0.04
                    } else {
                        0.08
                    }
                } else {
                    0.05
                };
                (estimated, calculate_seller_receives(estimated))
            };

            items.push(InventoryItem {
                asset_id: assetid.to_string(),
                class_id: classid.to_string(),
                instance_id: instanceid.to_string(),
                name,
                market_hash_name,
                icon_url,
                item_type,
                game_name,
                app_id: default_app_id,
                amount,
                marketable,
                tradable,
                market_price: Some(base_price),
                user_receives: Some(seller_receives),
            });
        }
    }
}

static LIVE_PRICE_CACHE: OnceLock<Mutex<HashMap<(u32, String), (f64, f64)>>> = OnceLock::new();

fn get_live_price_cache() -> &'static Mutex<HashMap<(u32, String), (f64, f64)>> {
    LIVE_PRICE_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn get_cached_item_price(app_id: u32, market_hash_name: &str) -> Option<(f64, f64)> {
    if let Ok(c) = get_live_price_cache().lock() {
        if let Some(price) = c.get(&(app_id, market_hash_name.to_string())) {
            return Some(*price);
        }
    }
    None
}

pub fn set_cached_item_price(app_id: u32, market_hash_name: &str, price: (f64, f64)) {
    if let Ok(mut c) = get_live_price_cache().lock() {
        c.insert((app_id, market_hash_name.to_string()), price);
    }
}

fn parse_usd_price(raw: &str) -> Option<f64> {
    let cleaned: String = raw
        .replace('$', "")
        .replace(',', ".")
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    cleaned.parse::<f64>().ok().filter(|&p| p > 0.0)
}

/// Fetch real-time market price for a single item by market_hash_name
pub async fn fetch_market_price(market_hash_name: &str, app_id: u32, proxy_url: Option<String>) -> Option<(f64, f64)> {
    if let Some(cached) = get_cached_item_price(app_id, market_hash_name) {
        return Some(cached);
    }

    let client = build_http_client(proxy_url);
    let url = format!(
        "https://steamcommunity.com/market/priceoverview/?country=US&currency=1&appid={}&market_hash_name={}",
        app_id,
        crate::data::urlencoding_encode(market_hash_name)
    );

    for attempt in 0..2 {
        let req = client
            .get(&url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36")
            .header("Accept", "application/json, text/javascript, */*; q=0.01")
            .header("Referer", "https://steamcommunity.com/market/");

        if let Ok(resp) = req.send().await {
            if resp.status().as_u16() == 429 && attempt == 0 {
                tokio::time::sleep(Duration::from_millis(2500)).await;
                continue;
            }
            if resp.status().is_success() {
                if let Ok(json) = resp.json::<serde_json::Value>().await {
                    if json.get("success").and_then(|s| s.as_bool()).unwrap_or(false) {
                        let lowest = json.get("lowest_price").and_then(|p| p.as_str()).and_then(parse_usd_price);
                        let median = json.get("median_price").and_then(|p| p.as_str()).and_then(parse_usd_price);
                        // Protect against outlier lowest_price spikes (e.g. $12.00 on $0.04 graffiti)
                        let price_opt = match (lowest, median) {
                            (Some(l), Some(m)) if l > m * 2.5 => Some(m),
                            (Some(l), _) => Some(l),
                            (None, Some(m)) => Some(m),
                            (None, None) => None,
                        };
                        if let Some(price) = price_opt {
                            let receives = calculate_seller_receives(price);
                            set_cached_item_price(app_id, market_hash_name, (price, receives));
                            return Some((price, receives));
                        }
                    }
                }
            }
        }
        break;
    }
    None
}

/// Background updater for inventory items market prices
pub async fn update_inventory_market_prices(
    steam_id: u64,
    needed: Vec<(u32, String)>,
    proxy_url: Option<String>,
) {
    let mut updated_any = false;
    for (app_id, market_hash_name) in needed {
        if let Some((price, receives)) = fetch_market_price(&market_hash_name, app_id, proxy_url.clone()).await {
            if let Ok(mut cache) = get_detailed_cache().lock() {
                if let Some(items) = cache.get_mut(&steam_id) {
                    for it in items.iter_mut() {
                        if it.app_id == app_id && it.market_hash_name == market_hash_name {
                            it.market_price = Some(price);
                            it.user_receives = Some(receives);
                            updated_any = true;
                        }
                    }
                }
            }
        }
        tokio::time::sleep(Duration::from_millis(450)).await;
    }
    if updated_any {
        logger::log_info(&format!("✓ Updated real-time Steam Community Market prices for SteamID {}", steam_id));
    }
}

/// Bulk sell result
#[derive(Debug, Clone)]
pub struct BulkSellResult {
    pub total_sold: usize,
    pub total_gross: f64,
    pub total_net: f64,
    pub requires_mobile_confirmation: bool,
    pub mobile_confirmation_count: usize,
    pub error_count: usize,
    pub error_message: Option<String>,
    pub sold_asset_ids: Vec<String>,
}

/// Obtain Steam web session cookies (`sessionid` and `steamLoginSecure`) from a refresh token
pub async fn get_steam_web_session(
    refresh_token: &str,
    proxy_url: Option<&str>,
) -> Result<(String, String), String> {
    let client = if let Some(proxy_str) = proxy_url.filter(|p| !p.trim().is_empty()) {
        if let Ok(proxy) = reqwest::Proxy::all(proxy_str) {
            reqwest::Client::builder()
                .proxy(proxy)
                .timeout(Duration::from_secs(12))
                .build()
                .unwrap_or_default()
        } else {
            reqwest::Client::builder()
                .timeout(Duration::from_secs(12))
                .build()
                .unwrap_or_default()
        }
    } else {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(12))
            .build()
            .unwrap_or_default()
    };

    let session_id = format!(
        "{:016x}{:08x}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        std::process::id()
    );

    let finalize_url = "https://login.steampowered.com/jwt/finalizelogin";
    let body_str = format!(
        "nonce={}&sessionid={}&redir={}",
        crate::data::urlencoding_encode(refresh_token),
        crate::data::urlencoding_encode(&session_id),
        crate::data::urlencoding_encode("https://steamcommunity.com/login/home/?goto=")
    );

    let resp = client
        .post(finalize_url)
        .header(
            "User-Agent",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
        )
        .header("Content-Type", "application/x-www-form-urlencoded; charset=UTF-8")
        .body(body_str)
        .send()
        .await
        .map_err(|e| format!("Failed to connect to Steam login: {}", e))?;

    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Invalid JSON from Steam login: {}", e))?;

    let steam_id = json
        .get("steamID")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Steam login returned no steamID".to_string())?;

    let transfers = json
        .get("transfer_info")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "Steam login returned no transfer_info".to_string())?;

    let mut community_transfer = None;
    for t in transfers {
        if let Some(url) = t.get("url").and_then(|u| u.as_str()) {
            if url.contains("steamcommunity.com") {
                community_transfer = Some(t);
                break;
            }
        }
    }

    let transfer = community_transfer.ok_or_else(|| "No steamcommunity transfer URL found".to_string())?;
    let set_url = transfer.get("url").and_then(|u| u.as_str()).unwrap_or("");
    let params_obj = transfer
        .get("params")
        .and_then(|p| p.as_object())
        .ok_or_else(|| "Missing transfer params".to_string())?;

    let mut set_pairs = Vec::new();
    set_pairs.push(format!("steamID={}", crate::data::urlencoding_encode(steam_id)));
    for (k, v) in params_obj {
        if let Some(s) = v.as_str() {
            set_pairs.push(format!("{}={}", crate::data::urlencoding_encode(k), crate::data::urlencoding_encode(s)));
        }
    }
    let set_body = set_pairs.join("&");

    let set_resp = client
        .post(set_url)
        .header(
            "User-Agent",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
        )
        .header("Content-Type", "application/x-www-form-urlencoded; charset=UTF-8")
        .body(set_body)
        .send()
        .await
        .map_err(|e| format!("Failed to set token for steamcommunity: {}", e))?;

    let mut steam_login_secure = None;
    let mut final_session_id = session_id;
    for cookie_header in set_resp.headers().get_all(reqwest::header::SET_COOKIE) {
        if let Ok(cookie_str) = cookie_header.to_str() {
            if let Some(val) = cookie_str.split(';').next() {
                if let Some(token_val) = val.strip_prefix("steamLoginSecure=") {
                    steam_login_secure = Some(token_val.to_string());
                } else if let Some(sid_val) = val.strip_prefix("sessionid=") {
                    if !sid_val.trim().is_empty() {
                        final_session_id = sid_val.trim().to_string();
                    }
                }
            }
        }
    }

    let login_secure = steam_login_secure.ok_or_else(|| "steamLoginSecure cookie not received from Steam".to_string())?;
    Ok((final_session_id, login_secure))
}

/// Execute bulk sell operation with strategy and real Steam Market listing
pub async fn execute_bulk_sell(
    items: &[InventoryItem],
    strategy: SellStrategy,
    custom_offset: f64,
    refresh_token: Option<String>,
    steam_id: u64,
    proxy_url: Option<String>,
    _identity_secret: Option<String>,
) -> BulkSellResult {
    let mut total_sold = 0;
    let mut total_gross = 0.0;
    let mut total_net = 0.0;
    let mut mobile_confirmation_count = 0;
    let mut error_count = 0;
    let mut last_error = None;
    let mut sold_asset_ids = Vec::new();

    let token = match refresh_token.filter(|t| !t.trim().is_empty()) {
        Some(t) => t,
        None => {
            return BulkSellResult {
                total_sold: 0,
                total_gross: 0.0,
                total_net: 0.0,
                requires_mobile_confirmation: false,
                mobile_confirmation_count: 0,
                error_count: items.len(),
                error_message: Some("No Steam session token. Please log in with QR code in Accounts tab to enable Market selling.".to_string()),
                sold_asset_ids: Vec::new(),
            };
        }
    };

    // Obtain real session cookies
    let (session_id, steam_login_secure) = match get_steam_web_session(&token, proxy_url.as_deref()).await {
        Ok(s) => s,
        Err(e) => {
            logger::log_error(&format!("Steam Market Web Auth failed: {}", e));
            return BulkSellResult {
                total_sold: 0,
                total_gross: 0.0,
                total_net: 0.0,
                requires_mobile_confirmation: false,
                mobile_confirmation_count: 0,
                error_count: items.len(),
                error_message: Some(format!("Steam Web Login failed: {}. Please re-authenticate via QR code.", e)),
                sold_asset_ids: Vec::new(),
            };
        }
    };

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .unwrap_or_default();

    for item in items {
        if !item.marketable {
            continue;
        }

        let base_price = item.market_price.unwrap_or(0.05);
        let gross = strategy.compute_price(base_price, custom_offset);
        let net = calculate_seller_receives(gross);
        let seller_cents = ((net * 100.0).round().max(1.0)) as u64;

        let context_id = if item.app_id == 753 { "6" } else { "2" };
        let app_id_str = item.app_id.to_string();
        let seller_cents_str = seller_cents.to_string();

        let form_body = format!(
            "sessionid={}&appid={}&contextid={}&assetid={}&amount=1&price={}",
            crate::data::urlencoding_encode(&session_id),
            crate::data::urlencoding_encode(&app_id_str),
            crate::data::urlencoding_encode(context_id),
            crate::data::urlencoding_encode(&item.asset_id),
            crate::data::urlencoding_encode(&seller_cents_str),
        );

        let sell_url = "https://steamcommunity.com/market/sellitem/";
        let referer_url = format!("https://steamcommunity.com/profiles/{}/inventory/", steam_id);
        let cookie_header = format!("sessionid={}; steamLoginSecure={};", session_id, steam_login_secure);

        let resp_res = client
            .post(sell_url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .header("Referer", &referer_url)
            .header("Origin", "https://steamcommunity.com")
            .header("Cookie", &cookie_header)
            .header("Content-Type", "application/x-www-form-urlencoded; charset=UTF-8")
            .body(form_body)
            .send()
            .await;

        match resp_res {
            Ok(resp) => {
                if let Ok(json) = resp.json::<serde_json::Value>().await {
                    let is_success = json.get("success").and_then(|v| v.as_bool()).unwrap_or(false);
                    if is_success {
                        let requires_conf = json.get("requires_confirmation").and_then(|v| v.as_i64()).unwrap_or(0) == 1
                            || json.get("needs_mobile_confirmation").and_then(|v| v.as_bool()).unwrap_or(false);

                        if requires_conf {
                            mobile_confirmation_count += 1;
                        }

                        total_sold += 1;
                        total_gross += gross;
                        total_net += net;
                        sold_asset_ids.push(item.asset_id.clone());

                        logger::log_success(&format!(
                            "🛒 [Market Listing] Listed '{}' at ${:.2} (Seller: ${:.2}){}",
                            item.name, gross, net,
                            if requires_conf { " [Mobile Confirmation Required]" } else { "" }
                        ));
                    } else {
                        error_count += 1;
                        let msg = json.get("message").and_then(|m| m.as_str()).unwrap_or("Listing rejected by Steam");
                        last_error = Some(msg.to_string());
                        logger::log_error(&format!("Market Listing Failed for '{}': {}", item.name, msg));
                    }
                } else {
                    error_count += 1;
                    last_error = Some("Invalid response from Steam Community Market".to_string());
                }
            }
            Err(e) => {
                error_count += 1;
                last_error = Some(format!("Network request failed: {}", e));
            }
        }

        // Safe delay between market listings to prevent Steam 429
        tokio::time::sleep(Duration::from_millis(350)).await;
    }

    BulkSellResult {
        total_sold,
        total_gross,
        total_net,
        requires_mobile_confirmation: mobile_confirmation_count > 0,
        mobile_confirmation_count,
        error_count,
        error_message: last_error,
        sold_asset_ids,
    }
}

pub async fn check_inventory_tracker(
    username: String,
    steam_id: u64,
    proxy_url: Option<String>,
) -> Option<AccountInventoryInfo> {
    if steam_id == 0 {
        return None;
    }

    let client = build_http_client(proxy_url);

    // 1. Steam Topluluk Envanteri (753/6 - Kartlar, Rozetler)
    let steam_cards_url = format!("https://steamcommunity.com/inventory/{}/753/6?l=english&count=500", steam_id);
    let mut card_count = 0;
    if let Ok(resp) = client.get(&steam_cards_url).send().await {
        if let Ok(json) = resp.json::<serde_json::Value>().await {
            if let Some(assets) = json.get("assets").and_then(|a| a.as_array()) {
                card_count = assets.len();
            }
        }
    }

    // 2. CS2 Envanteri (730/2)
    let cs2_url = format!("https://steamcommunity.com/inventory/{}/730/2?l=english&count=500", steam_id);
    let mut cs2_count = 0;
    if let Ok(resp) = client.get(&cs2_url).send().await {
        if let Ok(json) = resp.json::<serde_json::Value>().await {
            if let Some(assets) = json.get("assets").and_then(|a| a.as_array()) {
                cs2_count = assets.len();
            }
        }
    }

    // 3. TF2 Envanteri (440/2)
    let tf2_url = format!("https://steamcommunity.com/inventory/{}/440/2?l=english&count=500", steam_id);
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
        if prev.card_count > 0 && card_count > prev.card_count {
            let diff = card_count - prev.card_count;
            logger::log_success(&format!(
                "🃏 [CARD DROP] [{}] +{} new trading card(s) dropped! Total: {} cards",
                username, diff, card_count
            ));
            let global_settings = crate::data::get_global_settings();
            if global_settings.telegram_notify_cards {
                crate::telegram::send_telegram_alert(&format!(
                    "🃏 *[Trading Card Dropped!]* [{}]\n+{} new card(s) added to inventory! Total cards: {}",
                    username, diff, card_count
                ));
            }
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

/// Asynchronously fetch Steam profile avatar URL (supporting static and animated avatars / GIF)
pub async fn fetch_account_avatar(steam_id: u64, proxy_url: Option<String>) -> Option<String> {
    if steam_id == 0 {
        return None;
    }
    let account_id = (steam_id & 0xFFFFFFFF) as u32;
    let url = format!("https://steamcommunity.com/miniprofile/{}/json", account_id);

    let client = if let Some(proxy_str) = proxy_url.filter(|p| !p.trim().is_empty()) {
        if let Ok(proxy) = reqwest::Proxy::all(proxy_str) {
            reqwest::Client::builder()
                .proxy(proxy)
                .timeout(Duration::from_secs(8))
                .build()
                .unwrap_or_default()
        } else {
            reqwest::Client::builder()
                .timeout(Duration::from_secs(8))
                .build()
                .unwrap_or_default()
        }
    } else {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(8))
            .build()
            .unwrap_or_default()
    };

    if let Ok(resp) = client
        .get(&url)
        .header(
            "User-Agent",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
        )
        .header("Referer", "https://steamcommunity.com/")
        .send()
        .await
    {
        if let Ok(json) = resp.json::<serde_json::Value>().await {
            // Check animated avatar first if available
            if let Some(anim) = json.get("animated_avatar").and_then(|v| v.as_str()) {
                if !anim.is_empty() {
                    return Some(anim.to_string());
                }
            }
            if let Some(av) = json.get("avatar_url").and_then(|v| v.as_str()) {
                if !av.is_empty() {
                    return Some(av.to_string());
                }
            }
        }
    }
    None
}

static OWNED_GAMES_CACHE: OnceLock<Mutex<HashMap<u64, Vec<crate::data::GameEntry>>>> = OnceLock::new();

fn get_owned_games_cache() -> &'static Mutex<HashMap<u64, Vec<crate::data::GameEntry>>> {
    OWNED_GAMES_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn store_owned_games_cache(steam_id: u64, games: Vec<crate::data::GameEntry>) {
    if let Ok(mut c) = get_owned_games_cache().lock() {
        c.insert(steam_id, games);
    }
}

pub fn get_cached_owned_games(steam_id: u64) -> Vec<crate::data::GameEntry> {
    if let Ok(c) = get_owned_games_cache().lock() {
        if let Some(games) = c.get(&steam_id) {
            return games.clone();
        }
    }
    Vec::new()
}

/// Asynchronously fetch games owned by the account
pub async fn fetch_account_owned_games_force(
    steam_id: u64,
    proxy_url: Option<String>,
) -> Vec<crate::data::GameEntry> {
    fetch_account_owned_games_internal(steam_id, proxy_url, true).await
}

pub async fn fetch_account_owned_games(
    steam_id: u64,
    proxy_url: Option<String>,
) -> Vec<crate::data::GameEntry> {
    fetch_account_owned_games_internal(steam_id, proxy_url, false).await
}

async fn fetch_account_owned_games_internal(
    steam_id: u64,
    proxy_url: Option<String>,
    force: bool,
) -> Vec<crate::data::GameEntry> {
    if steam_id == 0 {
        return Vec::new();
    }

    if !force {
        let cached = get_cached_owned_games(steam_id);
        if !cached.is_empty() {
            return cached;
        }
    }

    let mut games_map: HashMap<u32, String> = HashMap::new();

    // 1. Check runtime accounts
    let container = crate::telegram::get_runtime_accounts();
    for arc in container {
        if let Ok(a) = arc.lock() {
            if (a.steam_id == steam_id || a.steam_id == 0) && !a.owned_games.is_empty() {
                for g in &a.owned_games {
                    games_map.insert(g.app_id, g.name.clone());
                }
            }
        }
    }

    // 2. Fetch badges pages (p=1 to 4)
    let client = build_http_client(proxy_url.clone());
    for page in 1..=4 {
        let url = format!("https://steamcommunity.com/profiles/{}/badges/?p={}", steam_id, page);
        if let Ok(resp) = client.get(&url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36")
            .send()
            .await 
        {
            if let Ok(html) = resp.text().await {
                let rows: Vec<&str> = html.split("badge_row ").collect();
                if rows.len() <= 1 {
                    break;
                }
                let mut found_page = false;
                for r in &rows[1..] {
                    if let Some(pos) = r.find("/gamecards/") {
                        let sub = &r[pos + 11..];
                        if let Some(slash_pos) = sub.find('/') {
                            if let Ok(appid) = sub[..slash_pos].parse::<u32>() {
                                if let Some(t_idx) = r.find("badge_title\">") {
                                    let sub_title = &r[t_idx + 13..];
                                    if let Some(end_idx) = sub_title.find("</div>") {
                                        let raw_name = sub_title[..end_idx]
                                            .split("&nbsp;")
                                            .next()
                                            .unwrap_or("")
                                            .replace("&amp;", "&")
                                            .replace("&quot;", "\"")
                                            .replace("&#39;", "'")
                                            .trim()
                                            .to_string();
                                        if !raw_name.is_empty() && !raw_name.contains("badge_view_details") {
                                            games_map.entry(appid).or_insert(raw_name);
                                            found_page = true;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                if !found_page {
                    break;
                }
            }
        }
    }

    // 3. Include games from inventory items
    let inv_items = get_cached_detailed_inventory(steam_id);
    for item in inv_items {
        if item.app_id > 0 && !item.game_name.is_empty() && item.game_name != "Steam" {
            games_map.entry(item.app_id).or_insert(item.game_name);
        }
    }

    // 4. Fallback essentials
    games_map.entry(730).or_insert_with(|| "Counter-Strike 2".to_string());
    games_map.entry(440).or_insert_with(|| "Team Fortress 2".to_string());
    games_map.entry(570).or_insert_with(|| "Dota 2".to_string());

    let mut result: Vec<crate::data::GameEntry> = games_map
        .into_iter()
        .map(|(app_id, name)| crate::data::GameEntry::new(app_id, name))
        .collect();

    result.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    store_owned_games_cache(steam_id, result.clone());
    result
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TradeOfferStatus {
    Sent,
    PendingConfirmation,
    Error,
}

#[derive(Debug, Clone)]
pub struct ConsolidateTradeResult {
    pub source_username: String,
    pub target_username: String,
    pub item_count: usize,
    pub trade_offer_id: Option<String>,
    pub status: TradeOfferStatus,
    pub message: String,
}

/// Sends a Steam Trade Offer from a source alt account to a target main account
pub async fn execute_consolidate_trade(
    source_username: String,
    target_username: String,
    target_steam_id: u64,
    trade_url: Option<String>,
    items: Vec<InventoryItem>,
    source_refresh_token: Option<String>,
    proxy_url: Option<String>,
) -> ConsolidateTradeResult {
    let item_count = items.len();
    if item_count == 0 {
        return ConsolidateTradeResult {
            source_username,
            target_username,
            item_count: 0,
            trade_offer_id: None,
            status: TradeOfferStatus::Error,
            message: "No tradable items selected.".to_string(),
        };
    }

    let token = match source_refresh_token.filter(|t| !t.trim().is_empty()) {
        Some(t) => t,
        None => {
            return ConsolidateTradeResult {
                source_username,
                target_username,
                item_count,
                trade_offer_id: None,
                status: TradeOfferStatus::Error,
                message: "Source account has no saved Steam session token (QR login required).".to_string(),
            };
        }
    };

    let (session_id, steam_login_secure) = match get_steam_web_session(&token, proxy_url.as_deref()).await {
        Ok(s) => s,
        Err(e) => {
            return ConsolidateTradeResult {
                source_username,
                target_username,
                item_count,
                trade_offer_id: None,
                status: TradeOfferStatus::Error,
                message: format!("Steam Web Auth failed: {}", e),
            };
        }
    };

    // Parse partner & token from trade_url if provided, else use target_steam_id
    let mut trade_token = String::new();
    let mut partner_account_id: u64 = if target_steam_id > 76561197960265728 {
        target_steam_id - 76561197960265728
    } else {
        target_steam_id
    };

    if let Some(ref url) = trade_url {
        if let Some(q_pos) = url.find('?') {
            for pair in url[q_pos + 1..].split('&') {
                if let Some((k, v)) = pair.split_once('=') {
                    if k == "partner" {
                        if let Ok(p) = v.parse::<u64>() {
                            partner_account_id = p;
                        }
                    } else if k == "token" {
                        trade_token = v.to_string();
                    }
                }
            }
        }
    }

    if partner_account_id == 0 {
        return ConsolidateTradeResult {
            source_username,
            target_username,
            item_count,
            trade_offer_id: None,
            status: TradeOfferStatus::Error,
            message: "Target SteamID or Trade URL is invalid.".to_string(),
        };
    }

    let assets_json: Vec<serde_json::Value> = items
        .iter()
        .map(|it| {
            let ctx_id = if it.app_id == 753 { "6" } else { "2" };
            serde_json::json!({
                "appid": it.app_id,
                "contextid": ctx_id,
                "amount": it.amount.max(1),
                "assetid": it.asset_id,
            })
        })
        .collect();

    let json_tradeoffer = serde_json::json!({
        "newversion": true,
        "version": 4,
        "me": {
            "assets": assets_json,
            "currency": [],
            "ready": false
        },
        "them": {
            "assets": [],
            "currency": [],
            "ready": false
        }
    })
    .to_string();

    let mut params = serde_json::json!({});
    if !trade_token.is_empty() {
        params["trade_offer_access_token"] = serde_json::json!(trade_token);
    }

    let partner_steamid64 = partner_account_id + 76561197960265728;
    let referer = if let Some(ref url) = trade_url {
        if !url.trim().is_empty() {
            url.clone()
        } else {
            format!("https://steamcommunity.com/tradeoffer/new/?partner={}", partner_account_id)
        }
    } else {
        format!("https://steamcommunity.com/tradeoffer/new/?partner={}", partner_account_id)
    };

    let mut active_session_id = session_id;
    let mut cookie_header = format!("sessionid={}; steamLoginSecure={};", active_session_id, steam_login_secure);

    let mut client_builder = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36")
        .cookie_store(true)
        .timeout(Duration::from_secs(15));
    if let Some(ref p) = proxy_url.as_deref().filter(|s| !s.trim().is_empty()) {
        if let Ok(prx) = reqwest::Proxy::all(*p) {
            client_builder = client_builder.proxy(prx);
        }
    }
    let client = client_builder.build().unwrap_or_default();

    // Step 1: Hit steamcommunity.com home so Steam issues a canonical sessionid cookie matching steamLoginSecure
    if let Ok(home_resp) = client
        .get("https://steamcommunity.com/")
        .header("Cookie", &cookie_header)
        .send()
        .await
    {
        for ch in home_resp.headers().get_all(reqwest::header::SET_COOKIE) {
            if let Ok(cs) = ch.to_str() {
                if let Some(kv) = cs.split(';').next() {
                    if let Some(new_sid) = kv.strip_prefix("sessionid=") {
                        if !new_sid.trim().is_empty() && new_sid.trim() != "deleted" {
                            active_session_id = new_sid.trim().to_string();
                            cookie_header = format!("sessionid={}; steamLoginSecure={};", active_session_id, steam_login_secure);
                        }
                    }
                }
            }
        }
    }

    // Step 2: Preflight GET on tradeoffer page so Steam validates friendship / trade token / cooldown
    if let Ok(pre_resp) = client
        .get(&referer)
        .header("Cookie", &cookie_header)
        .send()
        .await
    {
        for cookie_header_val in pre_resp.headers().get_all(reqwest::header::SET_COOKIE) {
            if let Ok(cs) = cookie_header_val.to_str() {
                if let Some(kv) = cs.split(';').next() {
                    if let Some(new_sid) = kv.strip_prefix("sessionid=") {
                        if !new_sid.trim().is_empty() && new_sid.trim() != "deleted" {
                            active_session_id = new_sid.trim().to_string();
                            cookie_header = format!("sessionid={}; steamLoginSecure={};", active_session_id, steam_login_secure);
                        }
                    }
                }
            }
        }
        if let Ok(html) = pre_resp.text().await {
            if html.contains("g_steamID = false") || html.contains("steamcommunity.com/login/home") {
                return ConsolidateTradeResult {
                    source_username,
                    target_username,
                    item_count,
                    trade_offer_id: None,
                    status: TradeOfferStatus::Error,
                    message: "Steam web session is not logged in. Please re-login via QR code for this account.".to_string(),
                };
            }
            if let Some(err_idx) = html.find("id=\"error_msg\"") {
                let slice = &html[err_idx..];
                if let Some(gt) = slice.find('>') {
                    if let Some(end) = slice[gt + 1..].find("</div>") {
                        let raw = &slice[gt + 1..gt + 1 + end];
                        let cleaned = strip_html_tags(raw);
                        if !cleaned.is_empty() {
                            return ConsolidateTradeResult {
                                source_username,
                                target_username,
                                item_count,
                                trade_offer_id: None,
                                status: TradeOfferStatus::Error,
                                message: format!("Steam Trade Error: {}", cleaned),
                            };
                        }
                    }
                }
            }
        }
    }

    let form_body = format!(
        "sessionid={}&serverid=1&partner={}&tradeoffermessage={}&json_tradeoffer={}&captcha=&trade_offer_create_params={}",
        crate::data::urlencoding_encode(&active_session_id),
        partner_steamid64,
        crate::data::urlencoding_encode("SupBooster Consolidate Trade"),
        crate::data::urlencoding_encode(&json_tradeoffer),
        crate::data::urlencoding_encode(&params.to_string()),
    );

    let resp = client
        .post("https://steamcommunity.com/tradeoffer/new/send")
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36")
        .header("Accept", "application/json, text/javascript, */*; q=0.01")
        .header("X-Requested-With", "XMLHttpRequest")
        .header("Referer", &referer)
        .header("Origin", "https://steamcommunity.com")
        .header("Cookie", &cookie_header)
        .header("Content-Type", "application/x-www-form-urlencoded; charset=UTF-8")
        .body(form_body)
        .send()
        .await;

    match resp {
        Ok(r) => {
            let status_code = r.status().as_u16();
            let body_text = r.text().await.unwrap_or_default();
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body_text) {
                if let Some(offer_id) = json.get("tradeofferid").and_then(|v| v.as_str()) {
                    let needs_conf = json.get("needs_mobile_confirmation").and_then(|v| v.as_bool()).unwrap_or(false)
                        || json.get("needs_email_confirmation").and_then(|v| v.as_bool()).unwrap_or(false);
                    let status = if needs_conf {
                        TradeOfferStatus::PendingConfirmation
                    } else {
                        TradeOfferStatus::Sent
                    };
                    logger::log_success(&format!(
                        "📦 [Consolidate] Trade Offer #{} created ({} -> {}, {} items)",
                        offer_id, source_username, target_username, item_count
                    ));
                    ConsolidateTradeResult {
                        source_username,
                        target_username,
                        item_count,
                        trade_offer_id: Some(offer_id.to_string()),
                        status,
                        message: if needs_conf {
                            format!("Offer #{} created — Mobile/Email confirmation required.", offer_id)
                        } else {
                            format!("Offer #{} sent successfully!", offer_id)
                        },
                    }
                } else {
                    let err_msg = json
                        .get("strError")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Trade offer rejected by Steam")
                        .to_string();
                    logger::log_error(&format!("📦 [Consolidate] Failed ({} -> {}): {}", source_username, target_username, err_msg));
                    ConsolidateTradeResult {
                        source_username,
                        target_username,
                        item_count,
                        trade_offer_id: None,
                        status: TradeOfferStatus::Error,
                        message: err_msg,
                    }
                }
            } else {
                let cleaned = strip_html_tags(&body_text);
                let snippet: String = cleaned.chars().take(160).collect();
                let detail = if snippet.is_empty() {
                    format!("HTTP {} (empty response — check if accounts are Steam friends or paste target Trade URL)", status_code)
                } else {
                    format!("HTTP {}: {}", status_code, snippet)
                };
                logger::log_error(&format!("📦 [Consolidate] Non-JSON ({} -> {}): {}", source_username, target_username, detail));
                ConsolidateTradeResult {
                    source_username,
                    target_username,
                    item_count,
                    trade_offer_id: None,
                    status: TradeOfferStatus::Error,
                    message: detail,
                }
            }
        }
        Err(e) => ConsolidateTradeResult {
            source_username,
            target_username,
            item_count,
            trade_offer_id: None,
            status: TradeOfferStatus::Error,
            message: format!("Network error: {}", e),
        },
    }
}

fn strip_html_tags(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_tag = false;
    for ch in input.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                out.push(' ');
            }
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}
