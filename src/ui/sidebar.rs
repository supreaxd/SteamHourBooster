use std::sync::{Arc, Mutex};
use eframe::egui::{self, Color32, FontId, Pos2, Response, RichText, Rounding, Sense, Stroke, Ui, Vec2};
use crate::data::{Account, AccountStatus, CustomThemeColors, ThemeMode};
use crate::i18n::Strings;
use crate::inventory::get_cached_active_drops;
use crate::ui::components::{muted_text, prism_brand_badge, prism_progress_bar};
use crate::updater::{UpdateManager, UpdateState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavTab {
    Dashboard,
    ActiveDrops,
    Inventory,
    TradeBalance,
    Accounts,
    Logs,
    Telegram,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarAction {
    None,
    OpenAllAccounts,
    SelectAccount(u64),
    AddAccount,
    ThemeToggled,
}

pub struct Sidebar;

impl Sidebar {
    pub fn draw(
        ui: &mut Ui,
        current_tab: &mut NavTab,
        accounts_expanded: &mut bool,
        selected_account_id: Option<u64>,
        accounts: &[Arc<Mutex<Account>>],
        colors: &CustomThemeColors,
        theme_mode: &mut ThemeMode,
        strings: &Strings,
        updater: &UpdateManager,
        show_changelogs: &mut bool,
    ) -> SidebarAction {
        let mut action = SidebarAction::None;
        let accent = Color32::from_rgb(colors.accent[0], colors.accent[1], colors.accent[2]);
        let text_color = Color32::from_rgb(colors.text[0], colors.text[1], colors.text[2]);
        let muted = muted_text(*theme_mode);

        let mut active_accounts = 0usize;
        let mut total_drops_left = 0u32;
        for arc in accounts {
            if let Ok(a) = arc.lock() {
                if matches!(a.status, AccountStatus::Boosting) {
                    active_accounts += 1;
                }
                if a.steam_id != 0 {
                    for d in get_cached_active_drops(a.steam_id) {
                        total_drops_left += d.remaining_cards;
                    }
                }
            }
        }

        let inner_w = ui.available_width();

        ui.vertical(|ui| {
            ui.set_width(inner_w);
            ui.add_space(6.0);

            // ─── BRAND HEADER (SupBooster + 12px rounded icon) ───
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                prism_brand_badge(ui, accent);
                ui.add_space(8.0);
                ui.vertical(|ui| {
                    ui.label(RichText::new("SupBooster").font(FontId::proportional(15.5)).strong());
                    ui.label(
                        RichText::new(format!("v{}", env!("CARGO_PKG_VERSION")))
                            .font(FontId::proportional(10.0))
                            .color(muted),
                    );
                });
            });

            ui.add_space(10.0);

            // ─── MAIN VIEWS ───
            let main_tabs = [
                (NavTab::Dashboard, strings.tab_dashboard, "■"),
                (NavTab::ActiveDrops, strings.tab_active_drops, "♦"),
                (NavTab::Inventory, strings.tab_inventory, "▣"),
                (NavTab::TradeBalance, strings.tab_trade_balance, "⚖"),
            ];

            for (tab, label, icon) in main_tabs {
                let is_active = *current_tab == tab;
                if draw_prism_nav_item(ui, icon, None, label, None, is_active, *theme_mode, accent, text_color).clicked() {
                    *current_tab = tab;
                }
                ui.add_space(2.0);
            }

            ui.add_space(8.0);

            // ─── COLLAPSIBLE ACCOUNTS SECTION ───
            // Triangle button toggles collapse; "ACCOUNTS (N)" text opens all accounts grid on the right
            ui.horizontal(|ui| {
                ui.add_space(6.0);
                let arrow = if *accounts_expanded { "▾" } else { "▸" };
                if ui
                    .add(
                        egui::Button::new(RichText::new(arrow).font(FontId::proportional(12.0)).color(muted))
                            .fill(Color32::TRANSPARENT)
                            .stroke(Stroke::NONE)
                            .min_size(Vec2::splat(18.0)),
                    )
                    .clicked()
                {
                    *accounts_expanded = !*accounts_expanded;
                }

                let is_all_acc_active = *current_tab == NavTab::Accounts && selected_account_id.is_none();
                let header_col = if is_all_acc_active { Color32::WHITE } else { muted };
                if ui
                    .add(
                        egui::Button::new(
                            RichText::new(format!(
                                "{} ({})",
                                strings.pick("ACCOUNTS", "HESAPLAR", "АККАУНТЫ"),
                                accounts.len()
                            ))
                            .font(FontId::proportional(10.5))
                            .color(header_col)
                            .strong(),
                        )
                        .fill(Color32::TRANSPARENT)
                        .stroke(Stroke::NONE),
                    )
                    .clicked()
                {
                    action = SidebarAction::OpenAllAccounts;
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(6.0);
                    if ui
                        .small_button(RichText::new("+").font(FontId::proportional(13.0)).color(muted))
                        .on_hover_text(strings.btn_manual_add)
                        .clicked()
                    {
                        action = SidebarAction::AddAccount;
                    }
                });
            });

            if *accounts_expanded {
                ui.add_space(2.0);
                if accounts.is_empty() {
                    ui.horizontal(|ui| {
                        ui.add_space(16.0);
                        ui.label(
                            RichText::new(strings.pick("No accounts", "Hesap yok", "Нет аккаунтов"))
                                .font(FontId::proportional(11.0))
                                .color(muted),
                        );
                    });
                } else {
                    for arc in accounts {
                        if let Ok(a) = arc.lock() {
                            let dot_color = match &a.status {
                                AccountStatus::Boosting => Color32::from_rgb(80, 225, 130),
                                AccountStatus::PausedForGame { .. } => Color32::from_rgb(255, 145, 35),
                                AccountStatus::Connecting | AccountStatus::Reconnecting { .. } => Color32::from_rgb(255, 200, 60),
                                AccountStatus::AwaitingSteamGuard { .. } => Color32::from_rgb(255, 115, 50),
                                AccountStatus::Error(_) => Color32::from_rgb(255, 85, 85),
                                AccountStatus::Idle => Color32::from_rgb(120, 130, 150),
                            };
                            let status_tip = match &a.status {
                                AccountStatus::Boosting => format!("● {}", strings.status_boosting),
                                AccountStatus::PausedForGame { app_name, .. } => format!("🎮 {} ({})", strings.status_paused_game, app_name),
                                AccountStatus::Reconnecting { attempt, retry_in_secs } => format!("⏳ {} (#{} · {}s)", strings.status_reconnecting, attempt, retry_in_secs),
                                AccountStatus::Connecting => format!("⏳ {}", strings.status_connecting),
                                AccountStatus::AwaitingSteamGuard { .. } => format!("🛡 {}", strings.status_guard_needed),
                                AccountStatus::Error(e) => format!("⚠ {}", e),
                                AccountStatus::Idle => format!("● {}", strings.status_idle),
                            };
                            let is_acc_active = *current_tab == NavTab::Accounts && selected_account_id == Some(a.id);
                            if draw_prism_nav_item(
                                ui,
                                "",
                                Some(dot_color),
                                &a.username,
                                Some(a.games.len()),
                                is_acc_active,
                                *theme_mode,
                                accent,
                                text_color,
                            )
                            .on_hover_text(status_tip)
                            .clicked()
                            {
                                action = SidebarAction::SelectAccount(a.id);
                            }
                            ui.add_space(1.0);
                        }
                    }
                }
            }

            ui.add_space(8.0);

            // ─── TOOLS & QUEUES (No duplicate Accounts!) ───
            ui.horizontal(|ui| {
                ui.add_space(10.0);
                ui.label(
                    RichText::new(strings.pick("TOOLS & QUEUES", "KUYRUKLAR & ARAÇLAR", "ИНСТРУМЕНТЫ"))
                        .font(FontId::proportional(10.5))
                        .color(muted)
                        .strong(),
                );
            });
            ui.add_space(2.0);

            let tool_tabs = [
                (NavTab::Logs, strings.tab_logs, "☰"),
                (NavTab::Telegram, strings.tab_telegram, "✈"),
            ];
            for (tab, label, icon) in tool_tabs {
                let is_active = *current_tab == tab;
                if draw_prism_nav_item(ui, icon, None, label, None, is_active, *theme_mode, accent, text_color).clicked() {
                    *current_tab = tab;
                }
                ui.add_space(2.0);
            }

            // ─── BOTTOM SECTION (Changelogs at very bottom + Active Boost card above it) ───
            ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                // Bottom-most row: Changelogs (+ update badge) on left, Theme Cycle + Settings on right
                ui.add_space(4.0);
                let update_state = updater.state.lock().unwrap().clone();
                ui.allocate_ui_with_layout(
                    Vec2::new((inner_w - 8.0).max(160.0), 28.0),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        ui.add_space(8.0);
                        if let UpdateState::UpdateAvailable { version, .. } = &update_state {
                            ui.label(
                                RichText::new(format!("● v{}", version))
                                    .font(FontId::proportional(10.5))
                                    .color(Color32::from_rgb(255, 175, 45))
                                    .strong(),
                            );
                        }
                        if ui
                            .link(RichText::new(strings.changelogs_text).font(FontId::proportional(11.5)).color(muted))
                            .clicked()
                        {
                            *show_changelogs = true;
                        }

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.add_space(4.0);
                            let is_settings = *current_tab == NavTab::Settings;
                            let settings_col = if is_settings {
                                accent
                            } else if *theme_mode == ThemeMode::Light {
                                Color32::from_rgb(20, 26, 34)
                            } else {
                                text_color
                            };
                            let settings_btn = egui::Button::new(
                                RichText::new("⚙")
                                    .font(FontId::proportional(14.0))
                                    .color(settings_col),
                            )
                            .min_size(Vec2::splat(26.0));
                            if ui.add(settings_btn).on_hover_text(strings.tab_settings).clicked() {
                                *current_tab = NavTab::Settings;
                            }

                            let theme_icon = match *theme_mode {
                                ThemeMode::Dark => "🌙",
                                ThemeMode::Light => "☀",
                                ThemeMode::Blur => "🌫",
                                ThemeMode::LiquidGlass => "💎",
                                ThemeMode::System => "💻",
                            };
                            if ui
                                .add(egui::Button::new(RichText::new(theme_icon).font(FontId::proportional(13.0))).min_size(Vec2::splat(26.0)))
                                .on_hover_text(strings.pick("Cycle Theme", "Temayı Değiştir", "Сменить тему"))
                                .clicked()
                            {
                                *theme_mode = match *theme_mode {
                                    ThemeMode::Dark => ThemeMode::Blur,
                                    ThemeMode::Blur => ThemeMode::LiquidGlass,
                                    ThemeMode::LiquidGlass => ThemeMode::Light,
                                    ThemeMode::Light => ThemeMode::Dark,
                                    ThemeMode::System => ThemeMode::Dark,
                                };
                                action = SidebarAction::ThemeToggled;
                            }
                        });
                    },
                );

                ui.add_space(26.0);

                // Live Boosting / Card Drops Summary Box (narrower, shifted 10px up & 5px left, high contrast)
                let card_stroke = if *theme_mode == ThemeMode::LiquidGlass {
                    Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 38))
                } else if *theme_mode == ThemeMode::Light {
                    Stroke::new(1.0_f32, Color32::from_rgb(200, 210, 224))
                } else {
                    Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 36))
                };
                let card_fill = if *theme_mode == ThemeMode::Light {
                    Color32::from_rgb(232, 238, 246)
                } else if matches!(*theme_mode, ThemeMode::LiquidGlass | ThemeMode::Blur) {
                    let a = (colors.glass_alpha as f32 * 0.42 + colors.blur_level.clamp(0.0, 10.0) * 1.5).clamp(14.0, 195.0) as u8;
                    Color32::from_rgba_unmultiplied(colors.surface[0], colors.surface[1], colors.surface[2], a)
                } else {
                    Color32::from_rgba_unmultiplied(18, 22, 34, 90)
                };
                let title_col = if *theme_mode == ThemeMode::Light {
                    Color32::from_rgb(30, 38, 50)
                } else {
                    Color32::from_rgb(210, 224, 242)
                };
                let val_col = if *theme_mode == ThemeMode::Light {
                    Color32::from_rgb(12, 16, 22)
                } else {
                    Color32::WHITE
                };

                ui.horizontal(|ui| {
                    ui.add_space(5.0);
                    let box_resp = egui::Frame::none()
                        .fill(card_fill)
                        .stroke(card_stroke)
                        .rounding(Rounding::same(12.0))
                        .inner_margin(egui::Margin::same(8.0))
                        .show(ui, |ui| {
                            ui.set_width((inner_w - 58.0).max(105.0));
                            ui.label(
                                RichText::new(strings.pick("ACTIVE BOOST / DROPS", "AKTİF KASMA / KART", "АКТИВНЫЙ БУСТ"))
                                    .font(FontId::proportional(9.5))
                                    .color(title_col)
                                    .strong(),
                            );
                            ui.add_space(3.0);
                            ui.label(
                                RichText::new(format!(
                                    "{} {} · {} {}",
                                    active_accounts,
                                    strings.pick("acc", "hesap", "акк"),
                                    total_drops_left,
                                    strings.pick("drops", "kart", "дроп.")
                                ))
                                .font(FontId::proportional(12.5))
                                .color(val_col)
                                .strong(),
                            );
                            ui.add_space(5.0);
                            let frac = if accounts.is_empty() {
                                0.0
                            } else {
                                active_accounts as f32 / accounts.len() as f32
                            };
                            prism_progress_bar(ui, frac, ui.available_width(), 4.0, accent);
                        });
                    if *theme_mode == ThemeMode::LiquidGlass {
                        crate::ui::theme::ThemeEngine::paint_liquid_refraction(ui.painter(), box_resp.response.rect, 12.0);
                    }
                });
            });
        });

        action
    }
}

