use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use eframe::egui::{self, Color32, FontId, RichText, Stroke, Ui, Vec2};

use crate::autostart;
use crate::data::{Account, AccountStatus, AppConfig, GameEntry, PersonaState, POPULAR_GAMES};
use crate::i18n::{get_strings, Language, Strings};
use crate::logger::{self, LogEntry, LogLevel};
use crate::steam;

#[derive(PartialEq, Clone, Copy)]
enum Tab {
    Accounts,
    Dashboard,
    Settings,
    Logs,
    Debug,
}

pub struct SteamBoosterApp {
    config: AppConfig,
    active_tab: Tab,

    // Hesap düzenleme
    editing_account: Option<Account>,
    is_adding: bool,

    // Oyun arama & Store API
    game_search: String,
    last_store_search_query: String,
    store_search_results: Arc<Mutex<Vec<(u32, String)>>>,
    store_search_loading: Arc<AtomicBool>,

    // Manuel AppID ekleme
    custom_app_id_input: String,
    custom_game_name_input: String,
    app_lookup_result: Arc<Mutex<Option<(u32, String)>>>,
    app_lookup_loading: bool,
    app_lookup_msg: Option<String>,

    // Steam Guard geçici kod girdileri
    guard_inputs: HashMap<u64, String>,

    // QR Girişi
    qr_auth: crate::qr_auth::QrAuthManager,
    show_qr_modal: bool,

    // Runtime hesap referansları
    runtime_accounts: Vec<Arc<Mutex<Account>>>,
    next_id: u64,

    // Ayarlar sekmesi önbelleği
    autostart_run_key: bool,
    autostart_minimized: bool,
    autostart_task_service: bool,

    // Log sekmesi filtresi
    log_filter_level: Option<LogLevel>,

    // Debug sekmesi filtresi
    debug_filter_category: Option<crate::debug_log::PacketCategory>,
    debug_search_query: String,

    // Sistem Tepsisi (Tray)
    tray_mgr: Option<crate::tray::TrayManager>,
}

