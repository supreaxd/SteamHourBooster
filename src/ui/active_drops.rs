use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use eframe::egui::{self, Color32, FontId, RichText, Rounding, ScrollArea, Stroke, Ui, Vec2};
use crate::data::{Account, ActiveDropGame, CustomThemeColors, GameEntry, ThemeMode};
use crate::i18n::Strings;
use crate::inventory::{fetch_active_drops, get_cached_active_drops};
use crate::ui::theme::ThemeEngine;

pub struct ActiveDropsView {
    pub search_query: String,
    pub selected_account_index: usize,
    pub is_scanning: Arc<AtomicBool>,
}

impl ActiveDropsView {
    pub fn new() -> Self {
        Self {
            search_query: String::new(),
            selected_account_index: 0,
            is_scanning: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn draw(
        &mut self,
        ui: &mut Ui,
        accounts: &[Arc<Mutex<Account>>],
        colors: &CustomThemeColors,
        theme_mode: ThemeMode,
        _strings: &Strings,
        auto_switch_enabled: &mut bool,
    ) {
        let accent = Color32::from_rgb(colors.accent[0], colors.accent[1], colors.accent[2]);

        if accounts.is_empty() {
            ThemeEngine::card_frame(theme_mode, colors).show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(20.0);
                    ui.label(RichText::new("⚠️ No registered Steam accounts found.").font(FontId::proportional(14.0)).color(Color32::from_rgb(255, 180, 60)));
                    ui.label(RichText::new("Add an account in the Accounts tab first to scan active card drops.").font(FontId::proportional(12.0)).color(Color32::from_rgb(120, 140, 160)));
                    ui.add_space(20.0);
                });
            });
            return;
        }

        if self.selected_account_index >= accounts.len() {
            self.selected_account_index = 0;
        }

        let current_acc_arc = accounts[self.selected_account_index].clone();
        let (current_username, current_steam_id, current_proxy, mut current_games) = {
            let a = current_acc_arc.lock().unwrap();
            (a.username.clone(), a.steam_id, a.proxy.clone(), a.games.clone())
        };

        let is_scan = self.is_scanning.load(Ordering::SeqCst);

        // ─── TOP CONTROL BAR ───
        ThemeEngine::card_frame(theme_mode, colors).show(ui, |ui| {
            ui.horizontal(|ui| {
                // Account selector
                ui.label(RichText::new("👤 Account:").font(FontId::proportional(12.5)).strong());
                egui::ComboBox::from_id_salt("active_drops_acc_combo")
                    .width(180.0)
                    .selected_text(format!("{} (ID: {})", current_username, current_steam_id))
                    .show_ui(ui, |ui| {
                        for (idx, arc) in accounts.iter().enumerate() {
                            let a = arc.lock().unwrap();
                            if ui.selectable_value(&mut self.selected_account_index, idx, &a.username).clicked() {
                                // Selection changed
                            }
                        }
                    });

                ui.add_space(10.0);

                // Search box
                ui.label(RichText::new("🔍").font(FontId::proportional(13.0)));
                ui.add(egui::TextEdit::singleline(&mut self.search_query).hint_text("Search games...").desired_width(160.0));

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Scan button
                    let scan_btn_label = if is_scan { "⏳ Scanning Drops..." } else { "🔄 Scan Drops" };
                    if ui.button(RichText::new(scan_btn_label).font(FontId::proportional(12.0))).clicked() && !is_scan {
                        let scan_arc = self.is_scanning.clone();
                        scan_arc.store(true, Ordering::SeqCst);
                        let sid = current_steam_id;
                        let proxy = current_proxy.clone();
                        let ctx_clone = ui.ctx().clone();
                        crate::steam::get_runtime().spawn(async move {
                            let _ = fetch_active_drops(sid, proxy).await;
                            scan_arc.store(false, Ordering::SeqCst);
                            ctx_clone.request_repaint();
                        });
                    }

                    // Add all drops button
                    if ui.button(RichText::new("⚡ Add All Active Drops").font(FontId::proportional(12.0)).color(accent)).clicked() {
                        let drops = get_cached_active_drops(current_steam_id);
                        let mut added_count = 0;
                        for d in drops {
                            if d.remaining_cards > 0 && !current_games.iter().any(|g| g.app_id == d.app_id) {
                                current_games.push(GameEntry::new(d.app_id, d.name));
                                added_count += 1;
                            }
                        }
                        if added_count > 0 {
                            let mut a = current_acc_arc.lock().unwrap();
                            a.games = current_games.clone();
                            crate::logger::log_success(&format!(
                                "[{}] ⚡ Added {} games with remaining card drops to boosting list!",
                                current_username, added_count
                            ));
                        }
                    }
                });
            });

            ui.add_space(6.0);

            // Auto-switch setting checkbox
            ui.horizontal(|ui| {
                ui.checkbox(auto_switch_enabled, RichText::new("Auto-Switch to next eligible drop game when remaining cards hit 0").font(FontId::proportional(11.5)));
            });
        });

        ui.add_space(8.0);

        // ─── DROPS LIST ───
        let active_drops = get_cached_active_drops(current_steam_id);

        let filtered_drops: Vec<&ActiveDropGame> = active_drops
            .iter()
            .filter(|d| {
                if self.search_query.trim().is_empty() {
                    true
                } else {
                    let q = self.search_query.to_lowercase();
                    d.name.to_lowercase().contains(&q) || d.app_id.to_string().contains(&q)
                }
            })
            .collect();

        if filtered_drops.is_empty() {
            ThemeEngine::card_frame(theme_mode, colors).show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(24.0);
                    if is_scan {
                        ui.label(RichText::new("⏳ Scanning Steam profile badges for card drops...").font(FontId::proportional(14.0)).color(Color32::from_rgb(140, 180, 240)));
                        ui.spinner();
                    } else if active_drops.is_empty() {
                        ui.label(RichText::new("🃏 No active card drops remaining on this account.").font(FontId::proportional(14.0)).color(Color32::from_rgb(140, 160, 180)));
                        ui.label(RichText::new("Click '🔄 Scan Drops' above to scan your Steam profile badges.").font(FontId::proportional(12.0)).color(Color32::from_rgb(110, 130, 150)));
                    } else {
                        ui.label(RichText::new("No games match the current search filter.").font(FontId::proportional(13.0)).color(Color32::from_rgb(130, 150, 175)));
                    }
                    ui.add_space(24.0);
                });
            });
            return;
        }

        ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            for drop in filtered_drops {
                let is_already_added = current_games.iter().any(|g| g.app_id == drop.app_id);

                ThemeEngine::card_frame(theme_mode, colors).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        // Game thumbnail from Steam CDN
                        let img_url = format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{}/capsule_sm_120.jpg", drop.app_id);
                        ui.add(egui::Image::new(img_url).fit_to_exact_size(Vec2::new(70.0, 32.0)).rounding(Rounding::same(4.0)));

                        ui.add_space(8.0);

                        // Game info
                        ui.vertical(|ui| {
                            ui.label(RichText::new(&drop.name).font(FontId::proportional(13.5)).strong());
                            ui.label(RichText::new(format!("AppID: {}", drop.app_id)).font(FontId::proportional(10.5)).color(Color32::from_rgb(110, 130, 155)));
                        });

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            // Add button
                            if is_already_added {
                                let _ = ui.button(RichText::new("✓ In Boosting List").font(FontId::proportional(11.5)).color(Color32::from_rgb(80, 200, 120)));
                            } else {
                                if ui.button(RichText::new("＋ Add to Boost").font(FontId::proportional(12.0)).color(accent)).clicked() {
                                    let mut a = current_acc_arc.lock().unwrap();
                                    a.games.push(GameEntry::new(drop.app_id, drop.name.clone()));
                                    crate::logger::log_success(&format!(
                                        "[{}] '{}' (AppID: {}) added to boost list! ({} drops remaining)",
                                        current_username, drop.name, drop.app_id, drop.remaining_cards
                                    ));
                                }
                            }

                            ui.add_space(10.0);

                            // Remaining cards badge with high contrast
                            let badge_text = format!("🎯 {} Cards Left", drop.remaining_cards);
                            egui::Frame::none()
                                .fill(Color32::from_rgba_unmultiplied(35, 25, 10, 220))
                                .rounding(Rounding::same(12.0))
                                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(255, 170, 40)))
                                .inner_margin(egui::Margin::symmetric(10.0, 3.5))
                                .show(ui, |ui| {
                                    ui.label(RichText::new(badge_text).font(FontId::proportional(11.5)).color(Color32::from_rgb(255, 195, 60)).strong());
                                });
                        });
                    });
                });
                ui.add_space(3.0);
            }
        });
    }
}
