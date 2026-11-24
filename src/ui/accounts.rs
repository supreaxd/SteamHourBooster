use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use eframe::egui::{self, Color32, FontId, RichText, Rounding, ScrollArea, Stroke, Ui, Vec2};
use crate::data::{Account, AccountStatus, CustomThemeColors, GameEntry, PersonaState, ThemeMode};
use crate::i18n::Strings;
use crate::ui::theme::ThemeEngine;

pub struct AccountsView {
    pub editing_account: Option<Account>,
    pub is_adding_new: bool,
    pub guard_inputs: HashMap<u64, String>,
    pub store_search_query: String,
    pub last_searched_query: String,
    pub store_search_results: Arc<Mutex<Vec<(u32, String)>>>,
    pub is_searching_store: Arc<std::sync::atomic::AtomicBool>,
    pub custom_appid_input: String,
    pub custom_appname_input: String,
}

impl AccountsView {
    pub fn new() -> Self {
        Self {
            editing_account: None,
            is_adding_new: false,
            guard_inputs: HashMap::new(),
            store_search_query: String::new(),
            last_searched_query: String::new(),
            store_search_results: Arc::new(Mutex::new(Vec::new())),
            is_searching_store: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            custom_appid_input: String::new(),
            custom_appname_input: String::new(),
        }
    }

