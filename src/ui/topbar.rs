use eframe::egui::{self, Color32, FontId, RichText, Rounding, Stroke, Ui, Vec2};
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
}

pub struct TopBar;

impl TopBar {
    pub fn draw(
        ui: &mut Ui,
        colors: &CustomThemeColors,
        _theme_mode: ThemeMode,
        strings: &Strings,
    ) -> TopBarAction {
        let mut action = TopBarAction::None;
        let accent = Color32::from_rgb(colors.accent[0], colors.accent[1], colors.accent[2]);

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
        });

        action
    }
}
