use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use eframe::egui::{self, Color32, FontId, RichText, Rounding, ScrollArea, Ui};
use crate::data::{Account, CustomThemeColors, InventoryItem, ThemeMode};
use crate::i18n::Strings;
use crate::inventory::{
    execute_consolidate_trade, fetch_detailed_inventory, get_cached_detailed_inventory,
    ConsolidateTradeResult, TradeOfferStatus,
};
use crate::ui::components::{muted_text, prism_filter_pill, prism_primary_button, prism_progress_bar};
use crate::ui::theme::ThemeEngine;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SubTab {
    Consolidate,
    Balance,
}

pub struct TradeBalanceView {
    sub_tab: SubTab,
    target_account_idx: usize,
    custom_trade_url: String,
    selected_sources: Vec<bool>,
    cards_only_filter: bool,
    selected_trade_assets: HashSet<String>,
    known_trade_assets: HashSet<String>,
    is_sending: Arc<AtomicBool>,
    is_refreshing: Arc<AtomicBool>,
    trade_results: Arc<Mutex<Vec<ConsolidateTradeResult>>>,
    export_status_msg: Option<String>,
}

impl TradeBalanceView {
    pub fn new() -> Self {
        Self {
            sub_tab: SubTab::Consolidate,
            target_account_idx: 0,
            custom_trade_url: String::new(),
            selected_sources: Vec::new(),
            cards_only_filter: false,
            selected_trade_assets: HashSet::new(),
            known_trade_assets: HashSet::new(),
            is_sending: Arc::new(AtomicBool::new(false)),
            is_refreshing: Arc::new(AtomicBool::new(false)),
            trade_results: Arc::new(Mutex::new(Vec::new())),
            export_status_msg: None,
        }
    }

