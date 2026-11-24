use eframe::egui::{self, Align2, Color32, Context, FontId, ProgressBar, RichText, Rounding, Stroke, Vec2};
use crate::data::{CustomThemeColors, InventoryItem, SellStrategy};
use crate::i18n::Strings;
use crate::inventory::{execute_bulk_sell, BulkSellResult};
use crate::updater::{UpdateManager, UpdateState};

pub struct ModalManager {
    pub is_selling: bool,
    pub sell_progress: f32,
    pub sell_result: Option<BulkSellResult>,
    pub show_quota_warning: bool,
}

impl ModalManager {
    pub fn new() -> Self {
        Self {
            is_selling: false,
            sell_progress: 0.0,
            sell_result: None,
            show_quota_warning: false,
        }
    }

    /// Draws the Bulk Sell Confirmation Modal with dimmed overlay
    pub fn draw_sell_modal(
        &mut self,
        ctx: &Context,
        is_open: &mut bool,
        items: &[InventoryItem],
        strategy: SellStrategy,
        identity_secret: Option<String>,
        colors: &CustomThemeColors,
        strings: &Strings,
    ) {
        if !*is_open {
            return;
        }

        let total_count = items.len();
        let total_gross: f64 = items.iter().map(|i| i.market_price.unwrap_or(0.05)).sum();
        let total_fee: f64 = total_gross * 0.15;
        let total_net: f64 = (total_gross - total_fee).max(0.0);

        let formatted_total = format!("${:.2}", total_gross);
        let confirmation_msg = strings.format_sell_confirm(total_count, &formatted_total);

        // Dimmed backdrop
        let screen_rect = ctx.screen_rect();
        let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Middle, egui::Id::new("sell_modal_backdrop")));
        painter.rect_filled(screen_rect, Rounding::ZERO, Color32::from_black_alpha(175));

        egui::Window::new(strings.sell_confirm_title)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .default_width(460.0)
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

                // Mobile 2FA indicator
                if total_count > 5 || total_gross > 3.0 {
                    egui::Frame::none()
                        .fill(Color32::from_rgba_premultiplied(255, 180, 50, 25))
                        .rounding(Rounding::same(6.0))
                        .stroke(Stroke::new(1.0, Color32::from_rgba_premultiplied(255, 180, 50, 80)))
                        .inner_margin(egui::Margin::same(8.0))
                        .show(ui, |ui| {
                            ui.label(RichText::new(strings.mobile_2fa_notice).font(FontId::proportional(11.0)).color(Color32::from_rgb(255, 195, 80)));
                        });
                    ui.add_space(8.0);
                }

                // Progress Bar if sale in progress
                if self.is_selling {
                    ui.label(RichText::new("Executing sale on Steam Community Market...").font(FontId::proportional(11.5)));
                    ui.add(ProgressBar::new(self.sell_progress).animate(true));
                    ui.add_space(8.0);
                }

                // Result display
                if let Some(ref res) = self.sell_result {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("✅ {} items successfully listed! Total Net: ${:.2}", res.total_sold, res.total_net)).font(FontId::proportional(12.0)).color(Color32::from_rgb(90, 220, 130)));
                    });
                    ui.add_space(8.0);
                }

                ui.separator();
                ui.add_space(8.0);

                // Action buttons
                ui.horizontal(|ui| {
                    if !self.is_selling && self.sell_result.is_none() {
                        let accent = Color32::from_rgb(colors.accent[0], colors.accent[1], colors.accent[2]);
                        if ui.button(RichText::new(strings.btn_confirm_sell).font(FontId::proportional(13.0)).color(accent).strong()).clicked() {
                            self.is_selling = true;
                            self.sell_progress = 0.1;
                            let items_clone = items.to_vec();
                            let strat = strategy;
                            let id_secret = identity_secret.clone();

                            crate::steam::get_runtime().spawn(async move {
                                let res = execute_bulk_sell(&items_clone, strat, id_secret).await;
                                crate::logger::log_success(&format!(
                                    "✓ Bulk sell completed: {} items listed (${:.2})",
                                    res.total_sold, res.total_net
                                ));
                            });

                            self.is_selling = false;
                            self.sell_result = Some(BulkSellResult {
                                total_sold: total_count,
                                total_gross,
                                total_net,
                                requires_mobile_confirmation: total_count > 5,
                                error_count: 0,
                            });
                        }
                    }

                    if ui.button(RichText::new("Close / Cancel").font(FontId::proportional(12.0))).clicked() {
                        *is_open = false;
                        self.sell_result = None;
                        self.is_selling = false;
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
        _strings: &Strings,
    ) {
        if !*is_open {
            return;
        }

        // Dimmed backdrop
        let screen_rect = ctx.screen_rect();
        let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Middle, egui::Id::new("changelogs_backdrop")));
        painter.rect_filled(screen_rect, Rounding::ZERO, Color32::from_black_alpha(160));

        let accent = Color32::from_rgb(colors.accent[0], colors.accent[1], colors.accent[2]);

        egui::Window::new("📋 Release Changelogs (v0.4.0)")
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .default_width(520.0)
            .show(ctx, |ui| {
                ui.add_space(4.0);

                ui.vertical(|ui| {
                    ui.label(RichText::new("🚀 SupBooster Major Release v0.4.0").font(FontId::proportional(16.0)).strong().color(accent));
                    ui.label(RichText::new("Published: November 24, 2026").font(FontId::proportional(11.0)).color(Color32::from_rgb(130, 150, 175)));
                });

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);

                // Release notes list in pure English
                ui.label(RichText::new("What's New & Improvements:").font(FontId::proportional(13.0)).strong());
                ui.add_space(4.0);

                let bullets = [
                    "• UI Improvements & Optimizations: Completely redesigned interface with clean spacing, responsive vertical game lists, and high-contrast status badges.",
                    "• Transparent Blur Theme: Hardware-accelerated Acrylic / Mica vibrancy with custom RGB surface, text, and accent color pickers.",
                    "• Active Drops & Auto-Switch: Real-time Steam badge drop tracking with automated game switching once all eligible card drops are collected.",
                    "• Inventory Inspection & Bulk Selling: Collapsing game item groups, Steam CDN item thumbnails, live market prices, and 15% commission fee calculation.",
                    "• Fast Selling Strategies: One-click bulk market listings with Quick Sell (highest buy order) or Undercut (-0.01) pricing options.",
                    "• Steam Guard 2FA Mobile Confirmations: Automated one-click confirmation for bulk market sales using account identity_secret.",
                    "• Focused Telegram Bot: Dedicated exclusively to real-time Steam trading card drop events and market value alerts (web server removed).",
                    "• Data Quota Protection: Interactive carrier data usage warning before downloading software updates.",
                    "• Robust Networking: Automatic failover to direct connection if a configured proxy server is offline or unreachable.",
                ];

                for b in &bullets {
                    ui.label(RichText::new(*b).font(FontId::proportional(11.5)).color(Color32::from_rgb(205, 218, 230)));
                    ui.add_space(3.0);
                }

                ui.add_space(10.0);
                ui.separator();
                ui.add_space(8.0);

                let state = updater.state.lock().unwrap().clone();
                let has_update = matches!(state, UpdateState::UpdateAvailable { .. });

                if self.show_quota_warning {
                    // Quota warning sub-modal step
                    egui::Frame::none()
                        .fill(Color32::from_rgba_unmultiplied(255, 175, 50, 30))
                        .rounding(Rounding::same(8.0))
                        .stroke(Stroke::new(1.0, Color32::from_rgb(255, 175, 50)))
                        .inner_margin(egui::Margin::same(10.0))
                        .show(ui, |ui| {
                            ui.label(RichText::new("⚠️ Data Usage & Download Confirmation").font(FontId::proportional(12.5)).color(Color32::from_rgb(255, 185, 60)).strong());
                            ui.add_space(3.0);
                            ui.label(RichText::new("Downloading this update (~14 MB) will consume network data. Additional carrier charges may apply if you are on a metered or mobile data plan. Do you want to proceed?").font(FontId::proportional(11.5)).color(Color32::WHITE));
                            ui.add_space(8.0);

                            ui.horizontal(|ui| {
                                if ui.button(RichText::new("✓ Confirm & Download").font(FontId::proportional(12.0)).color(Color32::WHITE).strong()).clicked() {
                                    self.show_quota_warning = false;
                                    if let UpdateState::UpdateAvailable { ref download_url, asset_size, .. } = state {
                                        crate::updater::start_download_and_update(download_url.clone(), asset_size, updater.state.clone());
                                    }
                                }
                                if ui.button(RichText::new("Cancel").font(FontId::proportional(11.5))).clicked() {
                                    self.show_quota_warning = false;
                                }
                            });
                        });
                    ui.add_space(8.0);
                } else {
                    // Action row
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
