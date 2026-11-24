use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use eframe::egui::{self, Color32, FontId, RichText, Vec2};

use crate::data::{Account, AccountStatus, AppConfig, InventoryItem, ThemeMode};
use crate::i18n::{get_strings, Strings};
use crate::inventory::get_cached_active_drops;
use crate::logger;
use crate::steam;
use crate::ui::accounts::AccountsView;
use crate::ui::active_drops::ActiveDropsView;
use crate::ui::dashboard::DashboardView;
use crate::ui::inventory::InventoryView;
use crate::ui::modals::ModalManager;
use crate::ui::settings::SettingsView;
use crate::ui::sidebar::{NavTab, Sidebar};
use crate::ui::telegram::TelegramView;
use crate::ui::theme::{apply_platform_vibrancy, ThemeEngine};
use crate::ui::TopStatusBar;

pub struct SteamBoosterApp {
    config: AppConfig,
    active_tab: NavTab,

    // Runtime accounts & identifier
    runtime_accounts: Vec<Arc<Mutex<Account>>>,
    next_id: u64,

    // QR Code Login
    qr_auth: crate::qr_auth::QrAuthManager,
    show_qr_modal: bool,

    // Modals
    show_changelogs_modal: bool,
    show_sell_modal: bool,
    modal_selected_items: Vec<InventoryItem>,
    modal_mgr: ModalManager,

    // View Components
    active_drops_view: ActiveDropsView,
    inventory_view: InventoryView,
    accounts_view: AccountsView,
    telegram_view: TelegramView,
    settings_view: SettingsView,

    // System Tray
    tray_mgr: Option<crate::tray::TrayManager>,

    // Updater
    updater: crate::updater::UpdateManager,

    // State Tracking
    last_theme_mode: Option<ThemeMode>,
    last_auto_switch_check: Instant,
}

