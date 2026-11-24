use std::sync::{Arc, Mutex};
use eframe::egui::{self, Color32, FontId, RichText, Rounding, ScrollArea, Stroke, Ui, Vec2};
use crate::data::{Account, AccountStatus, CustomThemeColors, ThemeMode};
use crate::i18n::Strings;
use crate::inventory::{get_account_inventory, get_cached_active_drops};
use crate::logger::{get_logs, LogLevel};
use crate::ui::theme::ThemeEngine;

pub struct DashboardView;

impl DashboardView {
    pub fn draw(
        ui: &mut Ui,
        accounts: &[Arc<Mutex<Account>>],
        colors: &CustomThemeColors,
        theme_mode: ThemeMode,
        _strings: &Strings,
    ) {
        let accent = Color32::from_rgb(colors.accent[0], colors.accent[1], colors.accent[2]);

        // Aggregate statistics
        let mut active_accounts = 0;
        let mut total_boosting_games = 0;
        let mut total_cards_left: u32 = 0;
        let mut total_items = 0;

        for arc in accounts {
            let a = arc.lock().unwrap();
            let is_boosting = matches!(a.status, AccountStatus::Boosting);
            if is_boosting {
                active_accounts += 1;
                total_boosting_games += a.games.len();
            }

            if a.steam_id != 0 {
                let drops = get_cached_active_drops(a.steam_id);
                for d in drops {
                    total_cards_left += d.remaining_cards;
                }
                if let Some(inv) = get_account_inventory(a.steam_id) {
                    total_items += inv.total_items;
                }
            }
        }

        ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            // ─── TOP 4 STAT SUMMARY CARDS ───
            ui.add_space(4.0);
            ui.columns(4, |cols| {
                // Stat 1: Active Accounts
                ThemeEngine::card_frame(theme_mode, colors).show(&mut cols[0], |ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new("👤 Aktif Hesap").font(FontId::proportional(11.0)).color(Color32::from_rgb(140, 160, 180)));
                        ui.add_space(2.0);
                        ui.label(RichText::new(format!("{}", active_accounts)).font(FontId::proportional(22.0)).color(Color32::from_rgb(90, 210, 120)));
                        ui.label(RichText::new(format!("Toplam {} hesap", accounts.len())).font(FontId::proportional(10.5)).color(Color32::from_rgb(110, 130, 150)));
                    });
                });

                // Stat 2: Boosting Games
                ThemeEngine::card_frame(theme_mode, colors).show(&mut cols[1], |ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new("🎮 Kasılan Oyun").font(FontId::proportional(11.0)).color(Color32::from_rgb(140, 160, 180)));
                        ui.add_space(2.0);
                        ui.label(RichText::new(format!("{}", total_boosting_games)).font(FontId::proportional(22.0)).color(accent));
                        ui.label(RichText::new("Eşzamanlı aktif").font(FontId::proportional(10.5)).color(Color32::from_rgb(110, 130, 150)));
                    });
                });

                // Stat 3: Remaining Card Drops
                ThemeEngine::card_frame(theme_mode, colors).show(&mut cols[2], |ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new("🃏 Kalan Kart Hakkı").font(FontId::proportional(11.0)).color(Color32::from_rgb(140, 160, 180)));
                        ui.add_space(2.0);
                        ui.label(RichText::new(format!("{}", total_cards_left)).font(FontId::proportional(22.0)).color(Color32::from_rgb(255, 185, 60)));
                        ui.label(RichText::new("Düşürülebilir kart").font(FontId::proportional(10.5)).color(Color32::from_rgb(110, 130, 150)));
                    });
                });

                // Stat 4: Inventory Items / Value
                ThemeEngine::card_frame(theme_mode, colors).show(&mut cols[3], |ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new("🎒 Envanter Öğeleri").font(FontId::proportional(11.0)).color(Color32::from_rgb(140, 160, 180)));
                        ui.add_space(2.0);
                        ui.label(RichText::new(format!("{}", total_items)).font(FontId::proportional(22.0)).color(Color32::from_rgb(180, 140, 240)));
                        ui.label(RichText::new("Kart & Eşyalar").font(FontId::proportional(10.5)).color(Color32::from_rgb(110, 130, 150)));
                    });
                });
            });

            ui.add_space(14.0);

            // ─── ACTIVE BOOSTING GAMES SECTION ───
            ui.horizontal(|ui| {
                ui.label(RichText::new("🚀 Aktif Saat Kasılan Oyunlar").font(FontId::proportional(15.0)).strong());
            });
            ui.add_space(6.0);

            let mut any_boosting = false;

            for arc in accounts {
                let (username, id, status, hours, custom_title, games) = {
                    let a = arc.lock().unwrap();
                    (
                        a.username.clone(),
                        a.id,
                        a.status.clone(),
                        a.hours_boosted,
                        a.custom_title.clone(),
                        a.games.clone(),
                    )
                };

                if matches!(status, AccountStatus::Boosting) && !games.is_empty() {
                    any_boosting = true;

                    ThemeEngine::card_frame(theme_mode, colors).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(format!("👤 {}", username)).font(FontId::proportional(13.5)).strong());
                            ui.label(RichText::new(format!("• {:.1}h", hours)).font(FontId::proportional(12.0)).color(Color32::from_rgb(130, 150, 175)));
                            if !custom_title.is_empty() {
                                ui.label(RichText::new(format!("• \"{}\"", custom_title)).font(FontId::proportional(12.0)).color(Color32::from_rgb(100, 170, 230)));
                            }

                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.button(RichText::new("⏹ Durdur").font(FontId::proportional(11.5)).color(Color32::from_rgb(255, 120, 120))).clicked() {
                                    crate::steam::stop_boost(id, arc.clone());
                                }
                            });
                        });

                        ui.add_space(8.0);

                        // Horizontal list of games with appid and header thumbnails
                        ui.horizontal_wrapped(|ui| {
                            for g in &games {
                                egui::Frame::none()
                                    .fill(Color32::from_rgba_premultiplied(0, 0, 0, 60))
                                    .rounding(Rounding::same(6.0))
                                    .inner_margin(egui::Margin::symmetric(8.0, 4.0))
                                    .stroke(Stroke::new(1.0_f32, Color32::from_rgba_premultiplied(255, 255, 255, 20)))
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            let img_url = format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{}/capsule_sm_120.jpg", g.app_id);
                                            ui.add(egui::Image::new(img_url).fit_to_exact_size(Vec2::new(32.0, 16.0)).rounding(Rounding::same(2.0)));
                                            ui.label(RichText::new(&g.name).font(FontId::proportional(12.0)));
                                            ui.label(RichText::new(format!("(AppID: {})", g.app_id)).font(FontId::proportional(10.5)).color(Color32::from_rgb(100, 125, 150)));
                                        });
                                    });
                            }
                        });
                    });
                    ui.add_space(4.0);
                }
            }

            if !any_boosting {
                ThemeEngine::card_frame(theme_mode, colors).show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(14.0);
                        ui.label(RichText::new("⚪ Şu anda aktif kasılan oyun bulunmuyor").font(FontId::proportional(13.5)).color(Color32::from_rgb(130, 150, 175)));
                        ui.label(RichText::new("Hesaplar sekmesinden bir hesap seçip 'Başlat' butonuna basarak saat kasmaya başlayabilirsiniz.").font(FontId::proportional(11.5)).color(Color32::from_rgb(100, 120, 140)));
                        ui.add_space(14.0);
                    });
                });
            }

            ui.add_space(14.0);

            // ─── BOTTOM LIVE ACTIVITY FEED ───
            ui.horizontal(|ui| {
                ui.label(RichText::new("📋 Canlı Aktivite Akışı").font(FontId::proportional(14.0)).strong());
            });
            ui.add_space(4.0);

            ThemeEngine::card_frame(theme_mode, colors).show(ui, |ui| {
                ScrollArea::vertical().max_height(160.0).stick_to_bottom(true).show(ui, |ui| {
                    let logs = get_logs();
                    if logs.is_empty() {
                        ui.label(RichText::new("Henüz sistem aktivitesi yok.").font(FontId::proportional(11.5)).color(Color32::from_rgb(110, 130, 150)));
                    } else {
                        for entry in logs.iter().rev().take(30) {
                            ui.horizontal(|ui| {
                                let (color, prefix) = match entry.level {
                                    LogLevel::Success => (Color32::from_rgb(90, 215, 120), "✔"),
                                    LogLevel::Error => (Color32::from_rgb(255, 100, 100), "✖"),
                                    LogLevel::Warn => (Color32::from_rgb(255, 185, 60), "⚠"),
                                    LogLevel::Info => (Color32::from_rgb(140, 170, 205), "ℹ"),
                                };

                                ui.label(RichText::new(&entry.timestamp).font(FontId::monospace(10.5)).color(Color32::from_rgb(90, 110, 130)));
                                ui.label(RichText::new(prefix).font(FontId::monospace(11.0)).color(color));
                                ui.label(RichText::new(&entry.message).font(FontId::proportional(11.5)).color(color));
                            });
                        }
                    }
                });
            });

            ui.add_space(12.0);
        });
    }
}
