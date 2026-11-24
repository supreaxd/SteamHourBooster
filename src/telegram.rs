use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;
use serde_json::Value;
use crate::data::{get_global_settings, Account, AccountStatus};
use crate::i18n::Language;
use crate::logger;

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

                                    // Chat id filter if user specified one
                                    if !default_chat_id.is_empty() && chat_id != default_chat_id {
                                        logger::log_warn(&format!("🤖 Unauthorized Telegram user: {}", chat_id));
                                        send_raw_telegram(&client, &token, &chat_id, "⛔ You are not authorized to use this bot.").await;
                                        continue;
                                    }

                                    if !text.is_empty() {
                                        let (reply, web_app_opt) = handle_bot_command(text).await;
                                        send_telegram_reply(&client, &token, &chat_id, &reply, web_app_opt.as_deref()).await;
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
            return Ok(format!("▶️ `{}` başlatıldı.", target));
        }
    }
    Err(format!("❌ `{}` hesabı bulunamadı.", target))
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
            return Ok(format!("⏹️ `{}` durduruldu.", target));
        }
    }
    Err(format!("❌ `{}` hesabı bulunamadı.", target))
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
            return Ok(format!("🔄 `{}` yeniden başlatılıyor...", target));
        }
    }
    Err(format!("❌ `{}` hesabı bulunamadı.", target))
}

pub fn add_account_remote(
    user: &str,
    pass: &str,
    custom_title: Option<&str>,
    shared_secret: Option<&str>,
) -> Result<u64, String> {
    if user.trim().is_empty() {
        return Err("Kullanıcı adı boş olamaz".to_string());
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
        Ok(format!("🗑️ `{}` hesabı başarıyla silindi.", removed_name))
    } else {
        Err(format!("❌ `{}` hesabı bulunamadı.", target))
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
                return Err(format!("⚠️ Oyun zaten listede: `{}` ({})", game_name, app_id));
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
        Ok(format!("🎮 `{}` hesaba eklendi: `{}` ({})", target, game_name, app_id))
    } else {
        Err(format!("❌ `{}` hesabı bulunamadı.", target))
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
        Ok(format!("🗑️ `{}` hesaptan çıkarıldı: `{}` ({})", target, removed_title, app_id))
    } else {
        Err(format!("❌ Oyun veya hesap bulunamadı."))
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
                return Ok(format!("🛡️ `{}` hesabı için Steam Guard kodu iletildi.", target));
            }
        }
    }
    Err(format!("❌ `{}` hesabı bulunamadı.", target))
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

