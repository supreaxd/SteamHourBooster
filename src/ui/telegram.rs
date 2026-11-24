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
                ui.label(RichText::new("🤖 Telegram Bot Entegrasyonu").font(FontId::proportional(15.0)).strong());

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let (status_text, status_color) = if settings.telegram_enabled && !settings.telegram_bot_token.trim().is_empty() {
                        ("🟢 Servis Aktif", Color32::from_rgb(90, 215, 120))
                    } else {
                        ("⚪ Devre Dışı", Color32::from_rgb(130, 145, 165))
                    };

                    egui::Frame::none()
                        .fill(Color32::from_rgba_premultiplied(status_color.r(), status_color.g(), status_color.b(), 30))
                        .rounding(Rounding::same(12.0))
                        .stroke(Stroke::new(1.0, Color32::from_rgba_premultiplied(status_color.r(), status_color.g(), status_color.b(), 90)))
                        .inner_margin(egui::Margin::symmetric(10.0, 3.0))
                        .show(ui, |ui| {
                            ui.label(RichText::new(status_text).font(FontId::proportional(11.5)).color(status_color).strong());
                        });
                });
            });

            ui.add_space(4.0);
            ui.label(
                RichText::new("Telegram botu ile düşen koleksiyon kartlarını anında telefonunuza bildirim olarak alabilirsiniz.")
                    .font(FontId::proportional(11.5))
                    .color(Color32::from_rgb(120, 140, 165)),
            );

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(8.0);

            // Enable toggle
            ui.checkbox(&mut settings.telegram_enabled, RichText::new("Telegram Botunu Etkinleştir").font(FontId::proportional(13.0)).strong());

            ui.add_space(10.0);

            // Credentials
            ui.label(RichText::new("Bot Token (BotFather'dan alınan token):").font(FontId::proportional(12.0)));
            ui.add(egui::TextEdit::singleline(&mut settings.telegram_bot_token).password(true).desired_width(f32::INFINITY).hint_text("123456789:ABCdefGhIJKlmNoPQRsTUVwxyZ"));

            ui.add_space(6.0);

            ui.label(RichText::new("Chat ID (Yetkili kullanıcı/grup ID'niz):").font(FontId::proportional(12.0)));
            ui.add(egui::TextEdit::singleline(&mut settings.telegram_chat_id).desired_width(f32::INFINITY).hint_text("Örn: 987654321"));

            ui.add_space(12.0);
            ui.separator();
            ui.add_space(8.0);

            // Notification preferences: strictly card drops as requested
            ui.label(RichText::new("📢 Bildirim Yönetimi").font(FontId::proportional(13.5)).strong());
            ui.add_space(4.0);

            ui.checkbox(
                &mut settings.telegram_notify_cards,
                RichText::new("🃏 Koleksiyon Kartı Dropları Bildirimi").font(FontId::proportional(12.5)),
            );
            ui.label(
                RichText::new("Steam hesabınızda yeni bir kart düştüğünde oyun adı, kart adı ve güncel Steam Pazar fiyatını Telegram'a iletir.")
                    .font(FontId::proportional(11.0))
                    .color(Color32::from_rgb(110, 130, 150)),
            );

            ui.add_space(14.0);

            // Test connection button
            ui.horizontal(|ui| {
                if ui.button(RichText::new("📲 Bağlantıyı Test Et (Test Mesajı Gönder)").font(FontId::proportional(12.5)).color(accent)).clicked() {
                    if settings.telegram_bot_token.trim().is_empty() || settings.telegram_chat_id.trim().is_empty() {
                        self.test_status_text = Some("❌ Lütfen geçerli bir Bot Token ve Chat ID girin.".to_string());
                    } else {
                        crate::telegram::send_telegram_alert("🚀 *SupBooster Bildirimi:*\nTelegram bot entegrasyonu başarıyla bağlandı!");
                        self.test_status_text = Some("✓ Test mesajı Telegram'a gönderildi!".to_string());
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
