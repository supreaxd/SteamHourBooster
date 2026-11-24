pub mod accounts;
pub mod active_drops;
pub mod dashboard;
pub mod inventory;
pub mod modals;
pub mod settings;
pub mod sidebar;
pub mod telegram;
pub mod theme;

use std::sync::{Arc, Mutex};
use eframe::egui::{self, Color32, FontId, RichText, Rounding, Stroke, Ui, Vec2};
use crate::data::{Account, AccountStatus, CustomThemeColors, ThemeMode};
use crate::i18n::Strings;

pub struct TopStatusBar;

impl TopStatusBar {
    pub fn draw(
        ui: &mut Ui,
        accounts: &[Arc<Mutex<Account>>],
        colors: &CustomThemeColors,
        _theme_mode: ThemeMode,
        _strings: &Strings,
    ) {
        let accent = Color32::from_rgb(colors.accent[0], colors.accent[1], colors.accent[2]);

        // Aggregate statistics across accounts
        let mut any_boosting = false;
        let mut any_connecting = false;
        let mut total_hours: f64 = 0.0;
        let mut total_games_count = 0;
        let mut primary_username = String::new();

        for arc in accounts {
            let a = arc.lock().unwrap();
            total_hours += a.hours_boosted;
            match &a.status {
                AccountStatus::Boosting => {
                    any_boosting = true;
                    total_games_count += a.games.len();
                    if primary_username.is_empty() {
                        primary_username = a.username.clone();
                    }
                }
                AccountStatus::Connecting | AccountStatus::Reconnecting { .. } => {
                    any_connecting = true;
                    if primary_username.is_empty() {
                        primary_username = a.username.clone();
                    }
                }
                _ => {
                    if primary_username.is_empty() {
                        primary_username = a.username.clone();
                    }
                }
            }
        }

        ui.horizontal(|ui| {
            // Left: SupBooster Logo & Brand
            ui.add_space(4.0);
            ui.label(RichText::new("🚀 SupBooster").font(FontId::proportional(16.0)).strong().color(accent));

            ui.add_space(12.0);

            // Active Account Avatar & Username
            if !primary_username.is_empty() {
                let initials: String = primary_username.chars().take(2).collect::<String>().to_uppercase();
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(24.0), egui::Sense::hover());
                ui.painter().rect_filled(rect, Rounding::same(12.0), Color32::from_rgba_premultiplied(accent.r(), accent.g(), accent.b(), 70));
                ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, initials, FontId::proportional(10.5), Color32::WHITE);

                ui.add_space(3.0);
                ui.label(RichText::new(&primary_username).font(FontId::proportional(12.5)).strong());
            }

            // Center/Right: Connection Status Badge + Stats + Quick Action Button
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(4.0);

                // Quick Start / Stop Action Button
                if any_boosting {
                    if ui.button(RichText::new("⏹ Hepsini Durdur").font(FontId::proportional(11.5)).color(Color32::from_rgb(255, 120, 120))).clicked() {
                        for arc in accounts {
                            let id = arc.lock().unwrap().id;
                            crate::steam::stop_boost(id, arc.clone());
                        }
                    }
                } else {
                    if ui.button(RichText::new("▶ Hepsini Başlat").font(FontId::proportional(11.5)).color(Color32::from_rgb(90, 215, 120))).clicked() {
                        for arc in accounts {
                            crate::steam::start_boost(arc.clone());
                        }
                    }
                }

                ui.add_space(10.0);

                // Total hours summary
                ui.label(RichText::new(format!("⏱ {:.1}h", total_hours)).font(FontId::proportional(12.0)).color(Color32::from_rgb(140, 165, 190)));

                ui.add_space(6.0);

                // Active boosting games count
                ui.label(RichText::new(format!("🎮 {} Oyun", total_games_count)).font(FontId::proportional(12.0)).color(Color32::from_rgb(140, 165, 190)));

                ui.add_space(10.0);

                // Status Badge
                let (status_text, status_color) = if any_boosting {
                    ("🟢 Çevrimiçi (Kasıyor)", Color32::from_rgb(90, 215, 120))
                } else if any_connecting {
                    ("🟡 Bağlanıyor...", Color32::from_rgb(255, 185, 60))
                } else {
                    ("⚪ Çevrimdışı", Color32::from_rgb(130, 145, 165))
                };

                egui::Frame::none()
                    .fill(Color32::from_rgba_premultiplied(status_color.r(), status_color.g(), status_color.b(), 30))
                    .rounding(Rounding::same(12.0))
                    .inner_margin(egui::Margin::symmetric(9.0, 3.0))
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgba_premultiplied(status_color.r(), status_color.g(), status_color.b(), 90)))
                    .show(ui, |ui| {
                        ui.label(RichText::new(status_text).font(FontId::proportional(11.5)).color(status_color).strong());
                    });
            });
        });
    }
}
