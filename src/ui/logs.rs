use eframe::egui::{self, Color32, FontId, RichText, ScrollArea, Ui};
use crate::data::{CustomThemeColors, ThemeMode};
use crate::i18n::Strings;
use crate::logger::{clear_logs, get_logs, LogLevel};
use crate::ui::theme::ThemeEngine;

pub struct LogsView;

impl LogsView {
    pub fn draw(
        ui: &mut Ui,
        colors: &CustomThemeColors,
        theme_mode: ThemeMode,
        strings: &Strings,
    ) {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("📋 {}", strings.logs_title)).font(FontId::proportional(16.0)).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(RichText::new(strings.btn_clear_logs).font(FontId::proportional(12.0))).clicked() {
                    clear_logs();
                }
                if ui.button(RichText::new(strings.btn_copy_logs).font(FontId::proportional(12.0))).clicked() {
                    let logs = get_logs();
                    let all_text = logs
                        .iter()
                        .map(|e| format!("[{}] {}", e.timestamp, e.message))
                        .collect::<Vec<_>>()
                        .join("\n");
                    ui.ctx().copy_text(all_text);
                }
            });
        });
        ui.add_space(6.0);

        ThemeEngine::card_frame(theme_mode, colors).show(ui, |ui| {
            let available_h = (ui.available_height() - 12.0).max(200.0);
            ScrollArea::vertical()
                .id_salt("live_logs_tab_scroll")
                .max_height(available_h)
                .auto_shrink([false, false])
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    let logs = get_logs();
                    if logs.is_empty() {
                        ui.label(
                            RichText::new(strings.pick(
                                "No system activity yet.",
                                "Henüz sistem kaydı yok.",
                                "Пока нет системных записей.",
                            ))
                            .font(FontId::proportional(12.0))
                            .color(Color32::from_rgb(110, 130, 150)),
                        );
                    } else {
                        for entry in logs.iter() {
                            ui.horizontal_wrapped(|ui| {
                                let (color, prefix) = match entry.level {
                                    LogLevel::Success => (Color32::from_rgb(90, 215, 120), "✔"),
                                    LogLevel::Error => (Color32::from_rgb(255, 100, 100), "✖"),
                                    LogLevel::Warn => (Color32::from_rgb(255, 185, 60), "⚠"),
                                    LogLevel::Info => (Color32::from_rgb(140, 170, 205), "ℹ"),
                                };

                                ui.label(
                                    RichText::new(&entry.timestamp)
                                        .font(FontId::monospace(11.0))
                                        .color(Color32::from_rgb(90, 110, 130)),
                                );
                                ui.label(
                                    RichText::new(prefix)
                                        .font(FontId::monospace(11.5))
                                        .color(color),
                                );
                                ui.label(
                                    RichText::new(&entry.message)
                                        .font(FontId::proportional(12.0))
                                        .color(color),
                                );
                            });
                        }
                    }
                });
        });
    }
}
