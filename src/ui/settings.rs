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
        _strings: &Strings,
    ) {
        let colors = settings.custom_colors.clone();
        let theme_mode = settings.theme_mode;
        let accent = Color32::from_rgb(colors.accent[0], colors.accent[1], colors.accent[2]);

        ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            // ─── 1. APPEARANCE & THEME ───
            ThemeEngine::card_frame(theme_mode, &colors).show(ui, |ui| {
                ui.label(RichText::new("🎨 Appearance & Themes").font(FontId::proportional(14.5)).strong());
                ui.add_space(6.0);

                // Theme Mode Selector
                ui.label(RichText::new("Theme Mode:").font(FontId::proportional(12.0)));
                ui.horizontal(|ui| {
                    if ui.selectable_label(settings.theme_mode == ThemeMode::Dark, "🌙 Dark").clicked() {
                        settings.theme_mode = ThemeMode::Dark;
                        let lum = (settings.custom_colors.text[0] as u32 * 299 + settings.custom_colors.text[1] as u32 * 587 + settings.custom_colors.text[2] as u32 * 114) / 1000;
                        if lum < 70 {
                            settings.custom_colors.text = [198, 212, 223];
                            settings.custom_colors.surface = [22, 26, 36];
                        }
                    }
                    if ui.selectable_label(settings.theme_mode == ThemeMode::Light, "☀️ Light").clicked() {
                        settings.theme_mode = ThemeMode::Light;
                        let lum = (settings.custom_colors.text[0] as u32 * 299 + settings.custom_colors.text[1] as u32 * 587 + settings.custom_colors.text[2] as u32 * 114) / 1000;
                        if lum > 140 {
                            settings.custom_colors.text = [25, 30, 36];
                            settings.custom_colors.surface = [245, 247, 250];
                        }
                    }
                    if ui.selectable_label(settings.theme_mode == ThemeMode::System, "💻 System").clicked() {
                        settings.theme_mode = ThemeMode::System;
                    }
                    if ui.selectable_label(settings.theme_mode == ThemeMode::Transparent, "✨ Transparent (Blur)").clicked() {
                        settings.theme_mode = ThemeMode::Transparent;
                        let lum = (settings.custom_colors.text[0] as u32 * 299 + settings.custom_colors.text[1] as u32 * 587 + settings.custom_colors.text[2] as u32 * 114) / 1000;
                        if lum < 70 {
                            settings.custom_colors.text = [198, 212, 223];
                            settings.custom_colors.surface = [22, 26, 36];
                        }
                    }
                });

                if settings.theme_mode == ThemeMode::Transparent {
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new("✨ Transparent mode showcases your desktop background with an elegant Acrylic / Mica blur aesthetic.")
                            .font(FontId::proportional(11.0))
                            .color(Color32::from_rgb(100, 190, 240)),
                    );
                }

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(6.0);

                // Color Customization (Color Pickers)
                ui.label(RichText::new("🎨 Color Customization:").font(FontId::proportional(13.0)).strong());
                ui.add_space(4.0);

                ui.columns(3, |cols| {
                    // Window Surface / Tint Color
                    cols[0].vertical(|ui| {
                        ui.label(RichText::new("Surface / Window Tint:").font(FontId::proportional(11.5)));
                        let mut srgba = Color32::from_rgb(settings.custom_colors.surface[0], settings.custom_colors.surface[1], settings.custom_colors.surface[2]);
                        if ui.color_edit_button_srgba(&mut srgba).changed() {
                            settings.custom_colors.surface = [srgba.r(), srgba.g(), srgba.b()];
                        }
                    });

                    // Text Color
                    cols[1].vertical(|ui| {
                        ui.label(RichText::new("Text Color:").font(FontId::proportional(11.5)));
                        let mut srgba = Color32::from_rgb(settings.custom_colors.text[0], settings.custom_colors.text[1], settings.custom_colors.text[2]);
                        if ui.color_edit_button_srgba(&mut srgba).changed() {
                            settings.custom_colors.text = [srgba.r(), srgba.g(), srgba.b()];
                        }
                    });

                    // Accent Color
                    cols[2].vertical(|ui| {
                        ui.label(RichText::new("Accent Color:").font(FontId::proportional(11.5)));
                        let mut srgba = Color32::from_rgb(settings.custom_colors.accent[0], settings.custom_colors.accent[1], settings.custom_colors.accent[2]);
                        if ui.color_edit_button_srgba(&mut srgba).changed() {
                            settings.custom_colors.accent = [srgba.r(), srgba.g(), srgba.b()];
                        }
                    });
                });

                ui.add_space(6.0);
                if ui.button(RichText::new("Reset Colors to Default").font(FontId::proportional(11.5))).clicked() {
                    settings.custom_colors = if settings.theme_mode == ThemeMode::Light {
                        CustomThemeColors::light_default()
                    } else {
                        CustomThemeColors::dark_default()
                    };
                }

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(6.0);

                // Language selection
                ui.label(RichText::new("🌐 Interface Language:").font(FontId::proportional(12.5)).strong());
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut settings.language, Language::English, "🇬🇧 English");
                    ui.selectable_value(&mut settings.language, Language::Turkish, "🇹🇷 Türkçe");
                    ui.selectable_value(&mut settings.language, Language::Russian, "🇷🇺 Русский");
                });

                ui.add_space(6.0);

                // UI Zoom / Scale slider
                ui.label(RichText::new("Interface Scale & Zoom:").font(FontId::proportional(12.0)));
                ui.horizontal(|ui| {
                    ui.add(egui::Slider::new(&mut settings.ui_scale, 0.8..=1.5).step_by(0.05).text("x"));
                    if ui.button("Reset (1.0x)").clicked() {
                        settings.ui_scale = 1.0;
                    }
                });
            });

            ui.add_space(8.0);

            // ─── 2. STARTUP & SYSTEM TRAY ───
            ThemeEngine::card_frame(theme_mode, &colors).show(ui, |ui| {
                ui.label(RichText::new("🚀 Startup & System Tray").font(FontId::proportional(14.5)).strong());
                ui.add_space(6.0);

                // Windows registry autostart
                let mut reg_auto = self.autostart_reg;
                if ui.checkbox(&mut reg_auto, "Start automatically with Windows (Registry)").changed() {
                    self.autostart_reg = reg_auto;
                    let _ = autostart::set_registry_autostart(reg_auto, self.autostart_minimized);
                }

                // Start minimized
                ui.checkbox(&mut settings.start_minimized, "Start minimized to system tray on launch");

                // Minimize to tray on close
                ui.checkbox(&mut settings.minimize_to_tray, "Close button (X) minimizes to tray instead of exiting");

                // Auto-start boost on launch
                ui.checkbox(&mut settings.auto_start_boost_on_launch, "Automatically start boosting all accounts on application launch");

                ui.add_space(8.0);

                // Windows Task Scheduler Service
                ui.horizontal(|ui| {
                    if self.autostart_task_service {
                        if ui.button(RichText::new("🗑️ Remove Task Scheduler Service").color(Color32::from_rgb(255, 120, 120))).clicked() {
                            let _ = autostart::unregister_task_scheduler();
                            self.autostart_task_service = autostart::is_task_scheduler_enabled();
                        }
                        ui.label(RichText::new("✅ Service Installed (Runs in background even without login)").color(Color32::from_rgb(90, 215, 120)));
                    } else {
                        if ui.button(RichText::new("⚡ Install as Background Service (Task Scheduler)").color(accent)).clicked() {
                            let _ = autostart::register_task_scheduler(self.autostart_minimized);
                            self.autostart_task_service = autostart::is_task_scheduler_enabled();
                        }
                        ui.label(RichText::new("⚪ Service Not Installed").color(Color32::from_rgb(130, 145, 165)));
                    }
                });
            });

            ui.add_space(8.0);

            // ─── 3. SECURITY & REAL-GAME DETECTION ───
            ThemeEngine::card_frame(theme_mode, &colors).show(ui, |ui| {
                ui.label(RichText::new("🛡️ Security & Smart Detection").font(FontId::proportional(14.5)).strong());
                ui.add_space(6.0);

                ui.checkbox(&mut settings.encrypt_credentials, "Encrypt passwords and 2FA tokens with Windows DPAPI");
                ui.add_space(4.0);

                ui.checkbox(&mut settings.auto_pause_game, "Automatically pause boosting when a real Steam game is running");
                ui.add_space(4.0);

                ui.checkbox(&mut settings.card_drop_tracking, "Track Steam trading card drops in real-time");
            });

            ui.add_space(8.0);

            // ─── 4. AFK AUTO-REPLY & SCHEDULER ───
            ThemeEngine::card_frame(theme_mode, &colors).show(ui, |ui| {
                ui.label(RichText::new("💬 AFK Auto-Responder & Scheduler").font(FontId::proportional(14.5)).strong());
                ui.add_space(6.0);

                ui.checkbox(&mut settings.afk_auto_reply, "Automatically reply to incoming Steam chat messages when AFK");
                if settings.afk_auto_reply {
                    ui.add_space(4.0);
                    ui.label(RichText::new("AFK Message:").font(FontId::proportional(11.5)));
                    ui.add(egui::TextEdit::singleline(&mut settings.afk_reply_message).desired_width(f32::INFINITY));

                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Cooldown per friend:").font(FontId::proportional(11.5)));
                        ui.add(egui::Slider::new(&mut settings.afk_cooldown_mins, 5..=120).suffix(" min"));
                    });
                }

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(6.0);

                ui.checkbox(&mut settings.enable_scheduler, "Enable Scheduler (Boost hours only during specified times)");
                if settings.enable_scheduler {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Start Hour:").font(FontId::proportional(11.5)));
                        ui.add(egui::Slider::new(&mut settings.scheduler_start_hour, 0..=23));

                        ui.add_space(10.0);

                        ui.label(RichText::new("End Hour:").font(FontId::proportional(11.5)));
                        ui.add(egui::Slider::new(&mut settings.scheduler_end_hour, 0..=23));
                    });
                }
            });

            ui.add_space(12.0);
        });
    }
}
