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

            // Extract tags for game name and item type
            let mut game_name = String::new();
            let mut item_type = "Item".to_string();

            if let Some(tags) = desc.get("tags").and_then(|t| t.as_array()) {
                for tag in tags {
                    let cat = tag.get("category").and_then(|c| c.as_str()).unwrap_or("");
                    let loc_name = tag.get("localized_tag_name").and_then(|l| l.as_str()).unwrap_or("");
                    if cat == "Game" || cat == "game" {
                        game_name = loc_name.to_string();
                    } else if cat == "item_class" || cat == "Type" || cat == "cardborder" {
                        item_type = loc_name.to_string();
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

            // Estimate market price (default fallback $0.05 - $0.25 depending on type, or loaded asynchronously)
            let base_price = if item_type.contains("Parlak") || item_type.contains("Foil") {
                0.35
            } else if item_type.contains("Kart") || item_type.contains("Card") {
                0.08
            } else if default_app_id == 730 {
                0.20
            } else {
                0.05
            };
            let seller_receives = calculate_seller_receives(base_price);

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

/// Fetch real-time market price for a single item by market_hash_name
pub async fn fetch_market_price(market_hash_name: &str, app_id: u32) -> Option<(f64, f64)> {
    let client = build_http_client(None);
    let url = format!(
        "https://steamcommunity.com/market/priceoverview/?appid={}&currency=1&market_hash_name={}",
        app_id,
        crate::data::urlencoding_encode(market_hash_name)
    );

    if let Ok(resp) = client.get(&url).send().await {
        if let Ok(json) = resp.json::<serde_json::Value>().await {
            if json.get("success").and_then(|s| s.as_bool()).unwrap_or(false) {
                if let Some(lowest_str) = json.get("lowest_price").and_then(|p| p.as_str()) {
                    let cleaned: String = lowest_str.chars().filter(|c| c.is_ascii_digit() || *c == '.').collect();
                    if let Ok(price) = cleaned.parse::<f64>() {
                        let receives = calculate_seller_receives(price);
                        return Some((price, receives));
                    }
                }
            }
        }
    }
    None
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
    for cookie_header in set_resp.headers().get_all(reqwest::header::SET_COOKIE) {
        if let Ok(cookie_str) = cookie_header.to_str() {
            if cookie_str.starts_with("steamLoginSecure=") {
                if let Some(val) = cookie_str.split(';').next() {
                    if let Some(token_val) = val.strip_prefix("steamLoginSecure=") {
                        steam_login_secure = Some(token_val.to_string());
                    }
                }
            }
        }
    }

    let login_secure = steam_login_secure.ok_or_else(|| "steamLoginSecure cookie not received from Steam".to_string())?;
    Ok((session_id, login_secure))
}

/// Execute bulk sell operation with strategy and real Steam Market listing
pub async fn execute_bulk_sell(
    items: &[InventoryItem],
    strategy: SellStrategy,
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

        let gross = match strategy {
            SellStrategy::QuickSell => item.market_price.unwrap_or(0.05),
            SellStrategy::Undercut => (item.market_price.unwrap_or(0.05) - 0.01).max(0.03),
        };
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
pub async fn fetch_account_owned_games(
    steam_id: u64,
    proxy_url: Option<String>,
) -> Vec<crate::data::GameEntry> {
    if steam_id == 0 {
        return Vec::new();
    }

    // 1. Check existing in-memory cache first
    let cached = get_cached_owned_games(steam_id);
    if !cached.is_empty() {
        return cached;
    }

    // 2. Fetch badges page (all games with badges are owned games)
    let drops = fetch_active_drops(steam_id, proxy_url.clone()).await;
    let mut games_map: HashMap<u32, String> = HashMap::new();

    for d in drops {
        games_map.insert(d.app_id, d.name);
    }

    // 3. Query Steam Community user profile games or XML if public
    let client = build_http_client(proxy_url);
    let xml_url = format!("https://steamcommunity.com/profiles/{}/games/?tab=all&xml=1", steam_id);
    if let Ok(resp) = client.get(&xml_url).send().await {
        if let Ok(body) = resp.text().await {
            // Parse <game><appID>...</appID><name>...</name></game>
            for block in body.split("<game>") {
                if let (Some(id_start), Some(name_start)) = (block.find("<appID>"), block.find("<name>")) {
                    let id_str = block[id_start + 7..].split("</appID>").next().unwrap_or("");
                    let name_str = block[name_start + 6..].split("</name>").next().unwrap_or("");
                    if let Ok(appid) = id_str.trim().parse::<u32>() {
                        let clean_name = name_str
                            .trim()
                            .replace("<![CDATA[", "")
                            .replace("]]>", "")
                            .trim()
                            .to_string();
                        if !clean_name.is_empty() {
                            games_map.entry(appid).or_insert(clean_name);
                        }
                    }
                }
            }
        }
    }

    let mut result: Vec<crate::data::GameEntry> = games_map
        .into_iter()
        .map(|(app_id, name)| crate::data::GameEntry::new(app_id, name))
        .collect();

    result.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    if !result.is_empty() {
        store_owned_games_cache(steam_id, result.clone());
    }

    result
}