async fn handle_bot_command(command: &str) -> (String, Option<String>) {
    let parts: Vec<&str> = command.split_whitespace().collect();
    if parts.is_empty() {
        return (String::new(), None);
    }

    let cmd = parts[0].to_lowercase();
    let accounts_arc = RUNTIME_ACCOUNTS_REF.get_or_init(|| Arc::new(Mutex::new(Vec::new()))).clone();
    let settings = get_global_settings();
    let lang = settings.language;

    match cmd.as_str() {
        "/start" if parts.len() > 1 => {
            match start_account_remote(parts[1]) {
                Ok(msg) => (msg, None),
                Err(e) => (e, None),
            }
        }

        "/start" | "/help" => {
            let msg = match lang {
                Language::English => "🤖 *SupBooster Telegram Bot*\n\n\
                    • `/status` — View boosting status\n\
                    • `/start_all` / `/stop_all` — Start or stop boosting for all accounts\n\
                    • `/cards` — View recently dropped trading cards\n\
                    • `/games <account>` — List games configured for account\n\
                    • `/search <game>` — Search Steam Store games\n\
                    • `/addgame <account> <appid>` — Add game to account\n\
                    • `/removegame <account> <appid>` — Remove game\n\
                    • `/guard <account> <code>` — Submit Steam Guard code".to_string(),

                Language::Turkish => "🤖 *SupBooster Telegram Bot*\n\n\
                    • `/status` — Hesapların anlık durumunu göster\n\
                    • `/start_all` / `/stop_all` — Tüm hesapları başlat / durdur\n\
                    • `/cards` — Düşen Steam koleksiyon kartları\n\
                    • `/games <hesap>` — Hesabın kasılan oyun listesi\n\
                    • `/search <oyun>` — Steam mağazasında oyun ara\n\
                    • `/addgame <hesap> <appid>` — Hesaba oyun ekle\n\
                    • `/removegame <hesap> <appid>` — Hesaptan oyun çıkar\n\
                    • `/guard <hesap> <kod>` — Steam Guard kodunu gir".to_string(),

                Language::Russian => "🤖 *SupBooster Telegram Bot*\n\n\
                    • `/status` — Статус аккаунтов\n\
                    • `/start_all` / `/stop_all` — Запуск / Стоп всех\n\
                    • `/cards` — Выпавшие карточки\n\
                    • `/games <аккаунт>` — Список игр аккаунта\n\
                    • `/search <игра>` — Поиск игр в Steam\n\
                    • `/addgame <аккаунт> <appid>` — Добавить игру\n\
                    • `/removegame <аккаунт> <appid>` — Удалить игру\n\
                    • `/guard <аккаунт> <код>` — Ввести код Steam Guard".to_string(),
            };
            (msg, None)
        }

        "/status" => {
            let accounts = {
                let lock = accounts_arc.lock().unwrap();
                lock.clone()
            };

            if accounts.is_empty() {
                return (
                    match lang {
                        Language::English => "⚠️ No accounts added yet.".to_string(),
                        Language::Turkish => "⚠️ Henüz eklenmiş hesap yok.".to_string(),
                        Language::Russian => "⚠️ Нет добавленных аккаунтов.".to_string(),
                    },
                    None,
                );
            }

            let mut out = String::from("📋 *SteamBooster Hesap Durumları:*\n\n");
            for (idx, arc) in accounts.iter().enumerate() {
                let a = arc.lock().unwrap();
                let status_icon = match &a.status {
                    AccountStatus::Boosting => "🟢",
                    AccountStatus::Connecting => "🟡",
                    AccountStatus::AwaitingSteamGuard { .. } => "🛡️",
                    AccountStatus::PausedForGame { .. } => "⏸️",
                    AccountStatus::Reconnecting { .. } => "🔄",
                    AccountStatus::Error(_) => "🔴",
                    AccountStatus::Idle => "⚪",
                };
                let status_text = match &a.status {
                    AccountStatus::Boosting => "Kasılıyor",
                    AccountStatus::Connecting => "Bağlanıyor...",
                    AccountStatus::AwaitingSteamGuard { hint } => &format!("Guard Bekleniyor ({})", hint),
                    AccountStatus::PausedForGame { app_name, .. } => &format!("Oyundasınız: {}", app_name),
                    AccountStatus::Reconnecting { attempt, .. } => &format!("Yeniden bağlanıyor (Deneme {})", attempt),
                    AccountStatus::Error(e) => &format!("Hata: {}", e),
                    AccountStatus::Idle => "Durduruldu",
                };

                out.push_str(&format!(
                    "{}. {} *{}* (ID: `{}`)\n   └ Durum: {} | Saat: `{:.1}h` | Oyun: `{}`\n",
                    idx + 1,
                    status_icon,
                    a.username,
                    a.id,
                    status_text,
                    a.hours_boosted,
                    a.games.len()
                ));
            }
            (out, None)
        }

        "/start_all" => {
            start_all_remote();
            (
                match lang {
                    Language::English => "▶️ Boost started for all accounts.".to_string(),
                    Language::Turkish => "▶️ Tüm hesaplar için boost başlatıldı.".to_string(),
                    Language::Russian => "▶️ Буст запущен для всех аккаунтов.".to_string(),
                },
                None,
            )
        }

        "/stop_all" => {
            stop_all_remote();
            (
                match lang {
                    Language::English => "⏹️ Boost stopped for all accounts.".to_string(),
                    Language::Turkish => "⏹️ Tüm hesaplar durduruldu.".to_string(),
                    Language::Russian => "⏹️ Все аккаунты остановлены.".to_string(),
                },
                None,
            )
        }

        "/restart_all" => {
            restart_all_remote();
            (
                match lang {
                    Language::English => "🔄 Restarting boost for all accounts...".to_string(),
                    Language::Turkish => "🔄 Tüm hesaplar yeniden başlatılıyor...".to_string(),
                    Language::Russian => "🔄 Перезапуск всех аккаунтов...".to_string(),
                },
                None,
            )
        }

        "/stop" if parts.len() > 1 => {
            match stop_account_remote(parts[1]) {
                Ok(msg) => (msg, None),
                Err(e) => (e, None),
            }
        }

        "/restart" if parts.len() > 1 => {
            match restart_account_remote(parts[1]) {
                Ok(msg) => (msg, None),
                Err(e) => (e, None),
            }
        }

        "/addaccount" if parts.len() >= 3 => {
            let user = parts[1];
            let pass = parts[2];
            match add_account_remote(user, pass, None, None) {
                Ok(id) => (
                    format!("✅ `{}` hesabı başarıyla eklendi! (ID: `{}`)\nBaşlatmak için: `/start {}`", user, id, user),
                    None,
                ),
                Err(e) => (format!("❌ Hesap eklenemedi: {}", e), None),
            }
        }

        "/delaccount" if parts.len() > 1 => {
            match delete_account_remote(parts[1]) {
                Ok(msg) => (msg, None),
                Err(e) => (e, None),
            }
        }

        "/games" if parts.len() > 1 => {
            let target = parts[1].to_lowercase();
            let accounts = get_runtime_accounts();
            for arc in accounts {
                let a = arc.lock().unwrap();
                if a.username.to_lowercase() == target || a.id.to_string() == target {
                    if a.games.is_empty() {
                        return (format!("🎮 *{}* hesabında kayıtlı oyun yok.\nOyun eklemek için: `/addgame {} <appid>`", a.username, a.username), None);
                    }
                    let mut list = format!("🎮 *{}* Oyun Listesi (Toplam: {}):\n\n", a.username, a.games.len());
                    for (i, g) in a.games.iter().enumerate() {
                        list.push_str(&format!("{}. *{}* (AppID: `{}`)\n", i + 1, g.name, g.app_id));
                    }
                    return (list, None);
                }
            }
            (format!("❌ `{}` hesabı bulunamadı.", parts[1]), None)
        }

        "/search" if parts.len() > 1 => {
            let query = parts[1..].join(" ");
            let results = search_steam_games(&query).await;
            if results.is_empty() {
                return (format!("🔍 `{}` için Steam'de sonuç bulunamadı.", query), None);
            }
            let mut reply = format!("🔍 *\"{}\"* Arama Sonuçları:\n\n", query);
            for item in results.iter().take(8) {
                let id = item.get("app_id").and_then(|v| v.as_u64()).unwrap_or(0);
                let name = item.get("name").and_then(|v| v.as_str()).unwrap_or("Unknown");
                reply.push_str(&format!("• *{}* — AppID: `{}`\n  └ Ekleme: `/addgame <hesap> {}`\n", name, id, id));
            }
            (reply, None)
        }

        "/addgame" if parts.len() >= 3 => {
            let target = parts[1];
            if let Ok(app_id) = parts[2].parse::<u32>() {
                match add_game_remote(target, app_id, "") {
                    Ok(msg) => (msg, None),
                    Err(e) => (e, None),
                }
            } else {
                ("❌ Geçersiz AppID. Örnek: `/addgame user1 730`".to_string(), None)
            }
        }

        "/removegame" if parts.len() >= 3 => {
            let target = parts[1];
            if let Ok(app_id) = parts[2].parse::<u32>() {
                match remove_game_remote(target, app_id) {
                    Ok(msg) => (msg, None),
                    Err(e) => (e, None),
                }
            } else {
                ("❌ Geçersiz AppID. Örnek: `/removegame user1 730`".to_string(), None)
            }
        }

        "/guard" if parts.len() >= 3 => {
            let target = parts[1];
            let code = parts[2];
            match submit_guard_remote(target, code) {
                Ok(msg) => (msg, None),
                Err(e) => (e, None),
            }
        }

        "/cards" => {
            let cards = crate::inventory::get_recent_card_drops();
            if cards.is_empty() {
                return ("🃏 Henüz düşen koleksiyon kartı kaydedilmedi.".to_string(), None);
            }
            let mut msg = format!("🃏 *Son Düşen Koleksiyon Kartları ({})*:\n\n", cards.len());
            for c in cards.iter().rev().take(10) {
                let price_str = if c.market_price.is_empty() { "-" } else { &c.market_price };
                msg.push_str(&format!(
                    "• *{}* — `{}` hesabı\n  └ Kart: *{}* (Fiyat: `{}`)\n",
                    c.game_name, c.username, c.card_name, price_str
                ));
            }
            (msg, None)
        }

        _ => {
            let msg = match lang {
                Language::English => "❓ Unknown command. Type `/help` for available commands.".to_string(),
                Language::Turkish => "❓ Bilinmeyen komut. Komut listesi için `/help` yazabilirsiniz.".to_string(),
                Language::Russian => "❓ Неизвестная команда. Введите `/help` для списка команд.".to_string(),
            };
            (msg, None)
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
        send_raw_telegram(&client, &token, &chat_id, &text).await;
    });
}

async fn send_raw_telegram(client: &reqwest::Client, token: &str, chat_id: &str, text: &str) {
    send_telegram_reply(client, token, chat_id, text, None).await;
}

async fn send_telegram_reply(
    client: &reqwest::Client,
    token: &str,
    chat_id: &str,
    text: &str,
    web_app_url: Option<&str>,
) {
    let url = format!("https://api.telegram.org/bot{}/sendMessage", token);

    let mut body = serde_json::json!({
        "chat_id": chat_id,
        "text": text,
        "parse_mode": "Markdown"
    });

    if let Some(app_url) = web_app_url {
        if app_url.starts_with("https://") {
            body["reply_markup"] = serde_json::json!({
                "inline_keyboard": [
                    [
                        {
                            "text": "🚀 Mini App'i Aç",
                            "web_app": { "url": app_url }
                        }
                    ]
                ]
            });
        } else {
            body["reply_markup"] = serde_json::json!({
                "inline_keyboard": [
                    [
                        {
                            "text": "🌐 Web Panelini Aç",
                            "url": app_url
                        }
                    ]
                ]
            });
        }
    }

    let _ = client.post(&url).json(&body).send().await;
}
