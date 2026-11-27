use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use eframe::egui::{self, Align2, Color32, Context, FontId, RichText, Rounding, Stroke, Vec2};
use crate::data::{CustomThemeColors, InventoryItem, SellStrategy};
use crate::i18n::Strings;
use crate::inventory::{calculate_seller_receives, execute_bulk_sell, BulkSellResult};
use crate::updater::{UpdateManager, UpdateState};

pub struct ModalManager {
    pub is_selling: Arc<AtomicBool>,
    pub sell_result: Arc<Mutex<Option<BulkSellResult>>>,
    pub sell_status_msg: Arc<Mutex<String>>,
    pub show_quota_warning: bool,
    pub balance_credited: bool,
}

impl ModalManager {
    pub fn new() -> Self {
        Self {
            is_selling: Arc::new(AtomicBool::new(false)),
            sell_result: Arc::new(Mutex::new(None)),
            sell_status_msg: Arc::new(Mutex::new(String::new())),
            show_quota_warning: false,
            balance_credited: false,
        }
    }

    /// Draws the Bulk Sell Confirmation Modal without darkened backdrop
    pub fn draw_sell_modal(
        &mut self,
        ctx: &Context,
        is_open: &mut bool,
        items: &[InventoryItem],
        strategy: SellStrategy,
        custom_offset: f64,
        refresh_token: Option<String>,
        steam_id: u64,
        proxy_url: Option<String>,
        identity_secret: Option<String>,
        colors: &CustomThemeColors,
        strings: &Strings,
    ) {
        if !*is_open {
            return;
        }

        let total_count = items.len();
        let total_gross: f64 = items
            .iter()
            .map(|i| strategy.compute_price(i.market_price.unwrap_or(0.05), custom_offset))
            .sum();
        let total_net: f64 = items
            .iter()
            .map(|i| calculate_seller_receives(strategy.compute_price(i.market_price.unwrap_or(0.05), custom_offset)))
            .sum();
        let total_fee: f64 = (total_gross - total_net).max(0.0);

        let formatted_total = format!("${:.2}", total_gross);
        let confirmation_msg = strings.format_sell_confirm(total_count, &formatted_total);

        let is_selling = self.is_selling.load(Ordering::Relaxed);
        let result_opt = self.sell_result.lock().unwrap().clone();
        let status_msg = self.sell_status_msg.lock().unwrap().clone();

        egui::Window::new(strings.sell_confirm_title)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .default_width(480.0)
            .show(ctx, |ui| {
                ui.add_space(8.0);

                // Prominent confirmation message
                ui.vertical_centered(|ui| {
                    ui.label(
                        RichText::new(&confirmation_msg)
                            .font(FontId::proportional(14.5))
                            .color(Color32::WHITE)
                            .strong(),
                    );
                });

                ui.add_space(12.0);
                ui.separator();
                ui.add_space(8.0);

                // Breakdown table
                ui.columns(2, |cols| {
                    cols[0].vertical(|ui| {
                        ui.label(RichText::new(strings.sell_summary_gross).font(FontId::proportional(12.0)));
                        ui.label(RichText::new(strings.sell_summary_fee).font(FontId::proportional(12.0)));
                        ui.label(RichText::new(strings.sell_summary_net).font(FontId::proportional(13.0)).strong());
                    });

                    cols[1].vertical(|ui| {
                        ui.label(RichText::new(format!("${:.2}", total_gross)).font(FontId::proportional(12.0)).color(Color32::from_rgb(180, 205, 235)));
                        ui.label(RichText::new(format!("-${:.2}", total_fee)).font(FontId::proportional(12.0)).color(Color32::from_rgb(255, 120, 120)));
                        ui.label(RichText::new(format!("${:.2}", total_net)).font(FontId::proportional(13.0)).color(Color32::from_rgb(90, 220, 130)).strong());
                    });
                });

                ui.add_space(8.0);

                // Selling in progress view
                if is_selling {
                    egui::Frame::none()
                        .fill(Color32::from_rgba_unmultiplied(20, 30, 45, 230))
                        .rounding(Rounding::same(8.0))
                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(90, 160, 240)))
                        .inner_margin(egui::Margin::same(10.0))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.add_space(6.0);
                                ui.label(RichText::new(if status_msg.is_empty() { "Connecting to Steam Market & Listing Items..." } else { &status_msg })
                                    .font(FontId::proportional(12.0)).color(Color32::from_rgb(120, 200, 255)));
                            });
                        });
                    ui.add_space(8.0);
                    ctx.request_repaint();
                }

                // Result display
                if let Some(ref res) = result_opt {
                    if res.total_sold > 0 {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(format!("✅ {} items successfully listed! Total Net: ${:.2}", res.total_sold, res.total_net))
                                .font(FontId::proportional(12.5)).color(Color32::from_rgb(90, 220, 130)).strong());
                        });
                        ui.add_space(8.0);
                    }

                    // Mobile 2FA indicator - Highly visible alert when items require phone confirmation
                    if res.requires_mobile_confirmation || res.mobile_confirmation_count > 0 {
                        egui::Frame::none()
                            .fill(Color32::from_rgba_unmultiplied(20, 38, 26, 235))
                            .rounding(Rounding::same(8.0))
                            .stroke(Stroke::new(1.5_f32, Color32::from_rgb(70, 220, 130)))
                            .inner_margin(egui::Margin::same(12.0))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new("📱").font(FontId::proportional(26.0)));
                                    ui.add_space(4.0);
                                    ui.vertical(|ui| {
                                        ui.label(RichText::new("Steam Guard Mobile Confirmation Sent!").font(FontId::proportional(13.5)).strong().color(Color32::from_rgb(90, 240, 150)));
                                        ui.add_space(2.0);
                                        ui.label(RichText::new("A confirmation notification was sent to your phone's Steam app. Please open Steam -> Confirmations on your mobile device to approve the market listings.")
                                            .font(FontId::proportional(11.5)).color(Color32::from_rgb(215, 240, 225)));
                                    });
                                });
                            });
                        ui.add_space(8.0);
                    }

                    if res.error_count > 0 {
                        egui::Frame::none()
                            .fill(Color32::from_rgba_unmultiplied(40, 20, 20, 220))
                            .rounding(Rounding::same(6.0))
                            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(240, 90, 90)))
                            .inner_margin(egui::Margin::same(8.0))
                            .show(ui, |ui| {
                                let err_text = res.error_message.as_deref().unwrap_or("Failed to list some items on Steam Market");
                                ui.label(RichText::new(format!("⚠️ {}", err_text)).font(FontId::proportional(11.5)).color(Color32::from_rgb(255, 140, 130)));
                            });
                        ui.add_space(8.0);
                    }
                }

                ui.separator();
                ui.add_space(8.0);

                // Action buttons
                ui.horizontal(|ui| {
                    if !is_selling && result_opt.is_none() {
                        let accent = Color32::from_rgb(colors.accent[0], colors.accent[1], colors.accent[2]);
                        if ui.button(RichText::new(strings.btn_confirm_sell).font(FontId::proportional(13.0)).color(accent).strong()).clicked() {
                            self.is_selling.store(true, Ordering::Relaxed);
                            self.balance_credited = false;
                            *self.sell_status_msg.lock().unwrap() = "Authenticating Steam Web Session...".to_string();
                            *self.sell_result.lock().unwrap() = None;

                            let is_selling_clone = self.is_selling.clone();
                            let result_clone = self.sell_result.clone();
                            let status_clone = self.sell_status_msg.clone();
                            let items_clone = items.to_vec();
                            let strat = strategy;
                            let offset = custom_offset;
                            let token_clone = refresh_token.clone();
                            let proxy_clone = proxy_url.clone();
                            let id_secret_clone = identity_secret.clone();
                            let s_id = steam_id;

                            crate::steam::get_runtime().spawn(async move {
                                if let Ok(mut lock) = status_clone.lock() {
                                    *lock = "Listing items on Steam Community Market...".to_string();
                                }
                                let res = execute_bulk_sell(
                                    &items_clone,
                                    strat,
                                    offset,
                                    token_clone,
                                    s_id,
                                    proxy_clone,
                                    id_secret_clone,
                                ).await;

                                crate::logger::log_success(&format!(
                                    "✓ Bulk sell finished: {} items listed (${:.2}), {} errors",
                                    res.total_sold, res.total_net, res.error_count
                                ));

                                if let Ok(mut lock) = result_clone.lock() {
                                    *lock = Some(res);
                                }
                                is_selling_clone.store(false, Ordering::Relaxed);
                            });
                        }
                    }

                    if ui.button(RichText::new("Close / Cancel").font(FontId::proportional(12.0))).clicked() {
                        *is_open = false;
                        *self.sell_result.lock().unwrap() = None;
                        self.is_selling.store(false, Ordering::Relaxed);
                    }
                });
            });
    }

    /// Draws the Changelogs & Update Quota Modal (Strictly English as requested)
    pub fn draw_changelogs_modal(
        &mut self,
        ctx: &Context,
        is_open: &mut bool,
        updater: &UpdateManager,
        colors: &CustomThemeColors,
        strings: &Strings,
    ) {
        if !*is_open {
            return;
        }

        let accent = Color32::from_rgb(colors.accent[0], colors.accent[1], colors.accent[2]);

        let state = updater.state.lock().unwrap().clone();
        let has_update = matches!(state, UpdateState::UpdateAvailable { .. });
        let current_ver = env!("CARGO_PKG_VERSION");

        let (window_title, header_text, sub_date) = match &state {
            UpdateState::UpdateAvailable { version, release_title, published_at, .. } => {
                let date_str = if published_at.len() >= 10 { &published_at[..10] } else { "Recent" };
                (
                    format!("🚀 New Version Changelogs (v{})", version),
                    release_title.clone(),
                    format!("Published: {}", date_str),
                )
            }
            _ => (
                format!("📋 Release Changelogs (v{})", current_ver),
                format!("SupBooster Official Release v{}", current_ver),
                "Current Version".to_string(),
            ),
        };

        egui::Window::new(window_title)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .default_width(540.0)
            .show(ctx, |ui| {
                ui.add_space(4.0);

                ui.vertical(|ui| {
                    ui.label(RichText::new(&header_text).font(FontId::proportional(16.0)).strong().color(accent));
                    ui.label(RichText::new(&sub_date).font(FontId::proportional(11.0)).color(Color32::from_rgb(130, 150, 175)));
                });

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);

                // Release notes list in pure English
                ui.label(RichText::new("What's New, Bug Fixes & Improvements:").font(FontId::proportional(13.0)).strong());
                ui.add_space(4.0);

                let bullets = [
                    "• SupBooster v1.4.0 Update — Live Behind-Window Liquid Glass & Blur Engine: Real-time compositor (PrintWindow + multi-monitor safe capture) that blurs active windows directly behind SupBooster, smooth 0.0-10.0 Blur Intensity slider (default 6.0), and renamed Blur mode.",
                    "• 4-Side Optical Glass Refraction & Borderless Resize: Symmetric 26px UV lens refraction and 5-ring perimeter shimmer across all 4 window edges, unified themed top/status bars, expanded frame padding, and native 8-direction Windows edge/corner resizing + Aero Snap.",
                    "• In-Game Orange Indicator, Waiting Fallback & AFK Fix: Immediate CMsgClientPlayingSessionState check pauses boosting when you are already in-game (orange indicator), enters Waiting/Reconnecting on transient Steam server drops, and eliminates duplicate AFK auto-replies.",
                    "• Trade Consolidate & Inventory Reliability Fix: Fixed (classid, instanceid) matching so trade-locked variants are never queued, increased inventory fetch limit to 500, fixed Cookie/sessionid handshakes, cleared sent assets from cache/selection, and added Steam (15)/(11) diagnostics.",
                    "• Core v1.3.0 Features Included: Prism UI dark/light contrast, Account Balance + Desktop CSV export, 4 Market Pricing strategies, Steam CM Ping monitor, Dedicated Logs tab, instant config save, and Telegram /lang & /addaccount.",
                ];

                for b in &bullets {
                    ui.label(RichText::new(*b).font(FontId::proportional(11.5)).color(Color32::from_rgb(205, 218, 230)));
                    ui.add_space(3.0);
                }

                ui.add_space(10.0);
                ui.separator();
                ui.add_space(8.0);

                if self.show_quota_warning {
                    egui::Frame::none()
                        .fill(Color32::from_rgba_unmultiplied(255, 175, 50, 30))
                        .rounding(Rounding::same(8.0))
                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(255, 175, 50)))
                        .inner_margin(egui::Margin::same(10.0))
                        .show(ui, |ui| {
                            ui.label(RichText::new(strings.quota_warning_title).font(FontId::proportional(12.5)).color(Color32::from_rgb(255, 185, 60)).strong());
                            ui.add_space(3.0);
                            ui.label(RichText::new(strings.quota_warning_msg).font(FontId::proportional(11.5)).color(Color32::WHITE));
                            ui.add_space(8.0);

                            ui.horizontal(|ui| {
                                if ui.button(RichText::new(strings.btn_proceed_download).font(FontId::proportional(12.0)).color(Color32::WHITE).strong()).clicked() {
                                    self.show_quota_warning = false;
                                    if let UpdateState::UpdateAvailable { ref download_url, asset_size, .. } = state {
                                        crate::updater::start_download_and_update(download_url.clone(), asset_size, updater.state.clone());
                                    }
                                }
                                if ui.button(RichText::new(strings.btn_cancel).font(FontId::proportional(11.5))).clicked() {
                                    self.show_quota_warning = false;
                                }
                            });
                        });
                    ui.add_space(8.0);
                } else {
                    ui.horizontal(|ui| {
                        if has_update {
                            if ui.button(RichText::new("🚀 Download Update").font(FontId::proportional(12.5)).color(accent).strong()).clicked() {
                                self.show_quota_warning = true;
                            }
                        }

                        if ui.button(RichText::new("Close").font(FontId::proportional(12.0))).clicked() {
                            *is_open = false;
                            self.show_quota_warning = false;
                        }
                    });
                }
            });
    }
}
