use eframe::egui::{self, Color32, FontId, RichText, Rounding, Sense, Stroke, Ui, Vec2};
use crate::data::{CustomThemeColors, ThemeMode};
use crate::i18n::Strings;
use crate::ui::components::prism_primary_button;

pub enum TopBarAction {
    None,
    QuickAdd,
    StartAll,
    StopAll,
    RestartAll,
    QrLogin,
    Minimize,
    ToggleMaximize,
    CloseWindow,
}

pub struct TopBar;

impl TopBar {
    pub fn draw(
        ui: &mut Ui,
        colors: &CustomThemeColors,
        theme_mode: ThemeMode,
        is_maximized: bool,
        strings: &Strings,
    ) -> TopBarAction {
        let mut action = TopBarAction::None;
        let accent = Color32::from_rgb(colors.accent[0], colors.accent[1], colors.accent[2]);
        let title_col = if theme_mode == ThemeMode::Light {
            Color32::from_rgb(20, 26, 36)
        } else {
            Color32::from_rgb(220, 230, 242)
        };

        ui.horizontal_centered(|ui| {
            ui.add_space(4.0);

            if prism_primary_button(
                ui,
                strings.pick("+ Add Game / Account", "+ Oyun / Hesap Ekle", "+ Игра / Аккаунт"),
                accent,
            )
            .clicked()
            {
                action = TopBarAction::QuickAdd;
            }

            ui.add_space(8.0);

            let icon_btn = |ui: &mut Ui, icon: &str, tip: &str, tint: Option<Color32>| -> bool {
                let txt = match tint {
                    Some(c) => RichText::new(icon).font(FontId::proportional(14.0)).color(c),
                    None => RichText::new(icon).font(FontId::proportional(14.0)),
                };
                ui.add(
                    egui::Button::new(txt)
                        .fill(Color32::TRANSPARENT)
                        .stroke(Stroke::NONE)
                        .rounding(Rounding::same(6.0))
                        .min_size(Vec2::splat(28.0)),
                )
                .on_hover_text(tip)
                .clicked()
            };

            if icon_btn(ui, "▶", strings.btn_start_all, Some(Color32::from_rgb(90, 220, 130))) {
                action = TopBarAction::StartAll;
            }
            if icon_btn(ui, "⏹", strings.btn_stop_all, Some(Color32::from_rgb(255, 115, 115))) {
                action = TopBarAction::StopAll;
            }
            if icon_btn(ui, "↻", strings.btn_restart_all, None) {
                action = TopBarAction::RestartAll;
            }
            if icon_btn(ui, "QR", strings.btn_qr_add, Some(accent)) {
                action = TopBarAction::QrLogin;
            }

            // Right side: Seamless Window Controls (Close, Maximize, Minimize) matching UI glass
            let right_reserve = 104.0;
            let drag_w = (ui.available_width() - right_reserve).max(40.0);
            let (drag_rect, drag_resp) = ui.allocate_exact_size(Vec2::new(drag_w, 38.0), Sense::click_and_drag());
            if drag_resp.drag_started() {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }
            if drag_resp.double_clicked() {
                action = TopBarAction::ToggleMaximize;
            }
            if drag_w > 140.0 {
                ui.painter().text(
                    drag_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    format!("SupBooster v{}", env!("CARGO_PKG_VERSION")),
                    FontId::proportional(11.5),
                    Color32::from_rgba_unmultiplied(title_col.r(), title_col.g(), title_col.b(), 140),
                );
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(4.0);
                if ui
                    .add(
                        egui::Button::new(RichText::new("✕").font(FontId::proportional(13.0)).color(title_col))
                            .fill(Color32::TRANSPARENT)
                            .stroke(Stroke::NONE)
                            .rounding(Rounding::same(6.0))
                            .min_size(Vec2::new(30.0, 26.0)),
                    )
                    .clicked()
                {
                    action = TopBarAction::CloseWindow;
                }

                let max_glyph = if is_maximized { "❐" } else { "□" };
                if ui
                    .add(
                        egui::Button::new(RichText::new(max_glyph).font(FontId::proportional(13.0)).color(title_col))
                            .fill(Color32::TRANSPARENT)
                            .stroke(Stroke::NONE)
                            .rounding(Rounding::same(6.0))
                            .min_size(Vec2::new(30.0, 26.0)),
                    )
                    .clicked()
                {
                    action = TopBarAction::ToggleMaximize;
                }

                if ui
                    .add(
                        egui::Button::new(RichText::new("—").font(FontId::proportional(12.0)).color(title_col))
                            .fill(Color32::TRANSPARENT)
                            .stroke(Stroke::NONE)
                            .rounding(Rounding::same(6.0))
                            .min_size(Vec2::new(30.0, 26.0)),
                    )
                    .clicked()
                {
                    action = TopBarAction::Minimize;
                }
            });
        });

        action
    }
}