impl SteamBoosterApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let config = AppConfig::load();
        let next_id = config.accounts.iter().map(|a| a.id).max().unwrap_or(0) + 1;
        let runtime_accounts: Vec<Arc<Mutex<Account>>> = config
            .accounts
            .iter()
            .cloned()
            .map(|a| Arc::new(Mutex::new(a)))
            .collect();

        let autostart_run_key = autostart::is_registry_autostart_enabled();
        let autostart_minimized = config.settings.minimize_to_tray;
        let autostart_task_service = autostart::is_task_scheduler_enabled();

        // Telegram Bot servisini kaydet ve başlat
        crate::telegram::register_accounts_for_bot(runtime_accounts.clone());
        crate::telegram::init_telegram_bot();

        // Sistem Tepsisi (Tray) yöneticisini başlat
        let mut tray_mgr = crate::tray::TrayManager::new();
        if let Some(tray) = &mut tray_mgr {
            tray.update_status(&runtime_accounts);
        }

        // Açılışta otomatik boost başlatma aktifse tüm hesapları başlat
        if config.settings.auto_start_boost_on_launch {
            for acc in &runtime_accounts {
                steam::start_boost(acc.clone());
            }
            logger::log_info("Auto-start: All accounts started boosting automatically on launch.");
        }

        logger::log_info(&format!("SteamBooster started. {} accounts loaded.", config.accounts.len()));

        Self {
            config,
            active_tab: Tab::Accounts,
            editing_account: None,
            is_adding: false,
            game_search: String::new(),
            last_store_search_query: String::new(),
            store_search_results: Arc::new(Mutex::new(Vec::new())),
            store_search_loading: Arc::new(AtomicBool::new(false)),
            custom_app_id_input: String::new(),
            custom_game_name_input: String::new(),
            app_lookup_result: Arc::new(Mutex::new(None)),
            app_lookup_loading: false,
            app_lookup_msg: None,
            guard_inputs: HashMap::new(),
            qr_auth: crate::qr_auth::QrAuthManager::new(),
            show_qr_modal: false,
            runtime_accounts,
            next_id,
            autostart_run_key,
            autostart_minimized,
            autostart_task_service,
            log_filter_level: None,
            debug_filter_category: None,
            debug_search_query: String::new(),
            tray_mgr,
        }
    }

    fn check_store_search(&mut self) {
        let query = self.game_search.trim().to_string();
        if query == self.last_store_search_query {
            return;
        }
        self.last_store_search_query = query.clone();

        if query.len() < 2 {
            self.store_search_results.lock().unwrap().clear();
            self.store_search_loading.store(false, Ordering::Relaxed);
            return;
        }

        self.store_search_loading.store(true, Ordering::Relaxed);
        let results_arc = self.store_search_results.clone();
        let loading_arc = self.store_search_loading.clone();

        steam::get_runtime().spawn(async move {
            let encoded = urlencoding_encode(&query);
            let url = format!(
                "https://store.steampowered.com/api/storesearch/?term={}&l=turkish&cc=TR",
                encoded
            );

            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(4))
                .build()
                .unwrap_or_default();

            let mut fetched_games = Vec::new();
            if let Ok(resp) = client.get(&url).send().await {
                if let Ok(json) = resp.json::<serde_json::Value>().await {
                    if let Some(items) = json.get("items").and_then(|i| i.as_array()) {
                        for item in items.iter().take(15) {
                            if let (Some(id), Some(name)) = (
                                item.get("id").and_then(|id| id.as_u64()),
                                item.get("name").and_then(|n| n.as_str()),
                            ) {
                                fetched_games.push((id as u32, name.to_string()));
                            }
                        }
                    }
                }
            }

            *results_arc.lock().unwrap() = fetched_games;
            loading_arc.store(false, Ordering::Relaxed);
        });
    }

    fn check_and_lookup_appid(&mut self) {
        let trimmed = self.custom_app_id_input.trim();
        if let Ok(id) = trimmed.parse::<u32>() {
            if let Some((name, _)) = POPULAR_GAMES.iter().find(|(_, aid)| *aid == id) {
                self.custom_game_name_input = name.to_string();
                self.app_lookup_msg = Some(format!("Found: {}", name));
                self.app_lookup_loading = false;
                return;
            }

            self.app_lookup_loading = true;
            self.app_lookup_msg = Some("Querying Steam Store API...".to_string());
            let result_arc = self.app_lookup_result.clone();
            steam::get_runtime().spawn(async move {
                let client = reqwest::Client::builder()
                    .timeout(std::time::Duration::from_secs(5))
                    .build()
                    .unwrap_or_default();
                let url = format!(
                    "https://store.steampowered.com/api/appdetails?appids={}&filters=basic",
                    id
                );
                if let Ok(resp) = client.get(&url).send().await {
                    if let Ok(json) = resp.json::<serde_json::Value>().await {
                        let id_str = id.to_string();
                        if let Some(app_data) = json.get(&id_str) {
                            if app_data.get("success").and_then(|s| s.as_bool()).unwrap_or(false) {
                                if let Some(name) = app_data
                                    .get("data")
                                    .and_then(|d| d.get("name"))
                                    .and_then(|n| n.as_str())
                                {
                                    let mut res = result_arc.lock().unwrap();
                                    *res = Some((id, name.to_string()));
                                    return;
                                }
                            }
                        }
                    }
                }
                let mut res = result_arc.lock().unwrap();
                *res = Some((id, String::new()));
            });
        } else {
            self.app_lookup_loading = false;
            self.app_lookup_msg = None;
        }
    }

    fn sync_from_runtime(&mut self) {
        let mut save_needed = false;
        for arc in &self.runtime_accounts {
            let rt = arc.lock().unwrap();
            if let Some(acc) = self.config.accounts.iter_mut().find(|a| a.id == rt.id) {
                acc.status = rt.status.clone();
                acc.hours_boosted = rt.hours_boosted;
                if acc.refresh_token != rt.refresh_token {
                    acc.refresh_token = rt.refresh_token.clone();
                    save_needed = true;
                }
                if acc.persona_state != rt.persona_state {
                    acc.persona_state = rt.persona_state;
                    save_needed = true;
                }
            }
        }
        if save_needed {
            self.config.save();
        }
    }

    fn draw_sidebar(&mut self, ui: &mut Ui, s: &Strings) {
        ui.add_space(16.0);
        ui.vertical_centered(|ui| {
            ui.label(
                RichText::new("⚡ SteamBooster")
                    .font(FontId::proportional(20.0))
                    .color(Color32::from_rgb(100, 180, 255)),
            );
        });
        ui.add_space(20.0);
        ui.separator();
        ui.add_space(12.0);

        let tabs = [
            (s.tab_accounts, Tab::Accounts),
            (s.tab_dashboard, Tab::Dashboard),
            (s.tab_settings, Tab::Settings),
            (s.tab_logs, Tab::Logs),
            (s.tab_debug, Tab::Debug),
        ];

        for (label, tab) in tabs {
            let selected = self.active_tab == tab;
            let bg = if selected {
                Color32::from_rgb(40, 80, 140)
            } else {
                Color32::TRANSPARENT
            };
            let text_color = if selected {
                Color32::WHITE
            } else {
                Color32::from_gray(180)
            };
            let btn = egui::Button::new(RichText::new(label).color(text_color).size(14.0))
                .fill(bg)
                .min_size(Vec2::new(165.0, 36.0));
            if ui.add(btn).clicked() {
                self.active_tab = tab;
            }
            ui.add_space(4.0);
        }

        ui.add_space(16.0);
        ui.separator();
        ui.add_space(8.0);

        let boosting_count = self
            .config
            .accounts
            .iter()
            .filter(|a| matches!(a.status, AccountStatus::Boosting))
            .count();
        let guard_count = self
            .config
            .accounts
            .iter()
            .filter(|a| matches!(a.status, AccountStatus::AwaitingSteamGuard { .. }))
            .count();
        let idle_count = self.config.accounts.len().saturating_sub(boosting_count + guard_count);

        ui.label(
            RichText::new(format!("{} {}", s.active_accounts, boosting_count))
                .color(Color32::from_rgb(80, 200, 100))
                .size(12.5),
        );
        if guard_count > 0 {
            ui.label(
                RichText::new(format!("{} {}", s.guard_pending, guard_count))
                    .color(Color32::from_rgb(255, 180, 50))
                    .size(12.5),
            );
        }
        ui.label(
            RichText::new(format!("{} {}", s.stopped, idle_count))
                .color(Color32::from_gray(150))
                .size(12.5),
        );
    }

    fn draw_accounts_tab(&mut self, ui: &mut Ui, s: &Strings) {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new(s.tab_accounts).font(FontId::proportional(22.0)));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if self.editing_account.is_none() {
                    if ui
                        .button(RichText::new(s.btn_manual_add).color(Color32::from_rgb(100, 200, 100)))
                        .clicked()
                    {
                        let mut new_acc = Account::new(self.next_id);
                        self.next_id += 1;
                        new_acc.custom_title = "Half-Life 3 (Beta)".into();
                        self.editing_account = Some(new_acc);
                        self.is_adding = true;
                        self.game_search.clear();
                        self.last_store_search_query.clear();
                        self.store_search_results.lock().unwrap().clear();
                        self.custom_app_id_input.clear();
                        self.custom_game_name_input.clear();
                        self.app_lookup_msg = None;
                    }
                    if ui
                        .button(RichText::new(s.btn_qr_add).color(Color32::from_rgb(100, 180, 255)))
                        .clicked()
                    {
                        self.show_qr_modal = true;
                        self.qr_auth.start();
                    }
                    if ui
                        .button(RichText::new(s.btn_restart_all).color(Color32::from_rgb(255, 180, 60)))
                        .clicked()
                    {
                        logger::log_info("Restarting all accounts...");
                        for arc in &self.runtime_accounts {
                            let id = arc.lock().unwrap().id;
                            steam::restart_boost(id, arc.clone());
                        }
                    }
                }
            });
        });
        ui.separator();
        ui.add_space(8.0);

        if let Some(edit_acc) = self.editing_account.clone() {
            self.draw_edit_panel(ui, edit_acc, s);
        } else {
            self.draw_account_cards(ui, s);
        }
    }

    fn draw_edit_panel(&mut self, ui: &mut Ui, mut edit_acc: Account, s: &Strings) {
        let title = if self.is_adding { s.edit_title_new } else { s.edit_title_edit };
        ui.label(RichText::new(title).font(FontId::proportional(17.0)));
        ui.add_space(8.0);

        egui::Grid::new("edit_grid")
            .num_columns(2)
            .spacing([14.0, 8.0])
            .show(ui, |ui| {
                ui.label(s.label_username);
                ui.text_edit_singleline(&mut edit_acc.username);
                ui.end_row();

                ui.label(s.label_password);
                ui.add(egui::TextEdit::singleline(&mut edit_acc.password).password(true));
                ui.end_row();

                ui.label(s.label_custom_title);
                ui.vertical(|ui| {
                    ui.text_edit_singleline(&mut edit_acc.custom_title);
                    ui.label(
                        RichText::new(s.label_custom_title_hint)
                            .size(11.0)
                            .color(Color32::from_rgb(140, 170, 220)),
                    );
                });
                ui.end_row();

                ui.label(s.label_persona_status);
                egui::ComboBox::from_id_salt("edit_persona_combo")
                    .selected_text(match edit_acc.persona_state {
                        PersonaState::Online => s.persona_online,
                        PersonaState::Invisible => s.persona_invisible,
                        PersonaState::Away => s.persona_away,
                        PersonaState::Busy => s.persona_busy,
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut edit_acc.persona_state, PersonaState::Online, s.persona_online);
                        ui.selectable_value(&mut edit_acc.persona_state, PersonaState::Invisible, s.persona_invisible);
                        ui.selectable_value(&mut edit_acc.persona_state, PersonaState::Away, s.persona_away);
                        ui.selectable_value(&mut edit_acc.persona_state, PersonaState::Busy, s.persona_busy);
                    });
                ui.end_row();

                ui.label(s.label_shared_secret);
                let mut secret_text = edit_acc.shared_secret.clone().unwrap_or_default();
                if ui.text_edit_singleline(&mut secret_text).changed() {
                    edit_acc.shared_secret = if secret_text.trim().is_empty() {
                        None
                    } else {
                        Some(secret_text.trim().to_string())
                    };
                }
                ui.end_row();

                ui.label(s.label_saved_token);
                ui.vertical(|ui| {
                    if edit_acc.refresh_token.is_some() {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(s.label_token_present).color(Color32::from_rgb(80, 200, 100)),
                            );
                            if ui.small_button(s.btn_clear_token).clicked() {
                                edit_acc.refresh_token = None;
                            }
                        });
                    } else {
                        ui.label(
                            RichText::new(s.label_no_token)
                                .color(Color32::from_gray(140))
                                .size(11.5),
                        );
                    }
                });
                ui.end_row();

                ui.label(s.label_custom_afk_reply);
                let mut afk_custom_text = edit_acc.afk_reply_custom.clone().unwrap_or_default();
                let afk_resp = ui.add(
                    egui::TextEdit::singleline(&mut afk_custom_text)
                        .hint_text(s.afk_custom_hint),
                );
                if afk_resp.changed() {
                    edit_acc.afk_reply_custom = if afk_custom_text.trim().is_empty() {
                        None
                    } else {
                        Some(afk_custom_text)
                    };
                }
                ui.end_row();

                ui.label(s.sec_proxy);
                let mut proxy_text = edit_acc.proxy.clone().unwrap_or_default();
                let proxy_resp = ui.add(
                    egui::TextEdit::singleline(&mut proxy_text)
                        .hint_text("socks5://127.0.0.1:1080 veya http://host:port"),
                );
                if proxy_resp.changed() {
                    edit_acc.proxy = if proxy_text.trim().is_empty() {
                        None
                    } else {
                        Some(proxy_text.trim().to_string())
                    };
                }
                ui.end_row();
            });

        ui.add_space(12.0);
        ui.label(
            RichText::new(format!("🎮 {}", s.label_games_to_boost))
                .size(14.0)
                .color(Color32::from_rgb(180, 180, 255)),
        );
        ui.add_space(4.0);

        // Seçilen Oyunlar Listesi
        let mut to_remove: Option<usize> = None;
        egui::ScrollArea::vertical()
            .id_salt("selected_games")
            .max_height(85.0)
            .show(ui, |ui| {
                if edit_acc.games.is_empty() {
                    ui.label(RichText::new(s.no_accounts).color(Color32::from_gray(130)).size(12.0));
                } else {
                    for (i, game) in edit_acc.games.iter().enumerate() {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!("🎮 {} ({})", game.name, game.app_id))
                                    .color(Color32::from_gray(220)),
                            );
                            if ui.small_button("❌").clicked() {
                                to_remove = Some(i);
                            }
                        });
                    }
                }
            });
        if let Some(idx) = to_remove {
            edit_acc.games.remove(idx);
        }

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);

        // Canlı Oyun Arama (Steam Store API + Yerel Filtreleme)
        ui.horizontal(|ui| {
            ui.label(RichText::new(s.search_store_hint).size(13.0).color(Color32::from_rgb(100, 180, 255)));
            if self.store_search_loading.load(Ordering::Relaxed) {
                ui.spinner();
            }
        });

        let search_response = ui.add(
            egui::TextEdit::singleline(&mut self.game_search)
                .hint_text("E.g.: Counter-Strike, Witcher, Rust...")
                .desired_width(ui.available_width() - 30.0),
        );
        if search_response.changed() {
            self.check_store_search();
        }

        // Arama Sonuçları Listesi
        let search_lower = self.game_search.to_lowercase().trim().to_string();
        let store_results = self.store_search_results.lock().unwrap().clone();

        egui::ScrollArea::vertical()
            .id_salt("game_search_list")
            .max_height(130.0)
            .show(ui, |ui| {
                if !store_results.is_empty() {
                    ui.label(RichText::new(s.store_results_label).size(11.5).color(Color32::from_rgb(140, 200, 140)));
                    for (app_id, name) in &store_results {
                        let already = edit_acc.games.iter().any(|g| g.app_id == *app_id);
                        ui.horizontal(|ui| {
                            let btn_label = if already {
                                format!("✓ {} ({})", name, app_id)
                            } else {
                                format!("＋ {} ({})", name, app_id)
                            };
                            let btn_color = if already {
                                Color32::from_rgb(80, 160, 80)
                            } else {
                                Color32::from_gray(210)
                            };
                            if ui.button(RichText::new(btn_label).color(btn_color).size(12.5)).clicked() && !already {
                                edit_acc.games.push(GameEntry::new(*app_id, name.clone()));
                            }
                        });
                    }
                    ui.separator();
                }

                ui.label(RichText::new(format!("⭐ {}:", s.label_popular_games)).size(11.5).color(Color32::from_gray(160)));
                for (name, app_id) in POPULAR_GAMES {
                    if search_lower.is_empty()
                        || name.to_lowercase().contains(&search_lower)
                        || app_id.to_string().contains(&search_lower)
                    {
                        let already = edit_acc.games.iter().any(|g| g.app_id == *app_id);
                        ui.horizontal(|ui| {
                            let btn_label = if already {
                                format!("✓ {} ({})", name, app_id)
                            } else {
                                format!("＋ {} ({})", name, app_id)
                            };
                            let btn_color = if already {
                                Color32::from_rgb(80, 160, 80)
                            } else {
                                Color32::from_gray(200)
                            };
                            if ui.button(RichText::new(btn_label).color(btn_color).size(12.5)).clicked() && !already {
                                edit_acc.games.push(GameEntry::new(*app_id, name.to_string()));
                            }
                        });
                    }
                }
            });

        ui.add_space(8.0);
        // Manuel AppID Girişi
        ui.horizontal(|ui| {
            ui.label(RichText::new(s.label_custom_appid).size(12.5).color(Color32::from_gray(180)));
            let resp = ui.add(
                egui::TextEdit::singleline(&mut self.custom_app_id_input)
                    .desired_width(70.0)
                    .hint_text("730"),
            );
            if resp.changed() {
                self.check_and_lookup_appid();
            }

            ui.add(
                egui::TextEdit::singleline(&mut self.custom_game_name_input)
                    .desired_width(140.0)
                    .hint_text("Otomatik bulunur..."),
            );

            if self.app_lookup_loading {
                ui.spinner();
            }

            if ui
                .button(RichText::new(s.btn_add_game).color(Color32::from_rgb(100, 220, 100)))
                .clicked()
            {
                if let Ok(id) = self.custom_app_id_input.trim().parse::<u32>() {
                    let name = if self.custom_game_name_input.trim().is_empty() {
                        format!("Oyun {}", id)
                    } else {
                        self.custom_game_name_input.trim().to_string()
                    };
                    if !edit_acc.games.iter().any(|g| g.app_id == id) {
                        edit_acc.games.push(GameEntry::new(id, name));
                        self.custom_app_id_input.clear();
                        self.custom_game_name_input.clear();
                        self.app_lookup_msg = None;
                    }
                }
            }
        });

        if let Some(ref msg) = self.app_lookup_msg {
            ui.label(RichText::new(msg).color(Color32::from_rgb(120, 220, 140)).size(11.0));
        }

        let mut close_edit = false;
        let saved = edit_acc.clone();
        ui.add_space(14.0);
        ui.horizontal(|ui| {
            if ui
                .button(RichText::new(s.btn_save).color(Color32::from_rgb(100, 200, 100)).size(14.0))
                .clicked()
            {
                if self.is_adding {
                    let arc = Arc::new(Mutex::new(saved.clone()));
                    self.runtime_accounts.push(arc);
                    self.config.accounts.push(saved.clone());
                    logger::log_success(&format!("New account added: {}", saved.username));
                } else {
                    if let Some(acc) = self.config.accounts.iter_mut().find(|a| a.id == saved.id) {
                        acc.username = saved.username.clone();
                        acc.password = saved.password.clone();
                        acc.custom_title = saved.custom_title.clone();
                        acc.persona_state = saved.persona_state;
                        acc.games = saved.games.clone();
                        acc.shared_secret = saved.shared_secret.clone();
                        acc.refresh_token = saved.refresh_token.clone();
                        acc.afk_reply_custom = saved.afk_reply_custom.clone();
                        acc.proxy = saved.proxy.clone();
                    }
                    if let Some(arc) = self.runtime_accounts.iter().find(|a| a.lock().unwrap().id == saved.id) {
                        let mut rt = arc.lock().unwrap();
                        rt.username = saved.username.clone();
                        rt.password = saved.password.clone();
                        rt.custom_title = saved.custom_title.clone();
                        rt.persona_state = saved.persona_state;
                        rt.games = saved.games.clone();
                        rt.shared_secret = saved.shared_secret.clone();
                        rt.refresh_token = saved.refresh_token.clone();
                        rt.afk_reply_custom = saved.afk_reply_custom.clone();
                        rt.proxy = saved.proxy.clone();
                    }
                    // CANLI HOT-RELOAD
                    steam::update_presence(
                        saved.id,
                        saved.games.clone(),
                        saved.custom_title.clone(),
                        saved.persona_state,
                        saved.afk_reply_custom.clone(),
                    );
                    logger::log_info(&format!("[{}] Account settings updated (Hot-Reload).", saved.username));
                }
                self.config.save();
                close_edit = true;
            }
            if ui.button(RichText::new(s.btn_cancel).size(14.0)).clicked() {
                close_edit = true;
            }
        });

        if close_edit {
            self.editing_account = None;
            self.is_adding = false;
        } else {
            self.editing_account = Some(edit_acc);
        }
    }

    fn draw_account_cards(&mut self, ui: &mut Ui, s: &Strings) {
        if self.config.accounts.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(60.0);
                ui.label(RichText::new(s.no_accounts).size(16.0).color(Color32::from_gray(130)));
                ui.label(RichText::new(s.no_accounts_hint).size(13.0).color(Color32::from_gray(100)));
            });
            return;
        }

        let mut to_delete: Option<u64> = None;
        let mut to_edit: Option<u64> = None;
        let mut to_toggle: Option<u64> = None;
        let mut to_restart: Option<u64> = None;
        let mut to_submit_guard: Option<(u64, String)> = None;

        egui::ScrollArea::vertical().show(ui, |ui| {
            for acc in &self.config.accounts {
                let (status_color, status_dot, status_label_text) = match &acc.status {
                    AccountStatus::Idle => (Color32::from_gray(130), "⚪", s.status_idle.to_string()),
                    AccountStatus::Connecting => (Color32::from_rgb(255, 200, 50), "🟡", s.status_connecting.to_string()),
                    AccountStatus::AwaitingSteamGuard { .. } => (Color32::from_rgb(255, 160, 40), "🛡️", s.status_guard_needed.to_string()),
                    AccountStatus::Boosting => (Color32::from_rgb(80, 200, 100), "🟢", s.status_boosting.to_string()),
                    AccountStatus::PausedForGame { app_name, .. } => (
                        Color32::from_rgb(180, 140, 240),
                        "⏸️",
                        format!("{} ({})", s.status_paused_game, app_name),
                    ),
                    AccountStatus::Reconnecting { attempt, retry_in_secs } => (
                        Color32::from_rgb(240, 160, 60),
                        "🔄",
                        format!("{} (#{}, {}s)", s.status_reconnecting, attempt, retry_in_secs),
                    ),
                    AccountStatus::Error(msg) => (Color32::from_rgb(220, 80, 80), "🔴", format!("{}: {}", s.status_error, msg)),
                };

                let is_awaiting_guard = matches!(&acc.status, AccountStatus::AwaitingSteamGuard { .. });

                egui::Frame::none()
                    .fill(Color32::from_rgb(28, 32, 40))
                    .rounding(10.0)
                    .stroke(Stroke::new(
                        1.0_f32,
                        if is_awaiting_guard {
                            Color32::from_rgb(200, 140, 40)
                        } else {
                            Color32::from_rgb(50, 60, 80)
                        },
                    ))
                    .inner_margin(egui::Margin::symmetric(16.0, 12.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(status_dot));
                                    ui.label(
                                        RichText::new(&acc.username)
                                            .font(FontId::proportional(15.5))
                                            .color(Color32::WHITE),
                                    );
                                    let persona_str = match acc.persona_state {
                                        PersonaState::Online => s.persona_online,
                                        PersonaState::Invisible => s.persona_invisible,
                                        PersonaState::Away => s.persona_away,
                                        PersonaState::Busy => s.persona_busy,
                                    };
                                    ui.label(RichText::new(persona_str).size(11.0).color(Color32::from_rgb(140, 180, 240)));
                                    if acc.refresh_token.is_some() {
                                        ui.label(RichText::new(s.label_token_present).size(11.0).color(Color32::from_rgb(100, 200, 140)));
                                    }
                                });
                                if !acc.custom_title.trim().is_empty() {
                                    ui.label(
                                        RichText::new(format!("{}: {}", s.label_custom_title, acc.custom_title))
                                            .size(12.0)
                                            .color(Color32::from_rgb(180, 160, 220)),
                                    );
                                }
                                ui.label(
                                    RichText::new(format!("🎮 {} {}", acc.games.len(), s.games_count_unit))
                                        .size(12.0)
                                        .color(Color32::from_gray(160)),
                                );
                                ui.label(
                                    RichText::new(format!("⏱ {:.1} {}", acc.hours_boosted, s.hours_unit))
                                        .size(12.0)
                                        .color(Color32::from_gray(160)),
                                );
                                ui.label(RichText::new(&status_label_text).color(status_color).size(12.0));
                                if let Some(ref p) = acc.proxy {
                                    ui.label(RichText::new(format!("🌐 {}", p)).color(Color32::from_rgb(120, 200, 140)).size(11.0));
                                }
                            });

                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.small_button("🗑").on_hover_text(s.btn_delete).clicked() {
                                    to_delete = Some(acc.id);
                                }
                                if ui.small_button("✏").on_hover_text(s.btn_edit).clicked() {
                                    to_edit = Some(acc.id);
                                }

                                if ui
                                    .button(RichText::new(s.btn_restart).color(Color32::from_rgb(255, 180, 60)))
                                    .on_hover_text(s.btn_restart)
                                    .clicked()
                                {
                                    to_restart = Some(acc.id);
                                }

                                let (btn_text, btn_color) = match &acc.status {
                                    AccountStatus::Boosting
                                    | AccountStatus::Connecting
                                    | AccountStatus::AwaitingSteamGuard { .. }
                                    | AccountStatus::PausedForGame { .. }
                                    | AccountStatus::Reconnecting { .. } => {
                                        (s.btn_stop, Color32::from_rgb(220, 80, 80))
                                    }
                                    _ => (s.btn_start, Color32::from_rgb(80, 200, 100)),
                                };
                                if ui.button(RichText::new(btn_text).color(btn_color)).clicked() {
                                    to_toggle = Some(acc.id);
                                }
                            });
                        });

                        // Steam Guard Kutusu
                        if is_awaiting_guard {
                            ui.add_space(8.0);
                            ui.separator();
                            ui.add_space(6.0);
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(s.guard_code_label).color(Color32::from_rgb(255, 200, 80)).size(13.0));
                                let entry = self.guard_inputs.entry(acc.id).or_insert_with(String::new);
                                ui.add(egui::TextEdit::singleline(entry).desired_width(100.0).hint_text("R1234"));
                                if ui.button(RichText::new(s.guard_verify_btn).color(Color32::from_rgb(100, 220, 100))).clicked() {
                                    to_submit_guard = Some((acc.id, entry.clone()));
                                }
                            });
                            ui.label(
                                RichText::new(s.guard_hint)
                                    .size(11.0)
                                    .color(Color32::from_gray(140)),
                            );
                        }
                    });
                ui.add_space(8.0);
            }
        });

        if let Some((id, code)) = to_submit_guard {
            steam::submit_guard_code(id, &code);
            self.guard_inputs.remove(&id);
            if let Some(arc) = self.runtime_accounts.iter().find(|a| a.lock().unwrap().id == id) {
                arc.lock().unwrap().status = AccountStatus::Connecting;
            }
        }

        if let Some(id) = to_delete {
            self.config.accounts.retain(|a| a.id != id);
            self.runtime_accounts.retain(|a| a.lock().unwrap().id != id);
            self.config.save();
        }

        if let Some(id) = to_edit {
            if let Some(acc) = self.config.accounts.iter().find(|a| a.id == id) {
                self.editing_account = Some(acc.clone());
                self.is_adding = false;
                self.game_search.clear();
                self.last_store_search_query.clear();
                self.store_search_results.lock().unwrap().clear();
                self.custom_app_id_input.clear();
                self.custom_game_name_input.clear();
                self.app_lookup_msg = None;
            }
        }

        if let Some(id) = to_restart {
            if let Some(arc) = self.runtime_accounts.iter().find(|a| a.lock().unwrap().id == id) {
                steam::restart_boost(id, arc.clone());
            }
        }

        if let Some(id) = to_toggle {
            if let Some(arc) = self.runtime_accounts.iter().find(|a| a.lock().unwrap().id == id) {
                let status = arc.lock().unwrap().status.clone();
                match status {
                    AccountStatus::Boosting
                    | AccountStatus::Connecting
                    | AccountStatus::AwaitingSteamGuard { .. }
                    | AccountStatus::PausedForGame { .. }
                    | AccountStatus::Reconnecting { .. } => {
                        steam::stop_boost(id, arc.clone());
                    }
                    _ => {
                        steam::start_boost(arc.clone());
                    }
                }
            }
        }
    }

    fn draw_dashboard_tab(&mut self, ui: &mut Ui, s: &Strings) {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new(s.dashboard_title).font(FontId::proportional(22.0)));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if !self.config.accounts.is_empty() {
                    let any_boosting = self
                        .config
                        .accounts
                        .iter()
                        .any(|a| matches!(a.status, AccountStatus::Boosting));
                    if any_boosting {
                        if ui
                            .button(RichText::new(s.btn_stop_all).color(Color32::from_rgb(220, 80, 80)))
                            .clicked()
                        {
                            for arc in &self.runtime_accounts {
                                let id = arc.lock().unwrap().id;
                                steam::stop_boost(id, arc.clone());
                            }
                        }
                    } else {
                        if ui
                            .button(RichText::new(s.btn_start_all).color(Color32::from_rgb(80, 200, 100)))
                            .clicked()
                        {
                            for arc in &self.runtime_accounts {
                                steam::start_boost(arc.clone());
                            }
                        }
                    }
                    if ui
                        .button(RichText::new(s.btn_restart_all).color(Color32::from_rgb(255, 180, 60)))
                        .clicked()
                    {
                        for arc in &self.runtime_accounts {
                            let id = arc.lock().unwrap().id;
                            steam::restart_boost(id, arc.clone());
                        }
                    }
                }
            });
        });
        ui.separator();
        ui.add_space(8.0);

        if self.config.accounts.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(60.0);
                ui.label(RichText::new(s.no_accounts).size(16.0).color(Color32::from_gray(140)));
            });
            return;
        }

        let total_boosting = self
            .config
            .accounts
            .iter()
            .filter(|a| matches!(a.status, AccountStatus::Boosting))
            .count();
        let total_games: usize = self.config.accounts.iter().map(|a| a.games.len()).sum();
        let total_hours: f64 = self.config.accounts.iter().map(|a| a.hours_boosted).sum();

        ui.horizontal(|ui| {
            egui::Frame::none()
                .fill(Color32::from_rgb(24, 28, 38))
                .rounding(8.0)
                .inner_margin(egui::Margin::symmetric(14.0, 10.0))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(format!("{} {}/{}", s.total_active, total_boosting, self.config.accounts.len()))
                            .size(13.0)
                            .color(Color32::from_rgb(80, 200, 100)),
                    );
                });
            egui::Frame::none()
                .fill(Color32::from_rgb(24, 28, 38))
                .rounding(8.0)
                .inner_margin(egui::Margin::symmetric(14.0, 10.0))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(format!("{} {}", s.total_games, total_games))
                            .size(13.0)
                            .color(Color32::from_gray(200)),
                    );
                });
            egui::Frame::none()
                .fill(Color32::from_rgb(24, 28, 38))
                .rounding(8.0)
                .inner_margin(egui::Margin::symmetric(14.0, 10.0))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(format!("{} {:.1}h", s.total_hours, total_hours))
                            .size(13.0)
                            .color(Color32::from_rgb(100, 180, 255)),
                    );
                });
        });

        ui.add_space(14.0);

        let mut to_toggle: Option<u64> = None;
        let mut to_restart: Option<u64> = None;

        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::Grid::new("dashboard_grid")
                .num_columns(6)
                .spacing([16.0, 12.0])
                .striped(true)
                .show(ui, |ui| {
                    ui.label(RichText::new(s.col_status).color(Color32::from_gray(170)));
                    ui.label(RichText::new(s.col_account).color(Color32::from_gray(170)));
                    ui.label(RichText::new(s.col_games).color(Color32::from_gray(170)));
                    ui.label(RichText::new(s.col_title).color(Color32::from_gray(170)));
                    ui.label(RichText::new(s.col_hours).color(Color32::from_gray(170)));
                    ui.label(RichText::new(s.col_actions).color(Color32::from_gray(170)));
                    ui.end_row();

                    for acc in &self.config.accounts {
                        let (status_color, status_text) = match &acc.status {
                            AccountStatus::Idle => (Color32::from_gray(130), s.status_idle.to_string()),
                            AccountStatus::Connecting => (Color32::from_rgb(255, 200, 50), s.status_connecting.to_string()),
                            AccountStatus::AwaitingSteamGuard { .. } => (Color32::from_rgb(255, 160, 40), s.status_guard_needed.to_string()),
                            AccountStatus::Boosting => (Color32::from_rgb(80, 200, 100), s.status_boosting.to_string()),
                            AccountStatus::PausedForGame { app_name, .. } => (
                                Color32::from_rgb(180, 140, 240),
                                format!("{} ({})", s.status_paused_game, app_name),
                            ),
                            AccountStatus::Reconnecting { retry_in_secs, .. } => (
                                Color32::from_rgb(240, 160, 60),
                                format!("{} ({}s)", s.status_reconnecting, retry_in_secs),
                            ),
                            AccountStatus::Error(msg) => (Color32::from_rgb(220, 80, 80), format!("{}: {}", s.status_error, msg)),
                        };

                        ui.label(RichText::new(&status_text).color(status_color));
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&acc.username).color(Color32::WHITE).strong());
                            let persona_label = match acc.persona_state {
                                PersonaState::Online => s.persona_online,
                                PersonaState::Invisible => s.persona_invisible,
                                PersonaState::Away => s.persona_away,
                                PersonaState::Busy => s.persona_busy,
                            };
                            ui.label(RichText::new(persona_label).size(10.5).color(Color32::from_rgb(140, 180, 240)));
                        });
                        let games_str = if acc.games.is_empty() {
                            format!("({})", s.no_accounts)
                        } else {
                            acc.games.iter().map(|g| g.name.as_str()).collect::<Vec<_>>().join(", ")
                        };
                        ui.label(RichText::new(&games_str).color(Color32::from_gray(200)).size(12.0));
                        ui.label(RichText::new(&acc.custom_title).color(Color32::from_rgb(180, 160, 220)).size(12.0));
                        ui.label(RichText::new(format!("{:.2}h", acc.hours_boosted)).color(Color32::from_rgb(80, 200, 100)));

                        let is_active = matches!(
                            acc.status,
                            AccountStatus::Boosting
                                | AccountStatus::Connecting
                                | AccountStatus::AwaitingSteamGuard { .. }
                                | AccountStatus::PausedForGame { .. }
                                | AccountStatus::Reconnecting { .. }
                        );
                        let (btn_text, btn_color) = if is_active {
                            (s.btn_stop, Color32::from_rgb(220, 80, 80))
                        } else {
                            (s.btn_start, Color32::from_rgb(80, 200, 100))
                        };
                        ui.horizontal(|ui| {
                            if ui.button(RichText::new(btn_text).color(btn_color).size(12.0)).clicked() {
                                to_toggle = Some(acc.id);
                            }
                            if ui
                                .button(RichText::new("🔄").color(Color32::from_rgb(255, 180, 60)).size(12.0))
                                .on_hover_text(s.btn_restart)
                                .clicked()
                            {
                                to_restart = Some(acc.id);
                            }
                        });
                        ui.end_row();
                    }
                });
        });

        if let Some(id) = to_restart {
            if let Some(arc) = self.runtime_accounts.iter().find(|a| a.lock().unwrap().id == id) {
                steam::restart_boost(id, arc.clone());
            }
        }

        if let Some(id) = to_toggle {
            if let Some(arc) = self.runtime_accounts.iter().find(|a| a.lock().unwrap().id == id) {
                let status = arc.lock().unwrap().status.clone();
                match status {
                    AccountStatus::Boosting
                    | AccountStatus::Connecting
                    | AccountStatus::AwaitingSteamGuard { .. }
                    | AccountStatus::PausedForGame { .. }
                    | AccountStatus::Reconnecting { .. } => {
                        steam::stop_boost(id, arc.clone());
                    }
                    _ => {
                        steam::start_boost(arc.clone());
                    }
                }
            }
        }
    }

    fn draw_settings_tab(&mut self, ui: &mut Ui, s: &Strings) {
        ui.add_space(8.0);
        ui.label(RichText::new(s.settings_title).font(FontId::proportional(22.0)));
        ui.separator();
        ui.add_space(12.0);

        egui::ScrollArea::vertical().show(ui, |ui| {
            // 1. Dil Seçimi
            ui.label(RichText::new(s.sec_language).font(FontId::proportional(16.0)).color(Color32::from_rgb(100, 180, 255)));
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("Dil / Language / Язык:");
                let current_lang = self.config.settings.language;
                egui::ComboBox::from_id_salt("lang_combo")
                    .selected_text(current_lang.label())
                    .show_ui(ui, |ui| {
                        if ui.selectable_label(current_lang == Language::Turkish, Language::Turkish.label()).clicked() {
                            self.config.settings.language = Language::Turkish;
                            self.config.save();
                        }
                        if ui.selectable_label(current_lang == Language::English, Language::English.label()).clicked() {
                            self.config.settings.language = Language::English;
                            self.config.save();
                        }
                        if ui.selectable_label(current_lang == Language::Russian, Language::Russian.label()).clicked() {
                            self.config.settings.language = Language::Russian;
                            self.config.save();
                        }
                    });
            });

            ui.add_space(16.0);
            ui.separator();
            ui.add_space(8.0);

            // 2. Windows Başlangıcı ve Servis / Görev Zamanlayıcı
            ui.label(RichText::new(s.sec_startup).font(FontId::proportional(16.0)).color(Color32::from_rgb(100, 180, 255)));
            ui.add_space(4.0);

            // Kayıt Defteri Başlangıcı
            let mut run_key_val = self.autostart_run_key;
            if ui.checkbox(&mut run_key_val, s.opt_autostart).changed() {
                self.autostart_run_key = run_key_val;
                self.config.settings.autostart = run_key_val;
                if let Err(e) = autostart::set_registry_autostart(run_key_val, self.autostart_minimized) {
                    logger::log_error(&format!("Failed to update registry startup setting: {}", e));
                } else {
                    logger::log_success(&format!("Windows startup setting updated: {}", run_key_val));
                }
                self.config.save();
            }

            // Sistem tepsisine küçültülmüş başla
            let mut tray_minimized_val = self.autostart_minimized;
            if ui.checkbox(&mut tray_minimized_val, s.opt_start_minimized).changed() {
                self.autostart_minimized = tray_minimized_val;
                self.config.settings.start_minimized = tray_minimized_val;
                self.config.settings.minimize_to_tray = tray_minimized_val;
                if self.autostart_run_key {
                    let _ = autostart::set_registry_autostart(true, tray_minimized_val);
                }
                self.config.save();
            }

            // PC / Uygulama açılışında tüm hesapların boostunu otomatik başlat
            if ui.checkbox(&mut self.config.settings.auto_start_boost_on_launch, s.opt_auto_start_boost).changed() {
                self.config.save();
                logger::log_info(&format!("Auto-start boost setting updated: {}", self.config.settings.auto_start_boost_on_launch));
            }

            ui.add_space(6.0);
            // Windows Görev Zamanlayıcı / Servis Kurulumu
            ui.horizontal(|ui| {
                let task_active = self.autostart_task_service;
                let btn_text = if task_active { s.btn_unregister_task } else { s.btn_register_task };
                let btn_color = if task_active { Color32::from_rgb(220, 80, 80) } else { Color32::from_rgb(80, 200, 100) };
                if ui.button(RichText::new(btn_text).color(btn_color)).clicked() {
                    let res = if task_active {
                        autostart::unregister_task_scheduler()
                    } else {
                        autostart::register_task_scheduler(self.autostart_minimized)
                    };
                    match res {
                        Ok(()) => {
                            self.autostart_task_service = !task_active;
                            logger::log_success(&format!("Windows Task Scheduler task updated: {}", !task_active));
                        }
                        Err(e) => {
                            logger::log_error(&format!("Windows Görev Zamanlayıcı güncellenemedi: {}", e));
                        }
                    }
                }
                let status_msg = if task_active { s.service_active } else { s.service_inactive };
                let status_col = if task_active { Color32::from_rgb(80, 200, 100) } else { Color32::from_gray(140) };
                ui.label(RichText::new(status_msg).color(status_col).size(12.0));
            });
            ui.label(RichText::new(s.opt_task_scheduler).size(11.0).color(Color32::from_gray(140)));

            ui.add_space(16.0);
            ui.separator();
            ui.add_space(8.0);

            // 3. Güvenlik ve DPAPI Şifreleme
            ui.label(RichText::new(s.sec_security).font(FontId::proportional(16.0)).color(Color32::from_rgb(100, 180, 255)));
            ui.add_space(4.0);
            let mut dpapi_val = self.config.settings.encrypt_credentials;
            if ui.checkbox(&mut dpapi_val, s.opt_encrypt_data).changed() {
                self.config.settings.encrypt_credentials = dpapi_val;
                self.config.save();
                if dpapi_val {
                    logger::log_success("Windows DPAPI şifrelemesi aktif edildi. Config şifrelendi.");
                } else {
                    logger::log_info("DPAPI şifrelemesi kapatıldı.");
                }
            }

            ui.add_space(16.0);
            ui.separator();
            ui.add_space(8.0);

            // 4. Gerçek Oyun Algılama (Auto-Pause)
            ui.label(RichText::new(s.sec_autopause).font(FontId::proportional(16.0)).color(Color32::from_rgb(100, 180, 255)));
            ui.add_space(4.0);
            let mut auto_pause_val = self.config.settings.auto_pause_game;
            if ui.checkbox(&mut auto_pause_val, s.opt_autopause_game).changed() {
                self.config.settings.auto_pause_game = auto_pause_val;
                self.config.save();
                logger::log_info(&format!("Auto game detection setting: {}", auto_pause_val));
            }

            ui.add_space(16.0);
            ui.separator();
            ui.add_space(8.0);

            // 5. Zamanlayıcı (Scheduler)
            ui.label(RichText::new(s.sec_scheduler).font(FontId::proportional(16.0)).color(Color32::from_rgb(100, 180, 255)));
            ui.add_space(4.0);
            let mut sched_enabled = self.config.settings.enable_scheduler;
            if ui.checkbox(&mut sched_enabled, s.opt_enable_scheduler).changed() {
                self.config.settings.enable_scheduler = sched_enabled;
                self.config.save();
            }
            ui.horizontal(|ui| {
                ui.label(s.scheduler_start_hour);
                let mut start_h = self.config.settings.scheduler_start_hour;
                if ui.add(egui::DragValue::new(&mut start_h).range(0..=23)).changed() {
                    self.config.settings.scheduler_start_hour = start_h;
                    self.config.save();
                }

                ui.label(s.scheduler_end_hour);
                let mut end_h = self.config.settings.scheduler_end_hour;
                if ui.add(egui::DragValue::new(&mut end_h).range(0..=23)).changed() {
                    self.config.settings.scheduler_end_hour = end_h;
                    self.config.save();
                }
            });

            ui.add_space(16.0);
            ui.separator();
            ui.add_space(8.0);

            // 6. AFK Otomatik Yanıtlayıcı
            ui.label(RichText::new(s.sec_afk).font(FontId::proportional(16.0)).color(Color32::from_rgb(100, 180, 255)));
            ui.add_space(4.0);
            let mut afk_val = self.config.settings.afk_auto_reply;
            if ui.checkbox(&mut afk_val, s.opt_afk_auto_reply).changed() {
                self.config.settings.afk_auto_reply = afk_val;
                self.config.save();
                logger::log_info(&format!("AFK Otomatik Yanıtlayıcı: {}", if afk_val { "Açık" } else { "Kapalı" }));
            }
            if self.config.settings.afk_auto_reply {
                ui.horizontal(|ui| {
                    ui.label(s.label_afk_message);
                    let resp = ui.add(
                        egui::TextEdit::singleline(&mut self.config.settings.afk_reply_message)
                            .desired_width(ui.available_width() - 20.0),
                    );
                    if resp.changed() {
                        self.config.save();
                    }
                });
                ui.horizontal(|ui| {
                    ui.label(s.label_afk_cooldown);
                    let mut cd = self.config.settings.afk_cooldown_mins;
                    if ui.add(egui::DragValue::new(&mut cd).range(1..=120)).changed() {
                        self.config.settings.afk_cooldown_mins = cd;
                        self.config.save();
                    }
                });
            }

            ui.add_space(16.0);
            ui.separator();
            ui.add_space(8.0);

            // 7. Steam Koleksiyon Kartı Takibi (Card Drop Tracking)
            ui.label(RichText::new(s.sec_card_drop).font(FontId::proportional(16.0)).color(Color32::from_rgb(100, 180, 255)));
            ui.add_space(4.0);
            let mut card_val = self.config.settings.card_drop_tracking;
            if ui.checkbox(&mut card_val, s.opt_card_drop_tracking).changed() {
                self.config.settings.card_drop_tracking = card_val;
                self.config.save();
                logger::log_info(&format!("Kart Düşürme Takibi: {}", if card_val { "Açık" } else { "Kapalı" }));
            }

            ui.add_space(16.0);
            ui.separator();
            ui.add_space(8.0);

            // 8. Arayüz Boyutlandırması & Zoom (UI Scale)
            ui.label(RichText::new(s.label_ui_scale).font(FontId::proportional(16.0)).color(Color32::from_rgb(100, 180, 255)));
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(s.label_ui_scale);
                let mut scale = self.config.settings.ui_scale;
                if ui.add(egui::Slider::new(&mut scale, 0.8..=1.6).step_by(0.05).custom_formatter(|n, _| format!("{:.0}%", n * 100.0))).changed() {
                    self.config.settings.ui_scale = scale;
                    self.config.save();
                }
                if ui.button(s.zoom_reset_btn).clicked() {
                    self.config.settings.ui_scale = 1.0;
                    self.config.save();
                }
            });

            ui.add_space(16.0);
            ui.separator();
            ui.add_space(8.0);

            // 9. Telegram Bot Kontrolü
            ui.label(RichText::new(s.sec_telegram).font(FontId::proportional(16.0)).color(Color32::from_rgb(100, 180, 255)));
            ui.add_space(4.0);
            let mut tg_enabled = self.config.settings.telegram_enabled;
            if ui.checkbox(&mut tg_enabled, s.opt_telegram_enabled).changed() {
                self.config.settings.telegram_enabled = tg_enabled;
                self.config.save();
                if tg_enabled {
                    crate::telegram::register_accounts_for_bot(self.runtime_accounts.clone());
                    crate::telegram::init_telegram_bot();
                }
            }
            if self.config.settings.telegram_enabled {
                ui.horizontal(|ui| {
                    ui.label(s.label_telegram_token);
                    let resp = ui.add(
                        egui::TextEdit::singleline(&mut self.config.settings.telegram_bot_token)
                            .password(true)
                            .hint_text("123456789:ABCdefGhIJKlmNoPQRsTUVwxyZ"),
                    );
                    if resp.changed() {
                        self.config.save();
                    }
                });
                ui.horizontal(|ui| {
                    ui.label(s.label_telegram_chat_id);
                    let resp = ui.add(
                        egui::TextEdit::singleline(&mut self.config.settings.telegram_chat_id)
                            .hint_text("E.g.: 123456789"),
                    );
                    if resp.changed() {
                        self.config.save();
                    }
                });
                ui.horizontal(|ui| {
                    if ui.button("📩 Test Bildirimi Gönder").clicked() {
                        crate::telegram::send_telegram_alert("🚀 *SteamBooster Test Bildirimi:*\nTelegram Bot entegrasyonu başarıyla çalışıyor!");
                        logger::log_info("Telegram test bildirimi gönderildi.");
                    }
                    if ui.button("🔄 Botu Yeniden Başlat").clicked() {
                        crate::telegram::register_accounts_for_bot(self.runtime_accounts.clone());
                        crate::telegram::init_telegram_bot();
                    }
                });
            }

            ui.add_space(16.0);
            ui.separator();
            ui.add_space(8.0);

            // 10. Ücretsiz Oyun & Paket Avcısı (Free Package Claimer)
            ui.label(RichText::new(s.sec_free_games).font(FontId::proportional(16.0)).color(Color32::from_rgb(100, 180, 255)));
            ui.add_space(4.0);
            let mut free_val = self.config.settings.auto_claim_free_games;
            if ui.checkbox(&mut free_val, s.opt_free_games).changed() {
                self.config.settings.auto_claim_free_games = free_val;
                self.config.save();
            }
            ui.horizontal(|ui| {
                if ui.button(s.btn_scan_free_games).clicked() {
                    let _runtime_accs = self.runtime_accounts.clone();
                    steam::get_runtime().spawn(async move {
                        logger::log_info("🎁 Scanning Steam Store API for free promotions...");
                        let free_list = crate::free_games::find_featured_free_games().await;
                        if free_list.is_empty() {
                            logger::log_info("🎁 No featured free promotions found on Steam at this moment.");
                        } else {
                            logger::log_success(&format!("🎁 Found {} free games/promotions: {:?}", free_list.len(), free_list));
                        }
                    });
                }
            });

            ui.add_space(16.0);
            ui.separator();
            ui.add_space(8.0);

            // 11. Akıllı Durum & Dinamik Başlık Döngüsü (Smart Status Scenarios)
            ui.label(RichText::new(s.sec_dynamic_titles).font(FontId::proportional(16.0)).color(Color32::from_rgb(100, 180, 255)));
            ui.add_space(4.0);
            let mut rot_val = self.config.settings.title_rotation;
            if ui.checkbox(&mut rot_val, s.opt_title_rotation).changed() {
                self.config.settings.title_rotation = rot_val;
                self.config.save();
            }
            if self.config.settings.title_rotation {
                ui.horizontal(|ui| {
                    ui.label(s.label_rotation_interval);
                    let mut interval_sec = self.config.settings.title_rotation_interval_secs;
                    if ui.add(egui::DragValue::new(&mut interval_sec).range(30..=3600)).changed() {
                        self.config.settings.title_rotation_interval_secs = interval_sec;
                        self.config.save();
                    }
                });
                ui.label(RichText::new(s.title_rotation_list_label).size(12.0).color(Color32::from_gray(160)));
                let mut to_remove_title: Option<usize> = None;
                let mut title_changed = false;
                for (idx, title_item) in self.config.settings.title_rotation_list.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        let resp = ui.text_edit_singleline(title_item);
                        if resp.changed() {
                            title_changed = true;
                        }
                        if ui.small_button("❌").clicked() {
                            to_remove_title = Some(idx);
                        }
                    });
                }
                if title_changed {
                    self.config.save();
                }
                if let Some(idx) = to_remove_title {
                    self.config.settings.title_rotation_list.remove(idx);
                    self.config.save();
                }
                if ui.button(s.btn_add_rotation_template).clicked() {
                    self.config.settings.title_rotation_list.push("Playing {games} games | {hours}h boosted".into());
                    self.config.save();
                }
            }

            ui.add_space(16.0);
            ui.separator();
            ui.add_space(8.0);

            // 12. Envanter Değer ve Değişim Takibi
            ui.label(RichText::new(s.sec_inventory).font(FontId::proportional(16.0)).color(Color32::from_rgb(100, 180, 255)));
            ui.add_space(4.0);
            let mut inv_val = self.config.settings.inventory_tracking;
            if ui.checkbox(&mut inv_val, s.opt_inventory_tracking).changed() {
                self.config.settings.inventory_tracking = inv_val;
                self.config.save();
            }
            ui.horizontal(|ui| {
                if ui.button(s.btn_check_inventory).clicked() {
                    for arc in &self.runtime_accounts {
                        let (user, _proxy) = {
                            let a = arc.lock().unwrap();
                            (a.username.clone(), a.proxy.clone())
                        };
                        steam::get_runtime().spawn(async move {
                            logger::log_info(&format!("[{}] Checking inventory...", user));
                        });
                    }
                }
            });
        });
    }

    fn draw_logs_tab(&mut self, ui: &mut Ui, s: &Strings, ctx: &egui::Context) {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new(s.logs_title).font(FontId::proportional(22.0)));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(RichText::new(s.btn_clear_logs).color(Color32::from_rgb(220, 80, 80))).clicked() {
                    logger::clear_logs();
                }
                if ui.button(RichText::new(s.btn_copy_logs).color(Color32::from_rgb(100, 180, 255))).clicked() {
                    let logs = logger::get_logs();
                    let text = logs
                        .iter()
                        .map(|e| format!("[{}] [{:?}] {}", e.timestamp, e.level, e.message))
                        .collect::<Vec<_>>()
                        .join("\n");
                    ctx.output_mut(|o| o.copied_text = text);
                    logger::log_info("System logs copied to clipboard.");
                }

                // Filtreleme butonları
                ui.selectable_value(&mut self.log_filter_level, None, "Tümü / All");
                ui.selectable_value(&mut self.log_filter_level, Some(LogLevel::Info), "Info");
                ui.selectable_value(&mut self.log_filter_level, Some(LogLevel::Success), "Success");
                ui.selectable_value(&mut self.log_filter_level, Some(LogLevel::Warn), "Warn");
                ui.selectable_value(&mut self.log_filter_level, Some(LogLevel::Error), "Error");
            });
        });
        ui.separator();
        ui.add_space(8.0);

        let logs = logger::get_logs();
        let filtered_logs: Vec<&LogEntry> = logs
            .iter()
            .filter(|entry| match self.log_filter_level {
                None => true,
                Some(lvl) => entry.level == lvl,
            })
            .collect();

        egui::Frame::none()
            .fill(Color32::from_rgb(14, 16, 22))
            .rounding(6.0)
            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(40, 48, 64)))
            .inner_margin(egui::Margin::symmetric(12.0, 10.0))
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        if filtered_logs.is_empty() {
                            ui.label(RichText::new("(Henüz kayıtlı log yok / No logs yet)").color(Color32::from_gray(130)).size(13.0));
                        } else {
                            for entry in filtered_logs {
                                let (badge, color) = match entry.level {
                                    LogLevel::Info => ("ℹ️", Color32::from_rgb(140, 180, 240)),
                                    LogLevel::Success => ("✅", Color32::from_rgb(100, 220, 120)),
                                    LogLevel::Warn => ("⚠️", Color32::from_rgb(255, 200, 70)),
                                    LogLevel::Error => ("❌", Color32::from_rgb(255, 100, 100)),
                                };
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(&entry.timestamp)
                                            .size(11.5)
                                            .color(Color32::from_gray(120))
                                            .monospace(),
                                    );
                                    ui.label(RichText::new(badge).size(12.0));
                                    ui.label(
                                        RichText::new(&entry.message)
                                            .size(12.5)
                                            .color(color)
                                            .monospace(),
                                    );
                                });
                            }
                        }
                    });
            });
    }

    fn draw_debug_tab(&mut self, ui: &mut Ui, s: &Strings, ctx: &egui::Context) {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new(s.debug_title).font(FontId::proportional(22.0)));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(RichText::new(s.btn_clear_packets).color(Color32::from_rgb(220, 80, 80))).clicked() {
                    crate::debug_log::clear_packet_logs();
                }
                if ui.button(RichText::new(s.btn_copy_packets).color(Color32::from_rgb(100, 180, 255))).clicked() {
                    let packets = crate::debug_log::get_packet_logs();
                    let text = packets
                        .iter()
                        .map(|p| format!("[{}] [{:?}] [{}] {} -> {}", p.timestamp, p.direction, p.account, p.msg_kind, p.summary))
                        .collect::<Vec<_>>()
                        .join("\n");
                    ctx.output_mut(|o| o.copied_text = text);
                    logger::log_info("Protocol packets copied to clipboard.");
                }

                // Arama kutusu
                ui.add(egui::TextEdit::singleline(&mut self.debug_search_query).hint_text("🔍 Filter...").desired_width(120.0));
            });
        });
        ui.separator();
        ui.add_space(4.0);

        // Kategori filtreleri
        ui.horizontal(|ui| {
            ui.label(RichText::new(s.debug_col_category).size(12.0).color(Color32::from_gray(160)));
            ui.selectable_value(&mut self.debug_filter_category, None, "All");
            ui.selectable_value(&mut self.debug_filter_category, Some(crate::debug_log::PacketCategory::Chat), "💬 Chat");
            ui.selectable_value(&mut self.debug_filter_category, Some(crate::debug_log::PacketCategory::Presence), "🎮 Presence");
            ui.selectable_value(&mut self.debug_filter_category, Some(crate::debug_log::PacketCategory::Session), "🔒 Session");
            ui.selectable_value(&mut self.debug_filter_category, Some(crate::debug_log::PacketCategory::License), "🎁 License");
            ui.selectable_value(&mut self.debug_filter_category, Some(crate::debug_log::PacketCategory::Economy), "🃏 Economy");
            ui.selectable_value(&mut self.debug_filter_category, Some(crate::debug_log::PacketCategory::System), "⚙️ System");
        });
        ui.add_space(6.0);

        let packets = crate::debug_log::get_packet_logs();
        let search_low = self.debug_search_query.to_lowercase().trim().to_string();

        let filtered: Vec<_> = packets
            .iter()
            .filter(|p| {
                if let Some(cat) = self.debug_filter_category {
                    if p.category != cat {
                        return false;
                    }
                }
                if !search_low.is_empty() {
                    let acc_match = p.account.to_lowercase().contains(&search_low);
                    let kind_match = p.msg_kind.to_lowercase().contains(&search_low);
                    let sum_match = p.summary.to_lowercase().contains(&search_low);
                    if !acc_match && !kind_match && !sum_match {
                        return false;
                    }
                }
                true
            })
            .collect();

        egui::Frame::none()
            .fill(Color32::from_rgb(14, 16, 22))
            .rounding(6.0)
            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(40, 48, 64)))
            .inner_margin(egui::Margin::symmetric(12.0, 10.0))
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        if filtered.is_empty() {
                            ui.label(RichText::new("(No packet events recorded yet)").color(Color32::from_gray(130)).size(13.0));
                        } else {
                            egui::Grid::new("debug_packets_grid")
                                .num_columns(6)
                                .spacing([12.0, 6.0])
                                .striped(true)
                                .show(ui, |ui| {
                                    ui.label(RichText::new(s.debug_col_time).color(Color32::from_gray(140)).size(11.0));
                                    ui.label(RichText::new(s.debug_col_dir).color(Color32::from_gray(140)).size(11.0));
                                    ui.label(RichText::new(s.debug_col_account).color(Color32::from_gray(140)).size(11.0));
                                    ui.label(RichText::new(s.debug_col_type).color(Color32::from_gray(140)).size(11.0));
                                    ui.label(RichText::new(s.debug_col_category).color(Color32::from_gray(140)).size(11.0));
                                    ui.label(RichText::new(s.debug_col_summary).color(Color32::from_gray(140)).size(11.0));
                                    ui.end_row();

                                    for p in filtered {
                                        let (dir_badge, dir_color) = match p.direction {
                                            crate::debug_log::PacketDirection::Outbound => ("⬆️ OUT", Color32::from_rgb(100, 200, 255)),
                                            crate::debug_log::PacketDirection::Inbound => ("⬇️ IN", Color32::from_rgb(100, 255, 120)),
                                        };

                                        ui.label(RichText::new(&p.timestamp).size(11.0).color(Color32::from_gray(140)).monospace());
                                        ui.label(RichText::new(dir_badge).size(11.0).color(dir_color).monospace());
                                        ui.label(RichText::new(&p.account).size(11.5).color(Color32::WHITE).strong());
                                        ui.label(RichText::new(&p.msg_kind).size(11.5).color(Color32::from_rgb(255, 200, 80)).monospace());
                                        ui.label(RichText::new(p.category.label()).size(11.0).color(Color32::from_gray(180)));
                                        ui.label(RichText::new(&p.summary).size(12.0).color(Color32::from_gray(210)));
                                        ui.end_row();
                                    }
                                });
                        }
                    });
            });
    }

    fn draw_qr_modal(&mut self, ctx: &egui::Context, s: &Strings) {
        if !self.show_qr_modal {
            return;
        }

        let mut close_requested = false;
        let mut success_account: Option<(String, String)> = None;

        let status = {
            let st = self.qr_auth.status.lock().unwrap();
            st.clone()
        };

        if let crate::qr_auth::QrAuthStatus::Success { ref account_name, ref refresh_token } = status {
            success_account = Some((account_name.clone(), refresh_token.clone()));
        }

        egui::Window::new(format!("📱 {}", s.qr_modal_title))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .fixed_size(egui::vec2(340.0, 440.0))
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(8.0);
                    match &status {
                        crate::qr_auth::QrAuthStatus::Idle | crate::qr_auth::QrAuthStatus::Loading => {
                            ui.add_space(70.0);
                            ui.spinner();
                            ui.add_space(16.0);
                            ui.label(
                                RichText::new(s.qr_loading)
                                    .size(14.0)
                                    .color(Color32::from_gray(180)),
                            );
                        }
                        crate::qr_auth::QrAuthStatus::WaitingForScan { modules, width, .. } => {
                            ui.label(
                                RichText::new(s.qr_scan_title)
                                    .font(FontId::proportional(16.0))
                                    .color(Color32::WHITE),
                            );
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new(s.qr_scan_sub)
                                    .size(12.0)
                                    .color(Color32::from_gray(150)),
                            );
                            ui.add_space(12.0);

                            // Vektör QR Kod Çizimi
                            let qr_box_size = 220.0_f32;
                            let (rect, _response) =
                                ui.allocate_exact_size(Vec2::splat(qr_box_size), egui::Sense::hover());
                            let painter = ui.painter();

                            painter.rect_filled(rect, 8.0, Color32::WHITE);

                            let cell_size = (qr_box_size - 16.0) / (*width as f32);
                            let offset_x = rect.min.x + 8.0;
                            let offset_y = rect.min.y + 8.0;

                            for y in 0..*width {
                                for x in 0..*width {
                                    let idx = y * width + x;
                                    if let Some(color) = modules.get(idx) {
                                        if *color == qrcode::Color::Dark {
                                            let cell_rect = egui::Rect::from_min_size(
                                                egui::pos2(
                                                    offset_x + (x as f32) * cell_size,
                                                    offset_y + (y as f32) * cell_size,
                                                ),
                                                Vec2::splat(cell_size.ceil()),
                                            );
                                            painter.rect_filled(cell_rect, 0.0, Color32::BLACK);
                                        }
                                    }
                                }
                            }

                            ui.add_space(12.0);
                            ui.label(
                                RichText::new(s.qr_waiting)
                                    .size(13.0)
                                    .color(Color32::from_rgb(100, 200, 100)),
                            );
                        }
                        crate::qr_auth::QrAuthStatus::Success { account_name, .. } => {
                            ui.add_space(60.0);
                            ui.label(
                                RichText::new(s.qr_success)
                                    .font(FontId::proportional(18.0))
                                    .color(Color32::from_rgb(100, 220, 100)),
                            );
                            ui.add_space(8.0);
                            ui.label(
                                RichText::new(format!("{}, {}", s.qr_welcome, account_name))
                                    .size(14.0)
                                    .color(Color32::WHITE),
                            );
                        }
                        crate::qr_auth::QrAuthStatus::Error(err) => {
                            ui.add_space(40.0);
                            ui.label(
                                RichText::new(s.status_error)
                                    .font(FontId::proportional(16.0))
                                    .color(Color32::from_rgb(220, 80, 80)),
                            );
                            ui.add_space(8.0);
                            ui.label(RichText::new(err).color(Color32::from_gray(180)).size(12.0));
                            ui.add_space(14.0);
                            if ui.button(s.qr_retry).clicked() {
                                self.qr_auth.start();
                            }
                        }
                    }

                    ui.add_space(14.0);
                    if ui.button(s.qr_close).clicked() {
                        close_requested = true;
                    }
                });
            });

        if let Some((acc_name, token)) = success_account {
            let existing_id = self
                .config
                .accounts
                .iter()
                .find(|a| a.username.to_lowercase() == acc_name.to_lowercase())
                .map(|a| a.id);
            if let Some(id) = existing_id {
                if let Some(existing) = self.config.accounts.iter_mut().find(|a| a.id == id) {
                    existing.refresh_token = Some(token.clone());
                    existing.status = AccountStatus::Idle;
                }
                if let Some(arc) = self.runtime_accounts.iter().find(|a| a.lock().unwrap().id == id) {
                    let mut rt = arc.lock().unwrap();
                    rt.refresh_token = Some(token.clone());
                    rt.status = AccountStatus::Idle;
                }
                logger::log_success(&format!("Oturum token'ı yenilendi: {}", acc_name));
            } else {
                let mut new_acc = Account::new(self.next_id);
                self.next_id += 1;
                new_acc.username = acc_name.clone();
                new_acc.refresh_token = Some(token.clone());
                new_acc.custom_title = "Half-Life 3 (Beta)".into();
                new_acc.persona_state = PersonaState::Online;
                new_acc.games = vec![
                    GameEntry::new(730, "Counter-Strike 2"),
                    GameEntry::new(440, "Team Fortress 2"),
                ];
                let arc = Arc::new(Mutex::new(new_acc.clone()));
                self.runtime_accounts.push(arc);
                self.config.accounts.push(new_acc);
                logger::log_success(&format!("New account saved via QR: {}", acc_name));
            }
            self.config.save();
            self.qr_auth.stop();
            self.show_qr_modal = false;
        }

        if close_requested {
            self.qr_auth.stop();
            self.show_qr_modal = false;
        }
    }
}

