use eframe::egui::{self, Color32, FontId, RichText, Rounding, Stroke, Ui};
use crate::data::AppSettings;
use crate::i18n::Strings;
use crate::ui::theme::ThemeEngine;

pub struct TelegramView {
    pub test_message_sent: bool,
    pub test_status_text: Option<String>,
}

impl TelegramView {
    pub fn new() -> Self {
        Self {
            test_message_sent: false,
            test_status_text: None,
        }
    }

    pub fn draw(
        &mut self,
        ui: &mut Ui,
        settings: &mut AppSettings,
        _strings: &Strings,
    ) {
        let colors = settings.custom_colors.clone();
        let theme_mode = settings.theme_mode;
        let accent = Color32::from_rgb(colors.accent[0], colors.accent[1], colors.accent[2]);

        ThemeEngine::card_frame(theme_mode, &colors).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("🤖 Telegram Bot Integration").font(FontId::proportional(15.0)).strong());

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let (status_text, border_color, text_color) = if settings.telegram_enabled && !settings.telegram_bot_token.trim().is_empty() {
                        ("🟢 Service Active", Color32::from_rgb(46, 204, 113), Color32::from_rgb(80, 240, 140))
                    } else {
                        ("⚪ Disabled", Color32::from_rgb(130, 145, 165), Color32::from_rgb(170, 185, 205))
                    };

                    egui::Frame::none()
                        .fill(Color32::from_rgba_unmultiplied(16, 24, 20, 220))
                        .rounding(Rounding::same(12.0))
                        .stroke(Stroke::new(1.0, border_color))
                        .inner_margin(egui::Margin::symmetric(10.0, 3.5))
                        .show(ui, |ui| {
                            ui.label(RichText::new(status_text).font(FontId::proportional(11.5)).color(text_color).strong());
                        });
                });
            });

            ui.add_space(4.0);
            ui.label(
                RichText::new("Receive instant notifications on your mobile device whenever Steam trading cards drop.")
                    .font(FontId::proportional(11.5))
                    .color(Color32::from_rgb(120, 140, 165)),
            );

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(8.0);

            // Enable toggle
            ui.checkbox(&mut settings.telegram_enabled, RichText::new("Enable Telegram Bot Notifications").font(FontId::proportional(13.0)).strong());

            ui.add_space(10.0);

            // Credentials
            ui.label(RichText::new("Bot Token (obtained from @BotFather):").font(FontId::proportional(12.0)));
            ui.add(egui::TextEdit::singleline(&mut settings.telegram_bot_token).password(true).desired_width(f32::INFINITY).hint_text("123456789:ABCdefGhIJKlmNoPQRsTUVwxyZ"));

            ui.add_space(6.0);

            ui.label(RichText::new("Chat ID (Authorized user or group ID):").font(FontId::proportional(12.0)));
            ui.add(egui::TextEdit::singleline(&mut settings.telegram_chat_id).desired_width(f32::INFINITY).hint_text("e.g. 987654321"));

            ui.add_space(12.0);
            ui.separator();
            ui.add_space(8.0);

            // Notification preferences: strictly card drops as requested
            ui.label(RichText::new("📢 Notification Management").font(FontId::proportional(13.5)).strong());
            ui.add_space(4.0);

            ui.checkbox(
                &mut settings.telegram_notify_cards,
                RichText::new("🃏 Trading Card Drop Notifications").font(FontId::proportional(12.5)),
            );
            ui.label(
                RichText::new("When a new trading card drops on your Steam account, sends an alert with the game name, card name, and market price.")
                    .font(FontId::proportional(11.0))
                    .color(Color32::from_rgb(110, 130, 150)),
            );

            ui.add_space(14.0);

            // Test connection button
            ui.horizontal(|ui| {
                if ui.button(RichText::new("📲 Send Test Message").font(FontId::proportional(12.5)).color(accent)).clicked() {
                    if settings.telegram_bot_token.trim().is_empty() || settings.telegram_chat_id.trim().is_empty() {
                        self.test_status_text = Some("❌ Please enter a valid Bot Token and Chat ID.".to_string());
                    } else {
                        crate::telegram::send_telegram_alert("🚀 *SupBooster Notification:*\nTelegram bot integration connected successfully!");
                        self.test_status_text = Some("✓ Test message sent to Telegram!".to_string());
                    }
                }

                if let Some(ref status_msg) = self.test_status_text {
                    let color = if status_msg.starts_with('✓') {
                        Color32::from_rgb(90, 215, 120)
                    } else {
                        Color32::from_rgb(255, 100, 100)
                    };
                    ui.label(RichText::new(status_msg).font(FontId::proportional(11.5)).color(color));
                }
            });
        });
    }
}