impl SteamBoosterApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let config = AppConfig::load();
        let next_id = config.accounts.iter().map(|a| a.id).max().unwrap_or(0) + 1;
        let runtime_accounts: Vec<Arc<Mutex<Account>>> = config
            .accounts
            .iter()
            .cloned()
            .map(|a| Arc::new(Mutex::new(a)))
            .collect();

        // Register accounts for Telegram Bot and start service
        crate::telegram::register_accounts_for_bot(runtime_accounts.clone());
        crate::telegram::init_telegram_bot();

        // Initialize System Tray
        let mut tray_mgr = crate::tray::TrayManager::new();
        if let Some(tray) = &mut tray_mgr {
            tray.update_status(&runtime_accounts);
        }

        // Apply platform vibrancy if supported
        #[cfg(target_os = "windows")]
        apply_platform_vibrancy(cc, config.settings.theme_mode, &config.settings.custom_colors);

        // Auto-start boost on launch if enabled
        if config.settings.auto_start_boost_on_launch {
            for acc in &runtime_accounts {
                steam::start_boost(acc.clone());
            }
            logger::log_info("Auto-start: All accounts started boosting automatically on launch.");
        }

        logger::log_info(&format!("SupBooster v0.4 started. {} accounts loaded.", config.accounts.len()));

        // Cleanup old executables from previous updates
        crate::updater::cleanup_old_executable();

        // Check for updates via GitHub releases on startup
        let updater = crate::updater::UpdateManager::new();
        crate::updater::check_for_updates(updater.state.clone(), config.settings.github_repo.clone());

        Self {
            config,
            active_tab: NavTab::Dashboard,
            runtime_accounts,
            next_id,
            qr_auth: crate::qr_auth::QrAuthManager::new(),
            show_qr_modal: false,
            show_changelogs_modal: false,
            show_sell_modal: false,
            modal_selected_items: Vec::new(),
            modal_mgr: ModalManager::new(),
            active_drops_view: ActiveDropsView::new(),
            inventory_view: InventoryView::new(),
            accounts_view: AccountsView::new(),
            telegram_view: TelegramView::new(),
            settings_view: SettingsView::new(),
            tray_mgr,
            updater,
            last_theme_mode: None,
            last_auto_switch_check: Instant::now(),
        }
    }

    /// Syncs state changes from background runtime threads back into persistent config
    fn sync_from_runtime(&mut self) {
        let mut save_needed = false;
        for arc in &self.runtime_accounts {
            let rt = arc.lock().unwrap();
            if let Some(acc) = self.config.accounts.iter_mut().find(|a| a.id == rt.id) {
                acc.status = rt.status.clone();
                acc.hours_boosted = rt.hours_boosted;
                if acc.steam_id != rt.steam_id && rt.steam_id != 0 {
                    acc.steam_id = rt.steam_id;
                    save_needed = true;
                }
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

    /// Auto-Switch feature: when a game's remaining card drops hit 0, automatically switch to the next eligible game
    fn check_auto_switch_drops(&mut self) {
        if !self.config.settings.auto_switch_drops {
            return;
        }

        if self.last_auto_switch_check.elapsed() < Duration::from_secs(5) {
            return;
        }
        self.last_auto_switch_check = Instant::now();

        for arc in &self.runtime_accounts {
            let (is_boosting, steam_id, username, mut games) = {
                let a = arc.lock().unwrap();
                (
                    matches!(a.status, AccountStatus::Boosting),
                    a.steam_id,
                    a.username.clone(),
                    a.games.clone(),
                )
            };

            if is_boosting && steam_id != 0 && !games.is_empty() {
                let cached_drops = get_cached_active_drops(steam_id);
                if cached_drops.is_empty() {
                    continue;
                }

                let mut modified = false;

                // Check if any game currently being boosted has remaining_cards == 0
                for i in (0..games.len()).rev() {
                    let g_appid = games[i].app_id;
                    if let Some(drop_info) = cached_drops.iter().find(|d| d.app_id == g_appid) {
                        if drop_info.remaining_cards == 0 {
                            logger::log_info(&format!(
                                "[Auto-Switch] [{}] '{}' has 0 card drops remaining. Finding next game...",
                                username, games[i].name
                            ));
                            games.remove(i);
                            modified = true;
                        }
                    }
                }

                // If games were removed, find the next game in active drops not yet in games
                if modified {
                    for drop in &cached_drops {
                        if drop.remaining_cards > 0 && !games.iter().any(|g| g.app_id == drop.app_id) {
                            logger::log_success(&format!(
                                "[Auto-Switch] [{}] Automatically switched to '{}' (AppID: {}, {} cards left)",
                                username, drop.name, drop.app_id, drop.remaining_cards
                            ));
                            games.push(crate::data::GameEntry::new(drop.app_id, drop.name.clone()));
                            break;
                        }
                    }

                    let (acc_id, title, persona, afk) = {
                        let mut a = arc.lock().unwrap();
                        a.games = games.clone();
                        (a.id, a.custom_title.clone(), a.persona_state, a.afk_reply_custom.clone())
                    };

                    // Hot-reload presence for the running task
                    steam::update_presence(acc_id, games, title, persona, afk);
                }
            }
        }
    }

    /// Draws Steam QR Code Authentication Modal
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

        if matches!(status, crate::qr_auth::QrAuthStatus::Idle) {
            self.qr_auth.start();
        }

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
                            ui.label(RichText::new(s.qr_loading).size(14.0).color(Color32::from_gray(180)));
                        }
                        crate::qr_auth::QrAuthStatus::WaitingForScan { modules, width, .. } => {
                            ui.label(RichText::new(s.qr_scan_title).font(FontId::proportional(16.0)).color(Color32::WHITE));
                            ui.add_space(4.0);
                            ui.label(RichText::new(s.qr_scan_sub).size(12.0).color(Color32::from_gray(150)));
                            ui.add_space(12.0);

                            // Draw vector QR code
                            let qr_box_size = 220.0_f32;
                            let (rect, _) = ui.allocate_exact_size(Vec2::splat(qr_box_size), egui::Sense::hover());
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
                                                egui::pos2(offset_x + (x as f32) * cell_size, offset_y + (y as f32) * cell_size),
                                                Vec2::splat(cell_size.ceil()),
                                            );
                                            painter.rect_filled(cell_rect, 0.0, Color32::BLACK);
                                        }
                                    }
                                }
                            }

                            ui.add_space(14.0);
                            ui.label(RichText::new(s.qr_waiting).size(12.0).color(Color32::from_rgb(255, 180, 50)));
                        }
                        crate::qr_auth::QrAuthStatus::Success { account_name, .. } => {
                            ui.add_space(50.0);
                            ui.label(RichText::new(s.qr_success).font(FontId::proportional(18.0)).color(Color32::from_rgb(80, 200, 80)));
                            ui.add_space(8.0);
                            ui.label(RichText::new(format!("{}: {}", s.qr_welcome, account_name)).size(14.0).color(Color32::WHITE));
                            ui.add_space(20.0);
                            if ui.button(RichText::new(s.qr_close).size(13.0)).clicked() {
                                close_requested = true;
                            }
                        }
                        crate::qr_auth::QrAuthStatus::Error(e) => {
                            ui.add_space(60.0);
                            ui.label(RichText::new(format!("Hata: {}", e)).size(13.0).color(Color32::from_rgb(220, 80, 80)));
                            ui.add_space(16.0);
                            if ui.button(RichText::new(s.qr_retry).size(13.0)).clicked() {
                                self.qr_auth.start();
                            }
                        }
                    }

                    if !matches!(&status, crate::qr_auth::QrAuthStatus::Success { .. }) {
                        ui.add_space(16.0);
                        if ui.button(RichText::new(s.qr_close).size(12.0)).clicked() {
                            close_requested = true;
                        }
                    }
                });
            });

        if let Some((account_name, refresh_token)) = success_account {
            let existing = self.config.accounts.iter_mut().find(|a| a.username.to_lowercase() == account_name.to_lowercase());
            if let Some(acc) = existing {
                acc.refresh_token = Some(refresh_token);
                logger::log_success(&format!("Token refreshed via QR for account '{}'.", account_name));
            } else {
                let mut new_acc = Account::new(self.next_id);
                self.next_id += 1;
                new_acc.username = account_name.clone();
                new_acc.refresh_token = Some(refresh_token);
                self.config.accounts.push(new_acc.clone());
                let arc = Arc::new(Mutex::new(new_acc));
                self.runtime_accounts.push(arc.clone());
                crate::telegram::update_accounts_for_bot(&self.runtime_accounts);
                logger::log_success(&format!("New account '{}' added via QR code login.", account_name));
            }
            self.config.save();
            close_requested = true;
        }

        if close_requested {
            self.qr_auth.stop();
            self.show_qr_modal = false;
        }
    }
}

