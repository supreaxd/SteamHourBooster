use std::sync::{Arc, Mutex};
use eframe::egui::{self, Color32, FontId, RichText, Rounding, ScrollArea, Stroke, Ui, Vec2};
use crate::data::{Account, ActiveDropGame, CustomThemeColors, GameEntry, ThemeMode};
use crate::i18n::Strings;
use crate::inventory::{fetch_active_drops, get_cached_active_drops};
use crate::ui::theme::ThemeEngine;

pub struct ActiveDropsView {
    pub search_query: String,
    pub selected_account_index: usize,
    pub is_scanning: bool,
}

impl ActiveDropsView {
    pub fn new() -> Self {
        Self {
            search_query: String::new(),
            selected_account_index: 0,
            is_scanning: false,
        }
    }

    pub fn draw(
        &mut self,
        ui: &mut Ui,
        accounts: &[Arc<Mutex<Account>>],
        colors: &CustomThemeColors,
        theme_mode: ThemeMode,
        strings: &Strings,
        auto_switch_enabled: &mut bool,
    ) {
        let accent = Color32::from_rgb(colors.accent[0], colors.accent[1], colors.accent[2]);

        if accounts.is_empty() {
            ThemeEngine::card_frame(theme_mode, colors).show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(20.0);
                    ui.label(RichText::new("⚠️ Henüz kayıtlı bir Steam hesabı bulunmuyor.").font(FontId::proportional(14.0)).color(Color32::from_rgb(255, 180, 60)));
                    ui.label(RichText::new("Aktif kart droplarını taramak için önce Hesaplar sekmesinden bir hesap ekleyin.").font(FontId::proportional(12.0)).color(Color32::from_rgb(120, 140, 160)));
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

        // ─── TOP CONTROL BAR ───
        ThemeEngine::card_frame(theme_mode, colors).show(ui, |ui| {
            ui.horizontal(|ui| {
                // Account selector
                ui.label(RichText::new("👤 Hesap:").font(FontId::proportional(12.5)).strong());
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
                ui.add(egui::TextEdit::singleline(&mut self.search_query).hint_text("Oyun ara...").desired_width(160.0));

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Scan button
                    let scan_btn_label = if self.is_scanning { "⏳ Taranıyor..." } else { "🔄 Dropları Tara" };
                    if ui.button(RichText::new(scan_btn_label).font(FontId::proportional(12.0))).clicked() && !self.is_scanning {
                        self.is_scanning = true;
                        let sid = current_steam_id;
                        let proxy = current_proxy.clone();
                        crate::steam::get_runtime().spawn(async move {
                            let _ = fetch_active_drops(sid, proxy).await;
                        });
                        self.is_scanning = false;
                    }

                    // Add all drops button
                    if ui.button(RichText::new(strings.btn_add_all_drops).font(FontId::proportional(12.0)).color(accent)).clicked() {
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
                                "[{}] ⚡ {} adet aktif kart droplu oyun kasma listesine eklendi!",
                                current_username, added_count
                            ));
                        }
                    }
                });
            });

            ui.add_space(6.0);

            // Auto-switch setting checkbox
            ui.horizontal(|ui| {
                ui.checkbox(auto_switch_enabled, RichText::new(strings.opt_auto_switch_drops).font(FontId::proportional(11.5)));
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
                    if active_drops.is_empty() {
                        ui.label(RichText::new("🃏 Henüz taranmış aktif kart düşürme hakkı bulunamadı.").font(FontId::proportional(14.0)).color(Color32::from_rgb(140, 160, 180)));
                        ui.label(RichText::new("Steam profilinizin rozetler sayfasını denetlemek için yukarıdaki '🔄 Dropları Tara' butonuna tıklayın.").font(FontId::proportional(12.0)).color(Color32::from_rgb(110, 130, 150)));
                    } else {
                        ui.label(RichText::new("Arama kriterinize uygun oyun bulunamadı.").font(FontId::proportional(13.0)).color(Color32::from_rgb(130, 150, 175)));
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
                                let _ = ui.button(RichText::new("✓ Kasılıyor").font(FontId::proportional(11.5)).color(Color32::from_rgb(80, 200, 120)));
                            } else {
                                if ui.button(RichText::new(strings.btn_add_to_boost).font(FontId::proportional(12.0)).color(accent)).clicked() {
                                    let mut a = current_acc_arc.lock().unwrap();
                                    a.games.push(GameEntry::new(drop.app_id, drop.name.clone()));
                                    crate::logger::log_success(&format!(
                                        "[{}] '{}' (AppID: {}) kasma listesine eklendi! (Kalan kart: {})",
                                        current_username, drop.name, drop.app_id, drop.remaining_cards
                                    ));
                                }
                            }

                            ui.add_space(10.0);

                            // Remaining cards badge
                            let badge_text = format!("🎯 {} {}", drop.remaining_cards, strings.badge_cards_left);
                            egui::Frame::none()
                                .fill(Color32::from_rgba_premultiplied(255, 170, 40, 35))
                                .rounding(Rounding::same(12.0))
                                .stroke(Stroke::new(1.0_f32, Color32::from_rgba_premultiplied(255, 170, 40, 120)))
                                .inner_margin(egui::Margin::symmetric(10.0, 3.0))
                                .show(ui, |ui| {
                                    ui.label(RichText::new(badge_text).font(FontId::proportional(11.5)).color(Color32::from_rgb(255, 190, 60)).strong());
                                });
                        });
                    });
                });
                ui.add_space(3.0);
            }
        });
    }
}
