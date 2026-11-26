use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use eframe::egui::{self, Color32, FontId, RichText, Rounding, ScrollArea, Stroke, Ui, Vec2};
use crate::data::{Account, ActiveDropGame, CustomThemeColors, GameEntry, ThemeMode};
use crate::i18n::Strings;
use crate::inventory::{fetch_active_drops, get_cached_active_drops};
use crate::ui::components::{muted_text, prism_filter_pill, prism_primary_button};
use crate::ui::theme::ThemeEngine;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DropFilter {
    All,
    NotQueued,
    Queued,
}

pub struct ActiveDropsView {
    pub search_query: String,
    pub selected_account_index: usize,
    pub is_scanning: Arc<AtomicBool>,
    filter: DropFilter,
}

impl ActiveDropsView {
    pub fn new() -> Self {
        Self {
            search_query: String::new(),
            selected_account_index: 0,
            is_scanning: Arc::new(AtomicBool::new(false)),
            filter: DropFilter::All,
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
        auto_switch_enabled: &mut bool,
    ) {
        let accent = Color32::from_rgb(colors.accent[0], colors.accent[1], colors.accent[2]);
        let muted = muted_text(theme_mode);

        if accounts.is_empty() {
            ThemeEngine::card_frame(theme_mode, colors).show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(20.0);
                    ui.label(RichText::new(strings.no_accounts).font(FontId::proportional(14.0)).color(Color32::from_rgb(255, 180, 60)));
                    ui.label(RichText::new(strings.no_accounts_hint).font(FontId::proportional(12.0)).color(muted));
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
        let active_drops = get_cached_active_drops(current_steam_id);
        let total_cards: u32 = active_drops.iter().map(|d| d.remaining_cards).sum();
        let queued_cnt = active_drops.iter().filter(|d| current_games.iter().any(|g| g.app_id == d.app_id)).count();
        let not_queued_cnt = active_drops.len().saturating_sub(queued_cnt);

        // ─── PRISM HEADER + FILTER PILLS ───
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(RichText::new(strings.active_drops_title).font(FontId::proportional(19.0)).strong());
                ui.label(
                    RichText::new(format!(
                        "{} {} · {} 🃏 {} · {}",
                        active_drops.len(),
                        strings.games_count_unit,
                        total_cards,
                        strings.badge_cards_left,
                        strings.pick("Unlimited Parallel Boost + Rate-Limit Queue", "Sınırsız Paralel Kasma + Rate-Limit Kuyruğu", "Безлимитный буст + очередь")
                    ))
                    .font(FontId::proportional(11.5))
                    .color(muted),
                );
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if prism_filter_pill(ui, strings.pick("Queued", "Kuyrukta", "В очереди"), Some(queued_cnt), self.filter == DropFilter::Queued, accent, theme_mode).clicked() {
                    self.filter = DropFilter::Queued;
                }
                if prism_filter_pill(ui, strings.pick("Pending", "Bekleyen", "Ожидают"), Some(not_queued_cnt), self.filter == DropFilter::NotQueued, accent, theme_mode).clicked() {
                    self.filter = DropFilter::NotQueued;
                }
                if prism_filter_pill(ui, strings.pick("All", "Tümü", "Все"), Some(active_drops.len()), self.filter == DropFilter::All, accent, theme_mode).clicked() {
                    self.filter = DropFilter::All;
                }
            });
        });

        ui.add_space(8.0);

        // ─── CONTROL BAR ───
        ThemeEngine::card_frame(theme_mode, colors).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("👤 {}:", strings.col_account)).font(FontId::proportional(12.5)).strong());
                egui::ComboBox::from_id_salt("active_drops_acc_combo")
                    .width(175.0)
                    .selected_text(format!("{} (ID: {})", current_username, current_steam_id))
                    .show_ui(ui, |ui| {
                        for (idx, arc) in accounts.iter().enumerate() {
                            let a = arc.lock().unwrap();
                            ui.selectable_value(&mut self.selected_account_index, idx, &a.username);
                        }
                    });

                ui.add_space(8.0);
                ui.checkbox(auto_switch_enabled, RichText::new(strings.opt_auto_switch_drops).font(FontId::proportional(11.5)));

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if prism_primary_button(ui, strings.btn_add_all_drops, accent).clicked() {
                        let mut added_count = 0;
                        for d in &active_drops {
                            if d.remaining_cards > 0 && !current_games.iter().any(|g| g.app_id == d.app_id) {
                                current_games.push(GameEntry::new(d.app_id, d.name.clone()));
                                added_count += 1;
                            }
                        }
                        if added_count > 0 {
                            let mut a = current_acc_arc.lock().unwrap();
                            a.games = current_games.clone();
                            crate::logger::log_success(&format!(
                                "[{}] ⚡ Added {} games with remaining card drops!",
                                current_username, added_count
                            ));
                        }
                    }

                    let scan_btn_label = if is_scan {
                        strings.pick("⏳ Scanning...", "⏳ Taranıyor...", "⏳ Сканирование...")
                    } else {
                        strings.pick("🔄 Scan Drops", "🔄 Kartları Tara", "🔄 Сканировать")
                    };
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
                });
            });
        });

        ui.add_space(8.0);

        let effective_q = if !global_search.trim().is_empty() {
            global_search.trim().to_lowercase()
        } else {
            self.search_query.trim().to_lowercase()
        };

        let filtered_drops: Vec<&ActiveDropGame> = active_drops
            .iter()
            .filter(|d| {
                let is_added = current_games.iter().any(|g| g.app_id == d.app_id);
                match self.filter {
                    DropFilter::Queued if !is_added => return false,
                    DropFilter::NotQueued if is_added => return false,
                    _ => {}
                }
                if effective_q.is_empty() {
                    true
                } else {
                    d.name.to_lowercase().contains(&effective_q) || d.app_id.to_string().contains(&effective_q)
                }
            })
            .collect();

        if filtered_drops.is_empty() {
            ThemeEngine::card_frame(theme_mode, colors).show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(24.0);
                    if is_scan {
                        ui.label(RichText::new(strings.pick("⏳ Scanning Steam badges...", "⏳ Steam rozetleri taranıyor...", "⏳ Сканирование значков...")).font(FontId::proportional(14.0)));
                        ui.spinner();
                    } else {
                        ui.label(RichText::new(strings.pick("🃏 No active card drops found.", "🃏 Aktif kart düşürme hakkı bulunamadı.", "🃏 Активных дропов нет.")).font(FontId::proportional(14.0)).color(muted));
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
                        let img_url = format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{}/capsule_sm_120.jpg", drop.app_id);
                        ui.add(egui::Image::new(img_url).fit_to_exact_size(Vec2::new(70.0, 32.0)).rounding(Rounding::same(4.0)));
                        ui.add_space(8.0);

                        ui.vertical(|ui| {
                            ui.label(RichText::new(&drop.name).font(FontId::proportional(13.5)).strong());
                            ui.label(RichText::new(format!("AppID: {}", drop.app_id)).font(FontId::proportional(10.5)).color(muted));
                        });

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if is_already_added {
                                ui.label(RichText::new(strings.pick("✓ In Queue", "✓ Kuyrukta", "✓ В очереди")).font(FontId::proportional(11.5)).color(Color32::from_rgb(80, 220, 130)).strong());
                            } else if ui.button(RichText::new(strings.btn_add_to_boost).font(FontId::proportional(12.0)).color(accent)).clicked() {
                                let mut a = current_acc_arc.lock().unwrap();
                                a.games.push(GameEntry::new(drop.app_id, drop.name.clone()));
                                crate::logger::log_success(&format!(
                                    "[{}] '{}' (AppID: {}) added to boost list!",
                                    current_username, drop.name, drop.app_id
                                ));
                            }

                            ui.add_space(10.0);

                            egui::Frame::none()
                                .fill(Color32::from_rgba_unmultiplied(35, 25, 10, 220))
                                .rounding(Rounding::same(12.0))
                                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(255, 170, 40)))
                                .inner_margin(egui::Margin::symmetric(10.0, 3.5))
                                .show(ui, |ui| {
                                    ui.label(RichText::new(format!("🃏 {} {}", drop.remaining_cards, strings.badge_cards_left)).font(FontId::proportional(11.5)).color(Color32::from_rgb(255, 195, 60)).strong());
                                });
                        });
                    });
                });
                ui.add_space(3.0);
            }
        });
    }
}
