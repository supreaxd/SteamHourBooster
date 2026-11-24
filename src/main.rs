#![windows_subsystem = "windows"]

mod app;
mod autostart;
mod crypto;
mod data;
mod debug_log;
mod free_games;
mod i18n;
mod inventory;
mod logger;
mod qr_auth;
mod steam;
mod telegram;
mod tray;
mod ui;
mod updater;

use eframe::egui;

const APP_ICON_PNG: &[u8] = include_bytes!("../assets/icon.png");

fn main() -> eframe::Result<()> {
    crate::updater::cleanup_old_executable();
    let args: Vec<String> = std::env::args().collect();
    let config = crate::data::AppConfig::load();
    let start_minimized = args.iter().any(|arg| arg == "--minimized" || arg == "--tray" || arg == "--background")
        || config.settings.start_minimized;

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("SupBooster")
        .with_inner_size([1040.0, 700.0])
        .with_min_inner_size([960.0, 620.0])
        .with_transparent(true);

    if let Ok(icon) = eframe::icon_data::from_png_bytes(APP_ICON_PNG) {
        viewport = viewport.with_icon(icon);
    }

    if start_minimized {
        viewport = viewport.with_visible(false);
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "SupBooster",
        options,
        Box::new(|cc| {
            // Steam eşya ve profil simgeleri için egui_extras image loaders kurulumu
            egui_extras::install_image_loaders(&cc.egui_ctx);

            // Windows Segoe UI Emoji & Segoe UI font desteği
            let mut fonts = egui::FontDefinitions::default();
            if let Ok(emoji_bytes) = std::fs::read("C:\\Windows\\Fonts\\seguiemj.ttf") {
                fonts.font_data.insert("segoe_emoji".to_owned(), egui::FontData::from_owned(emoji_bytes));
                fonts.families.entry(egui::FontFamily::Proportional).or_default().push("segoe_emoji".to_owned());
                fonts.families.entry(egui::FontFamily::Monospace).or_default().push("segoe_emoji".to_owned());
            }
            if let Ok(segoe_bytes) = std::fs::read("C:\\Windows\\Fonts\\segoeui.ttf") {
                fonts.font_data.insert("segoe_ui".to_owned(), egui::FontData::from_owned(segoe_bytes));
                fonts.families.entry(egui::FontFamily::Proportional).or_default().insert(0, "segoe_ui".to_owned());
            }
            cc.egui_ctx.set_fonts(fonts);

            Ok(Box::new(app::SteamBoosterApp::new(cc)))
        }),
    )
}
