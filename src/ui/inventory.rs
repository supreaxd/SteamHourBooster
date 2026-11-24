use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use eframe::egui::{self, CollapsingHeader, Color32, FontId, RichText, Rounding, ScrollArea, Ui, Vec2};
use crate::data::{Account, CustomThemeColors, InventoryItem, SellStrategy, ThemeMode};
use crate::i18n::Strings;
use crate::inventory::{fetch_detailed_inventory, get_cached_detailed_inventory};
use crate::ui::theme::ThemeEngine;

pub struct InventoryView {
    pub selected_account_index: usize,
    pub search_query: String,
    pub marketable_only: bool,
    pub is_refreshing: bool,
    pub selected_asset_ids: HashSet<String>,
    pub sell_strategy: SellStrategy,
}

impl InventoryView {
    pub fn new() -> Self {
        Self {
            selected_account_index: 0,
            search_query: String::new(),
            marketable_only: true,
            is_refreshing: false,
            selected_asset_ids: HashSet::new(),
            sell_strategy: SellStrategy::QuickSell,
        }
    }

    pub fn draw(
        &mut self,
        ui: &mut Ui,
        accounts: &[Arc<Mutex<Account>>],
        colors: &CustomThemeColors,
        theme_mode: ThemeMode,
        strings: &Strings,
        open_sell_modal: &mut bool,
        modal_selected_items: &mut Vec<InventoryItem>,
    ) {
        let _accent = Color32::from_rgb(colors.accent[0], colors.accent[1], colors.accent[2]);

        if accounts.is_empty() {
            ThemeEngine::card_frame(theme_mode, colors).show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(20.0);
                    ui.label(RichText::new("⚠️ Henüz kayıtlı bir Steam hesabı bulunmuyor.").font(FontId::proportional(14.0)).color(Color32::from_rgb(255, 180, 60)));
                    ui.label(RichText::new("Envanter öğelerini görüntülemek için önce Hesaplar sekmesinden bir hesap ekleyin.").font(FontId::proportional(12.0)).color(Color32::from_rgb(120, 140, 160)));
                    ui.add_space(20.0);
                });
            });
            return;
        }

        if self.selected_account_index >= accounts.len() {
            self.selected_account_index = 0;
        }

        let (current_username, current_steam_id, current_proxy) = {
            let a = accounts[self.selected_account_index].lock().unwrap();
            (a.username.clone(), a.steam_id, a.proxy.clone())
        };

        // ─── TOP CONTROL BAR ───
        ThemeEngine::card_frame(theme_mode, colors).show(ui, |ui| {
            ui.horizontal(|ui| {
                // Account selector
                ui.label(RichText::new("👤 Hesap:").font(FontId::proportional(12.5)).strong());
                egui::ComboBox::from_id_salt("inventory_acc_combo")
                    .width(180.0)
                    .selected_text(format!("{} (ID: {})", current_username, current_steam_id))
                    .show_ui(ui, |ui| {
                        for (idx, arc) in accounts.iter().enumerate() {
                            let a = arc.lock().unwrap();
                            if ui.selectable_value(&mut self.selected_account_index, idx, &a.username).clicked() {
                                self.selected_asset_ids.clear();
                            }
                        }
                    });

                ui.add_space(8.0);

                // Search box
                ui.label(RichText::new("🔍").font(FontId::proportional(13.0)));
                ui.add(egui::TextEdit::singleline(&mut self.search_query).hint_text("Eşya veya oyun ara...").desired_width(160.0));

                ui.add_space(8.0);
                ui.checkbox(&mut self.marketable_only, RichText::new(strings.filter_marketable_only).font(FontId::proportional(11.5)));

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let refresh_label = if self.is_refreshing { "⏳ Yükleniyor..." } else { strings.btn_refresh_inventory };
                    if ui.button(RichText::new(refresh_label).font(FontId::proportional(12.0))).clicked() && !self.is_refreshing {
                        self.is_refreshing = true;
                        let sid = current_steam_id;
                        let proxy = current_proxy.clone();
                        crate::steam::get_runtime().spawn(async move {
                            let _ = fetch_detailed_inventory(sid, proxy).await;
                        });
                        self.is_refreshing = false;
                    }
                });
            });
        });

        ui.add_space(6.0);

        // ─── ITEMS LIST GROUPED BY GAME ───
        let cached_items = get_cached_detailed_inventory(current_steam_id);

        let filtered_items: Vec<&InventoryItem> = cached_items
            .iter()
            .filter(|item| {
                if self.marketable_only && !item.marketable {
                    return false;
                }
                if self.search_query.trim().is_empty() {
                    true
                } else {
                    let q = self.search_query.to_lowercase();
                    item.name.to_lowercase().contains(&q) || item.game_name.to_lowercase().contains(&q)
                }
            })
            .collect();

        if filtered_items.is_empty() {
            ThemeEngine::card_frame(theme_mode, colors).show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(24.0);
                    if cached_items.is_empty() {
                        ui.label(RichText::new("🎒 Bu hesap için henüz envanter verisi yüklenmedi.").font(FontId::proportional(14.0)).color(Color32::from_rgb(140, 160, 180)));
                        ui.label(RichText::new("Steam Topluluk envanterini çekmek için '🔄 Envanteri Yenile' butonuna tıklayın.").font(FontId::proportional(12.0)).color(Color32::from_rgb(110, 130, 150)));
                    } else {
                        ui.label(RichText::new("Filtrelere uygun eşya bulunamadı.").font(FontId::proportional(13.0)).color(Color32::from_rgb(130, 150, 175)));
                    }
                    ui.add_space(24.0);
                });
            });
            return;
        }

        // Group by Game Name
        let mut groups: HashMap<String, Vec<&InventoryItem>> = HashMap::new();
        for item in &filtered_items {
            groups.entry(item.game_name.clone()).or_default().push(item);
        }

        let mut group_keys: Vec<String> = groups.keys().cloned().collect();
        group_keys.sort();

        let available_height = if !self.selected_asset_ids.is_empty() {
            ui.available_height() - 70.0 // Leave room for floating bar
        } else {
            ui.available_height()
        };

        ScrollArea::vertical().max_height(available_height).auto_shrink([false, false]).show(ui, |ui| {
            for game_name in &group_keys {
                let items_in_game = &groups[game_name];
                let count = items_in_game.len();
                let est_game_value: f64 = items_in_game.iter().map(|i| i.market_price.unwrap_or(0.0)).sum();

                let all_in_game_selected = items_in_game.iter().all(|i| self.selected_asset_ids.contains(&i.asset_id));

                ThemeEngine::card_frame(theme_mode, colors).show(ui, |ui| {
                    let header_id = ui.make_persistent_id(format!("game_group_{}", game_name));
                    CollapsingHeader::new(
                        RichText::new(format!("🎮 {}  ({} öge • ~${:.2})", game_name, count, est_game_value))
                            .font(FontId::proportional(13.5))
                            .strong(),
                    )
                    .id_salt(header_id)
                    .default_open(true)
                    .show(ui, |ui| {
                        // "Select all in this game" checkbox
                        ui.horizontal(|ui| {
                            let mut select_all_toggle = all_in_game_selected;
                            if ui.checkbox(&mut select_all_toggle, RichText::new(strings.btn_select_all_game).font(FontId::proportional(11.5))).clicked() {
                                if select_all_toggle {
                                    for item in items_in_game {
                                        if item.marketable {
                                            self.selected_asset_ids.insert(item.asset_id.clone());
                                        }
                                    }
                                } else {
                                    for item in items_in_game {
                                        self.selected_asset_ids.remove(&item.asset_id);
                                    }
                                }
                            }
                        });

                        ui.separator();

                        // Item rows
                        for item in items_in_game {
                            let mut is_selected = self.selected_asset_ids.contains(&item.asset_id);

                            ui.horizontal(|ui| {
                                // Selection checkbox
                                if item.marketable {
                                    if ui.checkbox(&mut is_selected, "").clicked() {
                                        if is_selected {
                                            self.selected_asset_ids.insert(item.asset_id.clone());
                                        } else {
                                            self.selected_asset_ids.remove(&item.asset_id);
                                        }
                                    }
                                } else {
                                    ui.add_space(20.0);
                                }

                                // Item Icon
                                if !item.icon_url.is_empty() {
                                    ui.add(
                                        egui::Image::new(&item.icon_url)
                                            .fit_to_exact_size(Vec2::new(36.0, 36.0))
                                            .rounding(Rounding::same(4.0)),
                                    );
                                } else {
                                    // Elegant placeholder
                                    let (rect, _) = ui.allocate_exact_size(Vec2::new(36.0, 36.0), egui::Sense::hover());
                                    ui.painter().rect_filled(rect, Rounding::same(4.0), Color32::from_rgb(35, 45, 60));
                                    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, "🃏", FontId::proportional(16.0), Color32::WHITE);
                                }

                                ui.add_space(4.0);

                                // Title and Type
                                ui.vertical(|ui| {
                                    ui.label(RichText::new(&item.name).font(FontId::proportional(12.5)).strong());
                                    ui.label(
                                        RichText::new(format!("{} • x{}", item.item_type, item.amount))
                                            .font(FontId::proportional(10.5))
                                            .color(Color32::from_rgb(130, 150, 175)),
                                    );
                                });

                                // Price info on the right
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if item.marketable {
                                        let gross = item.market_price.unwrap_or(0.05);
                                        let net = item.user_receives.unwrap_or(0.04);

                                        ui.vertical(|ui| {
                                            ui.label(
                                                RichText::new(format!("Satış: ${:.2}", gross))
                                                    .font(FontId::proportional(12.0))
                                                    .color(Color32::from_rgb(90, 215, 120))
                                                    .strong(),
                                            );
                                            ui.label(
                                                RichText::new(format!("Alacağınız: ${:.2}", net))
                                                    .font(FontId::proportional(10.5))
                                                    .color(Color32::from_rgb(140, 165, 190)),
                                            );
                                        });
                                    } else {
                                        ui.label(RichText::new("Pazarlanamaz").font(FontId::proportional(11.0)).color(Color32::from_rgb(140, 150, 160)));
                                    }
                                });
                            });

                            ui.add_space(2.0);
                        }
                    });
                });
                ui.add_space(3.0);
            }
        });

        // ─── STICKY / FLOATING BOTTOM ACTION BAR ───
        if !self.selected_asset_ids.is_empty() {
            let selected_items: Vec<InventoryItem> = cached_items
                .into_iter()
                .filter(|i| self.selected_asset_ids.contains(&i.asset_id))
                .collect();

            let total_count = selected_items.len();
            let total_gross: f64 = selected_items.iter().map(|i| i.market_price.unwrap_or(0.05)).sum();
            let total_net: f64 = selected_items.iter().map(|i| i.user_receives.unwrap_or(0.04)).sum();

            ui.add_space(4.0);

            ThemeEngine::floating_bar_frame(theme_mode, colors).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("Seçili: {} Öğe", total_count))
                            .font(FontId::proportional(14.0))
                            .strong(),
                    );
                    ui.label(
                        RichText::new(format!("(Brüt: ${:.2}  •  Net: ${:.2})", total_gross, total_net))
                            .font(FontId::proportional(12.5))
                            .color(Color32::from_rgb(90, 215, 120)),
                    );

                    ui.add_space(10.0);

                    // Strategy selector
                    ui.label(RichText::new("Strateji:").font(FontId::proportional(11.5)));
                    egui::ComboBox::from_id_salt("bulk_sell_strategy_combo")
                        .width(170.0)
                        .selected_text(match self.sell_strategy {
                            SellStrategy::QuickSell => strings.strategy_quick_sell,
                            SellStrategy::Undercut => strings.strategy_undercut,
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.sell_strategy, SellStrategy::QuickSell, strings.strategy_quick_sell);
                            ui.selectable_value(&mut self.sell_strategy, SellStrategy::Undercut, strings.strategy_undercut);
                        });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Highlighted "Seçilenleri Sat" button
                        if ui.button(RichText::new(strings.btn_sell_selected).font(FontId::proportional(13.5)).color(Color32::WHITE).strong()).clicked() {
                            *modal_selected_items = selected_items;
                            *open_sell_modal = true;
                        }

                        if ui.button(RichText::new("Seçimi Temizle").font(FontId::proportional(11.5))).clicked() {
                            self.selected_asset_ids.clear();
                        }
                    });
                });
            });
        }
    }
}
