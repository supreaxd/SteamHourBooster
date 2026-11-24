use eframe::egui::{self, Color32, FontId, RichText, ScrollArea, Ui};
use crate::autostart;
use crate::data::{AppSettings, CustomThemeColors, ThemeMode};
use crate::i18n::{Language, Strings};
use crate::ui::theme::ThemeEngine;

pub struct SettingsView {
    pub autostart_reg: bool,
    pub autostart_minimized: bool,
    pub autostart_task_service: bool,
}

impl SettingsView {
    pub fn new() -> Self {
        Self {
            autostart_reg: autostart::is_registry_autostart_enabled(),
            autostart_minimized: false,
            autostart_task_service: autostart::is_task_scheduler_enabled(),
        }
    }

    pub fn draw(
        &mut self,
        ui: &mut Ui,
        settings: &mut AppSettings,
        strings: &Strings,
    ) {
        let colors = settings.custom_colors.clone();
        let theme_mode = settings.theme_mode;
        let accent = Color32::from_rgb(colors.accent[0], colors.accent[1], colors.accent[2]);

        ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            // ─── 1. APPEARANCE & THEME (Gelişmiş Tema ve Liquid Glass) ───
            ThemeEngine::card_frame(theme_mode, &colors).show(ui, |ui| {
                ui.label(RichText::new(strings.sec_appearance_theme).font(FontId::proportional(14.5)).strong());
                ui.add_space(6.0);

                // Theme Mode Selector
                ui.label(RichText::new("Tema Modu:").font(FontId::proportional(12.0)));
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut settings.theme_mode, ThemeMode::Dark, strings.theme_dark);
                    ui.selectable_value(&mut settings.theme_mode, ThemeMode::Light, strings.theme_light);
                    ui.selectable_value(&mut settings.theme_mode, ThemeMode::System, strings.theme_system);
                    ui.selectable_value(&mut settings.theme_mode, ThemeMode::LiquidGlass, strings.theme_liquid_glass);
                });

