use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;
use serde_json::Value;
use crate::data::{get_global_settings, Account, AccountStatus};
use crate::i18n::Language;
use crate::logger;

fn pick<'a>(lang: Language, en: &'a str, tr: &'a str, ru: &'a str) -> &'a str {
    match lang {
        Language::English => en,
        Language::Turkish => tr,
        Language::Russian => ru,
    }
}

fn set_telegram_language(new_lang: Language) {
    let mut config = crate::data::AppConfig::load();
    config.settings.telegram_language = new_lang;
    config.save();
    crate::data::update_global_settings(&config.settings);
}

fn parse_lang_arg(s: &str) -> Option<Language> {
    match s.trim().to_lowercase().as_str() {
        "en" | "eng" | "english" => Some(Language::English),
        "tr" | "tur" | "turkish" | "türkçe" | "turkce" => Some(Language::Turkish),
        "ru" | "rus" | "russian" | "rusça" | "rusca" | "русский" => Some(Language::Russian),
        _ => None,
    }
}

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
    let lang = get_global_settings().telegram_language;

    // Helper: resolve account name — if none given, fall back to /switch selection or first account
    let resolve_account = |name_opt: Option<&str>| -> Option<String> {
        if let Some(n) = name_opt {
            return Some(n.to_string());
        }
        let sel = get_selected_account();
        if !sel.is_empty() {
            return Some(sel);
        }
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
            let msg = pick(
                lang,
                "🤖 *SupBooster Bot*\nWelcome! Use `/help` to see all commands or `/lang` to change language.",
                "🤖 *SupBooster Bot*\nHoş geldin! Tüm komutlar için `/help`, dil değiştirmek için `/lang` yazabilirsin.",
                "🤖 *SupBooster Bot*\nДобро пожаловать! Введи `/help` для списка команд или `/lang` для смены языка.",
            ).to_string();
            (msg, None, None)
        }

        // ─────────────────────────────────────────────────────────────
        // /lang [en|tr|ru]  — select bot language
        // ─────────────────────────────────────────────────────────────
        "/lang" => {
            if let Some(arg) = parts.get(1) {
                if let Some(new_lang) = parse_lang_arg(arg) {
                    set_telegram_language(new_lang);
                    let msg = pick(
                        new_lang,
                        "✅ Bot language set to *English*.",
                        "✅ Bot dili *Türkçe* olarak ayarlandı.",
                        "✅ Язык бота переключён на *Русский*.",
                    ).to_string();
                    return (msg, None, None);
                }
            }

            let msg = pick(
                lang,
                "🌐 *Select Bot Language* (Current: English):",
                "🌐 *Bot Dilini Seçin* (Mevcut: Türkçe):",
                "🌐 *Выберите язык бота* (Текущий: Русский):",
            ).to_string();
            let keyboard = serde_json::json!({
                "inline_keyboard": [[
                    { "text": "🇬🇧 English", "callback_data": "set_lang:en" },
                    { "text": "🇹🇷 Türkçe", "callback_data": "set_lang:tr" },
                    { "text": "🇷🇺 Русский", "callback_data": "set_lang:ru" }
                ]]
            });
            (msg, None, Some(keyboard))
        }

        // ─────────────────────────────────────────────────────────────
        // /help  — full command list in selected language
        // ─────────────────────────────────────────────────────────────
        "/help" => {
            let msg = pick(
                lang,
                "🤖 *SupBooster — Command List*\n\n\
📌 *Basic*\n\
`/start` — Start bot\n\
`/help` — Show all commands\n\
`/lang` — Change bot language (EN/TR/RU)\n\
`/status` — Account status & quick controls\n\n\
🚀 *Boost Control*\n\
`/start_boost [account]` — Start boosting\n\
`/stop_boost [account]` — Stop boosting\n\
`/restart [account]` — Reconnect session\n\n\
🃏 *Cards & Drops*\n\
`/drops [account]` — Games with card drops\n\
`/auto_drops <on|off>` — Auto-switch on 0 drops\n\
`/drop_notify <on|off>` — Card drop notifications\n\n\
🎮 *Game Management*\n\
`/games [account]` — List boost games\n\
`/add_game <appid> [account]` — Add game\n\
`/remove_game <appid> [account]` — Remove game\n\n\
👤 *Account Management*\n\
`/accounts` — List all accounts\n\
`/addaccount <username> | <password>` — Add Steam account\n\
`/switch <account>` — Set default account\n\
`/qr [account]` — Send QR login image",
                "🤖 *SupBooster — Komut Listesi*\n\n\
📌 *Temel*\n\
`/start` — Botu başlatır\n\
`/help` — Tüm komutları listeler\n\
`/lang` — Bot dilini seçer (EN/TR/RU)\n\
`/status` — Hesap durumu ve hızlı kontrol\n\n\
🚀 *Boost Kontrolü*\n\
`/start_boost [hesap]` — Boost başlatır\n\
`/stop_boost [hesap]` — Boost durdurur\n\
`/restart [hesap]` — Oturumu yeniler\n\n\
🃏 *Kartlar & Droplar*\n\
`/drops [hesap]` — Kart düşen oyunlar\n\
`/auto_drops <on|off>` — 0 kartta otomatik geçiş\n\
`/drop_notify <on|off>` — Kart düşme bildirimleri\n\n\
🎮 *Oyun Yönetimi*\n\
`/games [hesap]` — Boost oyunlarını listeler\n\
`/add_game <appid> [hesap]` — Oyun ekler\n\
`/remove_game <appid> [hesap]` — Oyun kaldırır\n\n\
👤 *Hesap Yönetimi*\n\
`/accounts` — Kayıtlı hesapları listeler\n\
`/addaccount <kullanıcı adı> | <şifre>` — Steam hesabı ekler\n\
`/switch <hesap>` — Varsayılan hesabı seçer\n\
`/qr [hesap]` — QR giriş görseli gönderir",
                "🤖 *SupBooster — Список команд*\n\n\
📌 *Основные*\n\
`/start` — Запуск бота\n\
`/help` — Список всех команд\n\
`/lang` — Выбрать язык бота (EN/TR/RU)\n\
`/status` — Статус аккаунтов и управление\n\n\
🚀 *Управление бустом*\n\
`/start_boost [аккаунт]` — Запустить буст\n\
`/stop_boost [аккаунт]` — Остановить буст\n\
`/restart [аккаунт]` — Переподключить сессию\n\n\
🃏 *Карточки и дропы*\n\
`/drops [аккаунт]` — Игры с доступными дропами\n\
`/auto_drops <on|off>` — Авто-переключение при 0 карт\n\
`/drop_notify <on|off>` — Уведомления о дропах\n\n\
🎮 *Управление играми*\n\
`/games [аккаунт]` — Список игр для буста\n\
`/add_game <appid> [аккаунт]` — Добавить игру\n\
`/remove_game <appid> [аккаунт]` — Убрать игру\n\n\
👤 *Управление аккаунтами*\n\
`/accounts` — Список всех аккаунтов\n\
`/addaccount <логин> | <пароль>` — Добавить аккаунт Steam\n\
`/switch <аккаунт>` — Выбрать аккаунт по умолчанию\n\
`/qr [аккаунт]` — Отправить QR-код для входа",
            ).to_string();
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
                    pick(
                        lang,
                        "⚠️ No accounts added yet.",
                        "⚠️ Henüz hesap eklenmemiş.",
                        "⚠️ Аккаунты ещё не добавлены.",
                    ).to_string(),
                    None, None,
                );
            }

            let mut out = pick(
                lang,
                "📋 *SupBooster — Account Status*\n\n",
                "📋 *SupBooster — Hesap Durumu*\n\n",
                "📋 *SupBooster — Статус аккаунтов*\n\n",
            ).to_string();
            let mut buttons: Vec<Vec<Value>> = Vec::new();

            for (idx, arc) in accounts.iter().enumerate() {
                let a = arc.lock().unwrap();
                let (icon, st) = match &a.status {
                    AccountStatus::Boosting => ("🟢", pick(lang, "Boosting", "Kasıyor", "Работает")),
                    AccountStatus::Connecting => ("🟡", pick(lang, "Connecting", "Bağlanıyor", "Подключение")),
                    AccountStatus::AwaitingSteamGuard { .. } => ("🛡️", pick(lang, "Guard Needed", "Kod Bekleniyor", "Нужен код")),
                    AccountStatus::PausedForGame { .. } => ("⏸️", pick(lang, "In-Game", "Oyunda", "В игре")),
                    AccountStatus::Reconnecting { .. } => ("🔄", pick(lang, "Reconnecting", "Yeniden Bağlanıyor", "Переподключение")),
                    AccountStatus::Error(_) => ("🔴", pick(lang, "Error", "Hata", "Ошибка")),
                    AccountStatus::Idle => ("⚪", pick(lang, "Stopped", "Durduruldu", "Остановлен")),
                };

                let card_count = crate::inventory::get_cached_active_drops(a.steam_id).len();
                let games_lbl = pick(lang, "games", "oyun", "игр");
                let drops_lbl = pick(lang, "card drops", "kart hakkı", "дропов");

                out.push_str(&format!(
                    "{}. {} *{}*\n   └ {} | ⏱ `{:.1}h` | 🎮 `{}` {} | 🃏 `{}` {}\n\n",
                    idx + 1, icon, a.username, st, a.hours_boosted, a.games.len(), games_lbl, card_count, drops_lbl
                ));

                let stop_lbl = pick(lang, "Stop", "Durdur", "Стоп");
                let start_lbl = pick(lang, "Start", "Başlat", "Старт");
                buttons.push(vec![
                    serde_json::json!({
                        "text": format!("⏹ {} {}", stop_lbl, a.username),
                        "callback_data": format!("stop_boost:{}", a.username)
                    }),
                    serde_json::json!({
                        "text": format!("▶️ {} {}", start_lbl, a.username),
                        "callback_data": format!("start_boost:{}", a.username)
                    }),
                ]);
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
                let msg = match start_account_remote(target) {
                    Ok(_) => match lang {
                        Language::English => format!("▶️ Boost started for `{}`.", target),
                        Language::Turkish => format!("▶️ `{}` için boost başlatıldı.", target),
                        Language::Russian => format!("▶️ Буст запущен для `{}`.", target),
                    },
                    Err(e) => e,
                };
                (msg, None, None)
            } else {
                start_all_remote();
                (pick(
                    lang,
                    "▶️ Boost started for all accounts.",
                    "▶️ Tüm hesaplar için boost başlatıldı.",
                    "▶️ Буст запущен для всех аккаунтов.",
                ).to_string(), None, None)
            }
        }

        // ─────────────────────────────────────────────────────────────
        // /stop_boost [account]
        // ─────────────────────────────────────────────────────────────
        "/stop_boost" => {
            if parts.len() > 1 {
                let target = parts[1];
                let msg = match stop_account_remote(target) {
                    Ok(_) => match lang {
                        Language::English => format!("⏹️ Boost stopped for `{}`.", target),
                        Language::Turkish => format!("⏹️ `{}` için boost durduruldu.", target),
                        Language::Russian => format!("⏹️ Буст остановлен для `{}`.", target),
                    },
                    Err(e) => e,
                };
                (msg, None, None)
            } else {
                stop_all_remote();
                (pick(
                    lang,
                    "⏹️ Boost stopped for all accounts.",
                    "⏹️ Tüm hesaplarda boost durduruldu.",
                    "⏹️ Буст остановлен для всех аккаунтов.",
                ).to_string(), None, None)
            }
        }

        // ─────────────────────────────────────────────────────────────
        // /restart [account]
        // ─────────────────────────────────────────────────────────────
        "/restart" => {
            if parts.len() > 1 {
                let target = parts[1];
                let msg = match restart_account_remote(target) {
                    Ok(_) => match lang {
                        Language::English => format!("🔄 Restarting `{}`...", target),
                        Language::Turkish => format!("🔄 `{}` yeniden başlatılıyor...", target),
                        Language::Russian => format!("🔄 Перезапуск `{}`...", target),
                    },
                    Err(e) => e,
                };
                (msg, None, None)
            } else {
                restart_all_remote();
                (pick(
                    lang,
                    "🔄 Restarting all accounts...",
                    "🔄 Tüm hesaplar yeniden başlatılıyor...",
                    "🔄 Перезапуск всех аккаунтов...",
                ).to_string(), None, None)
            }
        }

        // ─────────────────────────────────────────────────────────────
        // /drops [account]  — games with card drops remaining
        // ─────────────────────────────────────────────────────────────
        "/drops" => {
            let target_opt = resolve_account(parts.get(1).copied());
            match target_opt {
                None => (
                    pick(lang, "❌ No account specified.", "❌ Hesap belirtilmedi.", "❌ Аккаунт не указан.").to_string(),
                    None, None
                ),
                Some(target) => {
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
                        None => (format!("❌ `{}` not found.", target), None, None),
                        Some(sid) => {
                            let drops = crate::inventory::get_cached_active_drops(sid);
                            if drops.is_empty() {
                                let msg = match lang {
                                    Language::English => format!("🃏 No card drops remaining for `{}`.", target),
                                    Language::Turkish => format!("🃏 `{}` için kalan kart düşürme hakkı yok.", target),
                                    Language::Russian => format!("🃏 Для `{}` не осталось дропов карт.", target),
                                };
                                (msg, None, None)
                            } else {
                                let header = pick(lang, "Active Card Drops", "Aktif Kart Hakları", "Активные дропы");
                                let rem_lbl = pick(lang, "cards remaining", "kart kaldı", "карт осталось");
                                let mut msg = format!("🃏 *{}* — {}\n\n", target, header);
                                let mut buttons: Vec<Vec<Value>> = Vec::new();
                                for d in &drops {
                                    msg.push_str(&format!(
                                        "• *{}* (AppID: `{}`)\n  🃏 `{}` {}\n",
                                        d.name, d.app_id, d.remaining_cards, rem_lbl
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
                    pick(lang, "❌ No account specified.", "❌ Hesap belirtilmedi.", "❌ Аккаунт не указан.").to_string(),
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
                                out = match lang {
                                    Language::English => format!("🎮 `{}` has no games.\nAdd: `/add_game <appid> {}`", a.username, a.username),
                                    Language::Turkish => format!("🎮 `{}` için ekli oyun yok.\nEkle: `/add_game <appid> {}`", a.username, a.username),
                                    Language::Russian => format!("🎮 У `{}` нет игр.\nДобавить: `/add_game <appid> {}`", a.username, a.username),
                                };
                            } else {
                                let title = pick(lang, "Boost Games", "Boost Oyunları", "Игры для буста");
                                out = format!("🎮 *{}* — {} ({})\n\n", a.username, title, a.games.len());
                                for (i, g) in a.games.iter().enumerate() {
                                    out.push_str(&format!("{}. *{}* — AppID: `{}`\n", i + 1, g.name, g.app_id));
                                }
                            }
                            break;
                        }
                    }
                    if !found {
                        out = format!("❌ `{}` not found.", target);
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
                return ("❌ `/add_game <appid> [account]`".to_string(), None, None);
            }
            if let Ok(app_id) = parts[1].parse::<u32>() {
                let target_opt = resolve_account(parts.get(2).copied());
                match target_opt {
                    None => (pick(lang, "❌ No account found.", "❌ Hesap bulunamadı.", "❌ Аккаунт не найден.").to_string(), None, None),
                    Some(target) => match add_game_remote(&target, app_id, "") {
                        Ok(msg) => (msg, None, None),
                        Err(e) => (e, None, None),
                    },
                }
            } else {
                ("❌ Invalid AppID. `/add_game <appid> [account]`".to_string(), None, None)
            }
        }

        // ─────────────────────────────────────────────────────────────
        // /remove_game <appid> [account]
        // ─────────────────────────────────────────────────────────────
        "/remove_game" => {
            if parts.len() < 2 {
                return ("❌ `/remove_game <appid> [account]`".to_string(), None, None);
            }
            if let Ok(app_id) = parts[1].parse::<u32>() {
                let target_opt = resolve_account(parts.get(2).copied());
                match target_opt {
                    None => (pick(lang, "❌ No account found.", "❌ Hesap bulunamadı.", "❌ Аккаунт не найден.").to_string(), None, None),
                    Some(target) => match remove_game_remote(&target, app_id) {
                        Ok(msg) => (msg, None, None),
                        Err(e) => (e, None, None),
                    },
                }
            } else {
                ("❌ Invalid AppID. `/remove_game <appid> [account]`".to_string(), None, None)
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
                    (pick(
                        lang,
                        "✅ Auto-drops ON.",
                        "✅ Otomatik kart geçişi AÇIK.",
                        "✅ Авто-переключение ВКЛЮЧЕНО.",
                    ).to_string(), None, None)
                }
                Some("off") => {
                    let mut config = crate::data::AppConfig::load();
                    config.settings.auto_switch_drops = false;
                    config.save();
                    crate::data::update_global_settings(&config.settings);
                    (pick(
                        lang,
                        "🔕 Auto-drops OFF.",
                        "🔕 Otomatik kart geçişi KAPALI.",
                        "🔕 Авто-переключение ВЫКЛЮЧЕНО.",
                    ).to_string(), None, None)
                }
                _ => {
                    let current = get_global_settings().auto_switch_drops;
                    let state = if current {
                        pick(lang, "ON", "AÇIK", "ВКЛ")
                    } else {
                        pick(lang, "OFF", "KAPALI", "ВЫКЛ")
                    };
                    (format!("⚙️ `auto_drops`: *{}*\n`/auto_drops on` | `/auto_drops off`", state), None, None)
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
                    pick(lang, "⚠️ No accounts added.", "⚠️ Hiç hesap eklenmemiş.", "⚠️ Аккаунты не добавлены.").to_string(),
                    None, None,
                );
            }
            let title = pick(lang, "Accounts", "Hesaplar", "Аккаунты");
            let mut out = format!("👤 *SupBooster — {}*\n\n", title);
            for (i, arc) in accounts.iter().enumerate() {
                let a = arc.lock().unwrap();
                let icon = match &a.status {
                    AccountStatus::Boosting => "🟢",
                    AccountStatus::Idle => "⚪",
                    AccountStatus::Error(_) => "🔴",
                    _ => "🟡",
                };
                out.push_str(&format!("{}. {} *{}* (ID: `{}`)\n", i + 1, icon, a.username, a.id));
            }
            let hint = pick(
                lang,
                "\nUse `/switch <account>` to set default.",
                "\nVarsayılan hesap için `/switch <hesap>` kullan.",
                "\nДля выбора по умолчанию: `/switch <аккаунт>`.",
            );
            out.push_str(hint);
            (out, None, None)
        }

        // ─────────────────────────────────────────────────────────────
        // /addaccount <username> | <password>
        // ─────────────────────────────────────────────────────────────
        "/addaccount" => {
            let rest = command.split_once(char::is_whitespace).map(|(_, r)| r.trim()).unwrap_or("");
            if let Some((u_raw, p_raw)) = rest.split_once('|') {
                let user = u_raw.trim();
                let pass = p_raw.trim();
                if user.is_empty() || pass.is_empty() {
                    return (
                        pick(
                            lang,
                            "❌ Usage: `/addaccount <username> | <password>`",
                            "❌ Kullanım: `/addaccount <kullanıcı adı> | <şifre>`",
                            "❌ Формат: `/addaccount <логин> | <пароль>`",
                        ).to_string(),
                        None, None,
                    );
                }
                match add_account_remote(user, pass, None, None) {
                    Ok(id) => {
                        let msg = match lang {
                            Language::English => format!("✅ Account `{}` added! (ID: `{}`)\nStart: `/start_boost {}`", user, id, user),
                            Language::Turkish => format!("✅ `{}` hesabı eklendi! (ID: `{}`)\nBaşlat: `/start_boost {}`", user, id, user),
                            Language::Russian => format!("✅ Аккаунт `{}` добавлен! (ID: `{}`)\nЗапуск: `/start_boost {}`", user, id, user),
                        };
                        (msg, None, None)
                    }
                    Err(e) => (format!("❌ {}", e), None, None),
                }
            } else {
                (
                    pick(
                        lang,
                        "❌ Usage: `/addaccount <username> | <password>`",
                        "❌ Kullanım: `/addaccount <kullanıcı adı> | <şifre>`",
                        "❌ Формат: `/addaccount <логин> | <пароль>`",
                    ).to_string(),
                    None, None,
                )
            }
        }

        // ─────────────────────────────────────────────────────────────
        // /switch <account>
        // ─────────────────────────────────────────────────────────────
        "/switch" => {
            if parts.len() < 2 {
                let current = get_selected_account();
                let display = if current.is_empty() { "-".to_string() } else { current };
                let msg = match lang {
                    Language::English => format!("⚙️ Default account: `{}`\nUsage: `/switch <account>`", display),
                    Language::Turkish => format!("⚙️ Varsayılan hesap: `{}`\nKullanım: `/switch <hesap>`", display),
                    Language::Russian => format!("⚙️ Аккаунт по умолчанию: `{}`\nФормат: `/switch <аккаунт>`", display),
                };
                return (msg, None, None);
            }
            let target = parts[1];
            let accounts = get_runtime_accounts();
            let target_l = target.to_lowercase();
            let found = accounts.iter().any(|arc| {
                let a = arc.lock().unwrap();
                a.username.to_lowercase() == target_l || a.id.to_string() == target_l
            });
            if found {
                set_selected_account(target);
                let msg = match lang {
                    Language::English => format!("✅ Default account set to `{}`.", target),
                    Language::Turkish => format!("✅ Varsayılan hesap `{}` olarak ayarlandı.", target),
                    Language::Russian => format!("✅ Аккаунт по умолчанию: `{}`.", target),
                };
                (msg, None, None)
            } else {
                (format!("❌ `{}` not found.", target), None, None)
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
                    (pick(
                        lang,
                        "🔔 Card drop notifications ON.",
                        "🔔 Kart düşme bildirimleri AÇIK.",
                        "🔔 Уведомления о дропах ВКЛЮЧЕНЫ.",
                    ).to_string(), None, None)
                }
                Some("off") => {
                    let mut config = crate::data::AppConfig::load();
                    config.settings.telegram_notify_cards = false;
                    config.save();
                    crate::data::update_global_settings(&config.settings);
                    (pick(
                        lang,
                        "🔕 Card drop notifications OFF.",
                        "🔕 Kart düşme bildirimleri KAPALI.",
                        "🔕 Уведомления о дропах ВЫКЛЮЧЕНЫ.",
                    ).to_string(), None, None)
                }
                _ => {
                    let current = get_global_settings().telegram_notify_cards;
                    let state = if current {
                        pick(lang, "ON", "AÇIK", "ВКЛ")
                    } else {
                        pick(lang, "OFF", "KAPALI", "ВЫКЛ")
                    };
                    (format!("⚙️ `drop_notify`: *{}*\n`/drop_notify on` | `/drop_notify off`", state), None, None)
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
                    pick(lang, "❌ No account specified.", "❌ Hesap belirtilmedi.", "❌ Аккаунт не указан.").to_string(),
                    None, None
                ),
                Some(target) => {
                    let qr_url = crate::qr_auth::get_current_challenge_url();
                    match qr_url {
                        Some(url) if !url.is_empty() => {
                            let hint = pick(
                                lang,
                                "Scan with Steam mobile app to log in.",
                                "Giriş yapmak için Steam mobil uygulamasıyla tara.",
                                "Отсканируй в приложении Steam для входа.",
                            );
                            (format!("📱 *QR ({})*\n{}\n\n🔗 `{}`", target, hint, url), None, None)
                        }
                        _ => {
                            let msg = match lang {
                                Language::English => format!("⚠️ No active QR code for `{}`. Account may already be logged in.", target),
                                Language::Turkish => format!("⚠️ `{}` için aktif QR kod yok. Hesap zaten giriş yapmış olabilir.", target),
                                Language::Russian => format!("⚠️ Нет активного QR-кода для `{}`. Аккаунт уже может быть авторизован.", target),
                            };
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
            (pick(lang, "▶️ Boost started for all accounts.", "▶️ Tüm hesaplar için boost başlatıldı.", "▶️ Буст запущен для всех аккаунтов.").to_string(), None, None)
        }
        "/stop_all" => {
            stop_all_remote();
            (pick(lang, "⏹️ Boost stopped for all accounts.", "⏹️ Tüm hesaplarda boost durduruldu.", "⏹️ Буст остановлен для всех аккаунтов.").to_string(), None, None)
        }
        "/restart_all" => {
            restart_all_remote();
            (pick(lang, "🔄 Restarting all accounts...", "🔄 Tüm hesaplar yeniden başlatılıyor...", "🔄 Перезапуск всех аккаунтов...").to_string(), None, None)
        }
        "/addgame" if parts.len() >= 3 => {
            let target = parts[1];
            if let Ok(app_id) = parts[2].parse::<u32>() {
                match add_game_remote(target, app_id, "") {
                    Ok(msg) => (msg, None, None),
                    Err(e) => (e, None, None),
                }
            } else {
                ("❌ Invalid AppID. `/addgame user1 730`".to_string(), None, None)
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
                ("❌ Invalid AppID. `/removegame user1 730`".to_string(), None, None)
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
                return (pick(lang, "🃏 No dropped cards recorded yet.", "🃏 Henüz düşen kart yok.", "🃏 Дропов пока нет.").to_string(), None, None);
            }
            let mut msg = format!("🃏 *Cards ({})*\n\n", cards.len());
            for c in cards.iter().rev().take(10) {
                let price_str = if c.market_price.is_empty() { "-" } else { &c.market_price };
                msg.push_str(&format!("• *{}* — `{}`\n  └ *{}* ({})\n", c.game_name, c.username, c.card_name, price_str));
            }
            (msg, None, None)
        }
        "/search" if parts.len() > 1 => {
            let query = parts[1..].join(" ");
            let results = search_steam_games(&query).await;
            if results.is_empty() {
                return (format!("🔍 `{}` — 0", query), None, None);
            }
            let mut reply = format!("🔍 *\"{}\"*:\n\n", query);
            for item in results.iter().take(8) {
                let id = item.get("app_id").and_then(|v| v.as_u64()).unwrap_or(0);
                let name = item.get("name").and_then(|v| v.as_str()).unwrap_or("Unknown");
                reply.push_str(&format!("• *{}* — `{}`\n  `/add_game {}`\n", name, id, id));
            }
            (reply, None, None)
        }

        // ─────────────────────────────────────────────────────────────
        // Unknown command
        // ─────────────────────────────────────────────────────────────
        _ => (
            pick(
                lang,
                "❓ Unknown command. Type `/help`.",
                "❓ Bilinmeyen komut. `/help` yazabilirsin.",
                "❓ Неизвестная команда. Введи `/help`.",
            ).to_string(),
            None, None,
        ),
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
    let lang = get_global_settings().telegram_language;
    let parts: Vec<&str> = data.split(':').collect();
    match parts.as_slice() {
        ["set_lang", code] => {
            if let Some(new_lang) = parse_lang_arg(code) {
                set_telegram_language(new_lang);
                pick(
                    new_lang,
                    "✅ Bot language set to *English*.",
                    "✅ Bot dili *Türkçe* olarak ayarlandı.",
                    "✅ Язык бота переключён на *Русский*.",
                ).to_string()
            } else {
                String::new()
            }
        }
        ["stop_boost", target] => match stop_account_remote(target) {
            Ok(_) => match lang {
                Language::English => format!("⏹️ Boost stopped for `{}`.", target),
                Language::Turkish => format!("⏹️ `{}` için boost durduruldu.", target),
                Language::Russian => format!("⏹️ Буст остановлен для `{}`.", target),
            },
            Err(e) => e,
        },
        ["start_boost", target] => match start_account_remote(target) {
            Ok(_) => match lang {
                Language::English => format!("▶️ Boost started for `{}`.", target),
                Language::Turkish => format!("▶️ `{}` için boost başlatıldı.", target),
                Language::Russian => format!("▶️ Буст запущен для `{}`.", target),
            },
            Err(e) => e,
        },
        ["add_game", target, app_id_str] => {
            if let Ok(app_id) = app_id_str.parse::<u32>() {
                match add_game_remote(target, app_id, "") {
                    Ok(msg) => msg,
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

    let lang = get_global_settings().telegram_language;
    let scan_hint = pick(
        lang,
        "Scan with Steam mobile app.",
        "Steam mobil uygulamasıyla tara.",
        "Отсканируй в приложении Steam.",
    );
    let caption = format!("📱 QR Code ({})\n{}", target, scan_hint);
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

