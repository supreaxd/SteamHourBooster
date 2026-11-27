use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use eframe::egui::{self, Color32, FontId, RichText, Rounding, ScrollArea, Stroke, Ui, Vec2};
use crate::data::{Account, AccountStatus, CustomThemeColors, ThemeMode};
use crate::i18n::Strings;
use crate::inventory::{get_account_inventory, get_cached_active_drops};
use crate::ui::components::{muted_text, prism_filter_pill, prism_progress_bar};
use crate::ui::theme::ThemeEngine;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DashFilter {
    All,
    Boosting,
    Stopped,
}

pub struct DashboardView {
    filter: DashFilter,
    expanded_accounts: HashSet<u64>,
}

impl DashboardView {
    pub fn new() -> Self {
        Self {
            filter: DashFilter::All,
            expanded_accounts: HashSet::new(),
        }
    }

    pub fn draw(
        &mut self,
        ui: &mut Ui,
        accounts: &[Arc<Mutex<Account>>],
        global_search: &str,
        colors: &CustomThemeColors,
        theme_mode: ThemeMode,
        strings: &Strings,
    ) -> Option<u64> {
        let mut edit_account_id: Option<u64> = None;
        let accent = Color32::from_rgb(colors.accent[0], colors.accent[1], colors.accent[2]);
        let muted = muted_text(theme_mode);

        let mut active_accounts = 0usize;
        let mut stopped_accounts = 0usize;
        let mut total_boosting_games = 0usize;
        let mut total_cards_left: u32 = 0;
        let mut total_items = 0usize;
        let mut total_hours = 0.0_f64;

        for arc in accounts {
            let a = arc.lock().unwrap();
            total_hours += a.hours_boosted;
            if matches!(a.status, AccountStatus::Boosting) {
                active_accounts += 1;
                total_boosting_games += a.games.len();
            } else {
                stopped_accounts += 1;
            }
            if a.steam_id != 0 {
                for d in get_cached_active_drops(a.steam_id) {
                    total_cards_left += d.remaining_cards;
                }
                if let Some(inv) = get_account_inventory(a.steam_id) {
                    total_items += inv.total_items;
                }
            }
        }

        // ─── PRISM HEADER + STATUS FILTER PILLS ───
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(RichText::new(strings.tab_dashboard).font(FontId::proportional(19.0)).strong());
                ui.label(
                    RichText::new(format!(
                        "{} {} · {} {} · {:.1}h · {} 🃏",
                        accounts.len(),
                        strings.pick("accounts", "hesap", "аккаунтов"),
                        total_boosting_games,
                        strings.games_count_unit,
                        total_hours,
                        total_cards_left
                    ))
                    .font(FontId::proportional(11.5))
                    .color(muted),
                );
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if prism_filter_pill(ui, strings.status_idle, Some(stopped_accounts), self.filter == DashFilter::Stopped, accent, theme_mode).clicked() {
                    self.filter = DashFilter::Stopped;
                }
                if prism_filter_pill(ui, strings.status_boosting, Some(active_accounts), self.filter == DashFilter::Boosting, accent, theme_mode).clicked() {
                    self.filter = DashFilter::Boosting;
                }
                if prism_filter_pill(ui, strings.pick("All", "Tümü", "Все"), Some(accounts.len()), self.filter == DashFilter::All, accent, theme_mode).clicked() {
                    self.filter = DashFilter::All;
                }
            });
        });

        ui.add_space(10.0);

        ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            // ─── TOP 4 COMPACT STAT CARDS ───
            ui.columns(4, |cols| {
                ThemeEngine::card_frame(theme_mode, colors).show(&mut cols[0], |ui| {
                    ui.label(RichText::new(strings.pick("👤 Active Accounts", "👤 Aktif Hesaplar", "👤 Активные")).font(FontId::proportional(11.0)).color(muted));
                    ui.label(RichText::new(format!("{}", active_accounts)).font(FontId::proportional(20.0)).color(Color32::from_rgb(80, 225, 130)).strong());
                });
                ThemeEngine::card_frame(theme_mode, colors).show(&mut cols[1], |ui| {
                    ui.label(RichText::new(strings.pick("🎮 Parallel Games", "🎮 Paralel Oyunlar", "🎮 Параллельно игр")).font(FontId::proportional(11.0)).color(muted));
                    ui.label(RichText::new(format!("{}", total_boosting_games)).font(FontId::proportional(20.0)).color(accent).strong());
                });
                ThemeEngine::card_frame(theme_mode, colors).show(&mut cols[2], |ui| {
                    ui.label(RichText::new(strings.pick("🃏 Remaining Drops", "🃏 Kalan Kart Hakkı", "🃏 Осталось дропов")).font(FontId::proportional(11.0)).color(muted));
                    ui.label(RichText::new(format!("{}", total_cards_left)).font(FontId::proportional(20.0)).color(Color32::from_rgb(255, 185, 60)).strong());
                });
                ThemeEngine::card_frame(theme_mode, colors).show(&mut cols[3], |ui| {
                    ui.label(RichText::new(strings.pick("🎒 Inventory Items", "🎒 Envanter Eşyası", "🎒 Инвентарь")).font(FontId::proportional(11.0)).color(muted));
                    ui.label(RichText::new(format!("{}", total_items)).font(FontId::proportional(20.0)).color(Color32::from_rgb(180, 140, 240)).strong());
                });
            });

            ui.add_space(12.0);

            // ─── PRISM HORIZONTAL ACCOUNT / PARALLEL BOOST LIST ───
            let q = global_search.trim().to_lowercase();
            let mut shown = 0usize;

            for arc in accounts {
                let (username, id, status, hours, custom_title, games, steam_id) = {
                    let a = arc.lock().unwrap();
                    (
                        a.username.clone(),
                        a.id,
                        a.status.clone(),
                        a.hours_boosted,
                        a.custom_title.clone(),
                        a.games.clone(),
                        a.steam_id,
                    )
                };

                let is_boosting = matches!(status, AccountStatus::Boosting);
                match self.filter {
                    DashFilter::Boosting if !is_boosting => continue,
                    DashFilter::Stopped if is_boosting => continue,
                    _ => {}
                }

                if !q.is_empty()
                    && !username.to_lowercase().contains(&q)
                    && !games.iter().any(|g| g.name.to_lowercase().contains(&q) || g.app_id.to_string().contains(&q))
                {
                    continue;
                }

                shown += 1;
                let drops_left: u32 = if steam_id != 0 {
                    get_cached_active_drops(steam_id).iter().map(|d| d.remaining_cards).sum()
                } else {
                    0
                };

                let is_expanded = self.expanded_accounts.contains(&id);

                let _card_resp = ThemeEngine::card_frame(theme_mode, colors).show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.horizontal(|ui| {
                        let (st_col, st_txt) = match &status {
                            AccountStatus::Boosting => (Color32::from_rgb(80, 225, 130), strings.status_boosting.to_string()),
                            AccountStatus::PausedForGame { app_name, .. } => (
                                Color32::from_rgb(255, 145, 35),
                                format!("{} ({})", strings.status_paused_game, app_name),
                            ),
                            AccountStatus::Reconnecting { attempt, retry_in_secs } => (
                                Color32::from_rgb(255, 200, 60),
                                format!("{} (#{} · {}s)", strings.status_reconnecting, attempt, retry_in_secs),
                            ),
                            AccountStatus::Connecting => (Color32::from_rgb(255, 200, 60), strings.status_connecting.to_string()),
                            AccountStatus::AwaitingSteamGuard { .. } => (Color32::from_rgb(255, 115, 50), strings.status_guard_needed.to_string()),
                            AccountStatus::Error(_) => (Color32::from_rgb(255, 85, 85), strings.status_error.to_string()),
                            AccountStatus::Idle => (muted, strings.status_idle.to_string()),
                        };

                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                let arrow = if is_expanded { "▾" } else { "▸" };
                                if ui
                                    .link(
                                        RichText::new(format!("{} 👤 {}", arrow, username))
                                            .font(FontId::proportional(13.5))
                                            .strong(),
                                    )
                                    .on_hover_text(strings.pick(
                                        "Click to show/hide boosted games",
                                        "Kasılan oyunları görmek için tıklayın",
                                        "Нажмите, чтобы показать игры",
                                    ))
                                    .clicked()
                                {
                                    if is_expanded {
                                        self.expanded_accounts.remove(&id);
                                    } else {
                                        self.expanded_accounts.insert(id);
                                    }
                                }
                                ui.label(RichText::new(format!("● {}", st_txt)).font(FontId::proportional(11.5)).color(st_col));
                            });
                            ui.label(
                                RichText::new(format!(
                                    "{} {} · \"{}\" · {} {}",
                                    games.len(),
                                    strings.games_count_unit,
                                    custom_title,
                                    drops_left,
                                    strings.pick("drops", "kart", "дроп.")
                                ))
                                .font(FontId::proportional(11.0))
                                .color(muted),
                            );
                        });

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.menu_button("⋯", |ui| {
                                if ui.button(strings.edit_title_edit).clicked() {
                                    edit_account_id = Some(id);
                                    ui.close_menu();
                                }
                                if is_boosting {
                                    if ui.button(strings.btn_stop).clicked() {
                                        crate::steam::stop_boost(id, arc.clone());
                                        ui.close_menu();
                                    }
                                    if ui.button(strings.btn_restart).clicked() {
                                        crate::steam::restart_boost(id, arc.clone());
                                        ui.close_menu();
                                    }
                                } else if ui.button(strings.btn_start).clicked() {
                                    crate::steam::start_boost(arc.clone());
                                    ui.close_menu();
                                }
                            });

                            ui.add_space(6.0);
                            ui.label(RichText::new(format!("{:.1}h", hours)).font(FontId::proportional(12.5)).strong());
                            ui.add_space(6.0);
                            prism_progress_bar(ui, if is_boosting { 1.0 } else { 0.0 }, 72.0, 5.0, st_col);
                        });
                    });

                    // Show boosted games when user clicks this account row
                    if is_expanded && !games.is_empty() {
                        ui.add_space(6.0);
                        for g in &games {
                            egui::Frame::none()
                                .fill(Color32::from_rgba_unmultiplied(255, 255, 255, 8))
                                .rounding(Rounding::same(6.0))
                                .inner_margin(egui::Margin::symmetric(8.0, 4.0))
                                .stroke(Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 18)))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        let img_url = format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{}/capsule_sm_120.jpg", g.app_id);
                                        ui.add(egui::Image::new(img_url).fit_to_exact_size(Vec2::new(38.0, 18.0)).rounding(Rounding::same(3.0)));
                                        ui.label(RichText::new(&g.name).font(FontId::proportional(12.0)));
                                        ui.label(RichText::new(format!("AppID: {}", g.app_id)).font(FontId::proportional(10.5)).color(muted));
                                    });
                                });
                            ui.add_space(2.0);
                        }
                    }
                });
                ui.add_space(4.0);
            }

            if shown == 0 {
                ThemeEngine::card_frame(theme_mode, colors).show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(16.0);
                        ui.label(RichText::new(strings.no_accounts).font(FontId::proportional(13.5)).color(muted));
                        ui.label(RichText::new(strings.no_accounts_hint).font(FontId::proportional(11.5)).color(muted));
                        ui.add_space(16.0);
                    });
                });
            }
        });

        edit_account_id
    }
}
