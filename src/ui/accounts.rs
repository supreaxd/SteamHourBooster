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
    pub store_search_results: Arc<Mutex<Vec<(u32, String)>>>,
    pub is_searching_store: bool,
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
            store_search_results: Arc::new(Mutex::new(Vec::new())),
            is_searching_store: false,
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
                    let title = if is_new { "＋ Yeni Steam Hesabı Ekle" } else { "✏ Steam Hesabını Düzenle" };
                    ui.label(RichText::new(title).font(FontId::proportional(15.0)).strong());
                });
                ui.add_space(8.0);

                ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    ui.columns(2, |cols| {
                        // Left column: Credentials & Persona
                        cols[0].vertical(|ui| {
                            ui.label(RichText::new("Steam Kullanıcı Adı:").font(FontId::proportional(12.0)));
                            ui.add(egui::TextEdit::singleline(&mut acc.username).desired_width(f32::INFINITY));
                            ui.add_space(4.0);

                            ui.label(RichText::new("Şifre:").font(FontId::proportional(12.0)));
                            ui.add(egui::TextEdit::singleline(&mut acc.password).password(true).desired_width(f32::INFINITY));
                            ui.add_space(4.0);

                            ui.label(RichText::new("Özel Durum / Oyun Başlığı:").font(FontId::proportional(12.0)));
                            ui.add(egui::TextEdit::singleline(&mut acc.custom_title).hint_text("Half-Life 3 (Beta)").desired_width(f32::INFINITY));
                            ui.add_space(4.0);

                            ui.label(RichText::new("Steam Görünüm Durumu:").font(FontId::proportional(12.0)));
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
                            ui.label(RichText::new("Shared Secret (Otomatik 2FA Girişi):").font(FontId::proportional(12.0)));
                            let mut ss_str = acc.shared_secret.clone().unwrap_or_default();
                            if ui.add(egui::TextEdit::singleline(&mut ss_str).hint_text("Base64 formatında shared_secret").desired_width(f32::INFINITY)).changed() {
                                acc.shared_secret = if ss_str.trim().is_empty() { None } else { Some(ss_str.trim().to_string()) };
                            }
                            ui.add_space(4.0);

                            ui.label(RichText::new("Identity Secret (Toplu Satış & Mobil Onay):").font(FontId::proportional(12.0)));
                            let mut is_str = acc.identity_secret.clone().unwrap_or_default();
                            if ui.add(egui::TextEdit::singleline(&mut is_str).hint_text("Base64 formatında identity_secret").desired_width(f32::INFINITY)).changed() {
                                acc.identity_secret = if is_str.trim().is_empty() { None } else { Some(is_str.trim().to_string()) };
                            }
                            ui.label(RichText::new("Pazarda toplu eşya satışı yapıldığında mobil onay için kullanılır.").font(FontId::proportional(10.5)).color(Color32::from_rgb(110, 130, 150)));
                            ui.add_space(4.0);

                            ui.label(RichText::new("Özel AFK Yanıt Mesajı (Opsiyonel):").font(FontId::proportional(12.0)));
                            let mut afk_str = acc.afk_reply_custom.clone().unwrap_or_default();
                            if ui.add(egui::TextEdit::singleline(&mut afk_str).hint_text("Boşsa genel ayarlardaki mesaj kullanılır").desired_width(f32::INFINITY)).changed() {
                                acc.afk_reply_custom = if afk_str.trim().is_empty() { None } else { Some(afk_str.trim().to_string()) };
                            }
                        });
                    });

                    ui.add_space(10.0);
                    ui.separator();
                    ui.add_space(6.0);

                    // ─── GAMES TO BOOST FOR THIS ACCOUNT ───
                    ui.label(RichText::new("🎮 Kasılan Oyunlar").font(FontId::proportional(14.0)).strong());
                    ui.add_space(4.0);

                    // Current games chips with remove button
                    ui.horizontal_wrapped(|ui| {
                        let mut to_remove_idx: Option<usize> = None;
                        for (idx, g) in acc.games.iter().enumerate() {
                            egui::Frame::none()
                                .fill(Color32::from_rgba_premultiplied(0, 0, 0, 80))
                                .rounding(Rounding::same(6.0))
                                .stroke(Stroke::new(1.0_f32, Color32::from_rgba_premultiplied(255, 255, 255, 25)))
                                .inner_margin(egui::Margin::symmetric(8.0, 4.0))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new(&g.name).font(FontId::proportional(12.0)));
                                        ui.label(RichText::new(format!("({})", g.app_id)).font(FontId::proportional(10.5)).color(Color32::from_rgb(110, 130, 150)));
                                        if ui.button(RichText::new("✕").font(FontId::proportional(11.0)).color(Color32::from_rgb(255, 120, 120))).clicked() {
                                            to_remove_idx = Some(idx);
                                        }
                                    });
                                });
                        }
                        if let Some(rem_idx) = to_remove_idx {
                            acc.games.remove(rem_idx);
                        }
                    });

                    ui.add_space(8.0);

                    // Add Game: Store Search + Custom AppID
                    ui.columns(2, |cols| {
                        cols[0].vertical(|ui| {
                            ui.label(RichText::new("Steam Mağazasında Ara:").font(FontId::proportional(12.0)));
                            ui.horizontal(|ui| {
                                ui.add(egui::TextEdit::singleline(&mut self.store_search_query).hint_text("Oyun adı...").desired_width(180.0));
                                if ui.button("🔍 Ara").clicked() && !self.store_search_query.trim().is_empty() {
                                    self.is_searching_store = true;
                                    let q = self.store_search_query.clone();
                                    let results_arc = self.store_search_results.clone();
                                    crate::steam::get_runtime().spawn(async move {
                                        let results = crate::telegram::search_steam_games(&q).await;
                                        let mut mapped = Vec::new();
                                        for val in results {
                                            if let (Some(id), Some(name)) = (val.get("app_id").and_then(|i| i.as_u64()), val.get("name").and_then(|n| n.as_str())) {
                                                mapped.push((id as u32, name.to_string()));
                                            }
                                        }
                                        if let Ok(mut l) = results_arc.lock() {
                                            *l = mapped;
                                        }
                                    });
                                    self.is_searching_store = false;
                                }
                            });

                            let results = self.store_search_results.lock().unwrap().clone();
                            if !results.is_empty() {
                                ScrollArea::vertical().max_height(100.0).show(ui, |ui| {
                                    for (app_id, name) in results.iter().take(8) {
                                        ui.horizontal(|ui| {
                                            ui.label(RichText::new(name).font(FontId::proportional(11.5)));
                                            if ui.button("＋ Ekle").clicked() {
                                                if !acc.games.iter().any(|g| g.app_id == *app_id) {
                                                    acc.games.push(GameEntry::new(*app_id, name.clone()));
                                                }
                                            }
                                        });
                                    }
                                });
                            }
                        });

                        cols[1].vertical(|ui| {
                            ui.label(RichText::new("Manuel AppID ile Ekle:").font(FontId::proportional(12.0)));
                            ui.horizontal(|ui| {
                                ui.add(egui::TextEdit::singleline(&mut self.custom_appid_input).hint_text("AppID (örn: 730)").desired_width(90.0));
                                ui.add(egui::TextEdit::singleline(&mut self.custom_appname_input).hint_text("Oyun Adı").desired_width(120.0));
                                if ui.button("＋ Ekle").clicked() {
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
                if is_new {
                    acc.id = *next_id;
                    *next_id += 1;
                    accounts.push(acc);
                } else {
                    if let Some(pos) = accounts.iter().position(|a| a.id == acc.id) {
                        accounts[pos] = acc;
                    }
                }
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

                        // Status Badge
                        let (status_text, status_color) = match &status {
                            AccountStatus::Boosting => ("🟢 Kasıyor", Color32::from_rgb(90, 215, 120)),
                            AccountStatus::Connecting => ("🟡 Bağlanıyor...", Color32::from_rgb(255, 190, 60)),
                            AccountStatus::AwaitingSteamGuard { .. } => ("🛡️ Guard Kodu Bekleniyor", Color32::from_rgb(255, 120, 60)),
                            AccountStatus::PausedForGame { .. } => ("⏸️ Oyundasınız", Color32::from_rgb(255, 200, 70)),
                            AccountStatus::Reconnecting { .. } => ("🔄 Yeniden Bağlanıyor", Color32::from_rgb(120, 180, 255)),
                            AccountStatus::Error(_e) => ("🔴 Hata", Color32::from_rgb(255, 90, 90)),
                            AccountStatus::Idle => ("⚪ Durduruldu", Color32::from_rgb(130, 145, 165)),
                        };

                        egui::Frame::none()
                            .fill(Color32::from_rgba_premultiplied(status_color.r(), status_color.g(), status_color.b(), 30))
                            .rounding(Rounding::same(12.0))
                            .inner_margin(egui::Margin::symmetric(10.0, 3.0))
                            .stroke(Stroke::new(1.0_f32, Color32::from_rgba_premultiplied(status_color.r(), status_color.g(), status_color.b(), 90)))
                            .show(ui, |ui| {
                                ui.label(RichText::new(status_text).font(FontId::proportional(11.5)).color(status_color).strong());
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
