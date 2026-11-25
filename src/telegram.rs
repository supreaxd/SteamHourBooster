use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;
use serde_json::Value;
use crate::data::{get_global_settings, Account, AccountStatus};
use crate::logger;

// Default account selected via /switch command
static SELECTED_ACCOUNT: OnceLock<Mutex<String>> = OnceLock::new();

fn get_selected_account() -> String {
    SELECTED_ACCOUNT
        .get_or_init(|| Mutex::new(String::new()))
        .lock()
        .map(|g| g.clone())
        .unwrap_or_default()
}

fn set_selected_account(name: &str) {
    let cell = SELECTED_ACCOUNT.get_or_init(|| Mutex::new(String::new()));
    if let Ok(mut g) = cell.lock() {
        *g = name.to_string();
    }
}

static TELEGRAM_RUNNING: AtomicBool = AtomicBool::new(false);
static RUNTIME_ACCOUNTS_REF: OnceLock<Arc<Mutex<Vec<Arc<Mutex<Account>>>>>> = OnceLock::new();

pub fn register_accounts_for_bot(accounts: Vec<Arc<Mutex<Account>>>) {
    let container = RUNTIME_ACCOUNTS_REF.get_or_init(|| Arc::new(Mutex::new(Vec::new())));
    if let Ok(mut lock) = container.lock() {
        *lock = accounts;
    }
}

pub fn update_accounts_for_bot(accounts: &[Arc<Mutex<Account>>]) {
    let container = RUNTIME_ACCOUNTS_REF.get_or_init(|| Arc::new(Mutex::new(Vec::new())));
    if let Ok(mut lock) = container.lock() {
        *lock = accounts.to_vec();
    }
}

pub fn get_runtime_accounts() -> Vec<Arc<Mutex<Account>>> {
    let container = RUNTIME_ACCOUNTS_REF.get_or_init(|| Arc::new(Mutex::new(Vec::new())));
    if let Ok(lock) = container.lock() {
        lock.clone()
    } else {
        Vec::new()
    }
}