    pub fn draw(
        &mut self,
        ui: &mut Ui,
        accounts: &mut Vec<Account>,
        runtime_accounts: &[Arc<Mutex<Account>>],
        colors: &CustomThemeColors,
        theme_mode: ThemeMode,
        strings: &Strings,
        next_id: &mut u64,
        show_qr_modal: &mut bool,
    ) {
        let accent = Color32::from_rgb(colors.accent[0], colors.accent[1], colors.accent[2]);

        // If currently in add/edit mode, show the account editor form
        if let Some(mut acc) = self.editing_account.take() {
            let is_new = self.is_adding_new;
            let mut save_account = false;
            let mut cancel_edit = false;

            ThemeEngine::card_frame(theme_mode, colors).show(ui, |ui| {
                ui.horizontal(|ui| {
                    let title = if is_new { "＋ Add New Steam Account" } else { "✏ Edit Steam Account" };
                    ui.label(RichText::new(title).font(FontId::proportional(15.0)).strong());
                });
                ui.add_space(8.0);

                ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    ui.columns(2, |cols| {
                        // Left column: Credentials & Persona
                        cols[0].vertical(|ui| {
                            ui.label(RichText::new("Steam Username:").font(FontId::proportional(12.0)));
                            ui.add(egui::TextEdit::singleline(&mut acc.username).desired_width(f32::INFINITY));
                            ui.add_space(4.0);

                            ui.label(RichText::new("Password:").font(FontId::proportional(12.0)));
                            ui.add(egui::TextEdit::singleline(&mut acc.password).password(true).desired_width(f32::INFINITY));
                            ui.add_space(4.0);

                            ui.label(RichText::new("Custom Status / Game Title:").font(FontId::proportional(12.0)));
                            ui.add(egui::TextEdit::singleline(&mut acc.custom_title).hint_text("Half-Life 3 (Beta) | (hours)h (games) games").desired_width(f32::INFINITY));
                            ui.label(RichText::new("💡 Auto tags: (hours) for hours, (games) for count, (time) for HH:MM. Refreshes every 10 min.").font(FontId::proportional(10.5)).color(Color32::from_rgb(120, 160, 210)));
                            ui.add_space(4.0);

                            ui.label(RichText::new("Steam Persona State:").font(FontId::proportional(12.0)));
                            let avail_w = ui.available_width() - 4.0;
                            egui::ComboBox::from_id_salt("edit_persona_combo")
                                .selected_text(acc.persona_state.label())
                                .width(avail_w)
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut acc.persona_state, PersonaState::Online, PersonaState::Online.label());
                                    ui.selectable_value(&mut acc.persona_state, PersonaState::Invisible, PersonaState::Invisible.label());
                                    ui.selectable_value(&mut acc.persona_state, PersonaState::Away, PersonaState::Away.label());
                                    ui.selectable_value(&mut acc.persona_state, PersonaState::Busy, PersonaState::Busy.label());
                                });
                            ui.add_space(4.0);

                            ui.label(RichText::new("Proxy (SOCKS5 / HTTP):").font(FontId::proportional(12.0)));
                            let mut proxy_str = acc.proxy.clone().unwrap_or_default();
                            if ui.add(egui::TextEdit::singleline(&mut proxy_str).hint_text("socks5://127.0.0.1:1080").desired_width(f32::INFINITY)).changed() {
                                acc.proxy = if proxy_str.trim().is_empty() { None } else { Some(proxy_str.trim().to_string()) };
                            }
                        });

                        // Right column: 2FA Secrets & Steam Guard
                        cols[1].vertical(|ui| {
                            ui.label(RichText::new("Shared Secret (Auto-2FA Login):").font(FontId::proportional(12.0)));
                            let mut ss_str = acc.shared_secret.clone().unwrap_or_default();
                            if ui.add(egui::TextEdit::singleline(&mut ss_str).hint_text("Base64 encoded shared_secret").desired_width(f32::INFINITY)).changed() {
                                acc.shared_secret = if ss_str.trim().is_empty() { None } else { Some(ss_str.trim().to_string()) };
                            }
                            ui.add_space(4.0);

                            ui.label(RichText::new("Identity Secret (Market Bulk Sell & 2FA Confirmations):").font(FontId::proportional(12.0)));
                            let mut is_str = acc.identity_secret.clone().unwrap_or_default();
                            if ui.add(egui::TextEdit::singleline(&mut is_str).hint_text("Base64 encoded identity_secret").desired_width(f32::INFINITY)).changed() {
                                acc.identity_secret = if is_str.trim().is_empty() { None } else { Some(is_str.trim().to_string()) };
                            }
                            ui.label(RichText::new("Used for Steam Guard mobile confirmations when bulk selling on the market.").font(FontId::proportional(10.5)).color(Color32::from_rgb(110, 130, 150)));
                            ui.add_space(4.0);

                            ui.label(RichText::new("Custom AFK Auto-Reply Message (Optional):").font(FontId::proportional(12.0)));
                            let mut afk_str = acc.afk_reply_custom.clone().unwrap_or_default();
                            if ui.add(egui::TextEdit::singleline(&mut afk_str).hint_text("Uses default message from Settings if left empty").desired_width(f32::INFINITY)).changed() {
                                acc.afk_reply_custom = if afk_str.trim().is_empty() { None } else { Some(afk_str.trim().to_string()) };
                            }
                        });
                    });

                    ui.add_space(10.0);
                    ui.separator();
                    ui.add_space(6.0);

                    // ─── GAMES TO BOOST FOR THIS ACCOUNT (Vertical rows - Alt Alta, never overflows) ───
                    ui.label(RichText::new(format!("🎮 Boosting Games ({})", acc.games.len())).font(FontId::proportional(13.5)).strong());
                    ui.add_space(4.0);

                    if acc.games.is_empty() {
                        ui.label(RichText::new("No games added yet. Use store search or manual AppID below to add games.").font(FontId::proportional(11.5)).color(Color32::from_rgb(130, 150, 175)));
                    } else {
                        egui::Frame::none()
                            .fill(Color32::from_rgba_unmultiplied(12, 16, 24, 180))
                            .rounding(Rounding::same(6.0))
                            .stroke(Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 20)))
                            .inner_margin(egui::Margin::symmetric(8.0, 6.0))
                            .show(ui, |ui| {
                                ScrollArea::vertical().max_height(160.0).auto_shrink([false, false]).show(ui, |ui| {
                                    let mut to_remove_idx: Option<usize> = None;
                                    for (idx, g) in acc.games.iter().enumerate() {
                                        ui.horizontal(|ui| {
                                            let img_url = format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{}/capsule_sm_120.jpg", g.app_id);
                                            ui.add(egui::Image::new(img_url).fit_to_exact_size(Vec2::new(36.0, 18.0)).rounding(Rounding::same(3.0)));
                                            ui.add_space(4.0);
                                            ui.label(RichText::new(&g.name).font(FontId::proportional(12.0)).strong());
                                            ui.label(RichText::new(format!("(AppID: {})", g.app_id)).font(FontId::proportional(10.5)).color(Color32::from_rgb(120, 140, 165)));
                                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                if ui.button(RichText::new("✕ Remove").font(FontId::proportional(11.0)).color(Color32::from_rgb(255, 110, 110))).clicked() {
                                                    to_remove_idx = Some(idx);
                                                }
                                            });
                                        });
                                        if idx + 1 < acc.games.len() {
                                            ui.separator();
                                        }
                                    }
                                    if let Some(rem_idx) = to_remove_idx {
                                        acc.games.remove(rem_idx);
                                    }
                                });
                            });
                    }

                    ui.add_space(8.0);

                    // Add Game: Store Search with Live Autocomplete + Custom AppID
                    ui.columns(2, |cols| {
                        cols[0].vertical(|ui| {
                            ui.label(RichText::new("Search Steam Store (Auto-suggests as you type):").font(FontId::proportional(12.0)));
                            ui.horizontal(|ui| {
                                let search_edit = ui.add(
                                    egui::TextEdit::singleline(&mut self.store_search_query)
                                        .hint_text("Type game name (e.g. Counter-Strike, Rust)...")
                                        .desired_width(190.0)
                                );

                                let trimmed = self.store_search_query.trim().to_string();
                                let searching = self.is_searching_store.load(std::sync::atomic::Ordering::Relaxed);

                                // Live autocomplete trigger on text edit change or new query
                                if (search_edit.changed() || (trimmed.len() >= 2 && trimmed != self.last_searched_query)) && !searching {
                                    self.last_searched_query = trimmed.clone();
                                    let q = trimmed.clone();
                                    let results_arc = self.store_search_results.clone();
                                    let is_searching_flag = self.is_searching_store.clone();
                                    is_searching_flag.store(true, std::sync::atomic::Ordering::Relaxed);

                                    crate::steam::get_runtime().spawn(async move {
                                        let results = crate::telegram::search_steam_games(&q).await;
                                        let mut mapped = Vec::new();
                                        for val in results {
                                            if let (Some(id), Some(name)) = (
                                                val.get("app_id").and_then(|i| i.as_u64()),
                                                val.get("name").and_then(|n| n.as_str()),
                                            ) {
                                                mapped.push((id as u32, name.to_string()));
                                            }
                                        }
                                        if let Ok(mut l) = results_arc.lock() {
                                            *l = mapped;
                                        }
                                        is_searching_flag.store(false, std::sync::atomic::Ordering::Relaxed);
                                    });
                                }

                                if searching {
                                    ui.spinner();
                                } else if ui.button("🔍 Search").clicked() && !trimmed.is_empty() {
                                    self.last_searched_query = trimmed.clone();
                                    let q = trimmed;
                                    let results_arc = self.store_search_results.clone();
                                    let is_searching_flag = self.is_searching_store.clone();
                                    is_searching_flag.store(true, std::sync::atomic::Ordering::Relaxed);

                                    crate::steam::get_runtime().spawn(async move {
                                        let results = crate::telegram::search_steam_games(&q).await;
                                        let mut mapped = Vec::new();
                                        for val in results {
                                            if let (Some(id), Some(name)) = (
                                                val.get("app_id").and_then(|i| i.as_u64()),
                                                val.get("name").and_then(|n| n.as_str()),
                                            ) {
                                                mapped.push((id as u32, name.to_string()));
                                            }
                                        }
                                        if let Ok(mut l) = results_arc.lock() {
                                            *l = mapped;
                                        }
                                        is_searching_flag.store(false, std::sync::atomic::Ordering::Relaxed);
                                    });
                                }
                            });

                            let results = self.store_search_results.lock().unwrap().clone();
                            if !results.is_empty() {
                                ScrollArea::vertical().max_height(140.0).show(ui, |ui| {
                                    for (app_id, name) in results.iter().take(12) {
                                        let is_already_added = acc.games.iter().any(|g| g.app_id == *app_id);
                                        let img_url = format!("https://shared.akamai.steamstatic.com/store_item_assets/steam/apps/{}/capsule_231x87.jpg", app_id);

                                        ui.horizontal(|ui| {
                                            ui.add(egui::Image::new(&img_url).fit_to_exact_size(Vec2::new(38.0, 18.0)).rounding(Rounding::same(3.0)));
                                            ui.label(RichText::new(name).font(FontId::proportional(11.5)));
                                            ui.label(RichText::new(format!("({})", app_id)).font(FontId::proportional(10.0)).color(Color32::from_rgb(130, 150, 175)));

                                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                if is_already_added {
                                                    ui.label(RichText::new("✓ Added").font(FontId::proportional(11.0)).color(Color32::from_rgb(100, 200, 130)));
                                                } else if ui.button(RichText::new("＋ Add").font(FontId::proportional(11.0)).color(Color32::from_rgb(90, 200, 255))).clicked() {
                                                    acc.games.push(GameEntry::new(*app_id, name.clone()));
                                                }
                                            });
                                        });
                                        ui.separator();
                                    }
                                });
                            }
                        });

                        cols[1].vertical(|ui| {
                            ui.label(RichText::new("Add via Manual AppID:").font(FontId::proportional(12.0)));
                            ui.horizontal(|ui| {
                                ui.add(egui::TextEdit::singleline(&mut self.custom_appid_input).hint_text("AppID (e.g. 730)").desired_width(90.0));
                                ui.add(egui::TextEdit::singleline(&mut self.custom_appname_input).hint_text("Game Name").desired_width(120.0));
                                if ui.button("＋ Add").clicked() {
                                    if let Ok(id) = self.custom_appid_input.trim().parse::<u32>() {
                                        let name = if self.custom_appname_input.trim().is_empty() {
                                            format!("AppID {}", id)
                                        } else {
                                            self.custom_appname_input.trim().to_string()
                                        };
                                        if !acc.games.iter().any(|g| g.app_id == id) {
                                            acc.games.push(GameEntry::new(id, name));
                                            self.custom_appid_input.clear();
                                            self.custom_appname_input.clear();
                                        }
                                    }
                                }
                            });
                        });
                    });

                    ui.add_space(14.0);

                    // Action buttons: Save & Cancel
                    ui.horizontal(|ui| {
                        if ui.button(RichText::new(strings.btn_save).font(FontId::proportional(13.0)).color(Color32::WHITE).strong()).clicked() {
                            save_account = true;
                        }
                        if ui.button(RichText::new(strings.btn_cancel).font(FontId::proportional(12.0))).clicked() {
                            cancel_edit = true;
                        }
                    });
                });
            });

            if save_account {
                let saved_id = acc.id;
                let saved_games = acc.games.clone();
                let saved_title = acc.custom_title.clone();
                let saved_persona = acc.persona_state;
                let saved_afk = acc.afk_reply_custom.clone();

                if is_new {
                    acc.id = *next_id;
                    *next_id += 1;
                    accounts.push(acc.clone());
                } else {
                    if let Some(pos) = accounts.iter().position(|a| a.id == acc.id) {
                        accounts[pos] = acc.clone();
                    }
                }

                // Update runtime account if currently loaded
                for r in runtime_accounts {
                    let mut lock = r.lock().unwrap();
                    if lock.id == saved_id {
                        lock.games = saved_games.clone();
                        lock.custom_title = saved_title.clone();
                        lock.persona_state = saved_persona;
                        lock.afk_reply_custom = saved_afk.clone();
                        lock.username = acc.username.clone();
                    }
                }

                // Hot-reload presence for active boosting session
                crate::steam::update_presence(saved_id, saved_games, saved_title, saved_persona, saved_afk);

                self.editing_account = None;
                self.is_adding_new = false;
            } else if cancel_edit {
                self.editing_account = None;
                self.is_adding_new = false;
            } else {
                self.editing_account = Some(acc);
            }

            return;
        }

        // ─── TOP ACTION BAR ───
        ThemeEngine::card_frame(theme_mode, colors).show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui.button(RichText::new(strings.btn_manual_add).font(FontId::proportional(12.5)).color(accent)).clicked() {
                    let new_acc = Account::new(*next_id);
                    self.editing_account = Some(new_acc);
                    self.is_adding_new = true;
                }

                if ui.button(RichText::new(strings.btn_qr_add).font(FontId::proportional(12.5))).clicked() {
                    *show_qr_modal = true;
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(RichText::new(strings.btn_stop_all).font(FontId::proportional(12.0)).color(Color32::from_rgb(255, 120, 120))).clicked() {
                        for arc in runtime_accounts {
                            let id = arc.lock().unwrap().id;
                            crate::steam::stop_boost(id, arc.clone());
                        }
                    }

                    if ui.button(RichText::new(strings.btn_start_all).font(FontId::proportional(12.0)).color(Color32::from_rgb(90, 215, 120))).clicked() {
                        for arc in runtime_accounts {
                            crate::steam::start_boost(arc.clone());
                        }
                    }
                });
            });
        });

        ui.add_space(8.0);

        // ─── ACCOUNTS LIST ───
        if accounts.is_empty() {
            ThemeEngine::card_frame(theme_mode, colors).show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(24.0);
                    ui.label(RichText::new(strings.no_accounts).font(FontId::proportional(14.0)).color(Color32::from_rgb(140, 160, 180)));
                    ui.label(RichText::new(strings.no_accounts_hint).font(FontId::proportional(12.0)).color(Color32::from_rgb(110, 130, 150)));
                    ui.add_space(24.0);
                });
            });
            return;
        }

        let mut to_delete_id: Option<u64> = None;
        let mut to_edit_acc: Option<Account> = None;

        ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            for acc in accounts.iter() {
                // Find matching runtime account status
                let runtime_arc = runtime_accounts.iter().find(|ra| ra.lock().unwrap().id == acc.id).cloned();
                let status = runtime_arc.as_ref().map(|ra| ra.lock().unwrap().status.clone()).unwrap_or(acc.status.clone());
                let hours = runtime_arc.as_ref().map(|ra| ra.lock().unwrap().hours_boosted).unwrap_or(acc.hours_boosted);

                ThemeEngine::card_frame(theme_mode, colors).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        // User Avatar + Username
                        let initials: String = acc.username.chars().take(2).collect::<String>().to_uppercase();
                        let (rect, _) = ui.allocate_exact_size(Vec2::splat(36.0), egui::Sense::hover());
                        ui.painter().rect_filled(rect, Rounding::same(18.0), Color32::from_rgba_premultiplied(accent.r(), accent.g(), accent.b(), 60));
                        ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, initials, FontId::proportional(13.0), Color32::WHITE);

                        ui.add_space(8.0);

                        ui.vertical(|ui| {
                            ui.label(RichText::new(&acc.username).font(FontId::proportional(14.0)).strong());
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(format!("⏱ {:.1}h", hours)).font(FontId::proportional(11.0)).color(Color32::from_rgb(120, 140, 160)));
                                ui.label(RichText::new(format!("• 🎮 {} oyun", acc.games.len())).font(FontId::proportional(11.0)).color(Color32::from_rgb(120, 140, 160)));
                                if acc.identity_secret.is_some() {
                                    ui.label(RichText::new("• 📱 2FA Hazır").font(FontId::proportional(10.5)).color(Color32::from_rgb(80, 200, 120)));
                                }
                            });
                        });

                        ui.add_space(12.0);

                        // Status Badge with high contrast border and readable text
                        let (status_text, border_color, text_color) = match &status {
                            AccountStatus::Boosting => ("🟢 Boosting", Color32::from_rgb(46, 204, 113), Color32::from_rgb(80, 240, 140)),
                            AccountStatus::Connecting => ("🟡 Connecting...", Color32::from_rgb(255, 190, 60), Color32::from_rgb(255, 215, 100)),
                            AccountStatus::AwaitingSteamGuard { .. } => ("🛡️ Guard Code Needed", Color32::from_rgb(255, 120, 60), Color32::from_rgb(255, 160, 100)),
                            AccountStatus::PausedForGame { .. } => ("⏸️ In-Game Paused", Color32::from_rgb(255, 200, 70), Color32::from_rgb(255, 220, 120)),
                            AccountStatus::Reconnecting { .. } => ("🔄 Reconnecting...", Color32::from_rgb(120, 180, 255), Color32::from_rgb(160, 210, 255)),
                            AccountStatus::Error(_e) => ("🔴 Error", Color32::from_rgb(255, 90, 90), Color32::from_rgb(255, 130, 130)),
                            AccountStatus::Idle => ("⚪ Stopped", Color32::from_rgb(130, 145, 165), Color32::from_rgb(170, 185, 205)),
                        };

                        egui::Frame::none()
                            .fill(Color32::from_rgba_unmultiplied(16, 24, 20, 220))
                            .rounding(Rounding::same(12.0))
                            .inner_margin(egui::Margin::symmetric(10.0, 3.5))
                            .stroke(Stroke::new(1.0_f32, border_color))
                            .show(ui, |ui| {
                                ui.label(RichText::new(status_text).font(FontId::proportional(11.5)).color(text_color).strong());
                            });

                        // Right action buttons
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button(RichText::new(strings.btn_delete).font(FontId::proportional(12.0))).clicked() {
                                to_delete_id = Some(acc.id);
                            }

                            if ui.button(RichText::new(strings.btn_edit).font(FontId::proportional(12.0))).clicked() {
                                to_edit_acc = Some(acc.clone());
                            }

                            if let Some(ref ra) = runtime_arc {
                                if matches!(status, AccountStatus::Boosting) {
                                    if ui.button(RichText::new(strings.btn_stop).font(FontId::proportional(12.0)).color(Color32::from_rgb(255, 120, 120))).clicked() {
                                        crate::steam::stop_boost(acc.id, ra.clone());
                                    }
                                } else {
                                    if ui.button(RichText::new(strings.btn_start).font(FontId::proportional(12.0)).color(Color32::from_rgb(90, 215, 120))).clicked() {
                                        crate::steam::start_boost(ra.clone());
                                    }
                                }
                            }
                        });
                    });

                    // If waiting for steam guard code, show code input row
                    if let AccountStatus::AwaitingSteamGuard { ref hint } = status {
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(format!("🛡️ Steam Guard Kodu Gerekli ({}):", hint)).font(FontId::proportional(11.5)).color(Color32::from_rgb(255, 175, 50)));
                            let guard_input = self.guard_inputs.entry(acc.id).or_insert_with(String::new);
                            ui.add(egui::TextEdit::singleline(guard_input).desired_width(100.0));
                            if ui.button("✓ Gönder").clicked() && !guard_input.trim().is_empty() {
                                let code = guard_input.trim().to_string();
                                crate::steam::submit_guard_code(acc.id, &code);
                                guard_input.clear();
                            }
                        });
                    }
                });
                ui.add_space(3.0);
            }
        });

        if let Some(del_id) = to_delete_id {
            if let Some(pos) = accounts.iter().position(|a| a.id == del_id) {
                accounts.remove(pos);
            }
        }

        if let Some(edit_acc) = to_edit_acc {
            self.editing_account = Some(edit_acc);
            self.is_adding_new = false;
        }
    }
}