impl eframe::App for SteamBoosterApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Apply theme and custom colors to egui
        ThemeEngine::apply_theme(ctx, self.config.settings.theme_mode, &self.config.settings.custom_colors);
        if self.last_theme_mode != Some(self.config.settings.theme_mode) {
            self.last_theme_mode = Some(self.config.settings.theme_mode);
        }

        // Apply UI scale factor
        ctx.set_pixels_per_point(self.config.settings.ui_scale.clamp(0.8, 2.0));

        let strings = get_strings(self.config.settings.language);

        // Periodic state synchronization and auto-switch check
        self.sync_from_runtime();
        self.check_auto_switch_drops();

        ctx.request_repaint_after(Duration::from_millis(500));

        // Poll System Tray actions
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
                            let id = arc_id(acc);
                            steam::stop_boost(id, acc.clone());
                        }
                    }
                    crate::tray::TrayAction::Quit => {
                        std::process::exit(0);
                    }
                }
            }
        }

        // Minimize to tray on close if enabled
        if ctx.input(|i| i.viewport().close_requested()) {
            if self.config.settings.minimize_to_tray {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            }
        }

        // ─── 1. TOP STATUS BAR ───
        egui::TopBottomPanel::top("top_status_bar")
            .exact_height(46.0)
            .show(ctx, |ui| {
                TopStatusBar::draw(
                    ui,
                    &self.runtime_accounts,
                    &self.config.settings.custom_colors,
                    self.config.settings.theme_mode,
                    &strings,
                );
            });

        // ─── 2. LEFT SIDEBAR (Fixed 200px width) ───
        egui::SidePanel::left("left_sidebar")
            .exact_width(200.0)
            .resizable(false)
            .show(ctx, |ui| {
                Sidebar::draw(
                    ui,
                    &mut self.active_tab,
                    &self.config.settings.custom_colors,
                    &strings,
                    &self.updater,
                    &mut self.show_changelogs_modal,
                );
            });

        // ─── 3. CENTRAL PANEL (Active Tab Router) ───
        egui::CentralPanel::default().show(ctx, |ui| {
            match self.active_tab {
                NavTab::Dashboard => {
                    DashboardView::draw(
                        ui,
                        &self.runtime_accounts,
                        &self.config.settings.custom_colors,
                        self.config.settings.theme_mode,
                        &strings,
                    );
                }
                NavTab::ActiveDrops => {
                    self.active_drops_view.draw(
                        ui,
                        &self.runtime_accounts,
                        &self.config.settings.custom_colors,
                        self.config.settings.theme_mode,
                        &strings,
                        &mut self.config.settings.auto_switch_drops,
                    );
                }
                NavTab::Inventory => {
                    self.inventory_view.draw(
                        ui,
                        &self.runtime_accounts,
                        &self.config.settings.custom_colors,
                        self.config.settings.theme_mode,
                        &strings,
                        &mut self.show_sell_modal,
                        &mut self.modal_selected_items,
                    );
                }
                NavTab::Accounts => {
                    self.accounts_view.draw(
                        ui,
                        &mut self.config.accounts,
                        &self.runtime_accounts,
                        &self.config.settings.custom_colors,
                        self.config.settings.theme_mode,
                        &strings,
                        &mut self.next_id,
                        &mut self.show_qr_modal,
                    );
                }
                NavTab::Telegram => {
                    self.telegram_view.draw(
                        ui,
                        &mut self.config.settings,
                        &strings,
                    );
                }
                NavTab::Settings => {
                    self.settings_view.draw(
                        ui,
                        &mut self.config.settings,
                        &self.updater,
                        &strings,
                    );
                }
            }
        });

        // ─── 4. MODALS ───
        // Bulk Sell Confirmation Modal
        let (active_token, active_steam_id, active_proxy, active_identity_secret) = {
            let idx = self.inventory_view.selected_account_index;
            if let Some(acc_arc) = self.runtime_accounts.get(idx) {
                let a = acc_arc.lock().unwrap();
                (a.refresh_token.clone(), a.steam_id, a.proxy.clone(), a.identity_secret.clone())
            } else if let Some(acc) = self.config.accounts.get(idx) {
                (acc.refresh_token.clone(), acc.steam_id, acc.proxy.clone(), acc.identity_secret.clone())
            } else {
                (None, 0, None, None)
            }
        };

        if let Ok(lock) = self.modal_mgr.sell_result.lock() {
            if let Some(ref res) = *lock {
                if !res.sold_asset_ids.is_empty() {
                    for sold_id in &res.sold_asset_ids {
                        self.inventory_view.selected_asset_ids.remove(sold_id);
                    }
                }
            }
        }

        self.modal_mgr.draw_sell_modal(
            ctx,
            &mut self.show_sell_modal,
            &self.modal_selected_items,
            self.inventory_view.sell_strategy,
            active_token,
            active_steam_id,
            active_proxy,
            active_identity_secret,
            &self.config.settings.custom_colors,
            &strings,
        );

        // Changelogs & Quota Warning Modal
        self.modal_mgr.draw_changelogs_modal(
            ctx,
            &mut self.show_changelogs_modal,
            &self.updater,
            &self.config.settings.custom_colors,
            &strings,
        );

        // Steam QR Auth Modal
        self.draw_qr_modal(ctx, &strings);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        for arc in &self.runtime_accounts {
            let id = arc_id(arc);
            steam::stop_boost(id, arc.clone());
        }
        self.config.save();
    }
}

fn arc_id(arc: &Arc<Mutex<Account>>) -> u64 {
    arc.lock().unwrap().id
}