pub fn init_telegram_bot() {
    let settings = get_global_settings();
    if !settings.telegram_enabled || settings.telegram_bot_token.trim().is_empty() {
        return;
    }

    if TELEGRAM_RUNNING.swap(true, Ordering::SeqCst) {
        return; // Already running
    }

    let token = settings.telegram_bot_token.trim().to_string();
    let default_chat_id = settings.telegram_chat_id.trim().to_string();

    crate::steam::get_runtime().spawn(async move {
        logger::log_info("🤖 Starting Telegram Bot service...");
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(40))
            .build()
            .unwrap_or_default();

        // Check bot identity
        let me_url = format!("https://api.telegram.org/bot{}/getMe", token);
        if let Ok(resp) = client.get(&me_url).send().await {
            if let Ok(json) = resp.json::<Value>().await {
                if let Some(bot_username) = json.get("result")
                    .and_then(|r| r.get("username"))
                    .and_then(|u| u.as_str())
                {
                    logger::log_success(&format!("🤖 Telegram Bot connected: @{}", bot_username));
                } else {
                    logger::log_error("🤖 Invalid Telegram Bot Token!");
                    TELEGRAM_RUNNING.store(false, Ordering::SeqCst);
                    return;
                }
            }
        }

        let mut offset: i64 = 0;

        while TELEGRAM_RUNNING.load(Ordering::Relaxed) {
            let current_settings = get_global_settings();
            if !current_settings.telegram_enabled {
                break;
            }

            let updates_url = format!(
                "https://api.telegram.org/bot{}/getUpdates?offset={}&timeout=25",
                token, offset
            );

            match client.get(&updates_url).send().await {
                Ok(resp) => {
                    if let Ok(json) = resp.json::<Value>().await {
                        if let Some(result) = json.get("result").and_then(|r| r.as_array()) {
                            for update in result {
                                if let Some(update_id) = update.get("update_id").and_then(|id| id.as_i64()) {
                                    offset = update_id + 1;
                                }

                                if let Some(msg) = update.get("message") {
                                    let chat_id = msg.get("chat")
                                        .and_then(|c| c.get("id"))
                                        .and_then(|id| id.as_i64())
                                        .map(|id| id.to_string())
                                        .unwrap_or_default();

                                    let text = msg.get("text").and_then(|t| t.as_str()).unwrap_or("").trim();

                                    // Chat id filter: silently ignore unauthorized users
                                    if !default_chat_id.is_empty() && chat_id != default_chat_id {
                                        logger::log_warn(&format!("🤖 Unauthorized Telegram user ignored: {}", chat_id));
                                        continue;
                                    }

                                    if !text.is_empty() {
                                        if text.to_lowercase().starts_with("/qr") {
                                            let parts: Vec<&str> = text.split_whitespace().collect();
                                            let target = parts.get(1).copied().map(|s| s.to_string()).unwrap_or_else(|| {
                                                let sel = get_selected_account();
                                                if !sel.is_empty() {
                                                    sel
                                                } else {
                                                    get_runtime_accounts()
                                                        .first()
                                                        .and_then(|a| a.lock().ok().map(|g| g.username.clone()))
                                                        .unwrap_or_default()
                                                }
                                            });
                                            if let Some(qr_url) = crate::qr_auth::get_current_challenge_url() {
                                                if !qr_url.is_empty() && send_qr_photo(&client, &token, &chat_id, &target, &qr_url).await {
                                                    continue;
                                                }
                                            }
                                        }
                                        let (reply, _web_app_opt, keyboard_opt) = handle_bot_command(text).await;
                                        send_telegram_reply_full(&client, &token, &chat_id, &reply, keyboard_opt).await;
                                    }
                                } else if let Some(cb) = update.get("callback_query") {
                                    let cb_id = cb.get("id").and_then(|v| v.as_str()).unwrap_or("");
                                    let chat_id = cb.get("message")
                                        .and_then(|m| m.get("chat"))
                                        .and_then(|c| c.get("id"))
                                        .and_then(|id| id.as_i64())
                                        .map(|id| id.to_string())
                                        .unwrap_or_default();
                                    let data = cb.get("data").and_then(|d| d.as_str()).unwrap_or("");

                                    if !default_chat_id.is_empty() && chat_id != default_chat_id {
                                        continue;
                                    }

                                    let answer_url = format!("https://api.telegram.org/bot{}/answerCallbackQuery", token);
                                    let _ = client.post(&answer_url).json(&serde_json::json!({ "callback_query_id": cb_id })).send().await;

                                    let reply_text = handle_callback_query(data).await;
                                    if !reply_text.is_empty() {
                                        send_telegram_reply_full(&client, &token, &chat_id, &reply_text, None).await;
                                    }
                                }
                            }
                        }
                    }
                }
                Err(_) => {
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
        }

        TELEGRAM_RUNNING.store(false, Ordering::SeqCst);
        logger::log_info("🤖 Telegram Bot service stopped.");
    });
}

// ==========================================
// Remote Management API helpers
// ==========================================

pub fn start_all_remote() {
    let accounts = get_runtime_accounts();
    for arc in accounts {
        let (_id, status) = {
            let a = arc.lock().unwrap();
            (a.id, a.status.clone())
        };
        if status == AccountStatus::Idle {
            crate::steam::start_boost(arc.clone());
        }
    }
}

pub fn stop_all_remote() {
    let accounts = get_runtime_accounts();
    for arc in accounts {
        let id = arc.lock().unwrap().id;
        crate::steam::stop_boost(id, arc.clone());
    }
}

pub fn restart_all_remote() {
    let accounts = get_runtime_accounts();
    for arc in accounts {
        let id = arc.lock().unwrap().id;
        crate::steam::restart_boost(id, arc.clone());
    }
}

pub fn start_account_remote(target: &str) -> Result<String, String> {
    let accounts = get_runtime_accounts();
    let target_l = target.to_lowercase();
    for arc in accounts {
        let (id, user) = {
            let a = arc.lock().unwrap();
            (a.id, a.username.to_lowercase())
        };
        if user == target_l || id.to_string() == target_l {
            crate::steam::start_boost(arc.clone());
            return Ok(format!("▶️ `{}` started.", target));
        }
    }
    Err(format!("❌ Account `{}` not found.", target))
}

pub fn stop_account_remote(target: &str) -> Result<String, String> {
    let accounts = get_runtime_accounts();
    let target_l = target.to_lowercase();
    for arc in accounts {
        let (id, user) = {
            let a = arc.lock().unwrap();
            (a.id, a.username.to_lowercase())
        };
        if user == target_l || id.to_string() == target_l {
            crate::steam::stop_boost(id, arc.clone());
            return Ok(format!("⏹️ `{}` stopped.", target));
        }
    }
    Err(format!("❌ Account `{}` not found.", target))
}

pub fn restart_account_remote(target: &str) -> Result<String, String> {
    let accounts = get_runtime_accounts();
    let target_l = target.to_lowercase();
    for arc in accounts {
        let (id, user) = {
            let a = arc.lock().unwrap();
            (a.id, a.username.to_lowercase())
        };
        if user == target_l || id.to_string() == target_l {
            crate::steam::restart_boost(id, arc.clone());
            return Ok(format!("🔄 `{}` restarting...", target));
        }
    }
    Err(format!("❌ Account `{}` not found.", target))
}

pub fn add_account_remote(
    user: &str,
    pass: &str,
    custom_title: Option<&str>,
    shared_secret: Option<&str>,
) -> Result<u64, String> {
    if user.trim().is_empty() {
        return Err("Username cannot be empty".to_string());
    }

    let mut config = crate::data::AppConfig::load();
    let id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let mut new_acc = Account::new(id);
    new_acc.username = user.trim().to_string();
    new_acc.password = pass.trim().to_string();
    if let Some(title) = custom_title {
        if !title.trim().is_empty() {
            new_acc.custom_title = title.trim().to_string();
        }
    }
    if let Some(sec) = shared_secret {
        if !sec.trim().is_empty() {
            new_acc.shared_secret = Some(sec.trim().to_string());
        }
    }

    // Default games
    new_acc.games = vec![
        crate::data::GameEntry::new(730, "Counter-Strike 2"),
        crate::data::GameEntry::new(440, "Team Fortress 2"),
        crate::data::GameEntry::new(570, "Dota 2"),
    ];

    config.accounts.push(new_acc.clone());
    config.save();

    let container = RUNTIME_ACCOUNTS_REF.get_or_init(|| Arc::new(Mutex::new(Vec::new())));
    if let Ok(mut lock) = container.lock() {
        lock.push(Arc::new(Mutex::new(new_acc)));
    }

    logger::log_success(&format!("👤 Remote added account: {} (ID: {})", user, id));
    Ok(id)
}

pub fn delete_account_remote(target: &str) -> Result<String, String> {
    let mut config = crate::data::AppConfig::load();
    let target_l = target.to_lowercase();
    let mut removed_id = None;
    let mut removed_name = String::new();

    config.accounts.retain(|a| {
        if a.username.to_lowercase() == target_l || a.id.to_string() == target_l {
            removed_id = Some(a.id);
            removed_name = a.username.clone();
            false
        } else {
            true
        }
    });

    if let Some(id) = removed_id {
        config.save();
        let container = RUNTIME_ACCOUNTS_REF.get_or_init(|| Arc::new(Mutex::new(Vec::new())));
        if let Ok(mut lock) = container.lock() {
            for arc in lock.iter() {
                if let Ok(a) = arc.lock() {
                    if a.id == id {
                        crate::steam::stop_boost(id, arc.clone());
                    }
                }
            }
            lock.retain(|arc| {
                if let Ok(a) = arc.lock() {
                    a.id != id
                } else {
                    true
                }
            });
        }
        logger::log_info(&format!("🗑️ Remote deleted account: {} (ID: {})", removed_name, id));
        Ok(format!("🗑️ `{}` account was successfully deleted.", removed_name))
    } else {
        Err(format!("❌ Account `{}` not found.", target))
    }
}

pub fn add_game_remote(target: &str, app_id: u32, name: &str) -> Result<String, String> {
    let target_l = target.to_lowercase();
    let mut config = crate::data::AppConfig::load();
    let mut found = false;

    let game_name = if name.trim().is_empty() {
        match crate::data::POPULAR_GAMES.iter().find(|(_, id)| *id == app_id) {
            Some((p_name, _)) => p_name.to_string(),
            None => format!("Steam App {}", app_id),
        }
    } else {
        name.to_string()
    };

    for a in &mut config.accounts {
        if a.username.to_lowercase() == target_l || a.id.to_string() == target_l {
            if a.games.iter().any(|g| g.app_id == app_id) {
                return Err(format!("⚠️ Game already in list: `{}` ({})", game_name, app_id));
            }
            a.games.push(crate::data::GameEntry::new(app_id, &game_name));
            found = true;
            break;
        }
    }

    if found {
        config.save();
        let container = RUNTIME_ACCOUNTS_REF.get_or_init(|| Arc::new(Mutex::new(Vec::new())));
        if let Ok(lock) = container.lock() {
            for arc in lock.iter() {
                if let Ok(mut a) = arc.lock() {
                    if a.username.to_lowercase() == target_l || a.id.to_string() == target_l {
                        if !a.games.iter().any(|g| g.app_id == app_id) {
                            a.games.push(crate::data::GameEntry::new(app_id, &game_name));
                        }
                    }
                }
            }
        }
        Ok(format!("🎮 Added to `{}`: `{}` ({})", target, game_name, app_id))
    } else {
        Err(format!("❌ Account `{}` not found.", target))
    }
}

pub fn remove_game_remote(target: &str, app_id: u32) -> Result<String, String> {
    let target_l = target.to_lowercase();
    let mut config = crate::data::AppConfig::load();
    let mut found = false;
    let mut removed_title = String::new();

    for a in &mut config.accounts {
        if a.username.to_lowercase() == target_l || a.id.to_string() == target_l {
            if let Some(pos) = a.games.iter().position(|g| g.app_id == app_id) {
                removed_title = a.games[pos].name.clone();
                a.games.remove(pos);
                found = true;
            }
            break;
        }
    }

    if found {
        config.save();
        let container = RUNTIME_ACCOUNTS_REF.get_or_init(|| Arc::new(Mutex::new(Vec::new())));
        if let Ok(lock) = container.lock() {
            for arc in lock.iter() {
                if let Ok(mut a) = arc.lock() {
                    if a.username.to_lowercase() == target_l || a.id.to_string() == target_l {
                        a.games.retain(|g| g.app_id != app_id);
                    }
                }
            }
        }
        Ok(format!("🗑️ Removed from `{}`: `{}` ({})", target, removed_title, app_id))
    } else {
        Err(format!("❌ Game or account not found."))
    }
}

pub fn submit_guard_remote(target: &str, code: &str) -> Result<String, String> {
    let target_l = target.to_lowercase();
    let container = RUNTIME_ACCOUNTS_REF.get_or_init(|| Arc::new(Mutex::new(Vec::new())));
    if let Ok(lock) = container.lock() {
        for arc in lock.iter() {
            let (id, user) = {
                let a = arc.lock().unwrap();
                (a.id, a.username.to_lowercase())
            };
            if user == target_l || id.to_string() == target_l {
                crate::steam::submit_guard_code(id, code);
                return Ok(format!("🛡️ Steam Guard code submitted for `{}`.", target));
            }
        }
    }
    Err(format!("❌ Account `{}` not found.", target))
}

pub async fn search_steam_games(query: &str) -> Vec<Value> {
    let q = query.trim().to_lowercase();
    let mut results: Vec<Value> = Vec::new();
    let mut seen_ids = std::collections::HashSet::new();

    // 1. Check local popular games
    for (name, id) in crate::data::POPULAR_GAMES {
        if name.to_lowercase().contains(&q) {
            seen_ids.insert(*id);
            results.push(serde_json::json!({
                "app_id": id,
                "name": name,
                "tiny_image": format!("https://shared.akamai.steamstatic.com/store_item_assets/steam/apps/{}/capsule_231x87.jpg", id)
            }));
        }
    }

    // 2. Query Steam Store Search API
    if !query.trim().is_empty() {
        let encoded = crate::data::urlencoding_encode(query);
        let url = format!("https://store.steampowered.com/api/storesearch/?term={}&l=turkish&cc=TR", encoded);
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap_or_default();

        if let Ok(resp) = client.get(&url).send().await {
            if let Ok(json) = resp.json::<Value>().await {
                if let Some(items) = json.get("items").and_then(|i| i.as_array()) {
                    for item in items.iter().take(12) {
                        if let (Some(id_u64), Some(name_str)) = (
                            item.get("id").and_then(|v| v.as_u64()),
                            item.get("name").and_then(|v| v.as_str()),
                        ) {
                            let app_id = id_u64 as u32;
                            if !seen_ids.contains(&app_id) {
                                seen_ids.insert(app_id);
                                let tiny_img = item.get("tiny_image")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("");
                                let img = if tiny_img.is_empty() {
                                    format!("https://shared.akamai.steamstatic.com/store_item_assets/steam/apps/{}/capsule_231x87.jpg", app_id)
                                } else {
                                    tiny_img.to_string()
                                };

                                results.push(serde_json::json!({
                                    "app_id": app_id,
                                    "name": name_str,
                                    "tiny_image": img
                                }));
                            }
                        }
                    }
                }
            }
        }
    }

    results
}

// ==========================================
// Bot Command Dispatcher
// ==========================================

// Returns: (message_text, unused_web_url, optional_inline_keyboard_json)
async fn handle_bot_command(command: &str) -> (String, Option<String>, Option<Value>) {
    let parts: Vec<&str> = command.split_whitespace().collect();
    if parts.is_empty() {
        return (String::new(), None, None);
    }

    let cmd = parts[0].to_lowercase();
    // Strip /cmd@BotName format
    let cmd = cmd.split('@').next().unwrap_or(&cmd).to_string();
    let accounts_arc = RUNTIME_ACCOUNTS_REF.get_or_init(|| Arc::new(Mutex::new(Vec::new()))).clone();

    // Helper: resolve account name — if none given, fall back to /switch selection or first account
    let resolve_account = |name_opt: Option<&str>| -> Option<String> {
        if let Some(n) = name_opt {
            return Some(n.to_string());
        }
        let sel = get_selected_account();
        if !sel.is_empty() {
            return Some(sel);
        }
        // first account
        if let Ok(lock) = accounts_arc.lock() {
            if let Some(first) = lock.first() {
                if let Ok(a) = first.lock() {
                    return Some(a.username.clone());
                }
            }
        }
        None
    };

    match cmd.as_str() {
        // ─────────────────────────────────────────────────────────────
        // /start  — welcome + command summary
        // ─────────────────────────────────────────────────────────────
        "/start" => {
            let msg = "🤖 *SupBooster Bot*\n\
EN: Welcome! Use /help to see all commands.\n\
TR: Hoş geldin! Tüm komutları görmek için /help yaz.\n\
RU: Добро пожаловать! Введи /help для списка команд.".to_string();
            (msg, None, None)
        }

        // ─────────────────────────────────────────────────────────────
        // /help  — full trilingual command list
        // ─────────────────────────────────────────────────────────────
        "/help" => {
            let msg = "🤖 *SupBooster — Command List / Komut Listesi / Список команд*\n\n\
📌 *Basic / Temel / Основные*\n\
`/start` — EN: Start bot | TR: Botu başlat | RU: Запуск бота\n\
`/help` — EN: Show all commands | TR: Tüm komutları göster | RU: Список команд\n\
`/status` — EN: Account status | TR: Hesap durumu | RU: Статус аккаунтов\n\n\
🚀 *Boost Control / Boost Kontrolü / Управление бустом*\n\
`/start_boost [hesap]` — EN: Start boosting | TR: Boost başlat | RU: Начать буст\n\
`/stop_boost [hesap]` — EN: Stop boosting | TR: Boost durdur | RU: Остановить буст\n\
`/restart [hesap]` — EN: Reconnect | TR: Yeniden bağlan | RU: Переподключить\n\n\
🃏 *Cards & Drops / Kartlar / Карточки*\n\
`/drops [hesap]` — EN: Games with card drops | TR: Kart düşen oyunlar | RU: Игры с дропами\n\
`/auto_drops <on|off>` — EN: Auto-switch on 0 drops | TR: Otomatik geçiş | RU: Авто-переключение\n\
`/drop_notify <on|off>` — EN: Card drop notifications | TR: Kart bildirimleri | RU: Уведомления о дропах\n\n\
🎮 *Game Management / Oyun Yönetimi / Игры*\n\
`/games [hesap]` — EN: List boost games | TR: Boost oyunlarını listele | RU: Список игр\n\
`/add_game <appid> [hesap]` — EN: Add game | TR: Oyun ekle | RU: Добавить игру\n\
`/remove_game <appid> [hesap]` — EN: Remove game | TR: Oyun kaldır | RU: Убрать игру\n\n\
👤 *Account Management / Hesap Yönetimi / Аккаунты*\n\
`/accounts` — EN: List all accounts | TR: Tüm hesapları listele | RU: Все аккаунты\n\
`/switch <hesap>` — EN: Set default account | TR: Varsayılan hesabı seç | RU: Выбрать аккаунт\n\
`/qr [hesap]` — EN: Send QR code | TR: QR kod gönder | RU: Отправить QR-код\n".to_string();
            (msg, None, None)
        }

        // ─────────────────────────────────────────────────────────────
        // /status  — all accounts with inline buttons
        // ─────────────────────────────────────────────────────────────
        "/status" => {
            let accounts = {
                let lock = accounts_arc.lock().unwrap();
                lock.clone()
            };

            if accounts.is_empty() {
                return (
                    "⚠️ EN: No accounts added yet.\nTR: Henüz hesap eklenmemiş.\nRU: Аккаунты не добавлены.".to_string(),
                    None, None,
                );
            }

            let mut out = String::from("📋 *SupBooster — Account Status / Hesap Durumu / Статус*\n\n");
            let mut buttons: Vec<Vec<Value>> = Vec::new();

            for (idx, arc) in accounts.iter().enumerate() {
                let a = arc.lock().unwrap();
                let (icon, en_status, tr_status, ru_status) = match &a.status {
                    AccountStatus::Boosting           => ("🟢", "Boosting",      "Kasıyor",          "Работает"),
                    AccountStatus::Connecting         => ("🟡", "Connecting",    "Bağlanıyor",       "Подключение"),
                    AccountStatus::AwaitingSteamGuard { .. } => ("🛡️", "Guard Needed", "Kod Bekleniyor", "Нужен код"),
                    AccountStatus::PausedForGame { .. }     => ("⏸️", "In-Game",      "Oyunda",          "В игре"),
                    AccountStatus::Reconnecting { .. }      => ("🔄", "Reconnecting", "Yeniden Bağlanıyor", "Переподключение"),
                    AccountStatus::Error(_)           => ("🔴", "Error",         "Hata",             "Ошибка"),
                    AccountStatus::Idle               => ("⚪", "Stopped",       "Durduruldu",       "Остановлен"),
                };

                let card_count = crate::inventory::get_cached_active_drops(a.steam_id).len();

                out.push_str(&format!(
                    "{}. {} *{}*\n   EN: {} | TR: {} | RU: {}\n   ⏱ `{:.1}h` | 🎮 `{}` games | 🃏 `{}` card drops\n\n",
                    idx + 1, icon, a.username,
                    en_status, tr_status, ru_status,
                    a.hours_boosted, a.games.len(), card_count
                ));

                // Inline row: Stop / Start button per account
                let row = vec![
                    serde_json::json!({
                        "text": format!("⏹ Stop {}", a.username),
                        "callback_data": format!("stop_boost:{}", a.username)
                    }),
                    serde_json::json!({
                        "text": format!("▶️ Start {}", a.username),
                        "callback_data": format!("start_boost:{}", a.username)
                    }),
                ];
                buttons.push(row);
            }

            let keyboard = serde_json::json!({ "inline_keyboard": buttons });
            (out, None, Some(keyboard))
        }

        // ─────────────────────────────────────────────────────────────
        // /start_boost [account]
        // ─────────────────────────────────────────────────────────────
        "/start_boost" => {
            if parts.len() > 1 {
                let target = parts[1];
                let result = start_account_remote(target);
                let msg = match result {
                    Ok(_) => format!("▶️ EN: Boost started for `{}`\nTR: `{}` için boost başlatıldı\nRU: Буст запущен для `{}`", target, target, target),
                    Err(e) => e,
                };
                (msg, None, None)
            } else {
                start_all_remote();
                ("▶️ EN: Boost started for all accounts.\nTR: Tüm hesaplar için boost başlatıldı.\nRU: Буст запущен для всех аккаунтов.".to_string(), None, None)
            }
        }

        // ─────────────────────────────────────────────────────────────
        // /stop_boost [account]
        // ─────────────────────────────────────────────────────────────
        "/stop_boost" => {
            if parts.len() > 1 {
                let target = parts[1];
                let result = stop_account_remote(target);
                let msg = match result {
                    Ok(_) => format!("⏹️ EN: Boost stopped for `{}`\nTR: `{}` için boost durduruldu\nRU: Буст остановлен для `{}`", target, target, target),
                    Err(e) => e,
                };
                (msg, None, None)
            } else {
                stop_all_remote();
                ("⏹️ EN: Boost stopped for all accounts.\nTR: Tüm hesaplarda boost durduruldu.\nRU: Буст остановлен для всех аккаунтов.".to_string(), None, None)
            }
        }

        // ─────────────────────────────────────────────────────────────
        // /restart [account]
        // ─────────────────────────────────────────────────────────────
        "/restart" => {
            if parts.len() > 1 {
                let target = parts[1];
                let result = restart_account_remote(target);
                let msg = match result {
                    Ok(_) => format!("🔄 EN: Restarting `{}`...\nTR: `{}` yeniden başlatılıyor...\nRU: Перезапуск `{}`...", target, target, target),
                    Err(e) => e,
                };
                (msg, None, None)
            } else {
                restart_all_remote();
                ("🔄 EN: Restarting all accounts...\nTR: Tüm hesaplar yeniden başlatılıyor...\nRU: Перезапуск всех аккаунтов...".to_string(), None, None)
            }
        }

        // ─────────────────────────────────────────────────────────────
        // /drops [account]  — games with card drops remaining
        // ─────────────────────────────────────────────────────────────
        "/drops" => {
            let target_opt = resolve_account(parts.get(1).copied());
            match target_opt {
                None => (
                    "❌ EN: No account specified or selected.\nTR: Hesap belirtilmedi.\nRU: Аккаунт не указан.".to_string(),
                    None, None
                ),
                Some(target) => {
                    // Find steam_id for this account
                    let target_l = target.to_lowercase();
                    let steam_id = {
                        let lock = accounts_arc.lock().unwrap();
                        lock.iter().find_map(|arc| {
                            let a = arc.lock().unwrap();
                            if a.username.to_lowercase() == target_l || a.id.to_string() == target_l {
                                Some(a.steam_id)
                            } else {
                                None
                            }
                        })
                    };

                    match steam_id {
                        None => (format!("❌ Account `{}` not found.", target), None, None),
                        Some(sid) => {
                            let drops = crate::inventory::get_cached_active_drops(sid);
                            if drops.is_empty() {
                                let msg = format!(
                                    "🃏 EN: No card drops remaining for `{}`.\nTR: `{}` için kart düşürme hakkı yok.\nRU: Дропов карт нет для `{}`.",
                                    target, target, target
                                );
                                (msg, None, None)
                            } else {
                                let mut msg = format!(
                                    "🃏 *{}* — EN: Active Card Drops | TR: Aktif Kart Hakları | RU: Активные дропы\n\n",
                                    target
                                );
                                let mut buttons: Vec<Vec<Value>> = Vec::new();
                                for d in &drops {
                                    msg.push_str(&format!(
                                        "• *{}* (AppID: `{}`)\n  🃏 `{}` cards remaining\n",
                                        d.name, d.app_id, d.remaining_cards
                                    ));
                                    buttons.push(vec![serde_json::json!({
                                        "text": format!("🎮 Boost {}", d.name),
                                        "callback_data": format!("add_game:{}:{}", target, d.app_id)
                                    })]);
                                }
                                let keyboard = serde_json::json!({ "inline_keyboard": buttons });
                                (msg, None, Some(keyboard))
                            }
                        }
                    }
                }
            }
        }

        // ─────────────────────────────────────────────────────────────
        // /games [account]
        // ─────────────────────────────────────────────────────────────
        "/games" => {
            let target_opt = resolve_account(parts.get(1).copied());
            match target_opt {
                None => (
                    "❌ EN: No account specified.\nTR: Hesap belirtilmedi.\nRU: Аккаунт не указан.".to_string(),
                    None, None
                ),
                Some(target) => {
                    let target_l = target.to_lowercase();
                    let accounts = get_runtime_accounts();
                    let mut found = false;
                    let mut out = String::new();
                    for arc in accounts {
                        let a = arc.lock().unwrap();
                        if a.username.to_lowercase() == target_l || a.id.to_string() == target_l {
                            found = true;
                            if a.games.is_empty() {
                                out = format!(
                                    "🎮 EN: `{}` has no games.\nTR: `{}` için oyun yok.\nRU: У `{}` нет игр.\nTo add: `/add_game <appid> {}`",
                                    a.username, a.username, a.username, a.username
                                );
                            } else {
                                out = format!(
                                    "🎮 *{}* — EN: Boost Games ({}) | TR: Boost Oyunları ({}) | RU: Игры для буста ({})\n\n",
                                    a.username, a.games.len(), a.games.len(), a.games.len()
                                );
                                for (i, g) in a.games.iter().enumerate() {
                                    out.push_str(&format!("{}. *{}* — AppID: `{}`\n", i + 1, g.name, g.app_id));
                                }
                            }
                            break;
                        }
                    }
                    if !found {
                        out = format!("❌ Account `{}` not found.", target);
                    }
                    (out, None, None)
                }
            }
        }

        // ─────────────────────────────────────────────────────────────
        // /add_game <appid> [account]
        // ─────────────────────────────────────────────────────────────
        "/add_game" => {
            if parts.len() < 2 {
                return ("❌ Usage: `/add_game <appid> [account]`".to_string(), None, None);
            }
            if let Ok(app_id) = parts[1].parse::<u32>() {
                let target_opt = resolve_account(parts.get(2).copied());
                match target_opt {
                    None => ("❌ EN: No account found.\nTR: Hesap bulunamadı.\nRU: Аккаунт не найден.".to_string(), None, None),
                    Some(target) => {
                        match add_game_remote(&target, app_id, "") {
                            Ok(msg) => (format!("{}\nTR: Oyun eklendi.\nRU: Игра добавлена.", msg), None, None),
                            Err(e) => (e, None, None),
                        }
                    }
                }
            } else {
                ("❌ Invalid AppID. Usage: `/add_game <appid> [account]`".to_string(), None, None)
            }
        }

        // ─────────────────────────────────────────────────────────────
        // /remove_game <appid> [account]
        // ─────────────────────────────────────────────────────────────
        "/remove_game" => {
            if parts.len() < 2 {
                return ("❌ Usage: `/remove_game <appid> [account]`".to_string(), None, None);
            }
            if let Ok(app_id) = parts[1].parse::<u32>() {
                let target_opt = resolve_account(parts.get(2).copied());
                match target_opt {
                    None => ("❌ EN: No account found.\nTR: Hesap bulunamadı.\nRU: Аккаунт не найден.".to_string(), None, None),
                    Some(target) => {
                        match remove_game_remote(&target, app_id) {
                            Ok(msg) => (format!("{}\nTR: Oyun kaldırıldı.\nRU: Игра убрана.", msg), None, None),
                            Err(e) => (e, None, None),
                        }
                    }
                }
            } else {
                ("❌ Invalid AppID. Usage: `/remove_game <appid> [account]`".to_string(), None, None)
            }
        }

        // ─────────────────────────────────────────────────────────────
        // /auto_drops <on|off>
        // ─────────────────────────────────────────────────────────────
        "/auto_drops" => {
            let arg = parts.get(1).map(|s| s.to_lowercase());
            match arg.as_deref() {
                Some("on") => {
                    let mut config = crate::data::AppConfig::load();
                    config.settings.auto_switch_drops = true;
                    config.save();
                    crate::data::update_global_settings(&config.settings);
                    ("✅ EN: Auto-drops ON\nTR: Otomatik kart geçişi AÇIK\nRU: Авто-переключение ВКЛЮЧЕНО".to_string(), None, None)
                }
                Some("off") => {
                    let mut config = crate::data::AppConfig::load();
                    config.settings.auto_switch_drops = false;
                    config.save();
                    crate::data::update_global_settings(&config.settings);
                    ("🔕 EN: Auto-drops OFF\nTR: Otomatik kart geçişi KAPALI\nRU: Авто-переключение ВЫКЛЮЧЕНО".to_string(), None, None)
                }
                _ => {
                    let current = get_global_settings().auto_switch_drops;
                    let state = if current { "ON / AÇIK / ВКЛЮЧЕНО" } else { "OFF / KAPALI / ВЫКЛЮЧЕНО" };
                    (format!("⚙️ auto_drops is currently: *{}*\nUsage: `/auto_drops on` or `/auto_drops off`", state), None, None)
                }
            }
        }

        // ─────────────────────────────────────────────────────────────
        // /accounts  — list all registered accounts
        // ─────────────────────────────────────────────────────────────
        "/accounts" => {
            let accounts = {
                let lock = accounts_arc.lock().unwrap();
                lock.clone()
            };
            if accounts.is_empty() {
                return (
                    "⚠️ EN: No accounts added.\nTR: Hiç hesap eklenmemiş.\nRU: Аккаунты не добавлены.".to_string(),
                    None, None,
                );
            }
            let mut out = String::from("👤 *SupBooster Accounts / Hesaplar / Аккаунты*\n\n");
            for (i, arc) in accounts.iter().enumerate() {
                let a = arc.lock().unwrap();
                let icon = match &a.status {
                    AccountStatus::Boosting  => "🟢",
                    AccountStatus::Idle      => "⚪",
                    AccountStatus::Error(_)  => "🔴",
                    _                        => "🟡",
                };
                out.push_str(&format!(
                    "{}. {} *{}* (ID: `{}`)\n",
                    i + 1, icon, a.username, a.id
                ));
            }
            out.push_str("\nUse `/switch <account>` to set default.");
            (out, None, None)
        }

        // ─────────────────────────────────────────────────────────────
        // /switch <account>
        // ─────────────────────────────────────────────────────────────
        "/switch" => {
            if parts.len() < 2 {
                let current = get_selected_account();
                let display = if current.is_empty() { "none".to_string() } else { current };
                return (format!("⚙️ EN: Current default account: `{}`\nUsage: `/switch <account>`", display), None, None);
            }
            let target = parts[1];
            // Verify account exists
            let accounts = get_runtime_accounts();
            let target_l = target.to_lowercase();
            let found = accounts.iter().any(|arc| {
                let a = arc.lock().unwrap();
                a.username.to_lowercase() == target_l || a.id.to_string() == target_l
            });
            if found {
                set_selected_account(target);
                (format!(
                    "✅ EN: Default account set to `{}`\nTR: Varsayılan hesap `{}` olarak ayarlandı\nRU: Аккаунт по умолчанию: `{}`",
                    target, target, target
                ), None, None)
            } else {
                (format!("❌ Account `{}` not found.", target), None, None)
            }
        }

        // ─────────────────────────────────────────────────────────────
        // /drop_notify <on|off>
        // ─────────────────────────────────────────────────────────────
        "/drop_notify" => {
            let arg = parts.get(1).map(|s| s.to_lowercase());
            match arg.as_deref() {
                Some("on") => {
                    let mut config = crate::data::AppConfig::load();
                    config.settings.telegram_notify_cards = true;
                    config.save();
                    crate::data::update_global_settings(&config.settings);
                    ("🔔 EN: Card drop notifications ON\nTR: Kart düşme bildirimleri AÇIK\nRU: Уведомления о дропах ВКЛЮЧЕНЫ".to_string(), None, None)
                }
                Some("off") => {
                    let mut config = crate::data::AppConfig::load();
                    config.settings.telegram_notify_cards = false;
                    config.save();
                    crate::data::update_global_settings(&config.settings);
                    ("🔕 EN: Card drop notifications OFF\nTR: Kart düşme bildirimleri KAPALI\nRU: Уведомления о дропах ВЫКЛЮЧЕНЫ".to_string(), None, None)
                }
                _ => {
                    let current = get_global_settings().telegram_notify_cards;
                    let state = if current { "ON / AÇIK / ВКЛЮЧЕНЫ" } else { "OFF / KAPALI / ВЫКЛЮЧЕНЫ" };
                    (format!("⚙️ drop_notify is currently: *{}*\nUsage: `/drop_notify on` or `/drop_notify off`", state), None, None)
                }
            }
        }

        // ─────────────────────────────────────────────────────────────
        // /qr [account]  — send QR code for login
        // ─────────────────────────────────────────────────────────────
        "/qr" => {
            let target_opt = resolve_account(parts.get(1).copied());
            match target_opt {
                None => (
                    "❌ EN: No account specified.\nTR: Hesap belirtilmedi.\nRU: Аккаунт не указан.".to_string(),
                    None, None
                ),
                Some(target) => {
                    // Try to get QR challenge URL from qr_auth module
                    let qr_url = crate::qr_auth::get_current_challenge_url();

                    match qr_url {
                        Some(url) if !url.is_empty() => {
                            let msg = format!(
                                "📱 QR Code for *{}*\n\
EN: Scan with Steam mobile app to log in.\n\
TR: Giriş yapmak için Steam mobil uygulamasıyla tara.\n\
RU: Отсканируй в приложении Steam для входа.\n\n\
🔗 `{}`",
                                target, url
                            );
                            (msg, None, None)
                        }
                        _ => {
                            let msg = format!(
                                "⚠️ EN: No active QR code for `{}`. Account may already be logged in.\n\
TR: `{}` için aktif QR kod yok. Hesap zaten giriş yapmış olabilir.\n\
RU: Нет активного QR-кода для `{}`. Аккаунт уже может быть авторизован.",
                                target, target, target
                            );
                            (msg, None, None)
                        }
                    }
                }
            }
        }

        // ─────────────────────────────────────────────────────────────
        // Legacy / convenience aliases
        // ─────────────────────────────────────────────────────────────
        "/start_all" => {
            start_all_remote();
            ("▶️ EN: Boost started for all accounts.\nTR: Tüm hesaplar için boost başlatıldı.\nRU: Буст запущен для всех аккаунтов.".to_string(), None, None)
        }
        "/stop_all" => {
            stop_all_remote();
            ("⏹️ EN: Boost stopped for all accounts.\nTR: Tüm hesaplarda boost durduruldu.\nRU: Буст остановлен для всех аккаунтов.".to_string(), None, None)
        }
        "/restart_all" => {
            restart_all_remote();
            ("🔄 EN: Restarting all accounts...\nTR: Tüm hesaplar yeniden başlatılıyor...\nRU: Перезапуск всех аккаунтов...".to_string(), None, None)
        }

        "/addgame" if parts.len() >= 3 => {
            let target = parts[1];
            if let Ok(app_id) = parts[2].parse::<u32>() {
                match add_game_remote(target, app_id, "") {
                    Ok(msg) => (msg, None, None),
                    Err(e) => (e, None, None),
                }
            } else {
                ("❌ Invalid AppID. Example: `/addgame user1 730`".to_string(), None, None)
            }
        }
        "/removegame" if parts.len() >= 3 => {
            let target = parts[1];
            if let Ok(app_id) = parts[2].parse::<u32>() {
                match remove_game_remote(target, app_id) {
                    Ok(msg) => (msg, None, None),
                    Err(e) => (e, None, None),
                }
            } else {
                ("❌ Invalid AppID. Example: `/removegame user1 730`".to_string(), None, None)
            }
        }
        "/addaccount" if parts.len() >= 3 => {
            let user = parts[1];
            let pass = parts[2];
            match add_account_remote(user, pass, None, None) {
                Ok(id) => (format!("✅ `{}` added! (ID: `{}`)\nTo start: `/start_boost {}`", user, id, user), None, None),
                Err(e) => (format!("❌ Failed: {}", e), None, None),
            }
        }
        "/delaccount" if parts.len() > 1 => {
            match delete_account_remote(parts[1]) {
                Ok(msg) => (msg, None, None),
                Err(e) => (e, None, None),
            }
        }
        "/guard" if parts.len() >= 3 => {
            match submit_guard_remote(parts[1], parts[2]) {
                Ok(msg) => (msg, None, None),
                Err(e) => (e, None, None),
            }
        }
        "/cards" => {
            let cards = crate::inventory::get_recent_card_drops();
            if cards.is_empty() {
                return ("🃏 EN: No dropped cards recorded yet.\nTR: Henüz düşen kart yok.\nRU: Дропов пока нет.".to_string(), None, None);
            }
            let mut msg = format!("🃏 *Recently Dropped Cards ({})*\n\n", cards.len());
            for c in cards.iter().rev().take(10) {
                let price_str = if c.market_price.is_empty() { "-" } else { &c.market_price };
                msg.push_str(&format!("• *{}* — `{}`\n  └ Card: *{}* ({})\n", c.game_name, c.username, c.card_name, price_str));
            }
            (msg, None, None)
        }
        "/search" if parts.len() > 1 => {
            let query = parts[1..].join(" ");
            let results = search_steam_games(&query).await;
            if results.is_empty() {
                return (format!("🔍 No results for `{}`.", query), None, None);
            }
            let mut reply = format!("🔍 *\"{}\"* Results:\n\n", query);
            for item in results.iter().take(8) {
                let id = item.get("app_id").and_then(|v| v.as_u64()).unwrap_or(0);
                let name = item.get("name").and_then(|v| v.as_str()).unwrap_or("Unknown");
                reply.push_str(&format!("• *{}* — `{}`\n  `/add_game {} <account>`\n", name, id, id));
            }
            (reply, None, None)
        }

        // ─────────────────────────────────────────────────────────────
        // Unknown command
        // ─────────────────────────────────────────────────────────────
        _ => {
            ("❓ EN: Unknown command. Type `/help`.\nTR: Bilinmeyen komut. `/help` yaz.\nRU: Неизвестная команда. Введи `/help`.".to_string(), None, None)
        }
    }
}

pub fn send_telegram_alert(message: &str) {
    let settings = get_global_settings();
    if !settings.telegram_enabled || settings.telegram_bot_token.trim().is_empty() || settings.telegram_chat_id.trim().is_empty() {
        return;
    }

    let token = settings.telegram_bot_token.trim().to_string();
    let chat_id = settings.telegram_chat_id.trim().to_string();
    let text = message.to_string();

    crate::steam::get_runtime().spawn(async move {
        let client = reqwest::Client::new();
        send_telegram_reply_full(&client, &token, &chat_id, &text, None).await;
    });
}

async fn handle_callback_query(data: &str) -> String {
    let parts: Vec<&str> = data.split(':').collect();
    match parts.as_slice() {
        ["stop_boost", target] => match stop_account_remote(target) {
            Ok(_) => format!(
                "⏹️ EN: Boost stopped for `{}`\nTR: `{}` için boost durduruldu\nRU: Буст остановлен для `{}`",
                target, target, target
            ),
            Err(e) => e,
        },
        ["start_boost", target] => match start_account_remote(target) {
            Ok(_) => format!(
                "▶️ EN: Boost started for `{}`\nTR: `{}` için boost başlatıldı\nRU: Буст запущен для `{}`",
                target, target, target
            ),
            Err(e) => e,
        },
        ["add_game", target, app_id_str] => {
            if let Ok(app_id) = app_id_str.parse::<u32>() {
                match add_game_remote(target, app_id, "") {
                    Ok(msg) => format!("{}\nTR: Oyun eklendi.\nRU: Игра добавлена.", msg),
                    Err(e) => e,
                }
            } else {
                "❌ Invalid AppID.".to_string()
            }
        }
        _ => String::new(),
    }
}

async fn send_qr_photo(
    client: &reqwest::Client,
    token: &str,
    chat_id: &str,
    target: &str,
    qr_url: &str,
) -> bool {
    let Ok(code) = qrcode::QrCode::new(qr_url.as_bytes()) else {
        return false;
    };
    let width = code.width();
    let scale: u32 = 8;
    let margin: u32 = 4;
    let img_size = (width as u32 + margin * 2) * scale;
    let mut img = image::RgbaImage::from_pixel(img_size, img_size, image::Rgba([255, 255, 255, 255]));
    let colors = code.into_colors();
    for (idx, c) in colors.iter().enumerate() {
        if *c == qrcode::Color::Dark {
            let x = (idx % width) as u32 + margin;
            let y = (idx / width) as u32 + margin;
            for dy in 0..scale {
                for dx in 0..scale {
                    img.put_pixel(x * scale + dx, y * scale + dy, image::Rgba([0, 0, 0, 255]));
                }
            }
        }
    }
    let mut png_bytes: Vec<u8> = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut png_bytes);
    if image::DynamicImage::ImageRgba8(img)
        .write_to(&mut cursor, image::ImageFormat::Png)
        .is_err()
    {
        return false;
    }

    let caption = format!(
        "📱 QR Code ({})\nEN: Scan with Steam mobile app.\nTR: Steam mobil uygulamasıyla tara.\nRU: Отсканируй в приложении Steam.",
        target
    );
    let part = reqwest::multipart::Part::bytes(png_bytes)
        .file_name("qr.png")
        .mime_str("image/png");
    let Ok(photo_part) = part else {
        return false;
    };
    let form = reqwest::multipart::Form::new()
        .text("chat_id", chat_id.to_string())
        .text("caption", caption)
        .part("photo", photo_part);
    let url = format!("https://api.telegram.org/bot{}/sendPhoto", token);
    client.post(&url).multipart(form).send().await.map(|r| r.status().is_success()).unwrap_or(false)
}

async fn send_telegram_reply_full(
    client: &reqwest::Client,
    token: &str,
    chat_id: &str,
    text: &str,
    keyboard: Option<Value>,
) {
    let url = format!("https://api.telegram.org/bot{}/sendMessage", token);

    let mut body = serde_json::json!({
        "chat_id": chat_id,
        "text": text,
        "parse_mode": "Markdown"
    });

    if let Some(kb) = keyboard {
        body["reply_markup"] = kb;
    }

    let _ = client.post(&url).json(&body).send().await;
}

