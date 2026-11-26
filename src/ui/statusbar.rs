use std::sync::{Arc, Mutex};
use eframe::egui::{self, Color32, FontId, RichText, Ui};
use crate::data::{Account, AccountStatus, CustomThemeColors, ThemeMode};
use crate::i18n::Strings;
use crate::ping::{get_ping_status, trigger_ping_now, ConnectionQuality};
use crate::ui::components::muted_text;

pub struct StatusBar;

impl StatusBar {
    pub fn draw(
        ui: &mut Ui,
        accounts: &[Arc<Mutex<Account>>],
        colors: &CustomThemeColors,
        theme_mode: ThemeMode,
        telegram_enabled: bool,
        strings: &Strings,
    ) {
        let _accent = Color32::from_rgb(colors.accent[0], colors.accent[1], colors.accent[2]);
        let muted = muted_text(theme_mode);

        let mut active_cnt = 0usize;
        let mut total_games = 0usize;
        let mut total_hours = 0.0_f64;

        for arc in accounts {
            if let Ok(a) = arc.lock() {
                total_hours += a.hours_boosted;
                if matches!(a.status, AccountStatus::Boosting) {
                    active_cnt += 1;
                    total_games += a.games.len();
                }
            }
        }

        let ping = get_ping_status();

        ui.allocate_ui_with_layout(
            eframe::egui::Vec2::new(ui.available_width(), 22.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.add_space(8.0);

                // Left: Live boosting metrics
                ui.label(
                    RichText::new(format!(
                        "⚡ {} {}  ·  {} {}  ·  {:.1}h",
                        active_cnt,
                        strings.pick("active", "etkin", "активно"),
                        total_games,
                        strings.games_count_unit,
                        total_hours
                    ))
                    .font(FontId::proportional(11.0))
                    .color(muted),
                );

                // Right: Steam CM Ping & Connection Quality + Integration status
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(8.0);

                    let tg_text = if telegram_enabled {
                        strings.pick("Telegram Ready", "Telegram Aktif", "Telegram Активен")
                    } else {
                        strings.pick("Telegram Off", "Telegram Kapalı", "Telegram Выкл")
                    };
                    let tg_col = if telegram_enabled {
                        Color32::from_rgb(90, 220, 130)
                    } else {
                        muted
                    };
                    ui.label(RichText::new(tg_text).font(FontId::proportional(11.0)).color(tg_col));

                    ui.label(RichText::new("·").color(muted));

                    let (dot_col, quality_label) = match ping.quality {
                        ConnectionQuality::Good => (
                            Color32::from_rgb(80, 225, 130),
                            strings.pick("Good", "İyi", "Отлично"),
                        ),
                        ConnectionQuality::Fair => (
                            Color32::from_rgb(255, 195, 65),
                            strings.pick("Fair", "Orta", "Средне"),
                        ),
                        ConnectionQuality::Poor => (
                            Color32::from_rgb(255, 105, 105),
                            strings.pick("Poor", "Zayıf", "Плохо"),
                        ),
                        ConnectionQuality::Disconnected => (
                            Color32::from_rgb(130, 140, 155),
                            strings.pick("No Connection", "Bağlantı Yok", "Нет связи"),
                        ),
                    };

                    let ping_text = match ping.ping_ms {
                        Some(ms) => format!("● Steam CM: {}ms ({})", ms, quality_label),
                        None => format!("● Steam CM: {}", quality_label),
                    };

                    let ping_resp = ui
                        .add(egui::Label::new(
                            RichText::new(ping_text).font(FontId::proportional(11.0)).color(dot_col),
                        ))
                        .on_hover_text(format!(
                            "{}: {}\n{}: {}\n{}",
                            strings.pick("Endpoint", "Sunucu", "Сервер"),
                            ping.endpoint,
                            strings.pick("Last checked", "Son ölçüm", "Последняя проверка"),
                            ping.last_checked,
                            strings.pick("Click to measure ping now", "Şimdi ölçmek için tıklayın", "Нажмите для проверки")
                        ));
                    if ping_resp.clicked() {
                        trigger_ping_now();
                    }
                });
            },
        );
    }
}
