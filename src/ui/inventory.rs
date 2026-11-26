use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use eframe::egui::{self, CollapsingHeader, Color32, FontId, RichText, Rounding, ScrollArea, Ui, Vec2};
use crate::data::{Account, CustomThemeColors, InventoryItem, SellStrategy, ThemeMode};
use crate::i18n::Strings;
use crate::inventory::{calculate_seller_receives, fetch_detailed_inventory, get_cached_detailed_inventory};
use crate::ui::components::{muted_text, prism_filter_pill, prism_primary_button};
use crate::ui::theme::ThemeEngine;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InvFilter {
    All,
    Marketable,
    Cards,
    Selected,
}

pub struct InventoryView {
    pub selected_account_index: usize,
    pub search_query: String,
    pub marketable_only: bool,
    pub is_refreshing: Arc<AtomicBool>,
    pub selected_asset_ids: HashSet<String>,
    pub sell_strategy: SellStrategy,
    filter: InvFilter,
}

impl InventoryView {
    pub fn new() -> Self {
        Self {
            selected_account_index: 0,
            search_query: String::new(),
            marketable_only: true,
            is_refreshing: Arc::new(AtomicBool::new(false)),
            selected_asset_ids: HashSet::new(),
            sell_strategy: SellStrategy::Undercut,
            filter: InvFilter::Marketable,
        }
    }

