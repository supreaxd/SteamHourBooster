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
        updater: &crate::updater::UpdateManager,
        strings: &Strings,
    ) {
        let colors = settings.custom_colors.clone();
        let theme_mode = settings.theme_mode;
        let accent = Color32::from_rgb(colors.accent[0], colors.accent[1], colors.accent[2]);

        ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            // ─── 1. APPEARANCE & THEME ───
            ThemeEngine::card_frame(theme_mode, &colors).show(ui, |ui| {
                ui.label(RichText::new(strings.sec_appearance_theme).font(FontId::proportional(14.5)).strong());
                ui.add_space(6.0);

                // Theme Mode Selector
                ui.label(RichText::new(strings.pick("Theme Mode:", "Tema Modu:", "Режим темы:")).font(FontId::proportional(12.0)));
                ui.horizontal(|ui| {
                    if ui.selectable_label(settings.theme_mode == ThemeMode::Dark, strings.theme_dark).clicked() {
                        settings.theme_mode = ThemeMode::Dark;
                        let lum = (settings.custom_colors.text[0] as u32 * 299 + settings.custom_colors.text[1] as u32 * 587 + settings.custom_colors.text[2] as u32 * 114) / 1000;
                        if lum < 70 {
                            settings.custom_colors.text = [198, 212, 223];
                            settings.custom_colors.surface = [22, 26, 36];
                        }
                    }
                    if ui.selectable_label(settings.theme_mode == ThemeMode::Light, strings.theme_light).clicked() {
                        settings.theme_mode = ThemeMode::Light;
                        let lum = (settings.custom_colors.text[0] as u32 * 299 + settings.custom_colors.text[1] as u32 * 587 + settings.custom_colors.text[2] as u32 * 114) / 1000;
                        if lum > 140 {
                            settings.custom_colors.text = [25, 30, 36];
                            settings.custom_colors.surface = [245, 247, 250];
                        }
                    }
                    if ui.selectable_label(settings.theme_mode == ThemeMode::System, strings.theme_system).clicked() {
                        settings.theme_mode = ThemeMode::System;
                    }
                    if ui.selectable_label(settings.theme_mode == ThemeMode::Transparent, strings.theme_transparent).clicked() {
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
                        RichText::new(strings.pick(
                            "✨ Transparent mode showcases your desktop background with an elegant Acrylic / Mica blur aesthetic.",
                            "✨ Şeffaf mod masaüstü arka planınızı Acrylic / Mica bulanıklık efektiyle yansıtır.",
                            "✨ Прозрачный режим отображает фон рабочего стола с эффектом Acrylic / Mica.",
                        ))
                            .font(FontId::proportional(11.0))
                            .color(Color32::from_rgb(100, 190, 240)),
                    );
                }

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(6.0);

                // Color Customization (Color Pickers)
                ui.label(RichText::new(strings.pick("🎨 Color Customization:", "🎨 Renk Özelleştirme:", "🎨 Настройка цветов:")).font(FontId::proportional(13.0)).strong());
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
                ui.label(RichText::new(strings.sec_language).font(FontId::proportional(12.5)).strong());
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut settings.language, Language::English, "🇬🇧 English");
                    ui.selectable_value(&mut settings.language, Language::Turkish, "🇹🇷 Türkçe");
                    ui.selectable_value(&mut settings.language, Language::Russian, "🇷🇺 Русский");
                });

                ui.add_space(6.0);

                // UI Zoom / Scale slider
                ui.label(RichText::new(strings.label_ui_scale).font(FontId::proportional(12.0)));
                ui.horizontal(|ui| {
                    ui.add(egui::Slider::new(&mut settings.ui_scale, 0.8..=1.5).step_by(0.05).text("x"));
                    if ui.button(strings.zoom_reset_btn).clicked() {
                        settings.ui_scale = 1.0;
                    }
                });
            });

            ui.add_space(8.0);

            // ─── 2. STARTUP & SYSTEM TRAY ───
            ThemeEngine::card_frame(theme_mode, &colors).show(ui, |ui| {
                ui.label(RichText::new(strings.sec_startup).font(FontId::proportional(14.5)).strong());
                ui.add_space(6.0);

                // Windows registry autostart
                let mut reg_auto = self.autostart_reg;
                if ui.checkbox(&mut reg_auto, strings.opt_autostart).changed() {
                    self.autostart_reg = reg_auto;
                    let _ = autostart::set_registry_autostart(reg_auto, self.autostart_minimized);
                }

                // Start minimized
                ui.checkbox(&mut settings.start_minimized, strings.opt_start_minimized);

                // Minimize to tray on close
                ui.checkbox(&mut settings.minimize_to_tray, strings.opt_minimize_to_tray);

                // Auto-start boost on launch
                ui.checkbox(&mut settings.auto_start_boost_on_launch, strings.opt_auto_start_boost);

                ui.add_space(8.0);

                // Windows Task Scheduler Service
                ui.horizontal(|ui| {
                    if self.autostart_task_service {
                        if ui.button(RichText::new(format!("🗑️ {}", strings.btn_unregister_task)).color(Color32::from_rgb(255, 120, 120))).clicked() {
                            let _ = autostart::unregister_task_scheduler();
                            self.autostart_task_service = autostart::is_task_scheduler_enabled();
                        }
                        ui.label(RichText::new(strings.service_active).color(Color32::from_rgb(90, 215, 120)));
                    } else {
                        if ui.button(RichText::new(strings.opt_task_scheduler).color(accent)).clicked() {
                            let _ = autostart::register_task_scheduler(self.autostart_minimized);
                            self.autostart_task_service = autostart::is_task_scheduler_enabled();
                        }
                        ui.label(RichText::new(strings.service_inactive).color(Color32::from_rgb(130, 145, 165)));
                    }
                });
            });

            ui.add_space(8.0);

            // ─── 3. SECURITY & REAL-GAME DETECTION ───
            ThemeEngine::card_frame(theme_mode, &colors).show(ui, |ui| {
                ui.label(RichText::new(strings.sec_security).font(FontId::proportional(14.5)).strong());
                ui.add_space(6.0);

                ui.horizontal(|ui| {
                    ui.label(RichText::new(strings.pick("🔒 Hardware Encryption:", "🔒 Donanım Şifrelemesi:", "🔒 Аппаратное шифрование:")).font(FontId::proportional(12.0)));
                    ui.label(RichText::new(strings.pick("Windows DPAPI Always Enforced", "Windows DPAPI Her Zaman Aktif", "Windows DPAPI всегда активен")).font(FontId::proportional(11.5)).color(Color32::from_rgb(90, 220, 130)).strong());
                });
                ui.add_space(4.0);

                ui.checkbox(&mut settings.auto_pause_game, strings.opt_autopause_game);
                ui.add_space(4.0);

                ui.checkbox(&mut settings.card_drop_tracking, strings.opt_card_drop_tracking);
            });

            ui.add_space(8.0);

            // ─── 4. AFK AUTO-REPLY & SCHEDULER ───
            ThemeEngine::card_frame(theme_mode, &colors).show(ui, |ui| {
                ui.label(RichText::new(strings.sec_afk).font(FontId::proportional(14.5)).strong());
                ui.add_space(6.0);

                ui.checkbox(&mut settings.afk_auto_reply, strings.opt_afk_auto_reply);
                if settings.afk_auto_reply {
                    ui.add_space(4.0);
                    ui.label(RichText::new(strings.label_afk_message).font(FontId::proportional(11.5)));
                    ui.add(egui::TextEdit::singleline(&mut settings.afk_reply_message).desired_width(f32::INFINITY));

                    ui.horizontal(|ui| {
                        ui.label(RichText::new(strings.label_afk_cooldown).font(FontId::proportional(11.5)));
                        ui.add(egui::Slider::new(&mut settings.afk_cooldown_mins, 5..=120).suffix(" min"));
                    });
                }

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(6.0);

                ui.checkbox(&mut settings.enable_scheduler, strings.opt_enable_scheduler);
                if settings.enable_scheduler {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(strings.scheduler_start_hour).font(FontId::proportional(11.5)));
                        ui.add(egui::Slider::new(&mut settings.scheduler_start_hour, 0..=23));

                        ui.add_space(10.0);

                        ui.label(RichText::new(strings.scheduler_end_hour).font(FontId::proportional(11.5)));
                        ui.add(egui::Slider::new(&mut settings.scheduler_end_hour, 0..=23));
                    });
                }
            });

            ui.add_space(8.0);

            // ─── 5. UPDATES & REPOSITORY ───
            ThemeEngine::card_frame(theme_mode, &colors).show(ui, |ui| {
                ui.label(RichText::new(strings.sec_updater).font(FontId::proportional(14.5)).strong());
                ui.add_space(6.0);

                let current_ver = env!("CARGO_PKG_VERSION");
                ui.label(RichText::new(format!("{}: v{}", strings.pick("Current Version", "Mevcut Sürüm", "Текущая версия"), current_ver)).font(FontId::proportional(12.0)));
                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    ui.label(RichText::new("GitHub Repository:").font(FontId::proportional(12.0)));
                    ui.add(egui::TextEdit::singleline(&mut settings.github_repo).desired_width(220.0));
                });
                ui.add_space(4.0);

                ui.checkbox(&mut settings.auto_check_updates, strings.opt_auto_check_updates);
                ui.add_space(6.0);

                let update_state = updater.state.lock().unwrap().clone();
                ui.horizontal(|ui| {
                    let check_btn = ui.button(RichText::new(strings.btn_check_updates).font(FontId::proportional(12.0)).strong());
                    if check_btn.clicked() {
                        crate::updater::check_for_updates(updater.state.clone(), settings.github_repo.clone());
                    }

                    match &update_state {
                        crate::updater::UpdateState::Checking => {
                            ui.spinner();
                            ui.label(RichText::new(strings.update_checking).font(FontId::proportional(11.5)).color(Color32::from_rgb(100, 180, 255)));
                        }
                        crate::updater::UpdateState::UpToDate { checked_at } => {
                            ui.label(RichText::new(format!("{} ({})", strings.update_up_to_date, checked_at)).font(FontId::proportional(11.5)).color(Color32::from_rgb(90, 220, 130)));
                        }
                        crate::updater::UpdateState::UpdateAvailable { version, .. } => {
                            ui.label(RichText::new(format!("{} (v{})", strings.update_available_title, version)).font(FontId::proportional(12.0)).strong().color(Color32::from_rgb(255, 175, 50)));
                        }
                        crate::updater::UpdateState::Downloading { progress, .. } => {
                            ui.label(RichText::new(format!("{} {:.0}%", strings.pick("Downloading:", "İndiriliyor:", "Загрузка:"), progress * 100.0)).font(FontId::proportional(11.5)).color(Color32::from_rgb(100, 195, 255)));
                        }
                        crate::updater::UpdateState::ReadyToRestart => {
                            ui.label(RichText::new(strings.pick("🔄 Restarting to apply update...", "🔄 Güncelleme için yeniden başlatılıyor...", "🔄 Перезапуск для обновления...")).font(FontId::proportional(11.5)).color(Color32::from_rgb(90, 230, 140)));
                        }
                        crate::updater::UpdateState::Error(err) => {
                            ui.label(RichText::new(format!("⚠️ {}", err)).font(FontId::proportional(11.0)).color(Color32::from_rgb(255, 120, 100)));
                        }
                        _ => {}
                    }
                });

                if settings.dev_debug_mode {
                    ui.add_space(6.0);
                    ui.separator();
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("🧪 Dev Test:").font(FontId::proportional(11.5)).color(Color32::from_rgb(180, 150, 230)));
                        if ui.button(RichText::new("Simulate Update Notification (v1.0.1)").font(FontId::proportional(11.0))).clicked() {
                            let mut s = updater.state.lock().unwrap();
                            *s = crate::updater::UpdateState::UpdateAvailable {
                                version: "1.0.1".to_string(),
                                tag_name: "v1.0.1".to_string(),
                                release_title: "SupBooster v1.0.1".to_string(),
                                release_notes: "Simulated release notes for testing".to_string(),
                                published_at: chrono::Local::now().to_rfc3339(),
                                download_url: format!("https://github.com/{}/releases/download/v1.0.1/SupBoost.exe", settings.github_repo),
                                asset_name: "SupBoost.exe".to_string(),
                                asset_size: 14_365_696,
                            };
                        }
                        if ui.button(RichText::new("Reset to Up To Date").font(FontId::proportional(11.0))).clicked() {
                            let mut s = updater.state.lock().unwrap();
                            *s = crate::updater::UpdateState::UpToDate { checked_at: chrono::Local::now().format("%H:%M:%S").to_string() };
                        }
                    });
                }
            });

            ui.add_space(12.0);
        });
    }
}