    pub fn draw(
        &mut self,
        ui: &mut Ui,
        accounts: &mut [Arc<Mutex<Account>>],
        settings: &mut crate::data::AppSettings,
        colors: &CustomThemeColors,
        theme_mode: ThemeMode,
        strings: &Strings,
        save_needed: &mut bool,
    ) {
        let accent = Color32::from_rgb(colors.accent[0], colors.accent[1], colors.accent[2]);
        let muted = muted_text(theme_mode);

        if self.selected_sources.len() != accounts.len() {
            self.selected_sources = vec![true; accounts.len()];
        }
        if !accounts.is_empty() && self.target_account_idx >= accounts.len() {
            self.target_account_idx = 0;
        }

        // ─── PRISM HEADER + FILTER PILLS ───
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(RichText::new(strings.tab_trade_balance).font(FontId::proportional(19.0)).strong());
                ui.label(
                    RichText::new(strings.pick(
                        "Consolidate alt items to main account & review per-account earnings",
                        "Alt hesaplardaki kartları ana hesaba topla ve hesap bilançosunu incele",
                        "Сбор карточек на основной аккаунт и баланс заработка",
                    ))
                    .font(FontId::proportional(11.5))
                    .color(muted),
                );
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if prism_filter_pill(
                    ui,
                    strings.pick("Account Balance", "Hesap Bilançosu", "Баланс аккаунтов"),
                    Some(accounts.len()),
                    self.sub_tab == SubTab::Balance,
                    accent,
                    theme_mode,
                )
                .clicked()
                {
                    self.sub_tab = SubTab::Balance;
                }
                if prism_filter_pill(
                    ui,
                    strings.pick("Consolidate", "Topla", "Собрать"),
                    None,
                    self.sub_tab == SubTab::Consolidate,
                    accent,
                    theme_mode,
                )
                .clicked()
                {
                    self.sub_tab = SubTab::Consolidate;
                }
            });
        });

        ui.add_space(10.0);

        if accounts.is_empty() {
            ThemeEngine::card_frame(theme_mode, colors).show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(20.0);
                    ui.label(RichText::new(strings.no_accounts).font(FontId::proportional(14.0)));
                    ui.label(RichText::new(strings.no_accounts_hint).font(FontId::proportional(12.0)).color(muted));
                    ui.add_space(20.0);
                });
            });
            return;
        }

        match self.sub_tab {
            SubTab::Consolidate => self.draw_consolidate(ui, accounts, settings, colors, theme_mode, strings, accent, muted),
            SubTab::Balance => self.draw_balance(ui, accounts, colors, theme_mode, strings, accent, muted, save_needed),
        }
    }

    fn draw_consolidate(
        &mut self,
        ui: &mut Ui,
        accounts: &[Arc<Mutex<Account>>],
        settings: &mut crate::data::AppSettings,
        colors: &CustomThemeColors,
        theme_mode: ThemeMode,
        strings: &Strings,
        accent: Color32,
        muted: Color32,
    ) {
        let (target_user, target_sid) = {
            let a = accounts[self.target_account_idx].lock().unwrap();
            (a.username.clone(), a.steam_id)
        };

        // Keep auto-send target synced with selected main account & trade URL
        if settings.auto_send_drops_to_main {
            settings.auto_send_main_steam_id = target_sid;
            settings.auto_send_main_trade_url = self.custom_trade_url.trim().to_string();
        }

        ThemeEngine::card_frame(theme_mode, colors).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(strings.pick("Target Main Account:", "Hedef Ana Hesap:", "Целевой аккаунт:")).font(FontId::proportional(12.5)).strong());
                egui::ComboBox::from_id_salt("consolidate_target_combo")
                    .width(200.0)
                    .selected_text(format!("{} (ID: {})", target_user, target_sid))
                    .show_ui(ui, |ui| {
                        for (idx, arc) in accounts.iter().enumerate() {
                            let a = arc.lock().unwrap();
                            ui.selectable_value(&mut self.target_account_idx, idx, &a.username);
                        }
                    });

                ui.add_space(10.0);
                ui.label(RichText::new(strings.pick("Trade URL (Optional):", "Takas URL (İsteğe bağlı):", "Trade URL:")).font(FontId::proportional(11.5)).color(muted));
                ui.add(
                    egui::TextEdit::singleline(&mut self.custom_trade_url)
                        .hint_text("https://steamcommunity.com/tradeoffer/new/?partner=...&token=...")
                        .desired_width(280.0),
                );
            });

            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.checkbox(
                    &mut self.cards_only_filter,
                    RichText::new(strings.pick("Trading Cards Only", "Yalnızca Koleksiyon Kartları", "Только карточки")).font(FontId::proportional(11.5)),
                );

                ui.add_space(12.0);
                if ui
                    .checkbox(
                        &mut settings.auto_send_drops_to_main,
                        RichText::new(strings.pick(
                            "Auto-send newly dropped cards to Target Main Account",
                            "Düşen kartları otomatik olarak Ana Hesaba gönder",
                            "Авто-отправка выпавших карт на основной аккаунт",
                        ))
                        .font(FontId::proportional(11.5)),
                    )
                    .changed()
                {
                    settings.auto_send_main_steam_id = target_sid;
                    settings.auto_send_main_trade_url = self.custom_trade_url.trim().to_string();
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let is_ref = self.is_refreshing.load(Ordering::SeqCst);
                    let ref_lbl = if is_ref {
                        strings.pick("⏳ Refreshing...", "⏳ Yenileniyor...", "⏳ Обновление...")
                    } else {
                        strings.pick("↻ Refresh Alt Inventories", "↻ Alt Envanterleri Yenile", "↻ Обновить инвентари")
                    };
                    if ui.button(ref_lbl).clicked() && !is_ref {
                        let ref_flag = self.is_refreshing.clone();
                        ref_flag.store(true, Ordering::SeqCst);
                        let targets: Vec<(u64, Option<String>)> = accounts
                            .iter()
                            .enumerate()
                            .filter(|(i, _)| *i != self.target_account_idx && *self.selected_sources.get(*i).unwrap_or(&false))
                            .filter_map(|(_, arc)| {
                                let a = arc.lock().unwrap();
                                if a.steam_id != 0 { Some((a.steam_id, a.proxy.clone())) } else { None }
                            })
                            .collect();
                        let ctx = ui.ctx().clone();
                        crate::steam::get_runtime().spawn(async move {
                            for (sid, proxy) in targets {
                                let _ = fetch_detailed_inventory(sid, proxy).await;
                            }
                            ref_flag.store(false, Ordering::SeqCst);
                            ctx.request_repaint();
                        });
                    }
                });
            });
        });

        ui.add_space(8.0);

        // Collect tradable items from selected source accounts & auto-select newly seen assets
        let mut all_batches: Vec<(String, Option<String>, Option<String>, Vec<InventoryItem>)> = Vec::new();
        let mut all_visible_asset_ids: Vec<String> = Vec::new();

        for (idx, arc) in accounts.iter().enumerate() {
            if idx == self.target_account_idx {
                continue;
            }
            let a = arc.lock().unwrap();
            let items: Vec<InventoryItem> = if a.steam_id != 0 {
                get_cached_detailed_inventory(a.steam_id)
                    .into_iter()
                    .filter(|it| it.tradable && (!self.cards_only_filter || it.app_id == 753))
                    .collect()
            } else {
                Vec::new()
            };
            if *self.selected_sources.get(idx).unwrap_or(&false) {
                for it in &items {
                    all_visible_asset_ids.push(it.asset_id.clone());
                    if self.known_trade_assets.insert(it.asset_id.clone()) {
                        self.selected_trade_assets.insert(it.asset_id.clone());
                    }
                }
                all_batches.push((a.username.clone(), a.refresh_token.clone(), a.proxy.clone(), items));
            }
        }

        let selected_count = all_visible_asset_ids
            .iter()
            .filter(|id| self.selected_trade_assets.contains(*id))
            .count();

        ThemeEngine::card_frame(theme_mode, colors).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(strings.pick("Source Alt Accounts:", "Kaynak Alt Hesaplar:", "Исходные аккаунты:")).font(FontId::proportional(12.5)).strong());
                for (idx, arc) in accounts.iter().enumerate() {
                    if idx == self.target_account_idx {
                        continue;
                    }
                    let uname = arc.lock().unwrap().username.clone();
                    if let Some(flag) = self.selected_sources.get_mut(idx) {
                        ui.checkbox(flag, uname);
                    }
                }
            });

            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!(
                        "{}: {} / {} {}",
                        strings.pick("Selected Items", "Seçili Öğeler", "Выбрано"),
                        selected_count,
                        all_visible_asset_ids.len(),
                        strings.pick("items", "öğe", "шт.")
                    ))
                    .font(FontId::proportional(12.0))
                    .color(accent),
                );

                ui.add_space(8.0);
                if ui.button(strings.pick("Select All", "Tümünü Seç", "Выбрать все")).clicked() {
                    for id in &all_visible_asset_ids {
                        self.selected_trade_assets.insert(id.clone());
                    }
                }
                if ui.button(strings.pick("Deselect All", "Seçimi Temizle", "Снять выбор")).clicked() {
                    for id in &all_visible_asset_ids {
                        self.selected_trade_assets.remove(id);
                    }
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let is_sending = self.is_sending.load(Ordering::SeqCst);
                    let send_lbl = if is_sending {
                        strings.pick("⏳ Sending Offers...", "⏳ Teklif Gönderiliyor...", "⏳ Отправка...")
                    } else {
                        strings.pick("📦 Create Trade Offer(s)", "📦 Takas Teklifi Oluştur", "📦 Отправить обмен")
                    };
                    if prism_primary_button(ui, send_lbl, accent).clicked() && !is_sending && selected_count > 0 {
                        let send_flag = self.is_sending.clone();
                        send_flag.store(true, Ordering::SeqCst);
                        let results_arc = self.trade_results.clone();
                        let t_user = target_user.clone();
                        let t_sid = target_sid;
                        let t_url = if self.custom_trade_url.trim().is_empty() {
                            None
                        } else {
                            Some(self.custom_trade_url.trim().to_string())
                        };
                        let batches: Vec<_> = all_batches
                            .iter()
                            .map(|(u, tok, prx, items)| {
                                let chosen: Vec<InventoryItem> = items
                                    .iter()
                                    .filter(|it| self.selected_trade_assets.contains(&it.asset_id))
                                    .cloned()
                                    .collect();
                                (u.clone(), tok.clone(), prx.clone(), chosen)
                            })
                            .filter(|b| !b.3.is_empty())
                            .collect();
                        let ctx = ui.ctx().clone();
                        crate::steam::get_runtime().spawn(async move {
                            let mut out = Vec::new();
                            for (src_user, tok, proxy, items) in batches {
                                let res = execute_consolidate_trade(
                                    src_user,
                                    t_user.clone(),
                                    t_sid,
                                    t_url.clone(),
                                    items,
                                    tok,
                                    proxy,
                                )
                                .await;
                                out.push(res);
                            }
                            if let Ok(mut lock) = results_arc.lock() {
                                *lock = out;
                            }
                            send_flag.store(false, Ordering::SeqCst);
                            ctx.request_repaint();
                        });
                    }
                });
            });
        });

        ui.add_space(8.0);

        let results = self.trade_results.lock().unwrap().clone();
        let has_mobile_conf = results
            .iter()
            .any(|r| r.status == TradeOfferStatus::PendingConfirmation || r.status == TradeOfferStatus::Sent);

        // Show Mobile Confirmation info ONLY when trade actually succeeded (not during loading or error)
        if !self.is_sending.load(Ordering::SeqCst) && has_mobile_conf {
            egui::Frame::none()
                .fill(Color32::from_rgba_unmultiplied(20, 48, 34, 195))
                .rounding(Rounding::same(10.0))
                .stroke(eframe::egui::Stroke::new(1.5_f32, Color32::from_rgb(75, 225, 135)))
                .inner_margin(eframe::egui::Margin::same(12.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("📱").font(FontId::proportional(24.0)));
                        ui.add_space(6.0);
                        ui.vertical(|ui| {
                            ui.label(
                                RichText::new(strings.pick(
                                    "Steam Guard Mobile Confirmation Notification Sent!",
                                    "Telefonunuza Steam Mobil Onay Bildirimi Gönderildi!",
                                    "Уведомление подтверждения отправлено в Steam Mobile!",
                                ))
                                .font(FontId::proportional(13.5))
                                .color(Color32::from_rgb(95, 240, 155))
                                .strong(),
                            );
                            ui.label(
                                RichText::new(strings.pick(
                                    "Please open the Steam Mobile App -> Confirmations on your phone to approve the trade offer(s).",
                                    "Takas teklifini onaylamak için telefonunuzdaki Steam Mobil Uygulaması -> Onaylamalar bölümünü açın.",
                                    "Откройте приложение Steam -> Подтверждения на телефоне, чтобы принять обмен.",
                                ))
                                .font(FontId::proportional(11.5))
                                .color(Color32::from_rgb(220, 242, 228)),
                            );
                        });
                    });
                });
            ui.add_space(8.0);
        }

        if !results.is_empty() {
            ThemeEngine::card_frame(theme_mode, colors).show(ui, |ui| {
                ui.label(RichText::new(strings.pick("Trade Offer Status", "Takas Teklifi Durumu", "Статус обменов")).font(FontId::proportional(13.0)).strong());
                ui.add_space(4.0);
                for r in &results {
                    let (badge_col, badge_txt) = match r.status {
                        TradeOfferStatus::Sent => (Color32::from_rgb(80, 225, 130), strings.pick("Sent", "Gönderildi", "Отправлено")),
                        TradeOfferStatus::PendingConfirmation => (Color32::from_rgb(255, 190, 50), strings.pick("Pending Confirmation", "Onay Bekliyor", "Ждёт подтверждения")),
                        TradeOfferStatus::Error => (Color32::from_rgb(255, 105, 105), strings.pick("Error", "Hata", "Ошибка")),
                    };
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("{} -> {} ({})", r.source_username, r.target_username, r.item_count)).strong());
                        ui.label(RichText::new(format!("[{}]", badge_txt)).color(badge_col).strong());
                        ui.label(RichText::new(&r.message).font(FontId::proportional(11.0)).color(muted));
                    });
                }
            });
            ui.add_space(8.0);
        }

        ScrollArea::vertical().id_salt("consolidate_preview_scroll").auto_shrink([false, false]).show(ui, |ui| {
            for (src_user, _, _, items) in &all_batches {
                if items.is_empty() {
                    continue;
                }
                ThemeEngine::card_frame(theme_mode, colors).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("👤 {} ({} {})", src_user, items.len(), strings.pick("items", "öğe", "шт."))).strong());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button(strings.pick("Deselect", "Bırak", "Снять")).clicked() {
                                for it in items {
                                    self.selected_trade_assets.remove(&it.asset_id);
                                }
                            }
                            if ui.small_button(strings.pick("Select All", "Tümünü Seç", "Выбрать все")).clicked() {
                                for it in items {
                                    self.selected_trade_assets.insert(it.asset_id.clone());
                                }
                            }
                        });
                    });
                    ui.add_space(4.0);
                    for it in items {
                        let mut checked = self.selected_trade_assets.contains(&it.asset_id);
                        ui.horizontal(|ui| {
                            if ui.checkbox(&mut checked, "").clicked() {
                                if checked {
                                    self.selected_trade_assets.insert(it.asset_id.clone());
                                } else {
                                    self.selected_trade_assets.remove(&it.asset_id);
                                }
                            }
                            if !it.icon_url.is_empty() {
                                ui.add(egui::Image::new(&it.icon_url).fit_to_exact_size(eframe::egui::Vec2::splat(24.0)).rounding(Rounding::same(4.0)));
                            }
                            ui.label(RichText::new(&it.name).font(FontId::proportional(12.0)));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                let p = it.market_price.unwrap_or(0.05);
                                ui.label(RichText::new(format!("${:.2}", p)).font(FontId::proportional(11.5)).color(Color32::from_rgb(80, 225, 130)));
                                ui.label(RichText::new(&it.game_name).font(FontId::proportional(11.0)).color(muted));
                            });
                        });
                    }
                });
                ui.add_space(4.0);
            }
        });
    }

    fn draw_balance(
        &mut self,
        ui: &mut Ui,
        accounts: &mut [Arc<Mutex<Account>>],
        colors: &CustomThemeColors,
        theme_mode: ThemeMode,
        strings: &Strings,
        accent: Color32,
        muted: Color32,
        save_needed: &mut bool,
    ) {
        let mut total_cards = 0u64;
        let mut total_sold = 0u64;
        let mut total_gross = 0.0_f64;
        let mut total_net = 0.0_f64;
        let mut total_hours = 0.0_f64;
        let mut total_inv_value = 0.0_f64;
        let mut total_inv_cards = 0usize;

        for arc in accounts.iter() {
            if let Ok(a) = arc.lock() {
                total_cards += a.total_cards_earned;
                total_sold += a.total_items_sold;
                total_gross += a.total_gross_earned;
                total_net += a.total_net_earned;
                total_hours += a.hours_boosted;
                if a.steam_id != 0 {
                    let inv = get_cached_detailed_inventory(a.steam_id);
                    for it in &inv {
                        if it.app_id == 753 {
                            total_inv_cards += 1;
                        }
                        if it.marketable {
                            total_inv_value += it.market_price.unwrap_or(0.05);
                        }
                    }
                }
            }
        }

        // Top 4 balance summary cards
        ui.columns(4, |cols| {
            ThemeEngine::card_frame(theme_mode, colors).show(&mut cols[0], |ui| {
                ui.label(RichText::new(strings.pick("Inventory Value", "Envanter Değeri", "Стоимость инвентаря")).font(FontId::proportional(11.0)).color(muted));
                ui.label(RichText::new(format!("${:.2}", total_inv_value)).font(FontId::proportional(20.0)).color(Color32::from_rgb(80, 225, 130)).strong());
                ui.label(RichText::new(format!("{} {}", total_inv_cards, strings.pick("cards in inv", "kart envanterde", "карт в инв."))).font(FontId::proportional(10.5)).color(muted));
            });
            ThemeEngine::card_frame(theme_mode, colors).show(&mut cols[1], |ui| {
                ui.label(RichText::new(strings.pick("Cards Earned", "Kazanılan Kart", "Получено карт")).font(FontId::proportional(11.0)).color(muted));
                ui.label(RichText::new(format!("{}", total_cards.max(total_inv_cards as u64))).font(FontId::proportional(20.0)).color(Color32::from_rgb(255, 190, 60)).strong());
            });
            ThemeEngine::card_frame(theme_mode, colors).show(&mut cols[2], |ui| {
                ui.label(RichText::new(strings.pick("Sold Net / Gross", "Satılan Net / Brüt", "Продано Нетто/Брутто")).font(FontId::proportional(11.0)).color(muted));
                ui.label(RichText::new(format!("${:.2} ({})", total_net, total_sold)).font(FontId::proportional(18.0)).color(accent).strong());
                ui.label(RichText::new(format!("Gross: ${:.2}", total_gross)).font(FontId::proportional(10.5)).color(muted));
            });
            ThemeEngine::card_frame(theme_mode, colors).show(&mut cols[3], |ui| {
                ui.label(RichText::new(strings.pick("Total Hours Boosted", "Toplam Kasılan Saat", "Всего часов")).font(FontId::proportional(11.0)).color(muted));
                ui.label(RichText::new(format!("{:.1}h", total_hours)).font(FontId::proportional(20.0)).color(Color32::from_rgb(160, 145, 255)).strong());
            });
        });

        ui.add_space(10.0);

        ui.horizontal(|ui| {
            ui.label(RichText::new(strings.pick("Per-Account Balance", "Hesap Bazlı Bilanço", "Баланс по аккаунтам")).font(FontId::proportional(14.0)).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let is_ref = self.is_refreshing.load(Ordering::SeqCst);
                let ref_lbl = if is_ref {
                    strings.pick("⏳ Refreshing...", "⏳ Yenileniyor...", "⏳ Обновление...")
                } else {
                    strings.pick("↻ Refresh All Values", "↻ Tüm Değerleri Yenile", "↻ Обновить все")
                };
                if ui.button(ref_lbl).clicked() && !is_ref {
                    let ref_flag = self.is_refreshing.clone();
                    ref_flag.store(true, Ordering::SeqCst);
                    let targets: Vec<(u64, Option<String>)> = accounts
                        .iter()
                        .filter_map(|arc| {
                            let a = arc.lock().unwrap();
                            if a.steam_id != 0 { Some((a.steam_id, a.proxy.clone())) } else { None }
                        })
                        .collect();
                    let ctx = ui.ctx().clone();
                    crate::steam::get_runtime().spawn(async move {
                        for (sid, proxy) in targets {
                            let _ = fetch_detailed_inventory(sid, proxy).await;
                        }
                        ref_flag.store(false, Ordering::SeqCst);
                        ctx.request_repaint();
                    });
                }
                if ui.button(strings.pick("Reset Stats", "İstatistikleri Sıfırla", "Сбросить")).clicked() {
                    for arc in accounts.iter() {
                        if let Ok(mut a) = arc.lock() {
                            a.total_cards_earned = 0;
                            a.total_gross_earned = 0.0;
                            a.total_net_earned = 0.0;
                            a.total_items_sold = 0;
                        }
                    }
                    *save_needed = true;
                }
                if ui.button(strings.pick("Export CSV", "CSV Dışa Aktar", "Экспорт CSV")).clicked() {
                    let mut csv = String::from("username,inv_value_usd,inv_cards,cards_earned,items_sold,gross_usd,net_usd,hours_boosted\n");
                    for arc in accounts.iter() {
                        if let Ok(a) = arc.lock() {
                            let inv = if a.steam_id != 0 { get_cached_detailed_inventory(a.steam_id) } else { Vec::new() };
                            let iv: f64 = inv.iter().filter(|i| i.marketable).map(|i| i.market_price.unwrap_or(0.05)).sum();
                            let ic = inv.iter().filter(|i| i.app_id == 753).count();
                            csv.push_str(&format!(
                                "{},{:.2},{},{},{},{:.2},{:.2},{:.2}\n",
                                a.username, iv, ic, a.total_cards_earned, a.total_items_sold, a.total_gross_earned, a.total_net_earned, a.hours_boosted
                            ));
                        }
                    }
                    ui.ctx().copy_text(csv.clone());
                    let desktop_path = std::env::var("USERPROFILE")
                        .map(|p| std::path::PathBuf::from(p).join("Desktop").join("SupBooster_Balance.csv"))
                        .unwrap_or_else(|_| std::path::PathBuf::from("SupBooster_Balance.csv"));
                    match std::fs::write(&desktop_path, &csv) {
                        Ok(_) => {
                            let msg = format!("✅ Exported CSV -> {} (+ Copied)", desktop_path.display());
                            crate::logger::log_success(&msg);
                            self.export_status_msg = Some(msg);
                        }
                        Err(e) => {
                            let msg = format!("⚠️ Clipboard copied; file write failed: {}", e);
                            crate::logger::log_warn(&msg);
                            self.export_status_msg = Some(msg);
                        }
                    }
                }
            });
        });

        if let Some(ref msg) = self.export_status_msg {
            ui.add_space(4.0);
            ui.label(RichText::new(msg).font(FontId::proportional(11.5)).color(Color32::from_rgb(80, 225, 130)));
        }

        ui.add_space(6.0);

        ScrollArea::vertical().id_salt("balance_accounts_scroll").auto_shrink([false, false]).show(ui, |ui| {
            let max_hours = total_hours.max(1.0);
            for arc in accounts.iter() {
                let a = arc.lock().unwrap().clone();
                let inv = if a.steam_id != 0 { get_cached_detailed_inventory(a.steam_id) } else { Vec::new() };
                let acc_inv_val: f64 = inv.iter().filter(|i| i.marketable).map(|i| i.market_price.unwrap_or(0.05)).sum();
                let acc_inv_cards = inv.iter().filter(|i| i.app_id == 753).count();

                ThemeEngine::card_frame(theme_mode, colors).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(RichText::new(&a.username).font(FontId::proportional(13.5)).strong());
                            ui.label(
                                RichText::new(format!("{} {} · ID: {}", a.games.len(), strings.games_count_unit, a.steam_id))
                                    .font(FontId::proportional(10.5))
                                    .color(muted),
                            );
                        });

                        ui.add_space(16.0);
                        ui.label(RichText::new(format!("Inv: ${:.2} ({} cards)", acc_inv_val, acc_inv_cards)).font(FontId::proportional(12.5)).color(Color32::from_rgb(80, 225, 130)).strong());
                        ui.add_space(12.0);
                        ui.label(RichText::new(format!("Sold: ${:.2} ({})", a.total_net_earned, a.total_items_sold)).font(FontId::proportional(11.5)).color(muted));

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(RichText::new(format!("{:.1}h", a.hours_boosted)).font(FontId::proportional(13.0)).strong());
                            ui.add_space(8.0);
                            prism_progress_bar(ui, (a.hours_boosted / max_hours) as f32, 90.0, 5.0, accent);
                        });
                    });
                });
                ui.add_space(4.0);
            }
        });
    }
}