    pub fn draw(
        &mut self,
        ui: &mut Ui,
        accounts: &[Arc<Mutex<Account>>],
        global_search: &str,
        custom_offset: &mut f64,
        colors: &CustomThemeColors,
        theme_mode: ThemeMode,
        strings: &Strings,
        open_sell_modal: &mut bool,
        modal_selected_items: &mut Vec<InventoryItem>,
    ) {
        let accent = Color32::from_rgb(colors.accent[0], colors.accent[1], colors.accent[2]);
        let muted = muted_text(theme_mode);

        if accounts.is_empty() {
            ThemeEngine::card_frame(theme_mode, colors).show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(20.0);
                    ui.label(RichText::new("⚠️ No registered Steam accounts found.").font(FontId::proportional(14.0)).color(Color32::from_rgb(255, 180, 60)));
                    ui.label(RichText::new("Add an account in the Accounts tab first to view its Steam Community inventory.").font(FontId::proportional(12.0)).color(Color32::from_rgb(120, 140, 160)));
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

        let is_ref = self.is_refreshing.load(Ordering::SeqCst);
        let cached_all = get_cached_detailed_inventory(current_steam_id);
        let marketable_cnt = cached_all.iter().filter(|i| i.marketable).count();
        let cards_cnt = cached_all.iter().filter(|i| i.app_id == 753).count();
        let selected_cnt = self.selected_asset_ids.len();

        // ─── PRISM HEADER + FILTER PILLS ───
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(RichText::new(strings.inventory_title).font(FontId::proportional(19.0)).strong());
                ui.label(
                    RichText::new(format!(
                        "{} {} · {} {}",
                        cached_all.len(),
                        strings.pick("items", "öğe", "предм."),
                        marketable_cnt,
                        strings.pick("marketable", "satılabilir", "продаваемых")
                    ))
                    .font(FontId::proportional(11.5))
                    .color(muted),
                );
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if prism_filter_pill(ui, strings.pick("Selected", "Seçili", "Выбрано"), Some(selected_cnt), self.filter == InvFilter::Selected, accent, theme_mode).clicked() {
                    self.filter = InvFilter::Selected;
                }
                if prism_filter_pill(ui, strings.pick("Cards", "Kartlar", "Карты"), Some(cards_cnt), self.filter == InvFilter::Cards, accent, theme_mode).clicked() {
                    self.filter = InvFilter::Cards;
                }
                if prism_filter_pill(ui, strings.pick("Marketable", "Satılabilir", "Продаваемые"), Some(marketable_cnt), self.filter == InvFilter::Marketable, accent, theme_mode).clicked() {
                    self.filter = InvFilter::Marketable;
                    self.marketable_only = true;
                }
                if prism_filter_pill(ui, strings.pick("All", "Tümü", "Все"), Some(cached_all.len()), self.filter == InvFilter::All, accent, theme_mode).clicked() {
                    self.filter = InvFilter::All;
                    self.marketable_only = false;
                }
            });
        });

        ui.add_space(8.0);

        // ─── TOP CONTROL BAR ───
        ThemeEngine::card_frame(theme_mode, colors).show(ui, |ui| {
            ui.horizontal(|ui| {
                // Account selector
                ui.label(RichText::new(format!("👤 {}:", strings.col_account)).font(FontId::proportional(12.5)).strong());
                egui::ComboBox::from_id_salt("inventory_acc_combo")
                    .width(170.0)
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

                ui.checkbox(&mut self.marketable_only, RichText::new(strings.filter_marketable_only).font(FontId::proportional(11.5)));

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let refresh_label = if is_ref {
                        strings.pick("⏳ Fetching...", "⏳ Yükleniyor...", "⏳ Загрузка...")
                    } else {
                        strings.btn_refresh_inventory
                    };
                    if ui.button(RichText::new(refresh_label).font(FontId::proportional(12.0))).clicked() && !is_ref {
                        let ref_arc = self.is_refreshing.clone();
                        ref_arc.store(true, Ordering::SeqCst);
                        let sid = current_steam_id;
                        let proxy = current_proxy.clone();
                        let ctx_clone = ui.ctx().clone();
                        crate::steam::get_runtime().spawn(async move {
                            let _ = fetch_detailed_inventory(sid, proxy).await;
                            ref_arc.store(false, Ordering::SeqCst);
                            ctx_clone.request_repaint();
                        });
                    }
                });
            });
        });

        ui.add_space(6.0);

        // ─── ITEMS LIST GROUPED BY GAME ───
        let cached_items = get_cached_detailed_inventory(current_steam_id);

        let effective_q = if !global_search.trim().is_empty() {
            global_search.trim().to_lowercase()
        } else {
            self.search_query.trim().to_lowercase()
        };

        let filtered_items: Vec<&InventoryItem> = cached_items
            .iter()
            .filter(|item| {
                if self.marketable_only && !item.marketable {
                    return false;
                }
                match self.filter {
                    InvFilter::Marketable if !item.marketable => return false,
                    InvFilter::Cards if item.app_id != 753 => return false,
                    InvFilter::Selected if !self.selected_asset_ids.contains(&item.asset_id) => return false,
                    _ => {}
                }
                if effective_q.is_empty() {
                    true
                } else {
                    item.name.to_lowercase().contains(&effective_q) || item.game_name.to_lowercase().contains(&effective_q)
                }
            })
            .collect();

        if filtered_items.is_empty() {
            ThemeEngine::card_frame(theme_mode, colors).show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(24.0);
                    if is_ref {
                        ui.label(RichText::new("⏳ Fetching inventory items from Steam Community...").font(FontId::proportional(14.0)).color(Color32::from_rgb(140, 180, 240)));
                        ui.spinner();
                    } else if cached_items.is_empty() {
                        ui.label(RichText::new("🎒 No inventory data loaded for this account yet.").font(FontId::proportional(14.0)).color(Color32::from_rgb(140, 160, 180)));
                        ui.label(RichText::new("Click '🔄 Refresh Inventory' above to fetch items directly from Steam.").font(FontId::proportional(12.0)).color(Color32::from_rgb(110, 130, 150)));
                    } else {
                        ui.label(RichText::new("No items match the current search or filters.").font(FontId::proportional(13.0)).color(Color32::from_rgb(130, 150, 175)));
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
                        RichText::new(format!("🎮 {}  ({} items • ~${:.2})", game_name, count, est_game_value))
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

                                // Price info on the right (computed dynamically with active SellStrategy)
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if item.marketable {
                                        let base = item.market_price.unwrap_or(0.05);
                                        let gross = self.sell_strategy.compute_price(base, *custom_offset);
                                        let net = calculate_seller_receives(gross);

                                        ui.vertical(|ui| {
                                            ui.label(
                                                RichText::new(format!("{}: ${:.2}", strings.pick("Price", "Fiyat", "Цена"), gross))
                                                    .font(FontId::proportional(12.0))
                                                    .color(Color32::from_rgb(90, 215, 120))
                                                    .strong(),
                                            );
                                            ui.label(
                                                RichText::new(format!("{}: ${:.2}", strings.pick("Net", "Net", "Нетто"), net))
                                                    .font(FontId::proportional(10.5))
                                                    .color(muted),
                                            );
                                        });
                                    } else {
                                        ui.label(RichText::new(strings.pick("Not Marketable", "Satılamaz", "Нельзя продать")).font(FontId::proportional(11.0)).color(muted));
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
            let total_gross: f64 = selected_items
                .iter()
                .map(|i| self.sell_strategy.compute_price(i.market_price.unwrap_or(0.05), *custom_offset))
                .sum();
            let total_net: f64 = selected_items
                .iter()
                .map(|i| calculate_seller_receives(self.sell_strategy.compute_price(i.market_price.unwrap_or(0.05), *custom_offset)))
                .sum();

            ui.add_space(4.0);

            ThemeEngine::floating_bar_frame(theme_mode, colors).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("{} {}", total_count, strings.pick("selected", "seçili", "выбрано")))
                            .font(FontId::proportional(13.5))
                            .strong(),
                    );
                    ui.label(
                        RichText::new(format!("(${:.2} · Net: ${:.2})", total_gross, total_net))
                            .font(FontId::proportional(12.0))
                            .color(Color32::from_rgb(90, 215, 120)),
                    );

                    ui.add_space(8.0);

                    // Strategy selector (All 4 modes)
                    egui::ComboBox::from_id_salt("bulk_sell_strategy_combo")
                        .width(210.0)
                        .selected_text(match self.sell_strategy {
                            SellStrategy::Undercut => strings.strategy_undercut,
                            SellStrategy::QuickSell => strings.strategy_quick_sell,
                            SellStrategy::MarketPrice => strings.strategy_market_price,
                            SellStrategy::CustomOffset => strings.strategy_custom_offset,
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.sell_strategy, SellStrategy::Undercut, strings.strategy_undercut);
                            ui.selectable_value(&mut self.sell_strategy, SellStrategy::QuickSell, strings.strategy_quick_sell);
                            ui.selectable_value(&mut self.sell_strategy, SellStrategy::MarketPrice, strings.strategy_market_price);
                            ui.selectable_value(&mut self.sell_strategy, SellStrategy::CustomOffset, strings.strategy_custom_offset);
                        });

                    if self.sell_strategy == SellStrategy::CustomOffset {
                        ui.add(
                            egui::DragValue::new(custom_offset)
                                .speed(0.01)
                                .range(-10.0..=50.0)
                                .prefix("$"),
                        );
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if prism_primary_button(ui, strings.btn_sell_selected, accent).clicked() {
                            *modal_selected_items = selected_items;
                            *open_sell_modal = true;
                        }

                        if ui.button(RichText::new(strings.pick("Clear", "Temizle", "Сброс")).font(FontId::proportional(11.5))).clicked() {
                            self.selected_asset_ids.clear();
                        }
                    });
                });
            });
        }
    }
}
