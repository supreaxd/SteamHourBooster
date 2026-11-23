use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{OnceLock, RwLock};
use crate::i18n::Language;
use crate::crypto::{decrypt_string, encrypt_string};

static GLOBAL_SETTINGS: OnceLock<RwLock<AppSettings>> = OnceLock::new();

pub fn get_global_settings() -> AppSettings {
    GLOBAL_SETTINGS
        .get_or_init(|| RwLock::new(AppSettings::default()))
        .read()
        .unwrap()
        .clone()
}

pub fn update_global_settings(settings: &AppSettings) {
    let lock = GLOBAL_SETTINGS.get_or_init(|| RwLock::new(settings.clone()));
    if let Ok(mut w) = lock.write() {
        *w = settings.clone();
    }
}

pub fn update_language_and_save(new_lang: Language) {
    let mut config = AppConfig::load();
    config.settings.language = new_lang;
    config.save();
}

pub fn save_web_app_url(url: &str) {
    let mut config = AppConfig::load();
    config.settings.web_app_url = url.trim().to_string();
    config.save();
}

pub fn urlencoding_encode(s: &str) -> String {
    let mut result = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.' || b == b'~' {
            result.push(b as char);
        } else {
            result.push_str(&format!("%{:02X}", b));
        }
    }
    result
}

