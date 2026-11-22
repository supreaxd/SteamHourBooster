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

use eframe::egui;

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let start_minimized = args.iter().any(|arg| arg == "--minimized" || arg == "--tray" || arg == "--background");

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("SteamBooster v0.2.0")
        .with_inner_size([1000.0, 680.0])
        .with_min_inner_size([780.0, 520.0])
        .with_icon(eframe::icon_data::from_png_bytes(&[]).unwrap_or_default());

    if start_minimized {
        viewport = viewport.with_visible(false);
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "SteamBooster",
        options,
        Box::new(|cc| {
            // Windows Segoe UI Emoji & Segoe UI font desteğini yükle (Simge ve Emojiler için)
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