fn draw_prism_nav_item(
    ui: &mut Ui,
    icon: &str,
    dot_color: Option<Color32>,
    label: &str,
    badge: Option<usize>,
    is_active: bool,
    theme_mode: ThemeMode,
    accent: Color32,
    text_color: Color32,
) -> Response {
    let desired_size = Vec2::new((ui.available_width() - 10.0).max(120.0), 30.0);
    let (rect, response) = ui.allocate_exact_size(desired_size, Sense::click());
    let rect = rect.translate(Vec2::new(5.0, 0.0));

    if ui.is_rect_visible(rect) {
        let is_hovered = response.hovered();
        let is_light = theme_mode == ThemeMode::Light;

        if is_active {
            ui.painter().rect_filled(
                rect,
                Rounding::same(8.0),
                Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), if is_light { 34 } else { 42 }),
            );
            ui.painter().rect_stroke(
                rect,
                Rounding::same(8.0),
                Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 140)),
            );
        } else if is_hovered {
            let hover_fill = if is_light {
                Color32::from_rgb(218, 224, 234)
            } else {
                Color32::from_rgba_unmultiplied(255, 255, 255, 20)
            };
            ui.painter().rect_filled(rect, Rounding::same(8.0), hover_fill);
        }

        let display_color = if is_light {
            Color32::from_rgb(16, 22, 28)
        } else if is_active || is_hovered {
            Color32::WHITE
        } else {
            text_color
        };

        let mut text_x = rect.min.x + 10.0;
        if let Some(dc) = dot_color {
            ui.painter().circle_filled(Pos2::new(text_x + 4.0, rect.center().y), 4.0, dc);
            text_x += 16.0;
        } else if !icon.is_empty() {
            ui.painter().text(
                Pos2::new(text_x, rect.center().y),
                egui::Align2::LEFT_CENTER,
                icon,
                FontId::proportional(12.5),
                display_color,
            );
            text_x += 20.0;
        }

        let clean_label = label.trim_start_matches(|c: char| !c.is_alphanumeric()).trim();
        let display = if clean_label.is_empty() { label } else { clean_label };

        ui.painter().text(
            Pos2::new(text_x, rect.center().y),
            egui::Align2::LEFT_CENTER,
            display,
            FontId::proportional(12.5),
            display_color,
        );

        if let Some(cnt) = badge {
            ui.painter().text(
                Pos2::new(rect.max.x - 10.0, rect.center().y),
                egui::Align2::RIGHT_CENTER,
                format!("{}", cnt),
                FontId::proportional(11.0),
                Color32::from_rgb(140, 150, 170),
            );
        }
    }

    response
}