pub const POPULAR_GAMES: &[(&str, u32)] = &[
    ("Counter-Strike 2", 730),
    ("Dota 2", 570),
    ("Team Fortress 2", 440),
    ("Terraria", 105600),
    ("Grand Theft Auto V", 271590),
    ("Rust", 252490),
    ("Apex Legends", 1172470),
    ("Valheim", 892970),
    ("Stardew Valley", 413150),
    ("Among Us", 945360),
    ("Cyberpunk 2077", 1091500),
    ("Elden Ring", 1245620),
    ("RimWorld", 294100),
    ("Hollow Knight", 367520),
    ("Dead by Daylight", 381210),
    ("Path of Exile", 238960),
    ("Monster Hunter: World", 582010),
    ("PUBG: BATTLEGROUNDS", 578080),
    ("The Forest", 242760),
    ("No Man's Sky", 275850),
    ("Subnautica", 264710),
    ("Hades", 1145360),
    ("Deep Rock Galactic", 548430),
    ("Risk of Rain 2", 632360),
    ("Satisfactory", 526870),
    ("Baldur's Gate 3", 1086940),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PersonaState {
    #[default]
    Online,     // 1: Çevrimiçi (Aktif)
    Invisible,  // 7: Görünmez
    Away,       // 3: Uzakta
    Busy,       // 2: Meşgul
}

impl PersonaState {
    pub fn to_u32(self) -> u32 {
        match self {
            PersonaState::Online => 1,
            PersonaState::Invisible => 7,
            PersonaState::Away => 3,
            PersonaState::Busy => 2,
        }
    }

    #[allow(dead_code)]
    pub fn label(self) -> &'static str {
        match self {
            PersonaState::Online => "🟢 Çevrimiçi",
            PersonaState::Invisible => "👻 Görünmez",
            PersonaState::Away => "🟡 Uzakta",
            PersonaState::Busy => "🔴 Meşgul",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameEntry {
    pub app_id: u32,
    pub name: String,
    #[serde(default)]
    pub target_hours: Option<f64>,
}

impl GameEntry {
    pub fn new(app_id: u32, name: impl Into<String>) -> Self {
        Self {
            app_id,
            name: name.into(),
            target_hours: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum AccountStatus {
    Idle,
    Connecting,
    AwaitingSteamGuard { hint: String },
    Boosting,
    PausedForGame { app_id: u32, app_name: String },
    Reconnecting { attempt: u32, retry_in_secs: u64 },
    Error(String),
}

impl Default for AccountStatus {
    fn default() -> Self {
        AccountStatus::Idle
    }
}

impl AccountStatus {
    #[allow(dead_code)]
    pub fn label(&self) -> &str {
        match self {
            AccountStatus::Idle => "Durduruldu",
            AccountStatus::Connecting => "Bağlanıyor...",
            AccountStatus::AwaitingSteamGuard { .. } => "🛡️ Steam Guard Kodu Gerekli",
            AccountStatus::Boosting => "Kasıyor",
            AccountStatus::PausedForGame { .. } => "⏸️ Duraklatıldı (Oyundasınız)",
            AccountStatus::Reconnecting { .. } => "🔄 Yeniden Bağlanıyor...",
            AccountStatus::Error(_) => "Hata",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: u64,
    pub username: String,
    pub password: String,
    pub custom_title: String,
    pub games: Vec<GameEntry>,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub shared_secret: Option<String>,
    #[serde(default)]
    pub persona_state: PersonaState,
    #[serde(skip)]
    pub status: AccountStatus,
    pub hours_boosted: f64,
    #[serde(default)]
    pub afk_reply_custom: Option<String>,
    #[serde(default)]
    pub proxy: Option<String>,
}

impl Account {
    pub fn new(id: u64) -> Self {
        Self {
            id,
            username: String::new(),
            password: String::new(),
            custom_title: String::from("Half-Life 3 (Beta)"),
            games: Vec::new(),
            refresh_token: None,
            shared_secret: None,
            persona_state: PersonaState::Online,
            status: AccountStatus::Idle,
            hours_boosted: 0.0,
            afk_reply_custom: None,
            proxy: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    #[serde(default)]
    pub language: Language,
    #[serde(default)]
    pub autostart: bool,
    #[serde(default)]
    pub start_minimized: bool,
    #[serde(default = "default_true")]
    pub minimize_to_tray: bool,
    #[serde(default)]
    pub auto_start_boost_on_launch: bool,
    #[serde(default = "default_true")]
    pub auto_pause_game: bool,
    #[serde(default = "default_true")]
    pub encrypt_credentials: bool,
    #[serde(default)]
    pub enable_scheduler: bool,
    #[serde(default = "default_scheduler_start")]
    pub scheduler_start_hour: u8,
    #[serde(default = "default_scheduler_end")]
    pub scheduler_end_hour: u8,
    #[serde(default = "default_true")]
    pub afk_auto_reply: bool,
    #[serde(default = "default_afk_message")]
    pub afk_reply_message: String,
    #[serde(default = "default_cooldown_mins")]
    pub afk_cooldown_mins: u32,
    #[serde(default = "default_true")]
    pub card_drop_tracking: bool,

    // Yeni Gelişmiş Özellikler
    #[serde(default = "default_ui_scale")]
    pub ui_scale: f32,
    #[serde(default = "default_true")]
    pub simultaneous_multi_game: bool,
    #[serde(default)]
    pub title_rotation: bool,
    #[serde(default = "default_rotation_interval")]
    pub title_rotation_interval_secs: u32,
    #[serde(default)]
    pub title_rotation_list: Vec<String>,
    #[serde(default)]
    pub auto_claim_free_games: bool,
    #[serde(default = "default_claim_interval")]
    pub auto_claim_interval_mins: u32,
    #[serde(default = "default_true")]
    pub inventory_tracking: bool,
    #[serde(default)]
    pub telegram_enabled: bool,
    #[serde(default)]
    pub telegram_bot_token: String,
    #[serde(default)]
    pub telegram_chat_id: String,
    #[serde(default = "default_true")]
    pub telegram_notify_cards: bool,
    #[serde(default = "default_true")]
    pub telegram_notify_afk: bool,
    #[serde(default = "default_true")]
    pub telegram_notify_status: bool,
    #[serde(default = "default_true")]
    pub dev_debug_mode: bool,
    #[serde(default = "default_web_app_port")]
    pub web_app_port: u16,
    #[serde(default)]
    pub web_app_url: String,
}

fn default_web_app_port() -> u16 {
    8080
}

fn default_true() -> bool {
    true
}

fn default_ui_scale() -> f32 {
    1.0
}

fn default_rotation_interval() -> u32 {
    60
}

fn default_claim_interval() -> u32 {
    120
}

fn default_afk_message() -> String {
    "Currently boosting hours, will reply when available.".to_string()
}

fn default_cooldown_mins() -> u32 {
    30
}

fn default_scheduler_start() -> u8 {
    2
}

fn default_scheduler_end() -> u8 {
    9
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            language: Language::English,
            autostart: false,
            start_minimized: false,
            minimize_to_tray: true,
            auto_start_boost_on_launch: false,
            auto_pause_game: true,
            encrypt_credentials: true,
            enable_scheduler: false,
            scheduler_start_hour: 2,
            scheduler_end_hour: 9,
            afk_auto_reply: true,
            afk_reply_message: default_afk_message(),
            afk_cooldown_mins: 30,
            card_drop_tracking: true,
            ui_scale: 1.0,
            simultaneous_multi_game: true,
            title_rotation: false,
            title_rotation_interval_secs: 60,
            title_rotation_list: vec![
                "Half-Life 3 (Beta)".into(),
                "Total Boosted: {hours}h".into(),
                "⚡ SteamBooster Active".into(),
            ],
            auto_claim_free_games: false,
            auto_claim_interval_mins: 120,
            inventory_tracking: true,
            telegram_enabled: false,
            telegram_bot_token: String::new(),
            telegram_chat_id: String::new(),
            telegram_notify_cards: true,
            telegram_notify_afk: true,
            telegram_notify_status: true,
            dev_debug_mode: true,
            web_app_port: 8080,
            web_app_url: String::new(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct AppConfig {
    pub accounts: Vec<Account>,
    #[serde(default)]
    pub settings: AppSettings,
}

impl AppConfig {
    pub fn config_path() -> PathBuf {
        let base_dir = if let Some(appdata) = std::env::var_os("APPDATA") {
            PathBuf::from(appdata).join("SteamBooster")
        } else if let Some(home) = std::env::var_os("USERPROFILE") {
            PathBuf::from(home).join(".steambooster")
        } else {
            let exe = std::env::current_exe().unwrap_or_default();
            exe.parent().unwrap_or_else(|| std::path::Path::new(".")).to_path_buf()
        };
        let _ = std::fs::create_dir_all(&base_dir);
        base_dir.join("booster_config.json")
    }

    pub fn load() -> Self {
        let path = Self::config_path();

        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(mut config) = serde_json::from_str::<AppConfig>(&content) {
                    // Şifrelenmiş verileri DPAPI ile çöz
                    for acc in &mut config.accounts {
                        acc.password = decrypt_string(&acc.password);
                        if let Some(ref t) = acc.refresh_token {
                            acc.refresh_token = Some(decrypt_string(t));
                        }
                        if let Some(ref s) = acc.shared_secret {
                            acc.shared_secret = Some(decrypt_string(s));
                        }
                    }
                    update_global_settings(&config.settings);
                    return config;
                }
            }
        }
        let def = Self::default();
        update_global_settings(&def.settings);
        def
    }

    pub fn save(&self) {
        update_global_settings(&self.settings);
        let path = Self::config_path();
        let mut to_save = AppConfig {
            accounts: self.accounts.clone(),
            settings: self.settings.clone(),
        };

        // Eğer DPAPI şifreleme aktifse hassas alanları şifrele
        if self.settings.encrypt_credentials {
            for acc in &mut to_save.accounts {
                if !acc.password.is_empty() {
                    acc.password = encrypt_string(&acc.password);
                }
                if let Some(ref t) = acc.refresh_token {
                    acc.refresh_token = Some(encrypt_string(t));
                }
                if let Some(ref s) = acc.shared_secret {
                    acc.shared_secret = Some(encrypt_string(s));
                }
            }
        }

        if let Ok(json) = serde_json::to_string_pretty(&to_save) {
            let _ = std::fs::write(path, json);
        }
    }
}
