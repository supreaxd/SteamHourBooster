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
    let lang = get_global_settings().language;

    match cmd.as_str() {
        "/help" | "/start" if parts.len() == 1 => {
            match lang {
                Language::English => {
                    "🚀 *SteamBooster Telegram Control Center*\n\n\
                    📋 *Commands:*\n\
                    • `/status` - Show status and boosted hours of all accounts\n\
                    • `/start_all` - Start boosting on all accounts\n\
                    • `/stop_all` - Stop boosting on all accounts\n\
                    • `/restart_all` - Restart boosting on all accounts\n\
                    • `/start <name>` - Start a specific account\n\
                    • `/stop <name>` - Stop a specific account\n\
                    • `/restart <name>` - Restart a specific account\n\
                    • `/lang [en|tr|ru]` - View or change language\n\
                    • `/logs` - Show recent system logs".to_string()
                }
                Language::Turkish => {
                    "🚀 *SteamBooster Telegram Kontrol Merkezi*\n\n\
                    📋 *Komutlar:*\n\
                    • `/status` - Tüm hesapların anlık durumunu ve kasılan saatleri gösterir\n\
                    • `/start_all` - Tüm hesaplarda saat kasmayı başlatır\n\
                    • `/stop_all` - Tüm hesaplarda saat kasmayı durdurur\n\
                    • `/restart_all` - Tüm hesapları yeniden başlatır\n\
                    • `/start <hesap_adı>` - Belirli bir hesabı başlatır\n\
                    • `/stop <hesap_adı>` - Belirli bir hesabı durdurur\n\
                    • `/restart <hesap_adı>` - Belirli bir hesabı yeniden başlatır\n\
                    • `/lang [en|tr|ru]` - Dili görüntüle veya değiştir\n\
                    • `/logs` - Son sistem loglarını listeler".to_string()
                }
                Language::Russian => {
                    "🚀 *SteamBooster Центр управления Telegram*\n\n\
                    📋 *Команды:*\n\
                    • `/status` - Показать статус и накрученные часы всех аккаунтов\n\
                    • `/start_all` - Запустить накрутку на всех аккаунтах\n\
                    • `/stop_all` - Остановить накрутку на всех аккаунтах\n\
                    • `/restart_all` - Перезапустить все аккаунты\n\
                    • `/start <имя>` - Запустить конкретный аккаунт\n\
                    • `/stop <имя>` - Остановить конкретный аккаунт\n\
                    • `/restart <имя>` - Перезапустить конкретный аккаунт\n\
                    • `/lang [en|tr|ru]` - Просмотреть или сменить язык\n\
                    • `/logs` - Показать последние системные логи".to_string()
                }
            }
        }

        "/lang" | "/language" => {
            if parts.len() == 1 {
                match lang {
                    Language::English => {
                        "🌐 *Current Language:* 🇬🇧 English\n\n\
                        To change language, send:\n\
                        • `/lang en` - 🇬🇧 English\n\
                        • `/lang tr` - 🇹🇷 Türkçe\n\
                        • `/lang ru` - 🇷🇺 Русский".to_string()
                    }
                    Language::Turkish => {
                        "🌐 *Mevcut Dil:* 🇹🇷 Türkçe\n\n\
                        Dili değiştirmek için gönderin:\n\
                        • `/lang en` - 🇬🇧 English\n\
                        • `/lang tr` - 🇹🇷 Türkçe\n\
                        • `/lang ru` - 🇷🇺 Русский".to_string()
                    }
                    Language::Russian => {
                        "🌐 *Текущий язык:* 🇷🇺 Русский\n\n\
                        Чтобы изменить язык, отправьте:\n\
                        • `/lang en` - 🇬🇧 English\n\
                        • `/lang tr` - 🇹🇷 Türkçe\n\
                        • `/lang ru` - 🇷🇺 Русский".to_string()
                    }
                }
            } else {
                let code = parts[1].to_lowercase();
                match code.as_str() {
                    "en" | "eng" | "english" => {
                        crate::data::update_language_and_save(Language::English);
                        "✅ Language switched to 🇬🇧 *English*.".to_string()
                    }
                    "tr" | "turk" | "turkish" | "turkce" | "türkçe" => {
                        crate::data::update_language_and_save(Language::Turkish);
                        "✅ Dil başarıyla 🇹🇷 *Türkçe* olarak değiştirildi.".to_string()
                    }
                    "ru" | "rus" | "russian" | "русский" => {
                        crate::data::update_language_and_save(Language::Russian);
                        "✅ Язык успешно изменен на 🇷🇺 *Русский*.".to_string()
                    }
                    _ => {
                        match lang {
                            Language::English => "❌ Invalid language! Options: `en`, `tr`, `ru`.".to_string(),
                            Language::Turkish => "❌ Geçersiz dil! Seçenekler: `en`, `tr`, `ru`.".to_string(),
                            Language::Russian => "❌ Неверный язык! Варианты: `en`, `tr`, `ru`.".to_string(),
                        }
                    }
                }
            }
        }

        "/status" => {
            let accounts = {
                let lock = accounts_arc.lock().unwrap();
                lock.clone()
            };

            if accounts.is_empty() {
                return match lang {
                    Language::English => "ℹ️ No registered accounts found.".to_string(),
                    Language::Turkish => "ℹ️ Kayıtlı hesap bulunamadı.".to_string(),
                    Language::Russian => "ℹ️ Зарегистрированных аккаунтов не найдено.".to_string(),
                };
            }

            let mut out = match lang {
                Language::English => format!("📊 *SteamBooster Accounts Status ({} Accounts):*\n\n", accounts.len()),
                Language::Turkish => format!("📊 *SteamBooster Hesap Durumları ({} Hesap):*\n\n", accounts.len()),
                Language::Russian => format!("📊 *Статус аккаунтов SteamBooster ({} аккаунтов):*\n\n", accounts.len()),
            };

            for arc in accounts {
                let a = arc.lock().unwrap();
                let status_icon = match (&a.status, lang) {
                    (AccountStatus::Boosting, Language::English) => "🟢 Boosting".to_string(),
                    (AccountStatus::Boosting, Language::Turkish) => "🟢 Kasıyor".to_string(),
                    (AccountStatus::Boosting, Language::Russian) => "🟢 Бустит".to_string(),

                    (AccountStatus::Connecting, Language::English) => "🟡 Connecting...".to_string(),
                    (AccountStatus::Connecting, Language::Turkish) => "🟡 Bağlanıyor".to_string(),
                    (AccountStatus::Connecting, Language::Russian) => "🟡 Подключение...".to_string(),

                    (AccountStatus::AwaitingSteamGuard { .. }, Language::English) => "🛡️ Guard Needed".to_string(),
                    (AccountStatus::AwaitingSteamGuard { .. }, Language::Turkish) => "🛡️ Guard Bekleniyor".to_string(),
                    (AccountStatus::AwaitingSteamGuard { .. }, Language::Russian) => "🛡️ Ожидание Guard".to_string(),

                    (AccountStatus::PausedForGame { .. }, Language::English) => "⏸️ In-Game (Paused)".to_string(),
                    (AccountStatus::PausedForGame { .. }, Language::Turkish) => "⏸️ Oyunda (Duraklatıldı)".to_string(),
                    (AccountStatus::PausedForGame { .. }, Language::Russian) => "⏸️ В игре (Пауза)".to_string(),

                    (AccountStatus::Reconnecting { .. }, Language::English) => "🔄 Reconnecting...".to_string(),
                    (AccountStatus::Reconnecting { .. }, Language::Turkish) => "🔄 Yeniden Bağlanıyor".to_string(),
                    (AccountStatus::Reconnecting { .. }, Language::Russian) => "🔄 Переподключение...".to_string(),

                    (AccountStatus::Error(e), Language::English) => format!("🔴 Error: {}", e),
                    (AccountStatus::Error(e), Language::Turkish) => format!("🔴 Hata: {}", e),
                    (AccountStatus::Error(e), Language::Russian) => format!("🔴 Ошибка: {}", e),

                    (AccountStatus::Idle, Language::English) => "⚪ Stopped".to_string(),
                    (AccountStatus::Idle, Language::Turkish) => "⚪ Durduruldu".to_string(),
                    (AccountStatus::Idle, Language::Russian) => "⚪ Остановлено".to_string(),
                };

                let line = match lang {
                    Language::English => format!(
                        "👤 *{}* | {}\n🎮 {} games | ⏱ {:.1} hrs\n🏷️ Title: {}\n\n",
                        a.username, status_icon, a.games.len(), a.hours_boosted, a.custom_title
                    ),
                    Language::Turkish => format!(
                        "👤 *{}* | {}\n🎮 {} oyun | ⏱ {:.1} saat\n🏷️ Başlık: {}\n\n",
                        a.username, status_icon, a.games.len(), a.hours_boosted, a.custom_title
                    ),
                    Language::Russian => format!(
                        "👤 *{}* | {}\n🎮 {} игр | ⏱ {:.1} ч.\n🏷️ Заголовок: {}\n\n",
                        a.username, status_icon, a.games.len(), a.hours_boosted, a.custom_title
                    ),
                };
                out.push_str(&line);
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
            match lang {
                Language::English => format!("✅ Hour boosting started for {} accounts!", accounts.len()),
                Language::Turkish => format!("✅ {} hesap için saat kasma başlatıldı!", accounts.len()),
                Language::Russian => format!("✅ Накрутка запущена для {} аккаунтов!", accounts.len()),
            }
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
            match lang {
                Language::English => format!("⏹️ Stopped boosting for {} accounts.", accounts.len()),
                Language::Turkish => format!("⏹️ {} hesap durduruldu.", accounts.len()),
                Language::Russian => format!("⏹️ Накрутка остановлена для {} аккаунтов.", accounts.len()),
            }
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
            match lang {
                Language::English => format!("🔄 Restarting {} accounts...", accounts.len()),
                Language::Turkish => format!("🔄 {} hesap yeniden başlatılıyor...", accounts.len()),
                Language::Russian => format!("🔄 Перезапуск {} аккаунтов...", accounts.len()),
            }
        }

        "/logs" => {
            let logs = logger::get_logs();
            let recent: Vec<_> = logs.iter().rev().take(12).collect();
            if recent.is_empty() {
                return match lang {
                    Language::English => "📜 No log entries found.".to_string(),
                    Language::Turkish => "📜 Log kaydı bulunmuyor.".to_string(),
                    Language::Russian => "📜 Записей в логах нет.".to_string(),
                };
            }
            let mut out = match lang {
                Language::English => "📜 *Recent System Logs:*\n".to_string(),
                Language::Turkish => "📜 *Son Sistem Logları:*\n".to_string(),
                Language::Russian => "📜 *Последние системные логи:*\n".to_string(),
            };
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
                    return match lang {
                        Language::English => format!("✅ Account `{}` started.", target),
                        Language::Turkish => format!("✅ `{}` hesabı başlatıldı.", target),
                        Language::Russian => format!("✅ Аккаунт `{}` запущен.", target),
                    };
                }
            }
            match lang {
                Language::English => format!("❌ Account `{}` not found.", target),
                Language::Turkish => format!("❌ `{}` isimli hesap bulunamadı.", target),
                Language::Russian => format!("❌ Аккаунт `{}` не найден.", target),
            }
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
                    return match lang {
                        Language::English => format!("⏹️ Account `{}` stopped.", target),
                        Language::Turkish => format!("⏹️ `{}` hesabı durduruldu.", target),
                        Language::Russian => format!("⏹️ Аккаунт `{}` остановлен.", target),
                    };
                }
            }
            match lang {
                Language::English => format!("❌ Account `{}` not found.", target),
                Language::Turkish => format!("❌ `{}` isimli hesap bulunamadı.", target),
                Language::Russian => format!("❌ Аккаунт `{}` не найден.", target),
            }
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
                    return match lang {
                        Language::English => format!("🔄 Account `{}` restarting...", target),
                        Language::Turkish => format!("🔄 `{}` hesabı yeniden başlatılıyor...", target),
                        Language::Russian => format!("🔄 Перезапуск аккаунта `{}`...", target),
                    };
                }
            }
            match lang {
                Language::English => format!("❌ Account `{}` not found.", target),
                Language::Turkish => format!("❌ `{}` isimli hesap bulunamadı.", target),
                Language::Russian => format!("❌ Аккаунт `{}` не найден.", target),
            }
        }

        _ => match lang {
            Language::English => "❓ Unknown command. Type `/help` for available commands.".to_string(),
            Language::Turkish => "❓ Bilinmeyen komut. Komut listesi için `/help` yazabilirsiniz.".to_string(),
            Language::Russian => "❓ Неизвестная команда. Введите `/help` для списка команд.".to_string(),
        },
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