impl eframe::App for SteamBoosterApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.set_visuals(egui::Visuals {
            dark_mode: true,
            panel_fill: Color32::from_rgb(18, 20, 28),
            window_fill: Color32::from_rgb(22, 26, 36),
            ..Default::default()
        });

        // Kullanıcı arayüz ölçeklendirmesini uygula
        ctx.set_pixels_per_point(self.config.settings.ui_scale.clamp(0.8, 2.0));

        let s = get_strings(self.config.settings.language);

        // AppID lookup asenkron sonucunu kontrol et
        if let Some((id, name)) = self.app_lookup_result.lock().unwrap().take() {
            self.app_lookup_loading = false;
            if let Ok(cur_id) = self.custom_app_id_input.trim().parse::<u32>() {
                if cur_id == id {
                    if !name.is_empty() {
                        self.custom_game_name_input = name.clone();
                        self.app_lookup_msg = Some(format!("Found: {}", name));
                    } else {
                        self.app_lookup_msg = Some("AppID not found in Steam Store.".to_string());
                    }
                }
            }
        }

        self.sync_from_runtime();
        ctx.request_repaint_after(std::time::Duration::from_millis(500));

        // Sistem Tepsisi (Tray) durumunu güncelle ve tıklama olaylarını dinle
        if let Some(tray) = &mut self.tray_mgr {
            tray.update_status(&self.runtime_accounts);
            if let Some(action) = tray.poll_events() {
                match action {
                    crate::tray::TrayAction::OpenWindow => {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                    }
                    crate::tray::TrayAction::StartAll => {
                        for acc in &self.runtime_accounts {
                            steam::start_boost(acc.clone());
                        }
                    }
                    crate::tray::TrayAction::StopAll => {
                        for acc in &self.runtime_accounts {
                            let id = acc.lock().unwrap().id;
                            steam::stop_boost(id, acc.clone());
                        }
                    }
                    crate::tray::TrayAction::Quit => {
                        std::process::exit(0);
                    }
                }
            }
        }

        // Pencere kapatma (X) butonuna basıldığında tepsiye küçült
        if ctx.input(|i| i.viewport().close_requested()) {
            if self.config.settings.minimize_to_tray {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            }
        }

        egui::SidePanel::left("sidebar")
            .exact_width(190.0)
            .resizable(false)
            .show(ctx, |ui| {
                self.draw_sidebar(ui, &s);
            });

        egui::CentralPanel::default().show(ctx, |ui| match self.active_tab {
            Tab::Accounts => self.draw_accounts_tab(ui, &s),
            Tab::Dashboard => self.draw_dashboard_tab(ui, &s),
            Tab::Settings => self.draw_settings_tab(ui, &s),
            Tab::Logs => self.draw_logs_tab(ui, &s, ctx),
            Tab::Debug => self.draw_debug_tab(ui, &s, ctx),
        });

        self.draw_qr_modal(ctx, &s);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        for arc in &self.runtime_accounts {
            let id = arc.lock().unwrap().id;
            steam::stop_boost(id, arc.clone());
        }
        self.config.save();
    }
}

fn urlencoding_encode(s: &str) -> String {
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
