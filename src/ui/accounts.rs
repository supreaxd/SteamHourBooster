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
    pub owned_games_filter: String,
    pub is_refreshing_owned_games: Arc<std::sync::atomic::AtomicBool>,
    pub refreshed_owned_games: Arc<Mutex<Option<(u64, Vec<GameEntry>)>>>,
    pub pending_delete_account: Option<(u64, String)>,
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
            owned_games_filter: String::new(),
            is_refreshing_owned_games: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            refreshed_owned_games: Arc::new(Mutex::new(None)),
            pending_delete_account: None,
        }
    }

    pub fn draw(
        &mut self,
        ui: &mut Ui,
        accounts: &mut Vec<Account>,
        runtime_accounts: &mut Vec<Arc<Mutex<Account>>>,
        colors: &CustomThemeColors,
        theme_mode: ThemeMode,
        strings: &Strings,
        next_id: &mut u64,
        show_qr_modal: &mut bool,
    ) -> bool {
        let accent = Color32::from_rgb(colors.accent[0], colors.accent[1], colors.accent[2]);
        let mut save_needed = false;

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

                ScrollArea::vertical().id_salt("edit_account_main_scroll").auto_shrink([false, false]).show(ui, |ui| {
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
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("🎮 Boosting Games ({})", acc.games.len())).font(FontId::proportional(13.5)).strong());
                        if !acc.games.is_empty() {
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.button(RichText::new("✕ Clear All").font(FontId::proportional(11.0)).color(Color32::from_rgb(255, 110, 110))).clicked() {
                                    acc.games.clear();
                                }
                            });
                        }
                    });
                    ui.add_space(4.0);

                    if acc.games.is_empty() {
                        ui.label(RichText::new("No games added yet. Use store search, manual AppID, or the library list below.").font(FontId::proportional(11.5)).color(Color32::from_rgb(130, 150, 175)));
                    } else {
                        egui::Frame::none()
                            .fill(Color32::from_rgba_unmultiplied(12, 16, 24, 180))
                            .rounding(Rounding::same(6.0))
                            .stroke(Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 20)))
                            .inner_margin(egui::Margin::symmetric(8.0, 6.0))
                            .show(ui, |ui| {
                                ScrollArea::vertical()
                                    .id_salt("boosting_games_box_scroll")
                                    .max_height(130.0)
                                    .auto_shrink([false, false])
                                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
                                    .show(ui, |ui| {
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
                                ScrollArea::vertical().id_salt("store_search_results_scroll").max_height(140.0).show(ui, |ui| {
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

                    ui.add_space(10.0);
                    ui.separator();
                    ui.add_space(6.0);

                    // ─── ACCOUNT OWNED GAMES (LIBRARY) ───
                    // Apply any background refreshed games
                    if let Ok(mut slot) = self.refreshed_owned_games.lock() {
                        if let Some((s_id, fresh)) = slot.take() {
                            if acc.steam_id == s_id || acc.steam_id == 0 {
                                acc.owned_games = fresh;
                            }
                        }
                    }

                    // Automatically load from cache or active runtime if acc.owned_games is empty
                    if acc.owned_games.is_empty() {
                        let cached = crate::inventory::get_cached_owned_games(acc.steam_id);
                        if !cached.is_empty() {
                            acc.owned_games = cached;
                        } else {
                            for ra in runtime_accounts.iter() {
                                if let Ok(l) = ra.lock() {
                                    if (l.id == acc.id || (acc.steam_id > 0 && l.steam_id == acc.steam_id)) && !l.owned_games.is_empty() {
                                        acc.owned_games = l.owned_games.clone();
                                        crate::inventory::store_owned_games_cache(acc.steam_id, acc.owned_games.clone());
                                        break;
                                    }
                                }
                            }
                        }
                    }

                    ui.horizontal(|ui| {
                        ui.label(RichText::new("🎮 Account Owned Games (Library)").font(FontId::proportional(13.0)).strong());
                        ui.label(RichText::new(format!("({} games)", acc.owned_games.len())).font(FontId::proportional(11.0)).color(Color32::from_rgb(130, 150, 175)));

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let is_refreshing = self.is_refreshing_owned_games.load(std::sync::atomic::Ordering::Relaxed);
                            if is_refreshing {
                                ui.spinner();
                                ui.label(RichText::new("Refreshing...").font(FontId::proportional(11.0)).color(Color32::from_rgb(130, 150, 175)));
                            } else {
                                if ui.button(RichText::new("🔄 Refresh Games").font(FontId::proportional(11.5)).color(accent)).clicked() {
                                    let s_id = acc.steam_id;
                                    let p_url = acc.proxy.clone();
                                    let flag = self.is_refreshing_owned_games.clone();
                                    let slot_clone = self.refreshed_owned_games.clone();
                                    let ctx_clone = ui.ctx().clone();
                                    flag.store(true, std::sync::atomic::Ordering::Relaxed);
                                    crate::steam::get_runtime().spawn(async move {
                                        let fresh_games = crate::inventory::fetch_account_owned_games_force(s_id, p_url).await;
                                        crate::inventory::store_owned_games_cache(s_id, fresh_games.clone());
                                        if let Ok(mut l) = slot_clone.lock() {
                                            *l = Some((s_id, fresh_games));
                                        }
                                        flag.store(false, std::sync::atomic::Ordering::Relaxed);
                                        ctx_clone.request_repaint();
                                    });
                                }

                                ui.add_space(4.0);
                                if ui.button(RichText::new("＋ Add All to Boost").font(FontId::proportional(11.5)).color(Color32::from_rgb(90, 220, 140)).strong()).clicked() {
                                    let filter = self.owned_games_filter.trim().to_lowercase();
                                    for g in &acc.owned_games {
                                        if (filter.is_empty() || g.name.to_lowercase().contains(&filter) || g.app_id.to_string().contains(&filter))
                                            && !acc.games.iter().any(|existing| existing.app_id == g.app_id)
                                        {
                                            acc.games.push(g.clone());
                                        }
                                    }
                                }
                            }
                        });
                    });

                    ui.add_space(4.0);

                    // Filter search input for owned games
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("🔍").font(FontId::proportional(12.0)));
                        ui.add(egui::TextEdit::singleline(&mut self.owned_games_filter)
                            .hint_text("Filter owned games by name or AppID...")
                            .desired_width(f32::INFINITY));
                    });

                    ui.add_space(4.0);

                    if acc.owned_games.is_empty() {
                        egui::Frame::none()
                            .fill(Color32::from_rgba_unmultiplied(20, 26, 36, 180))
                            .rounding(Rounding::same(6.0))
                            .inner_margin(egui::Margin::symmetric(10.0, 8.0))
                            .show(ui, |ui| {
                                ui.label(RichText::new("ℹ️ No owned games found yet. Click '🔄 Refresh Games' to load your library, or start boosting once to sync automatically via Steam.").font(FontId::proportional(11.0)).color(Color32::from_rgb(140, 160, 185)));
                            });
                    } else {
                        let filter = self.owned_games_filter.trim().to_lowercase();
                        let filtered_games: Vec<GameEntry> = acc.owned_games
                            .iter()
                            .filter(|g| {
                                if filter.is_empty() {
                                    true
                                } else {
                                    g.name.to_lowercase().contains(&filter) || g.app_id.to_string().contains(&filter)
                                }
                            })
                            .cloned()
                            .collect();

                        egui::Frame::none()
                            .fill(Color32::from_rgba_unmultiplied(15, 20, 28, 200))
                            .rounding(Rounding::same(6.0))
                            .stroke(Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 12)))
                            .inner_margin(egui::Margin::symmetric(8.0, 6.0))
                            .show(ui, |ui| {
                                ScrollArea::vertical()
                                    .id_salt("owned_games_library_scroll")
                                    .max_height(240.0)
                                    .auto_shrink([false, false])
                                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
                                    .show(ui, |ui| {
                                    let mut to_add_game: Option<GameEntry> = None;
                                    let mut to_remove_id: Option<u32> = None;

                                    for game in filtered_games.iter().take(300) {
                                        let is_in_boost = acc.games.iter().any(|g| g.app_id == game.app_id);
                                        let img_url = format!("https://shared.akamai.steamstatic.com/store_item_assets/steam/apps/{}/capsule_231x87.jpg", game.app_id);

                                        ui.horizontal(|ui| {
                                            ui.add(egui::Image::new(&img_url).fit_to_exact_size(Vec2::new(42.0, 20.0)).rounding(Rounding::same(3.0)));
                                            ui.label(RichText::new(&game.name).font(FontId::proportional(12.0)));
                                            ui.label(RichText::new(format!("({})", game.app_id)).font(FontId::proportional(10.0)).color(Color32::from_rgb(120, 140, 165)));

                                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                if is_in_boost {
                                                    if ui.button(RichText::new("✕ Remove").font(FontId::proportional(10.5)).color(Color32::from_rgb(255, 120, 120))).clicked() {
                                                        to_remove_id = Some(game.app_id);
                                                    }
                                                    ui.label(RichText::new("✓ Boosting").font(FontId::proportional(11.0)).color(Color32::from_rgb(90, 220, 130)).strong());
                                                } else {
                                                    if ui.button(RichText::new("＋ Add to Boost").font(FontId::proportional(11.0)).color(Color32::from_rgb(90, 200, 255))).clicked() {
                                                        to_add_game = Some(game.clone());
                                                    }
                                                }
                                            });
                                        });
                                        ui.separator();
                                    }

                                    if let Some(g) = to_add_game {
                                        acc.games.push(g);
                                    }
                                    if let Some(rem_id) = to_remove_id {
                                        acc.games.retain(|g| g.app_id != rem_id);
                                    }
                                });
                            });
                    }

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
                    let new_arc = Arc::new(Mutex::new(acc.clone()));
                    runtime_accounts.push(new_arc);
                    crate::telegram::update_accounts_for_bot(runtime_accounts);
                } else {
                    if let Some(pos) = accounts.iter().position(|a| a.id == acc.id) {
                        accounts[pos] = acc.clone();
                    }
                    // Update runtime account if currently loaded
                    for r in runtime_accounts.iter() {
                        let mut lock = r.lock().unwrap();
                        if lock.id == saved_id {
                            lock.games = saved_games.clone();
                            lock.custom_title = saved_title.clone();
                            lock.persona_state = saved_persona;
                            lock.afk_reply_custom = saved_afk.clone();
                            lock.username = acc.username.clone();
                            lock.password = acc.password.clone();
                            lock.shared_secret = acc.shared_secret.clone();
                            lock.identity_secret = acc.identity_secret.clone();
                            lock.proxy = acc.proxy.clone();
                        }
                    }
                }

                // Hot-reload presence for active boosting session
                crate::steam::update_presence(saved_id, saved_games, saved_title, saved_persona, saved_afk);

                self.editing_account = None;
                self.is_adding_new = false;
                save_needed = true;
            } else if cancel_edit {
                self.editing_account = None;
                self.is_adding_new = false;
            } else {
                self.editing_account = Some(acc);
            }

            return save_needed;
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
                        for arc in runtime_accounts.iter() {
                            let id = arc.lock().unwrap().id;
                            crate::steam::stop_boost(id, arc.clone());
                        }
                    }

                    if ui.button(RichText::new(strings.btn_start_all).font(FontId::proportional(12.0)).color(Color32::from_rgb(90, 215, 120))).clicked() {
                        for arc in runtime_accounts.iter() {
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
            return save_needed;
        }

        let mut to_delete_id: Option<u64> = None;
        let mut to_edit_acc: Option<Account> = None;

        ScrollArea::vertical().id_salt("accounts_list_scroll").auto_shrink([false, false]).show(ui, |ui| {
            for acc in accounts.iter() {
                // Find matching runtime account status
                let runtime_arc = runtime_accounts.iter().find(|ra| ra.lock().unwrap().id == acc.id).cloned();
                let status = runtime_arc.as_ref().map(|ra| ra.lock().unwrap().status.clone()).unwrap_or(acc.status.clone());
                let hours = runtime_arc.as_ref().map(|ra| ra.lock().unwrap().hours_boosted).unwrap_or(acc.hours_boosted);

                let _card_resp = ThemeEngine::card_frame(theme_mode, colors).show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.horizontal(|ui| {
                        // Clickable Avatar + Status Dot + Username -> opens account editor directly
                        let initials: String = acc.username.chars().take(2).collect::<String>().to_uppercase();
                        let (rect, av_resp) = ui.allocate_exact_size(Vec2::splat(36.0), egui::Sense::click());
                        ui.painter().rect_filled(rect, Rounding::same(18.0), Color32::from_rgba_premultiplied(accent.r(), accent.g(), accent.b(), 60));
                        ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, initials, FontId::proportional(13.0), Color32::WHITE);
                        let avatar_dot = match &status {
                            AccountStatus::Boosting => Color32::from_rgb(80, 225, 130),
                            AccountStatus::PausedForGame { .. } => Color32::from_rgb(255, 145, 35),
                            AccountStatus::Connecting | AccountStatus::Reconnecting { .. } => Color32::from_rgb(255, 200, 60),
                            AccountStatus::AwaitingSteamGuard { .. } => Color32::from_rgb(255, 115, 50),
                            AccountStatus::Error(_) => Color32::from_rgb(255, 85, 85),
                            AccountStatus::Idle => Color32::from_rgb(130, 145, 165),
                        };
                        ui.painter().circle_filled(egui::pos2(rect.max.x - 4.0, rect.max.y - 4.0), 5.0, avatar_dot);
                        if av_resp.clicked() {
                            to_edit_acc = Some(acc.clone());
                        }

                        ui.add_space(8.0);

                        ui.vertical(|ui| {
                            if ui
                                .link(RichText::new(&acc.username).font(FontId::proportional(14.0)).strong())
                                .on_hover_text(strings.edit_title_edit)
                                .clicked()
                            {
                                to_edit_acc = Some(acc.clone());
                            }
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(format!("⏱ {:.1}h", hours)).font(FontId::proportional(11.0)).color(Color32::from_rgb(120, 140, 160)));
                                ui.label(RichText::new(format!("• 🎮 {} {}", acc.games.len(), strings.games_count_unit)).font(FontId::proportional(11.0)).color(Color32::from_rgb(120, 140, 160)));
                                if acc.identity_secret.is_some() {
                                    ui.label(RichText::new("• 2FA Ready").font(FontId::proportional(10.5)).color(Color32::from_rgb(80, 200, 120)));
                                }
                            });
                        });

                        ui.add_space(12.0);

                        // Status Badge with high contrast border and readable text
                        let (status_text, border_color, text_color) = match &status {
                            AccountStatus::Boosting => (strings.status_boosting.to_string(), Color32::from_rgb(46, 204, 113), Color32::from_rgb(80, 240, 140)),
                            AccountStatus::Connecting => (strings.status_connecting.to_string(), Color32::from_rgb(255, 190, 60), Color32::from_rgb(255, 215, 100)),
                            AccountStatus::AwaitingSteamGuard { .. } => (strings.status_guard_needed.to_string(), Color32::from_rgb(255, 115, 50), Color32::from_rgb(255, 160, 100)),
                            AccountStatus::PausedForGame { app_name, .. } => (format!("{} ({})", strings.status_paused_game, app_name), Color32::from_rgb(255, 145, 35), Color32::from_rgb(255, 175, 75)),
                            AccountStatus::Reconnecting { attempt, retry_in_secs } => (format!("{} (#{} · {}s)", strings.status_reconnecting, attempt, retry_in_secs), Color32::from_rgb(255, 200, 60), Color32::from_rgb(255, 220, 115)),
                            AccountStatus::Error(_e) => (strings.status_error.to_string(), Color32::from_rgb(255, 90, 90), Color32::from_rgb(255, 130, 130)),
                            AccountStatus::Idle => (strings.status_idle.to_string(), Color32::from_rgb(130, 145, 165), Color32::from_rgb(170, 185, 205)),
                        };

                        egui::Frame::none()
                            .fill(Color32::from_rgba_unmultiplied(16, 24, 20, 190))
                            .rounding(Rounding::same(12.0))
                            .inner_margin(egui::Margin::symmetric(10.0, 3.5))
                            .stroke(Stroke::new(1.0_f32, border_color))
                            .show(ui, |ui| {
                                ui.label(RichText::new(format!("● {}", status_text)).font(FontId::proportional(11.5)).color(text_color).strong());
                            });

                        // Right action buttons (Pencil removed; Delete triggers confirmation dialog)
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button(RichText::new(strings.btn_delete).font(FontId::proportional(12.0))).clicked() {
                                self.pending_delete_account = Some((acc.id, acc.username.clone()));
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
                            ui.label(RichText::new(format!("🛡️ Steam Guard Code Required ({}):", hint)).font(FontId::proportional(11.5)).color(Color32::from_rgb(255, 175, 50)));
                            let guard_input = self.guard_inputs.entry(acc.id).or_insert_with(String::new);
                            ui.add(egui::TextEdit::singleline(guard_input).desired_width(100.0));
                            if ui.button("✓ Submit").clicked() && !guard_input.trim().is_empty() {
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

        // Delete Account Confirmation Modal
        if let Some((del_id, del_name)) = self.pending_delete_account.clone() {
            egui::Window::new(strings.pick("⚠️ Confirm Account Removal", "⚠️ Hesap Silme Onayı", "⚠️ Подтвердите удаление"))
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .default_width(400.0)
                .show(ui.ctx(), |ui| {
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(match strings.lang {
                            crate::i18n::Language::English => format!("Are you sure you want to delete account '{}'?\nBoosting will stop and saved settings/tokens for this account will be removed.", del_name),
                            crate::i18n::Language::Turkish => format!("'{}' hesabını silmek istediğinize emin misiniz?\nBoost durdurulacak ve bu hesaba ait kayıtlı ayarlar kalıcı olarak silinecektir.", del_name),
                            crate::i18n::Language::Russian => format!("Вы уверены, что хотите удалить аккаунт '{}'?\nБуст будет остановлен, а данные удалены.", del_name),
                        })
                        .font(FontId::proportional(13.0)),
                    );
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        if ui
                            .button(
                                RichText::new(strings.pick("🗑 Delete Account", "🗑 Hesabı Sil", "🗑 Удалить аккаунт"))
                                    .color(Color32::from_rgb(255, 110, 110))
                                    .strong(),
                            )
                            .clicked()
                        {
                            to_delete_id = Some(del_id);
                            self.pending_delete_account = None;
                        }
                        if ui.button(strings.btn_cancel).clicked() {
                            self.pending_delete_account = None;
                        }
                    });
                });
        }

        if let Some(del_id) = to_delete_id {
            if let Some(pos) = runtime_accounts.iter().position(|r| r.lock().unwrap().id == del_id) {
                let arc = runtime_accounts.remove(pos);
                crate::steam::stop_boost(del_id, arc);
                crate::telegram::update_accounts_for_bot(runtime_accounts);
            }
            if let Some(pos) = accounts.iter().position(|a| a.id == del_id) {
                accounts.remove(pos);
            }
            save_needed = true;
        }

        if let Some(edit_acc) = to_edit_acc {
            self.editing_account = Some(edit_acc);
            self.is_adding_new = false;
        }

        save_needed
    }
}