                if settings.theme_mode == ThemeMode::LiquidGlass {
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new("✨ Liquid Glass modu masaüstü arka planınızı şeffaf blurlu (Acrylic / Mica) estetikle gösterir.")
                            .font(FontId::proportional(11.0))
                            .color(Color32::from_rgb(100, 190, 240)),
                    );
                }

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(6.0);

                // Color Customization (Color Pickers)
                ui.label(RichText::new("🎨 Renk Özelleştirme:").font(FontId::proportional(13.0)).strong());
                ui.add_space(4.0);

                ui.columns(3, |cols| {
                    // Window Surface / Tint Color
                    cols[0].vertical(|ui| {
                        ui.label(RichText::new(strings.label_surface_color).font(FontId::proportional(11.5)));
                        let mut srgba = Color32::from_rgb(settings.custom_colors.surface[0], settings.custom_colors.surface[1], settings.custom_colors.surface[2]);
                        if ui.color_edit_button_srgba(&mut srgba).changed() {
                            settings.custom_colors.surface = [srgba.r(), srgba.g(), srgba.b()];
                        }
                    });

                    // Text Color
                    cols[1].vertical(|ui| {
                        ui.label(RichText::new(strings.label_text_color).font(FontId::proportional(11.5)));
                        let mut srgba = Color32::from_rgb(settings.custom_colors.text[0], settings.custom_colors.text[1], settings.custom_colors.text[2]);
                        if ui.color_edit_button_srgba(&mut srgba).changed() {
                            settings.custom_colors.text = [srgba.r(), srgba.g(), srgba.b()];
                        }
                    });

                    // Accent Color
                    cols[2].vertical(|ui| {
                        ui.label(RichText::new(strings.label_accent_color).font(FontId::proportional(11.5)));
                        let mut srgba = Color32::from_rgb(settings.custom_colors.accent[0], settings.custom_colors.accent[1], settings.custom_colors.accent[2]);
                        if ui.color_edit_button_srgba(&mut srgba).changed() {
                            settings.custom_colors.accent = [srgba.r(), srgba.g(), srgba.b()];
                        }
                    });
                });

                ui.add_space(6.0);
                if ui.button(RichText::new(strings.btn_reset_theme_colors).font(FontId::proportional(11.5))).clicked() {
                    settings.custom_colors = CustomThemeColors::default();
                }

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(6.0);

                // Language selection
                ui.label(RichText::new("🌐 Dil Seçimi (Language):").font(FontId::proportional(12.5)).strong());
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut settings.language, Language::Turkish, "🇹🇷 Türkçe");
                    ui.selectable_value(&mut settings.language, Language::English, "🇬🇧 English");
                    ui.selectable_value(&mut settings.language, Language::Russian, "🇷🇺 Русский");
                });

                ui.add_space(6.0);

                // UI Zoom / Scale slider
                ui.label(RichText::new("Arayüz Ölçeği & Zoom:").font(FontId::proportional(12.0)));
                ui.horizontal(|ui| {
                    ui.add(egui::Slider::new(&mut settings.ui_scale, 0.8..=1.5).step_by(0.05).text("x"));
                    if ui.button("Sıfırla (1.0x)").clicked() {
                        settings.ui_scale = 1.0;
                    }
                });
            });

            ui.add_space(8.0);

            // ─── 2. STARTUP & SYSTEM TRAY ───
            ThemeEngine::card_frame(theme_mode, &colors).show(ui, |ui| {
                ui.label(RichText::new("🚀 Başlangıç & Sistem Tepsisi (Tray)").font(FontId::proportional(14.5)).strong());
                ui.add_space(6.0);

                // Windows registry autostart
                let mut reg_auto = self.autostart_reg;
                if ui.checkbox(&mut reg_auto, "Windows açıldığında otomatik başlat (Registry)").changed() {
                    self.autostart_reg = reg_auto;
                    let _ = autostart::set_registry_autostart(reg_auto, self.autostart_minimized);
                }

                // Start minimized
                ui.checkbox(&mut settings.start_minimized, "Başlangıçta doğrudan sistem tepsisine küçültülmüş başla");

                // Minimize to tray on close
                ui.checkbox(&mut settings.minimize_to_tray, "Pencere kapatıldığında (X) uygulamadan çıkma, tepside çalışmaya devam et");

                // Auto-start boost on launch
                ui.checkbox(&mut settings.auto_start_boost_on_launch, "Uygulama açılır açılmaz tüm hesapların saat kasmasını otomatik başlat");

                ui.add_space(8.0);

                // Windows Task Scheduler Service
                ui.horizontal(|ui| {
                    if self.autostart_task_service {
                        if ui.button(RichText::new("🗑️ Görev Zamanlayıcı Hizmetini Kaldır").color(Color32::from_rgb(255, 120, 120))).clicked() {
                            let _ = autostart::unregister_task_scheduler();
                            self.autostart_task_service = autostart::is_task_scheduler_enabled();
                        }
                        ui.label(RichText::new("✅ Hizmet Kurulu (Kullanıcı girişi olmadan çalışır)").color(Color32::from_rgb(90, 215, 120)));
                    } else {
                        if ui.button(RichText::new("⚡ Arka Plan Hizmeti Olarak Kur (Görev Zamanlayıcı)").color(accent)).clicked() {
                            let _ = autostart::register_task_scheduler(self.autostart_minimized);
                            self.autostart_task_service = autostart::is_task_scheduler_enabled();
                        }
                        ui.label(RichText::new("⚪ Hizmet Kurulu Değil").color(Color32::from_rgb(130, 145, 165)));
                    }
                });
            });

            ui.add_space(8.0);

            // ─── 3. SECURITY & REAL-GAME DETECTION ───
            ThemeEngine::card_frame(theme_mode, &colors).show(ui, |ui| {
                ui.label(RichText::new("🛡️ Güvenlik & Akıllı Duraklatma").font(FontId::proportional(14.5)).strong());
                ui.add_space(6.0);

                ui.checkbox(&mut settings.encrypt_credentials, "Şifreleri ve 2FA Token'larını Windows DPAPI ile şifrele");
                ui.add_space(4.0);

                ui.checkbox(&mut settings.auto_pause_game, "Bilgisayarda gerçek bir Steam oyunu açıldığında saat kasmayı otomatik duraklat");
                ui.add_space(4.0);

                ui.checkbox(&mut settings.card_drop_tracking, "Kart düşme olaylarını canlı takip et");
            });

            ui.add_space(8.0);

            // ─── 4. AFK AUTO-REPLY & SCHEDULER ───
            ThemeEngine::card_frame(theme_mode, &colors).show(ui, |ui| {
                ui.label(RichText::new("💬 AFK Otomatik Yanıtlayıcı & Zamanlayıcı").font(FontId::proportional(14.5)).strong());
                ui.add_space(6.0);

                ui.checkbox(&mut settings.afk_auto_reply, "Steam arkadaşlarından gelen mesajlara otomatik AFK yanıtı gönder");
                if settings.afk_auto_reply {
                    ui.add_space(4.0);
                    ui.label(RichText::new("AFK Mesajı:").font(FontId::proportional(11.5)));
                    ui.add(egui::TextEdit::singleline(&mut settings.afk_reply_message).desired_width(f32::INFINITY));

                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Aynı arkadaş için bekleme süresi:").font(FontId::proportional(11.5)));
                        ui.add(egui::Slider::new(&mut settings.afk_cooldown_mins, 5..=120).suffix(" dk"));
                    });
                }

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(6.0);

                ui.checkbox(&mut settings.enable_scheduler, "Zamanlayıcıyı Etkinleştir (Yalnızca belirli saatler arasında saat kas)");
                if settings.enable_scheduler {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Başlangıç Saati:").font(FontId::proportional(11.5)));
                        ui.add(egui::Slider::new(&mut settings.scheduler_start_hour, 0..=23));

                        ui.add_space(10.0);

                        ui.label(RichText::new("Bitiş Saati:").font(FontId::proportional(11.5)));
                        ui.add(egui::Slider::new(&mut settings.scheduler_end_hour, 0..=23));
                    });
                }
            });

            ui.add_space(12.0);
        });
    }
}
