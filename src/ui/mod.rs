pub mod accounts;
pub mod active_drops;
pub mod dashboard;
pub mod inventory;
pub mod logs;
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

            ui.add_space(10.0);

            // Active & Boosting Accounts with Steam PFP (supporting animated GIFs & avatars)
            let boosting_accounts: Vec<Arc<Mutex<Account>>> = accounts
                .iter()
                .filter(|arc| arc.lock().unwrap().status == AccountStatus::Boosting)
                .cloned()
                .collect();

            let display_accounts = if !boosting_accounts.is_empty() {
                boosting_accounts
            } else {
                accounts.to_vec()
            };

            for arc in display_accounts.iter().take(3) {
                let (uname, is_boosting, avatar_url, games_count) = {
                    let mut a = arc.lock().unwrap();
                    if a.avatar_url.is_none() && a.steam_id != 0 {
                        a.avatar_url = Some(String::new()); // marker to prevent duplicate requests
                        let s_id = a.steam_id;
                        let proxy_opt = a.proxy.clone();
                        let arc_clone = arc.clone();
                        let ctx_clone = ui.ctx().clone();
                        crate::steam::get_runtime().spawn(async move {
                            if let Some(av) = crate::inventory::fetch_account_avatar(s_id, proxy_opt).await {
                                if let Ok(mut l) = arc_clone.lock() {
                                    l.avatar_url = Some(av);
                                }
                                ctx_clone.request_repaint();
                            } else {
                                if let Ok(mut l) = arc_clone.lock() {
                                    l.avatar_url = Some("none".to_string());
                                }
                                ctx_clone.request_repaint();
                            }
                        });
                    }
                    (
                        a.username.clone(),
                        a.status == AccountStatus::Boosting,
                        a.avatar_url.clone(),
                        a.games.len(),
                    )
                };

                if uname.is_empty() {
                    continue;
                }

                let pill_border = if is_boosting {
                    Color32::from_rgb(46, 204, 113)
                } else {
                    Color32::from_rgba_unmultiplied(70, 90, 115, 120)
                };

                egui::Frame::none()
                    .fill(Color32::from_rgba_unmultiplied(16, 22, 30, 210))
                    .rounding(Rounding::same(14.0))
                    .stroke(Stroke::new(1.0_f32, pill_border))
                    .inner_margin(egui::Margin::symmetric(6.0, 3.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let valid_avatar = avatar_url.as_ref().filter(|u| !u.is_empty() && *u != "none");
                            if let Some(url) = valid_avatar {
                                ui.add(
                                    egui::Image::new(url)
                                        .fit_to_exact_size(Vec2::splat(22.0))
                                        .rounding(Rounding::same(11.0)),
                                );
                            } else {
                                let initials: String = uname.chars().take(2).collect::<String>().to_uppercase();
                                let (rect, _) = ui.allocate_exact_size(Vec2::splat(22.0), egui::Sense::hover());
                                ui.painter().rect_filled(
                                    rect,
                                    Rounding::same(11.0),
                                    Color32::from_rgba_premultiplied(accent.r(), accent.g(), accent.b(), 70),
                                );
                                ui.painter().text(
                                    rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    initials,
                                    FontId::proportional(10.0),
                                    Color32::WHITE,
                                );
                            }

                            ui.add_space(3.0);
                            ui.label(RichText::new(&uname).font(FontId::proportional(12.0)).strong());

                            if is_boosting {
                                ui.add_space(2.0);
                                let (dot_rect, _) = ui.allocate_exact_size(Vec2::splat(6.0), egui::Sense::hover());
                                ui.painter().circle_filled(dot_rect.center(), 3.0, Color32::from_rgb(46, 204, 113));
                                ui.label(RichText::new(format!("🎮 {}", games_count)).font(FontId::proportional(10.5)).color(Color32::from_rgb(130, 220, 160)));
                            }
                        });
                    });

                ui.add_space(4.0);
            }

            // Center/Right: Connection Status Badge + Stats + Quick Action Button
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(4.0);

                // Quick Start / Stop / Restart Action Buttons
                if any_boosting {
                    if ui.button(RichText::new("⏹ Stop All").font(FontId::proportional(11.5)).color(Color32::from_rgb(255, 120, 120))).clicked() {
                        for arc in accounts {
                            let id = arc.lock().unwrap().id;
                            crate::steam::stop_boost(id, arc.clone());
                        }
                    }
                    ui.add_space(4.0);
                    if ui.button(RichText::new("🔄 Restart All").font(FontId::proportional(11.5)).color(Color32::from_rgb(255, 155, 40))).clicked() {
                        for arc in accounts {
                            let (id, arc_clone) = {
                                let a = arc.lock().unwrap();
                                (a.id, arc.clone())
                            };
                            crate::steam::stop_boost(id, arc_clone.clone());
                            crate::steam::start_boost(arc_clone);
                        }
                    }
                } else {
                    if ui.button(RichText::new("▶ Start All").font(FontId::proportional(11.5)).color(Color32::from_rgb(90, 215, 120))).clicked() {
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
                ui.label(RichText::new(format!("🎮 {} Games", total_games_count)).font(FontId::proportional(12.0)).color(Color32::from_rgb(140, 165, 190)));

                ui.add_space(10.0);

                // Status Badge with high-contrast text and border
                let (status_text, border_color, text_color) = if any_boosting {
                    ("🟢 Online (Boosting)", Color32::from_rgb(46, 204, 113), Color32::from_rgb(80, 240, 140))
                } else if any_connecting {
                    ("🟡 Connecting...", Color32::from_rgb(255, 185, 60), Color32::from_rgb(255, 210, 100))
                } else {
                    ("⚪ Offline", Color32::from_rgb(130, 145, 165), Color32::from_rgb(170, 185, 205))
                };

                egui::Frame::none()
                    .fill(Color32::from_rgba_unmultiplied(16, 24, 20, 220))
                    .rounding(Rounding::same(12.0))
                    .inner_margin(egui::Margin::symmetric(10.0, 3.5))
                    .stroke(Stroke::new(1.0_f32, border_color))
                    .show(ui, |ui| {
                        ui.label(RichText::new(status_text).font(FontId::proportional(11.5)).color(text_color).strong());
                    });
            });
        });
    }
}
