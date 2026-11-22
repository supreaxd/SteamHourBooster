use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;
use serde_json::Value;
use crate::data::{get_global_settings, Account, AccountStatus};
use crate::logger;

static TELEGRAM_RUNNING: AtomicBool = AtomicBool::new(false);
static RUNTIME_ACCOUNTS_REF: OnceLock<Arc<Mutex<Vec<Arc<Mutex<Account>>>>>> = OnceLock::new();

pub fn register_accounts_for_bot(accounts: Vec<Arc<Mutex<Account>>>) {
    let container = RUNTIME_ACCOUNTS_REF.get_or_init(|| Arc::new(Mutex::new(Vec::new())));
    if let Ok(mut lock) = container.lock() {
        *lock = accounts;
    }
}

#[allow(dead_code)]
pub fn update_accounts_for_bot(accounts: &[Arc<Mutex<Account>>]) {
    let container = RUNTIME_ACCOUNTS_REF.get_or_init(|| Arc::new(Mutex::new(Vec::new())));
    if let Ok(mut lock) = container.lock() {
        *lock = accounts.to_vec();
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
                                        let reply = handle_bot_command(text).await;
                                        send_raw_telegram(&client, &token, &chat_id, &reply).await;
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

async fn handle_bot_command(command: &str) -> String {
    let parts: Vec<&str> = command.split_whitespace().collect();
    if parts.is_empty() {
        return String::new();
    }

    let cmd = parts[0].to_lowercase();
    let accounts_arc = RUNTIME_ACCOUNTS_REF.get_or_init(|| Arc::new(Mutex::new(Vec::new()))).clone();

    match cmd.as_str() {
        "/help" | "/start" if parts.len() == 1 => {
            "🚀 *SteamBooster Telegram Kontrol Merkezi*\n\n\
            📋 *Komutlar:*\n\
            • `/status` - Tüm hesapların anlık durumunu ve kasılan saatleri gösterir\n\
            • `/start_all` - Tüm hesaplarda saat kasmayı başlatır\n\
            • `/stop_all` - Tüm hesaplarda saat kasmayı durdurur\n\
            • `/restart_all` - Tüm hesapları yeniden başlatır\n\
            • `/start <hesap_adı>` - Belirli bir hesabı başlatır\n\
            • `/stop <hesap_adı>` - Belirli bir hesabı durdurur\n\
            • `/restart <hesap_adı>` - Belirli bir hesabı yeniden başlatır\n\
            • `/logs` - Son sistem loglarını listeler".to_string()
        }

        "/status" => {
            let accounts = {
                let lock = accounts_arc.lock().unwrap();
                lock.clone()
            };

            if accounts.is_empty() {
                return "ℹ️ Kayıtlı hesap bulunamadı.".to_string();
            }

            let mut out = format!("📊 *SteamBooster Hesap Durumları ({} Hesap):*\n\n", accounts.len());
            for arc in accounts {
                let a = arc.lock().unwrap();
                let status_icon = match &a.status {
                    AccountStatus::Boosting => "🟢 Kasıyor",
                    AccountStatus::Connecting => "🟡 Bağlanıyor",
                    AccountStatus::AwaitingSteamGuard { .. } => "🛡️ Guard Bekleniyor",
                    AccountStatus::PausedForGame { .. } => "⏸️ Oyunda (Duraklatıldı)",
                    AccountStatus::Reconnecting { .. } => "🔄 Yeniden Bağlanıyor",
                    AccountStatus::Error(e) => &format!("🔴 Hata: {}", e),
                    AccountStatus::Idle => "⚪ Durduruldu",
                };

                out.push_str(&format!(
                    "👤 *{}* | {}\n🎮 {} oyun | ⏱ {:.1} saat\n🏷️ Başlık: {}\n\n",
                    a.username, status_icon, a.games.len(), a.hours_boosted, a.custom_title
                ));
            }
            out
        }

        "/start_all" | "/startall" => {
            let accounts = {
                let lock = accounts_arc.lock().unwrap();
                lock.clone()
            };
            for arc in &accounts {
                crate::steam::start_boost(arc.clone());
            }
            format!("✅ {} hesap için saat kasma başlatıldı!", accounts.len())
        }

        "/stop_all" | "/stopall" => {
            let accounts = {
                let lock = accounts_arc.lock().unwrap();
                lock.clone()
            };
            for arc in &accounts {
                let id = arc.lock().unwrap().id;
                crate::steam::stop_boost(id, arc.clone());
            }
            format!("⏹️ {} hesap durduruldu.", accounts.len())
        }

        "/restart_all" | "/restartall" => {
            let accounts = {
                let lock = accounts_arc.lock().unwrap();
                lock.clone()
            };
            for arc in &accounts {
                let id = arc.lock().unwrap().id;
                crate::steam::restart_boost(id, arc.clone());
            }
            format!("🔄 {} hesap yeniden başlatılıyor...", accounts.len())
        }

        "/logs" => {
            let logs = logger::get_logs();
            let recent: Vec<_> = logs.iter().rev().take(12).collect();
            if recent.is_empty() {
                return "📜 Log kaydı bulunmuyor.".to_string();
            }
            let mut out = "📜 *Son Sistem Logları:*\n".to_string();
            for l in recent.iter().rev() {
                out.push_str(&format!("`[{}]` {}\n", l.timestamp, l.message));
            }
            out
        }

        "/start" if parts.len() > 1 => {
            let target = parts[1].to_lowercase();
            let accounts = {
                let lock = accounts_arc.lock().unwrap();
                lock.clone()
            };
            for arc in &accounts {
                let (id, user) = {
                    let a = arc.lock().unwrap();
                    (a.id, a.username.to_lowercase())
                };
                if user == target || id.to_string() == target {
                    crate::steam::start_boost(arc.clone());
                    return format!("✅ `{}` hesabı başlatıldı.", target);
                }
            }
            format!("❌ `{}` isimli hesap bulunamadı.", target)
        }

        "/stop" if parts.len() > 1 => {
            let target = parts[1].to_lowercase();
            let accounts = {
                let lock = accounts_arc.lock().unwrap();
                lock.clone()
            };
            for arc in &accounts {
                let (id, user) = {
                    let a = arc.lock().unwrap();
                    (a.id, a.username.to_lowercase())
                };
                if user == target || id.to_string() == target {
                    crate::steam::stop_boost(id, arc.clone());
                    return format!("⏹️ `{}` hesabı durduruldu.", target);
                }
            }
            format!("❌ `{}` isimli hesap bulunamadı.", target)
        }

        "/restart" if parts.len() > 1 => {
            let target = parts[1].to_lowercase();
            let accounts = {
                let lock = accounts_arc.lock().unwrap();
                lock.clone()
            };
            for arc in &accounts {
                let (id, user) = {
                    let a = arc.lock().unwrap();
                    (a.id, a.username.to_lowercase())
                };
                if user == target || id.to_string() == target {
                    crate::steam::restart_boost(id, arc.clone());
                    return format!("🔄 `{}` hesabı yeniden başlatılıyor...", target);
                }
            }
            format!("❌ `{}` isimli hesap bulunamadı.", target)
        }

        _ => "❓ Bilinmeyen komut. Komut listesi için `/help` yazabilirsiniz.".to_string(),
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
    let url = format!("https://api.telegram.org/bot{}/sendMessage", token);
    let params = serde_json::json!({
        "chat_id": chat_id,
        "text": text,
        "parse_mode": "Markdown"
    });
    let _ = client.post(&url).json(&params).send().await;
}
